//! The desktop player: a window onto a skirmish.
//!   cargo run --release --bin play -- [--setting generic] [--map maps/skirmish-01.txt] [--seed 1] [--player 0]
//!
//! Arrow keys or WASD (or the mouse at a screen edge) scroll, the wheel zooms, a left click or drag selects your
//! units, and a right click sends them: onto an enemy to attack it, anywhere else to move there.
//!
//! The rail on the right builds: pick a factory's tab, left-click an item to queue one (shift: five), right-click to
//! cancel one with a refund. When a building is ready, click it and then a spot on the map; the ghost shows green
//! where it fits. Escape puts the building back, then clears the selection, then quits. Space pauses. There is no
//! computer opponent yet. `--frames N` quits after N frames, for smoke tests.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use classic_render::art::{self, Art};
use classic_render::hud::{Button, RAIL_W};
use classic_render::platform::{Font, Gpu, Rect, SpriteBatch};
use classic_render::{Camera, Hud, Scene, View};
use classic_sim::map::TILE;
use classic_sim::{CommandOrder, Game, GameOptions, Rules};
use classic_tools::setting;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// The simulation's fixed rate.
const TICK: Duration = Duration::from_micros(1_000_000 / 15);
/// Pixels per second the camera scrolls at zoom 1.
const SCROLL: f32 = 600.0;
/// How close to a screen edge the mouse scrolls the view.
const EDGE: f32 = 8.0;

fn arg(name: &str) -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == &format!("--{name}")).and_then(|i| args.get(i + 1).cloned())
}

struct Running {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    batch: SpriteBatch,
    art: Art,
    font: Font,
}

struct App {
    game: Game,
    scene: Scene,
    hud: Hud,
    pack: classic_data::Pack,
    player: u32,
    cam: Camera,
    run: Option<Running>,
    last: Instant,
    owed: Duration,
    paused: bool,
    keys: BTreeSet<KeyCode>,
    mouse: (f32, f32),
    /// Where a left-button drag started, in screen pixels.
    drag: Option<(f32, f32)>,
    frames: u64,
    max_frames: Option<u64>,
}

impl App {
    fn view(&self) -> View {
        let screen = self.run.as_ref().map_or((1280.0, 800.0), |r| (r.config.width as f32, r.config.height as f32));
        View { cam: self.cam, screen, tile: TILE as f32 * self.world_px() }
    }

