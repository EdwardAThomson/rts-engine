//! The renderer, drawn offscreen with no window: the generic pack's art on the test map, in each faction's colours.
//!
//! These need a GPU adapter; a software one is enough (CI installs Mesa's). Each test prints what it drew, and the
//! first writes the frame to `target/render-test.png` so a person can look at it.

use classic_render::art::{self, Art};
use classic_render::platform::{Gpu, SpriteBatch, gpu::OFFSCREEN_FORMAT};
use classic_render::{Camera, Scene};
use classic_sim::{CommandOrder, Game, GameOptions, Rules};
use classic_tools::setting;

const MAP: &str = include_str!("../../../maps/test-01.txt");
const W: u32 = 480;
const H: u32 = 320;

struct Rig {
    gpu: Gpu,
    batch: SpriteBatch,
    art: Art,
    game: Game,
    ramps: Vec<String>,
}

fn rig() -> Rig {
    let gpu = Gpu::headless().expect("a GPU adapter (a software one will do)");
    println!("adapter: {}", gpu.describe());
    let pack = setting::load("generic").unwrap();
    let rules = Rules::from_table(&pack.rules).unwrap();
    let game = Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    let ramps = art::player_ramps(&pack, game.state.players.len());
    let mut batch = SpriteBatch::new(&gpu, OFFSCREEN_FORMAT);
    let art = Art::load(&gpu, &mut batch, &art::art_dir(&pack), &ramps).unwrap();
    Rig { gpu, batch, art, game, ramps }
}

fn frame(r: &mut Rig, scene: &mut Scene, cam: &Camera) -> Vec<u8> {
    scene.draw(&mut r.batch, &r.art, &r.game, cam, (W as f32, H as f32), 1.0);
    r.batch.draw_to_image(&r.gpu, W, H, [0, 0, 0, 255])
}

fn ramp(name: &str) -> Vec<[u8; 3]> {
    let text = std::fs::read_to_string(setting::root().join("settings/generic/art/art.json")).unwrap();
    let doc = classic_data::json::parse(&text).unwrap();
    let list = doc.get(if name == "remap" { "remap" } else { "ramps" }).unwrap();
    let list = if name == "remap" { list } else { list.get(name).unwrap() };
    list.as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let s = &v.as_str().unwrap()[1..];
            [0, 2, 4].map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        })
        .collect()
}

