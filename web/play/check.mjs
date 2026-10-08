// Opens the browser player in headless Chromium and checks it draws the game, takes input and plays sound: once with WebGPU
// and once with WebGL2 (WebGPU switched off), each on Chromium's software GPU. Saves each frame as a PNG to look at.
//
//   cargo build --release --target wasm32-unknown-unknown -p classic-render --bin play
//   wasm-bindgen --target web --no-typescript --out-dir web/play/pkg target/wasm32-unknown-unknown/release/play.wasm
//   python3 -m http.server 8000 &
//   node web/play/check.mjs [http://localhost:8000] [folder for the PNGs]
//
// Needs the `playwright` package and its Chromium (npm i playwright && npx playwright install chromium).

import { writeFileSync } from "node:fs";
import { chromium } from "playwright";

const base = process.argv[2] ?? "http://localhost:8000";
const out = process.argv[3] ?? ".";
const software = ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"];
const runs = [
  { name: "webgpu", args: ["--enable-unsafe-webgpu", "--enable-features=Vulkan", ...software], backend: "BrowserWebGpu" },
  { name: "webgl2", args: ["--disable-features=WebGPU", ...software], backend: "Gl" },
];

// Headless Chromium draws WebGPU canvases but never composites them, so a screenshot comes out blank. Read the
// frame back on the GPU instead: let the canvas texture be copied, and after the next submit once `wantFrame` is
// set, copy the frame to a buffer.
function readWebGpuFrames() {
  if (!globalThis.GPUCanvasContext) return;
  const configure = GPUCanvasContext.prototype.configure;
  GPUCanvasContext.prototype.configure = function (c) {
    this.device = c.device;
    const usage = (c.usage ?? GPUTextureUsage.RENDER_ATTACHMENT) | GPUTextureUsage.COPY_SRC;
    return configure.call(this, { ...c, usage });
  };
  const current = GPUCanvasContext.prototype.getCurrentTexture;
  GPUCanvasContext.prototype.getCurrentTexture = function () {
    const texture = current.call(this);
    window.frameTexture = { texture, device: this.device };
    return texture;
  };
  const submit = GPUQueue.prototype.submit;
  GPUQueue.prototype.submit = function (commands) {
    submit.call(this, commands);
    const f = window.frameTexture;
    if (!f || !window.wantFrame) return;
    window.wantFrame = false;
    const { texture: t, device } = f;
    const row = Math.ceil((t.width * 4) / 256) * 256;
    const buffer = device.createBuffer({ size: row * t.height, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
    const enc = device.createCommandEncoder();
    enc.copyTextureToBuffer({ texture: t }, { buffer, bytesPerRow: row }, [t.width, t.height]);
    submit.call(this, [enc.finish()]);
    buffer.mapAsync(GPUMapMode.READ).then(() => {
      const src = new Uint8Array(buffer.getMappedRange());
      const bgra = t.format.startsWith("bgra");
      const rgba = new Uint8ClampedArray(t.width * t.height * 4);
      for (let y = 0; y < t.height; y++) {
        for (let x = 0; x < t.width; x++) {
          const i = y * row + x * 4, o = (y * t.width + x) * 4;
          rgba[o] = src[i + (bgra ? 2 : 0)];
          rgba[o + 1] = src[i + 1];
          rgba[o + 2] = src[i + (bgra ? 0 : 2)];
          rgba[o + 3] = 255;
        }
      }
      window.webGpuFrame = { width: t.width, height: t.height, rgba };
      buffer.unmap();
    });
  };
}

// Listen to what the page plays: tap every Web Audio context's output and keep the loudest sample seen.
function listen() {
  const Base = window.AudioContext;
  window.audioContexts = 0;
  window.loudest = 0;
  window.AudioContext = class extends Base {
    constructor(...args) {
      super(...args);
      window.audioContexts++;
      this.tap = this.createAnalyser();
      const buf = new Float32Array(this.tap.fftSize);
      setInterval(() => {
        this.tap.getFloatTimeDomainData(buf);
        for (const v of buf) window.loudest = Math.max(window.loudest, Math.abs(v));
      }, 10);
    }
  };
  const connect = AudioNode.prototype.connect;
  AudioNode.prototype.connect = function (to, ...rest) {
    if (to instanceof AudioDestinationNode && this.context.tap && this !== this.context.tap) connect.call(this, this.context.tap);
    return connect.call(this, to, ...rest);
  };
}

// The frame as a PNG and the number of distinct colours in it: a drawn map has dozens (the generic pack's art uses
// a small palette), a blank canvas one.
async function frame(page, run) {
  const png = run.name === "webgpu"
    ? await page.evaluate(async () => {
        window.wantFrame = true;
        for (let i = 0; i < 100 && !window.webGpuFrame; i++) await new Promise((r) => setTimeout(r, 100));
        const f = window.webGpuFrame;
        if (!f) return null;
        const c = new OffscreenCanvas(f.width, f.height);
        c.getContext("2d").putImageData(new ImageData(f.rgba, f.width, f.height), 0, 0);
        const bytes = new Uint8Array(await (await c.convertToBlob()).arrayBuffer());
        let s = "";
        for (const b of bytes) s += String.fromCharCode(b);
        return btoa(s);
      }).then((b64) => b64 && Buffer.from(b64, "base64"))
    : await page.locator("#game").screenshot();
  if (!png) return { png: null, colours: 0 };
  const colours = await page.evaluate(async (b64) => {
    const img = new Image();
    img.src = `data:image/png;base64,${b64}`;
    await img.decode();
    const c = new OffscreenCanvas(img.width, img.height);
    const g = c.getContext("2d");
    g.drawImage(img, 0, 0);
    const px = g.getImageData(0, 0, img.width, img.height).data;
    const seen = new Set();
    for (let i = 0; i < px.length; i += 4) seen.add((px[i] << 16) | (px[i + 1] << 8) | px[i + 2]);
    return seen.size;
  }, png.toString("base64"));
  return { png, colours };
}

let failed = false;
for (const run of runs) {
  const browser = await chromium.launch({ args: run.args });
  const page = await browser.newPage({ viewport: { width: 960, height: 600 } });
  await page.addInitScript(readWebGpuFrames);
  await page.addInitScript(listen);
  const errors = [];
  let drawing = "";
  page.on("pageerror", (e) => {
    // winit hands control back to the browser by throwing; that one is expected.
    if (!String(e).includes("Using exceptions for control flow")) errors.push(String(e));
  });
  page.on("console", (m) => {
    if (m.text().startsWith("drawing with")) drawing = m.text();
    // The optional pack files (tuning.json) are looked for and may be missing.
    if (m.type() === "error" && !m.text().startsWith("Failed to load resource")) errors.push(m.text());
  });
  await page.goto(`${base}/web/play/?seed=1`);
  const titled = (re) => page.waitForFunction((s) => new RegExp(s).test(document.title), re.source, { timeout: 30_000 })
    .then(() => true, () => false);
  const ticking = await titled(/tick ([2-9]|\d\d)/);
  const silentAtFirst = await page.evaluate(() => window.audioContexts === 0);
  const shot = await frame(page, run);
  if (shot.png) writeFileSync(`${out}/play-${run.name}.png`, shot.png);
  // Space pauses: the key reaches the game through the canvas.
  await page.locator("#game").focus();
  await page.keyboard.press("Space");
  const paused = await titled(/paused/);
  await page.keyboard.press("Space");
  // Browsers allow sound only after a click or key press. Then drag round the tank and send it off: the select and
  // order sounds play.
  await page.mouse.move(380, 380);
  await page.mouse.down();
  await page.mouse.move(450, 450, { steps: 5 });
  await page.mouse.up();
  await page.mouse.click(700, 250, { button: "right" });
  await page.waitForTimeout(1000);
  const loudest = await page.evaluate(() => window.loudest);
  const checks = {
    [`drew with ${run.backend}`]: drawing.includes(`(${run.backend},`),
    "the game ticks": ticking,
    "the frame shows the map": shot.colours >= 16,
    "Space pauses": paused,
    "no sound before any input": silentAtFirst,
    "sound plays after the first input": loudest > 0.01,
    "no errors": errors.length === 0,
  };
  const bad = Object.entries(checks).filter(([, ok]) => !ok).map(([name]) => name);
  failed ||= bad.length > 0;
  console.log(`${bad.length ? "FAIL" : "ok  "} ${run.name}: ${drawing}; ${shot.colours} colours; loudest sample ${loudest.toFixed(2)}; ${await page.title()}`);
  for (const b of bad) console.log(`     failed: ${b}`);
  for (const e of errors) console.log(`     error: ${e}`);
  await browser.close();
}
process.exit(failed ? 1 : 0);