    fn shift(&self) -> bool {
        self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight)
    }

    /// Give a click to the HUD first; returns whether it took it.
    fn hud_click(&mut self, button: Button) -> bool {
        let view = self.view();
        let shift = self.shift();
        self.hud.click(&mut self.game, &view, self.mouse, button, shift)
    }

    /// A right click goes to the HUD, else orders the selected units.
    fn right_click(&mut self) {
        if !self.hud_click(Button::Right) {
            self.command(self.mouse.0, self.mouse.1);
        }
    }

    fn world_px(&self) -> f32 {
        self.run.as_ref().map_or(32.0, |r| r.art.tile) / TILE as f32
    }

    /// The entity under a screen point: a unit within half a tile of its centre, or a building whose footprint
    /// holds the point. Units win over buildings.
    fn pick(&self, sx: f32, sy: f32) -> Option<u32> {
        let (wx, wy) = self.cam.to_world(sx, sy);
        let px = self.world_px();
        let half = TILE as f32 * px / 2.0;
        let mut found = None;
        for e in &self.game.state.entities {
            let k = self.game.rules.kind(e.kind);
            if k.building {
                let t = e.tile();
                let tile = TILE as f32 * px;
                let inside = wx >= t.x as f32 * tile
                    && wy >= t.y as f32 * tile
                    && wx < (t.x + k.width) as f32 * tile
                    && wy < (t.y + k.height) as f32 * tile;
                if inside && found.is_none() {
                    found = Some(e.id);
                }
            } else if (e.x as f32 * px - wx).abs() <= half && (e.y as f32 * px - wy).abs() <= half {
                return Some(e.id);
            }
        }
        found
    }

    fn select(&mut self, from: (f32, f32), to: (f32, f32)) {
        let mine = |id: u32| {
            self.game.state.entity(id).is_some_and(|e| e.owner == self.player && !self.game.rules.kind(e.kind).building)
        };
        if (from.0 - to.0).abs() < 4.0 && (from.1 - to.1).abs() < 4.0 {
            self.scene.selected = self.pick(to.0, to.1).filter(|&id| mine(id)).into_iter().collect();
            return;
        }
        let (a, b) = (self.cam.to_world(from.0, from.1), self.cam.to_world(to.0, to.1));
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        let px = self.world_px();
        self.scene.selected = self
            .game
            .state
            .entities
            .iter()
            .filter(|e| mine(e.id))
            .filter(|e| {
                let (x, y) = (e.x as f32 * px, e.y as f32 * px);
                x >= x0 && x <= x1 && y >= y0 && y <= y1
            })
            .map(|e| e.id)
            .collect();
    }

    fn command(&mut self, sx: f32, sy: f32) {
        if self.scene.selected.is_empty() {
            return;
        }
        let ids = self.scene.selected.clone();
        let enemy = self.pick(sx, sy).filter(|&id| self.game.state.entity(id).is_some_and(|e| e.owner != self.player));
        let order = match enemy {
            Some(target) => CommandOrder::Attack { target },
            None => {
                let (wx, wy) = self.cam.to_world(sx, sy);
                let tile = TILE as f32 * self.world_px();
                CommandOrder::Move { x: (wx / tile).floor() as i32, y: (wy / tile).floor() as i32 }
            }
        };
        self.game.order(self.player, &ids, order);
    }

    /// Advance the game by the ticks owed since the last frame, at most a few at once so a stall doesn't snowball.
    fn advance(&mut self) -> f32 {
        let now = Instant::now();
        let dt = now - self.last;
        self.last = now;
        if self.paused {
            return 1.0;
        }
        self.owed = (self.owed + dt).min(TICK * 5);
        while self.owed >= TICK {
            self.owed -= TICK;
            self.scene.before_step(&self.game);
            self.game.step(1);
            self.scene.after_step(&self.game);
        }
        // Events have been turned into effects; don't let them pile up.
        if self.game.events.len() > 10_000 {
            self.game.events.clear();
        }
        self.owed.as_secs_f32() / TICK.as_secs_f32()
    }

    fn scroll(&mut self, dt: f32, w: f32, h: f32) {
        let (mut dx, mut dy) = (0.0, 0.0);
        let held = |k: &[KeyCode]| k.iter().any(|k| self.keys.contains(k));
        if held(&[KeyCode::ArrowLeft, KeyCode::KeyA]) || self.mouse.0 < EDGE {
            dx -= 1.0;
        }
        if held(&[KeyCode::ArrowRight, KeyCode::KeyD]) || self.mouse.0 > w - EDGE {
            dx += 1.0;
        }
        if held(&[KeyCode::ArrowUp, KeyCode::KeyW]) || self.mouse.1 < EDGE {
            dy -= 1.0;
        }
        if held(&[KeyCode::ArrowDown, KeyCode::KeyS]) || self.mouse.1 > h - EDGE {
            dy += 1.0;
        }
        let speed = SCROLL * dt / self.cam.zoom;
        let tile = TILE as f32 * self.world_px();
        let (mw, mh) = (self.game.map.width as f32 * tile, self.game.map.height as f32 * tile);
        let (vw, vh) = (w / self.cam.zoom, h / self.cam.zoom);
        // The rail covers the right of the screen, so the map may scroll out from under it.
        let right = mw - vw + RAIL_W * self.hud.scale / self.cam.zoom;
        self.cam.x = (self.cam.x + dx * speed).clamp(0.0_f32.min(right), right.max(0.0));
        self.cam.y = (self.cam.y + dy * speed).clamp(0.0_f32.min(mh - vh), (mh - vh).max(0.0));
    }

    fn title(&self) -> String {
        let p = self.game.state.players.iter().find(|p| p.id == self.player);
        let power = self.game.power(self.player);
        format!(
            "{} | tick {} | credits {} | power {}/{}{}",
            self.pack.title,
            self.game.state.tick,
            p.map_or(0, |p| p.credits),
            power.supply,
            power.demand,
            if self.paused { " | paused" } else { "" }
        )
    }

    fn redraw(&mut self) {
        let alpha = self.advance();
        let title = self.title();
        let Some(run) = &self.run else { return };
        let (w, h) = (run.config.width as f32, run.config.height as f32);
        let frame_dt = 1.0 / 60.0;
        self.scroll(frame_dt, w, h);
        let Some(run) = &mut self.run else { return };
        let texture = match run.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                run.surface.configure(&run.gpu.device, &run.config);
                return;
            }
            _ => return,
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.scene.draw(&mut run.batch, &run.art, &self.game, &self.cam, (w, h), alpha);
        let world = View { cam: self.cam, screen: (w, h), tile: run.art.tile };
        self.hud.draw(&mut run.batch, &run.art, &run.font, &self.game, &world, self.mouse);
        if let Some(from) = self.drag {
            let (x0, y0) = (from.0.min(self.mouse.0), from.1.min(self.mouse.1));
            let r = Rect::new(x0, y0, (from.0 - self.mouse.0).abs(), (from.1 - self.mouse.1).abs());
            run.batch.outline(r, 1.0, [240, 240, 240, 255]);
        }
        run.batch.draw(&run.gpu, &view, run.config.width, run.config.height, [0, 0, 0, 255]);
        run.gpu.queue.present(texture);
        run.window.set_title(&title);
        self.frames += 1;
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.run.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title(self.title()).with_inner_size(PhysicalSize::new(1280, 800));
        let window = Arc::new(event_loop.create_window(attrs).expect("a window"));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
            event_loop.owned_display_handle(),
        )));
        let surface = instance.create_surface(window.clone()).expect("a surface for the window");
        let gpu = Gpu::open(instance, Some(&surface)).expect("a GPU that can draw to the window");
        println!("drawing with {}", gpu.describe());
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .expect("the surface works with this adapter");
        // A plain (not sRGB) format, so the art's pixels reach the screen unchanged.
        let caps = surface.get_capabilities(&gpu.adapter);
        if let Some(&f) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = f;
        }
        surface.configure(&gpu.device, &config);
        let mut batch = SpriteBatch::new(&gpu, config.format);
        let ramps = art::player_ramps(&self.pack, self.game.state.players.len());
        let art = Art::load(&gpu, &mut batch, &art::art_dir(&self.pack), &ramps).expect("the pack's art loads");
        let font = Font::new(&gpu, &mut batch);
        self.hud.scale = window.scale_factor().round().max(1.0) as f32;
        // Start over the player's own base.
        if let Some(e) = self.game.state.entities.iter().find(|e| e.owner == self.player) {
            let t = e.tile();
            self.cam.x = (t.x as f32 * art.tile - 320.0).max(0.0);
            self.cam.y = (t.y as f32 * art.tile - 240.0).max(0.0);
        }
        self.run = Some(Running { window, surface, config, gpu, batch, art, font });
        self.last = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(run) = &mut self.run
                    && size.width > 0
                    && size.height > 0
                {
                    run.config.width = size.width;
                    run.config.height = size.height;
                    run.surface.configure(&run.gpu.device, &run.config);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                if event.state == ElementState::Pressed {
                    self.keys.insert(code);
                    match code {
                        KeyCode::Escape => {
                            if !self.hud.cancel() && self.scene.selected.is_empty() {
                                event_loop.exit();
                            }
                            self.scene.selected.clear();
                        }
                        KeyCode::Space if !event.repeat => self.paused = !self.paused,
                        _ => {}
                    }
                } else {
                    self.keys.remove(&code);
                }
            }
            WindowEvent::CursorMoved { position: PhysicalPosition { x, y }, .. } => {
                self.mouse = (x as f32, y as f32);
            }
            WindowEvent::CursorLeft { .. } => self.mouse = (-1.0e4, -1.0e4),
            WindowEvent::MouseInput { state, button, .. } => match (button, state) {
                (MouseButton::Left, ElementState::Pressed) => {
                    if !self.hud_click(Button::Left) {
                        self.drag = Some(self.mouse);
                    }
                }
                (MouseButton::Left, ElementState::Released) => {
                    if let Some(from) = self.drag.take() {
                        self.select(from, self.mouse);
                    }
                }
                (MouseButton::Right, ElementState::Pressed) => self.right_click(),
                _ => {}
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let up = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                    MouseScrollDelta::PixelDelta(p) => p.y > 0.0,
                };
                if self.hud.over(&self.game, self.view().screen, self.mouse.0, self.mouse.1) {
                    self.hud.wheel(up);
                    return;
                }
                // Whole-number zooms keep every art pixel the same size; zoom about the mouse.
                let zoom = if up { (self.cam.zoom + 1.0).min(4.0) } else { (self.cam.zoom - 1.0).max(1.0) };
                let (wx, wy) = self.cam.to_world(self.mouse.0, self.mouse.1);
                self.cam.zoom = zoom;
                self.cam.x = wx - self.mouse.0 / zoom;
                self.cam.y = wy - self.mouse.1 / zoom;
            }
            WindowEvent::RedrawRequested => {
                self.redraw();
                if self.max_frames.is_some_and(|m| self.frames >= m) {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(run) = &self.run {
            run.window.request_redraw();
        }
    }
}