/// Pixels of `image` in the rectangle (x, y, w, h) whose colour is one of `colours`.
fn count(image: &[u8], (x, y, w, h): (u32, u32, u32, u32), colours: &[[u8; 3]]) -> usize {
    let mut n = 0;
    for py in y..(y + h).min(H) {
        for px in x..(x + w).min(W) {
            let i = ((py * W + px) * 4) as usize;
            if colours.iter().any(|c| c[..] == image[i..i + 3]) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn the_start_base_is_drawn_in_its_factions_colours() {
    let mut r = rig();
    let mut scene = Scene::default();
    let cam = Camera { x: 0.0, y: 0.0, zoom: 1.0 };
    let image = frame(&mut r, &mut scene, &cam);
    let png = classic_tools::art::png::encode(W as usize, H as usize, &image);
    let out = setting::root().join("target/render-test.png");
    std::fs::write(&out, png).unwrap();
    println!("wrote {}", out.display());

    // Player 0's base is at the top left: its yard, plant, refinery, harvester and tank, all 32 px a tile.
    let base = r.game.state.entities.iter().filter(|e| e.owner == 0).map(|e| e.tile()).collect::<Vec<_>>();
    let (x1, y1) = base.iter().fold((0, 0), |(x, y), t| (x.max(t.x + 3), y.max(t.y + 2)));
    let area = (0, 0, x1 as u32 * 32, y1 as u32 * 32);
    let own = count(&image, area, &ramp(&r.ramps[0]));
    let remap = count(&image, (0, 0, W, H), &ramp("remap"));
    println!("base area {area:?}: {own} pixels in {}; {remap} remap pixels on screen", r.ramps[0]);
    assert!(own > 200, "the base shows its faction colour");
    assert_eq!(remap, 0, "every remap colour was swapped");
    let lit = image.chunks_exact(4).filter(|p| p[..3] != [0, 0, 0]).count();
    assert_eq!(lit, (W * H) as usize, "the map covers the whole frame");
}

#[test]
fn the_other_faction_is_drawn_in_its_own_colours() {
    let mut r = rig();
    let mut scene = Scene::default();
    // Centre on player 1's tank.
    let t =
        r.game.state.entities.iter().find(|e| e.owner == 1 && r.game.rules.kind(e.kind).id == "battle_tank").unwrap();
    let (tx, ty) = (t.tile().x as f32 * 32.0, t.tile().y as f32 * 32.0);
    let cam = Camera { x: tx - W as f32 / 2.0, y: ty - H as f32 / 2.0, zoom: 1.0 };
    let image = frame(&mut r, &mut scene, &cam);
    let tank = (W / 2, H / 2, 32, 32);
    let theirs = count(&image, tank, &ramp(&r.ramps[1]));
    let ours = count(&image, tank, &ramp(&r.ramps[0]));
    println!("tank tile: {theirs} pixels in {}, {ours} in {}", r.ramps[1], r.ramps[0]);
    assert!(theirs > 20 && ours == 0);
}

#[test]
fn drawing_is_repeatable_and_never_changes_the_game() {
    let mut r = rig();
    let mut scene = Scene::default();
    let ids: Vec<u32> = r.game.state.entities.iter().filter(|e| e.owner == 0).map(|e| e.id).collect();
    r.game.order(0, &ids, CommandOrder::Move { x: 12, y: 8 });
    for _ in 0..40 {
        scene.before_step(&r.game);
        r.game.step(1);
        scene.after_step(&r.game);
    }
    let before = r.game.hash();
    let cam = Camera { x: 0.0, y: 0.0, zoom: 2.0 };
    let a = frame(&mut r, &mut scene, &cam);
    let b = frame(&mut r, &mut scene, &cam);
    println!("hash {before}, frames equal: {}", a == b);
    assert_eq!(a, b);
    assert_eq!(r.game.hash(), before);
}

#[test]
fn shots_and_explosions_are_drawn_from_events() {
    let mut r = rig();
    let mut scene = Scene::default();
    let tank = r.game.kind("battle_tank").unwrap();
    let a = r.game.spawn(tank, 0, 12, 8);
    r.game.spawn(tank, 1, 15, 8);
    let cam = Camera { x: 12.0 * 32.0 - 100.0, y: 8.0 * 32.0 - 100.0, zoom: 1.0 };
    let blank = {
        let mut s = Scene::default();
        frame(&mut r, &mut s, &cam)
    };
    let mut drew_effect = false;
    for _ in 0..90 {
        scene.before_step(&r.game);
        r.game.step(1);
        scene.after_step(&r.game);
        let shooting = !r.game.state.projectiles.is_empty();
        let image = frame(&mut r, &mut scene, &cam);
        drew_effect |= shooting && image != blank;
    }
    let fired = r.game.events.iter().filter(|e| e.name() == "fired").count();
    println!("unit {a}: {fired} shots fired");
    assert!(fired > 0 && drew_effect);
}

#[test]
fn recolouring_swaps_exact_remap_pixels_only() {
    let remap = [[0x40, 0, 0x40], [0x80, 0, 0x80]];
    let ramp = [[1, 2, 3], [4, 5, 6]];
    let mut px = vec![0x40, 0, 0x40, 255, 0x80, 0, 0x80, 128, 0x41, 0, 0x40, 255];
    art::recolour(&mut px, &remap, &ramp);
    assert_eq!(px, [1, 2, 3, 255, 4, 5, 6, 128, 0x41, 0, 0x40, 255]);
}
