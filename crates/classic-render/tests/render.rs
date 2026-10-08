//! The renderer, drawn offscreen with no window: the generic pack's art on the test map, in each faction's colours.
//!
//! These need a GPU adapter; a software one is enough (CI installs Mesa's). Each test prints what it drew, and the
//! first writes the frame to `target/render-test.png` so a person can look at it.

use classic_render::art::{self, Art};
use classic_render::platform::{Files, Gpu, SpriteBatch, gpu::OFFSCREEN_FORMAT};
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

/// Pixels of `image` in the rectangle (x, y, w, h) darker than any ground: sprite outlines and dark bodies. Rows with
/// a long dark run are a health bar and are left out.
fn count_dark(image: &[u8], (x, y, w, h): (u32, u32, u32, u32)) -> usize {
    let dark = |px: u32, py: u32| {
        let i = ((py * W + px) * 4) as usize;
        u32::from(image[i]) * 54 + u32::from(image[i + 1]) * 183 + u32::from(image[i + 2]) * 19 < 70 * 256
    };
    let mut n = 0;
    for py in y..(y + h).min(H) {
        let row = (x..(x + w).min(W)).filter(|&px| dark(px, py)).count();
        if row < 16 {
            n += row;
        }
    }
    n
}

/// Pixels of `image` in the rectangle (x, y, w, h) close to a shade between two neighbours of `ramp`: team paint on
/// a studio sprite, which takes shades between the ramp's own.
fn count_near(image: &[u8], (x, y, w, h): (u32, u32, u32, u32), ramp: &[[u8; 3]]) -> usize {
    let near = |p: &[u8]| {
        ramp.windows(2).any(|ab| {
            (0..=16).any(|t| {
                (0..3)
                    .map(|c| (i32::from(ab[0][c]) * (16 - t) + i32::from(ab[1][c]) * t) / 16)
                    .zip(p)
                    .all(|(v, &q)| (v - i32::from(q)).abs() <= 12)
            })
        })
    };
    let mut n = 0;
    for py in y..(y + h).min(H) {
        for px in x..(x + w).min(W) {
            let i = ((py * W + px) * 4) as usize;
            if near(&image[i..i + 3]) {
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
    let own = count_near(&image, area, &ramp(&r.ramps[0]));
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
    let theirs = count_near(&image, tank, &ramp(&r.ramps[1]));
    let ours = count_near(&image, tank, &ramp(&r.ramps[0]));
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

#[test]
fn art_fetched_into_memory_draws_the_same_as_art_read_from_its_folder() {
    // The browser build fetches the files `art.json` names, then loads them from memory.
    let mut r = rig();
    let dir = art::art_dir(&setting::load("generic").unwrap());
    let index = std::fs::read_to_string(dir.join(art::ART_INDEX)).unwrap();
    let mut files = std::collections::BTreeMap::new();
    // Files it names that don't exist (studio sprites most ids don't have yet) come back as nothing, then the
    // atlas pages the sprites it found are on are fetched in turn.
    for f in art::art_files(&index).unwrap().into_iter().chain([art::ART_INDEX.to_string()]) {
        if let Ok(bytes) = std::fs::read(dir.join(&f)) {
            files.insert(f, bytes);
        }
    }
    // A page with no team paint (effects) has no mask, so that one comes back as nothing too.
    for f in art::atlas_files(&files) {
        if let Ok(bytes) = std::fs::read(dir.join(&f)) {
            files.insert(f, bytes);
        }
    }
    assert!(files.contains_key("art/sprites/effects-0.png"), "and detailed effects");
    assert!(files.keys().any(|f| f.ends_with(".mask.png")), "the generic pack has studio sprites");
    let memory = Files::Memory { label: "fetched".into(), files };
    let mut scene = Scene::default();
    let cam = Camera { x: 0.0, y: 0.0, zoom: 1.0 };
    let from_folder = frame(&mut r, &mut scene, &cam);
    r.art = Art::from_files(&r.gpu, &mut r.batch, &memory, &r.ramps).unwrap();
    assert_eq!(frame(&mut r, &mut scene, &cam), from_folder);
}

#[test]
fn a_squad_is_drawn_as_soldiers_who_fall_as_it_is_hurt() {
    // Squads are still planned in the rules data, so this game gives the generic squad a body borrowed from the tank.
    let mut r = rig();
    let pack = setting::load("generic").unwrap();
    let mut table = pack.rules.clone();
    let mut squad = table.entities["battle_tank"].clone();
    squad.weapon = None;
    table.entities.insert("infantry_squad".into(), squad);
    let rules = Rules::from_table(&table).unwrap();
    r.game = Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    let kind = r.game.rules.kind_id("infantry_squad").unwrap();
    let id = r.game.spawn(kind, 0, 12, 8);
    assert!(r.art.squad("infantry_squad").is_some(), "the generic pack lists the squad");

    let mut scene = Scene::default();
    let cam = Camera { x: 12.0 * 32.0 - W as f32 / 2.0, y: 8.0 * 32.0 - H as f32 / 2.0, zoom: 1.0 };
    let area = (W / 2 - 24, H / 2 - 24, 80, 80);
    // A studio soldier is a few pixels of dark outline and body with a fleck of team paint, so count the dark pixels
    // (sand, shadows and the ground's specks are all lighter).
    let soldiers = |r: &mut Rig, scene: &mut Scene| count_dark(&frame(r, scene, &cam), area);
    let full = soldiers(&mut r, &mut scene);
    let max = r.game.rules.kind(kind).max_health;
    r.game.state.entities.iter_mut().find(|e| e.id == id).unwrap().health = max / 3;
    let falling = soldiers(&mut r, &mut scene);
    for _ in 0..50 {
        scene.before_step(&r.game);
        r.game.step(1);
        scene.after_step(&r.game);
    }
    let one = soldiers(&mut r, &mut scene);
    println!("soldier pixels: {full} at full health, {falling} as two fall, {one} once they have gone");
    assert!(full > 30, "the squad is on screen");
    // The first death frame is not quite the standing pose, so allow a little either way.
    assert!(falling.abs_diff(full) * 4 < full, "the two lost soldiers are still on screen as they start to fall");
    assert!(one * 2 < full && one * 4 > full, "one soldier of three is left");
}

#[test]
fn turret_barrel_tips_are_measured_from_their_frames() {
    let r = rig();
    for id in ["battle_tank", "gun_turret"] {
        let (s, _) = r.art.studio.get(id, 0).expect("the generic pack has studio sprites");
        let tip = |facing| s.muzzle(&[], facing, facing, 0).unwrap();
        let (n, e, so, w) = (tip(0), tip(64), tip(128), tip(192));
        println!("{id}: north {n:?}, east {e:?}, south {so:?}, west {w:?}");
        assert!(e.x > n.x + 8.0 && w.x < n.x - 8.0, "{id}: the tip swings out east and west");
        assert!(so.y > n.y + 8.0, "{id}: the tip is lower on screen facing south");
    }
}

#[test]
fn effects_play_from_shots_damage_and_kills() {
    let mut r = rig();
    let mut scene = Scene::default();
    let tank = r.game.kind("battle_tank").unwrap();
    r.game.spawn(tank, 0, 12, 8);
    r.game.spawn(tank, 1, 15, 8);
    let cam = Camera { x: 12.0 * 32.0 - 100.0, y: 8.0 * 32.0 - 100.0, zoom: 1.0 };
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..450 {
        scene.before_step(&r.game);
        r.game.step(1);
        scene.after_step(&r.game);
        frame(&mut r, &mut scene, &cam);
        seen.extend(scene.fx.counts().into_keys());
    }
    println!("effects seen: {seen:?}");
    for id in ["muzzle_flash_gun", "explosion_small", "smoke_puff", "explosion_medium"] {
        assert!(seen.contains(id), "{id} played");
    }
    assert!(r.art.fx("explosion_medium").is_some(), "the generic pack has detailed effects");
}