fn main() {
    let setting_name = arg("setting").or_else(|| std::env::var("SETTING").ok()).unwrap_or_else(|| "generic".into());
    let pack = setting::load(&setting_name).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let rules = Rules::from_table(&pack.rules).expect("pack rules match the simulation");
    let map_path = arg("map").unwrap_or_else(|| setting::root().join("maps/skirmish-01.txt").display().to_string());
    let text = std::fs::read_to_string(&map_path).unwrap_or_else(|e| panic!("{map_path}: {e}"));
    let seed = arg("seed").and_then(|s| s.parse().ok()).unwrap_or(1);
    let game = Game::new(GameOptions { map: &text, seed, players: None, rules: Some(&rules) }).expect("valid map");
    let player = arg("player").and_then(|s| s.parse().ok()).unwrap_or(0);
    let hud = Hud::new(&pack, &game, player);
    let mut app = App {
        game,
        scene: Scene::default(),
        hud,
        pack,
        player,
        cam: Camera { x: 0.0, y: 0.0, zoom: 2.0 },
        run: None,
        last: Instant::now(),
        owed: Duration::ZERO,
        paused: false,
        keys: BTreeSet::new(),
        mouse: (-1.0e4, -1.0e4),
        drag: None,
        frames: 0,
        max_frames: arg("frames").and_then(|s| s.parse().ok()),
    };
    let event_loop = EventLoop::new().expect("an event loop (is there a display?)");
    event_loop.run_app(&mut app).expect("the event loop runs");
    println!("quit at tick {} after {} frames, hash {}", app.game.state.tick, app.frames, app.game.hash());
}
