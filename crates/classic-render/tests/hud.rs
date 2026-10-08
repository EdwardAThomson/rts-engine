//! The production rail and building placement, driven by clicks the way a player drives them. Every click becomes
//! a command, so these check the commands' effect on the game. The last test draws the HUD (it needs a GPU adapter,
//! a software one will do) and writes it to `target/hud-test.png` for a person to look at.

use classic_render::art::{self, Art};
use classic_render::hud::{Button, Click, Hud, Icon, RAIL_W, View};
use classic_render::platform::{Files, Font, Gpu, Rect, SpriteBatch, gpu::OFFSCREEN_FORMAT};
use classic_render::{Camera, Scene, Skin};
use classic_sim::world::Event;
use classic_sim::{EntryState, Game, GameOptions, Kind, Rules};
use classic_tools::setting;

const MAP: &str = include_str!("../../../maps/skirmish-01.txt");
const SCREEN: (f32, f32) = (1024.0, 768.0);
const TILE: f32 = 32.0;

fn game() -> (Game, Hud) {
    let pack = setting::load("generic").unwrap();
    let rules = Rules::from_table(&pack.rules).unwrap();
    let game = Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    let hud = Hud::new(&pack, &Files::Dir(pack.dir.clone()), &game, 0, 0);
    (game, hud)
}

/// The map's top-left corner at the screen's, at zoom 1; the rail covers the right.
fn view() -> View {
    View { cam: Camera { x: 0.0, y: 0.0, zoom: 1.0 }, screen: SCREEN, tile: TILE }
}

fn centre(r: Rect) -> (f32, f32) {
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

fn icon(game: &Game, hud: &Hud, id: &str) -> Icon {
    let kind = game.kind(id).unwrap();
    *hud.layout(game, SCREEN).icons.iter().find(|i| i.item == kind).unwrap_or_else(|| panic!("no {id} icon"))
}

/// The screen point at the middle of tile (x, y).
fn tile_point(x: i32, y: i32) -> (f32, f32) {
    let v = view();
    v.cam.to_screen((x as f32 + 0.5) * TILE, (y as f32 + 0.5) * TILE)
}

fn queued(game: &Game, kind: Kind) -> usize {
    game.state.entities.iter().filter(|e| e.owner == 0).flat_map(|e| &e.queue).filter(|q| q.item == kind).count()
}

#[test]
fn the_rail_offers_what_the_players_factories_can_build() {
    let (game, hud) = game();
    let l = hud.layout(&game, SCREEN);
    let tabs: Vec<&str> = l.tabs.iter().map(|t| game.rules.kind(t.factory).id.as_str()).collect();
    let items: Vec<(&str, bool)> =
        l.icons.iter().map(|i| (game.rules.kind(i.item).id.as_str(), i.status.needs.is_some())).collect();
    println!("tabs {tabs:?}\nopen tab items (id, locked) {items:?}");
    assert_eq!(tabs[0], "construction_yard", "the yard's tab comes first and is open");
    assert_eq!(l.open, Some(l.tabs[0].factory));
    // Everything shown is buildable now or one step away; nothing further up the tree shows.
    for i in &l.icons {
        match i.status.needs {
            None => assert!(game.can_build(0, i.item).is_ok()),
            Some(need) => assert!(game.can_build(0, need).is_ok(), "{:?} is one step away", i.item),
        }
    }
    assert!(items.contains(&("power_plant", false)));
    assert!(items.iter().any(|(_, locked)| *locked), "the next tier shows, locked");
    // Each tab is another factory kind the player owns.
    for t in &l.tabs {
        assert!(game.state.entities.iter().any(|e| e.owner == 0 && e.kind == t.factory));
    }
}

#[test]
fn clicks_on_the_rail_never_reach_the_world() {
    let (mut game, mut hud) = game();
    let v = view();
    assert!(hud.click(&mut game, &v, (SCREEN.0 - RAIL_W + 2.0, SCREEN.1 - 2.0), Button::Left, false).taken());
    assert!(
        hud.click(&mut game, &v, (SCREEN.0 - 2.0, 2.0), Button::Left, false).taken(),
        "the readout is part of the rail"
    );
    assert!(
        !hud.click(&mut game, &v, (600.0, 400.0), Button::Left, false).taken(),
        "the world gets clicks off the HUD"
    );
    assert!(!hud.click(&mut game, &v, (600.0, 400.0), Button::Right, false).taken());
    assert!(game.command_log().is_empty(), "empty rail and world clicks order nothing");
}

#[test]
fn shift_click_queues_five_and_right_clicks_cancel_with_refunds() {
    let (mut game, mut hud) = game();
    let v = view();
    let plant = game.kind("power_plant").unwrap();
    let start = game.state.players[0].credits;
    let at = centre(icon(&game, &hud, "power_plant").rect);
    hud.click(&mut game, &v, at, Button::Left, true);
    game.step(30);
    let paid = start - game.state.players[0].credits;
    println!("queued {}, paid {paid} after 30 ticks", queued(&game, plant));
    assert_eq!(queued(&game, plant), 5);
    assert!(paid > 0, "the head entry is being paid for");
    let st = icon(&game, &hud, "power_plant").status;
    assert_eq!(st.queued, 5);
    assert!(matches!(st.head, Some((EntryState::Building, p)) if p > 0.0));

    for _ in 0..2 {
        hud.click(&mut game, &v, at, Button::Right, false);
    }
    game.step(1);
    assert_eq!(queued(&game, plant), 3, "two right clicks cancel two");
    for _ in 0..3 {
        hud.click(&mut game, &v, at, Button::Right, false);
    }
    game.step(1);
    println!("after cancelling all: {} credits (started with {start})", game.state.players[0].credits);
    assert_eq!(queued(&game, plant), 0);
    assert_eq!(game.state.players[0].credits, start, "every credit paid came back");
}

#[test]
fn a_ready_building_goes_where_the_simulation_allows_and_nowhere_else() {
    let (mut game, mut hud) = game();
    let v = view();
    let plant = game.kind("power_plant").unwrap();
    let at = centre(icon(&game, &hud, "power_plant").rect);
    hud.click(&mut game, &v, at, Button::Left, false);
    let mut ticks = 0;
    while !matches!(icon(&game, &hud, "power_plant").status.head, Some((EntryState::Ready, _))) {
        game.step(1);
        ticks += 1;
        assert!(ticks < 5000, "the power plant finishes");
    }
    println!("ready after {ticks} ticks");
    // Before picking it up, a world click is the world's.
    assert!(!hud.click(&mut game, &v, (600.0, 400.0), Button::Left, false).taken());
    hud.click(&mut game, &v, at, Button::Left, false);
    assert_eq!(hud.placing, Some(plant), "clicking a ready icon picks the building up");

    // Every ghost agrees with the simulation's own check.
    let (mut good, mut bad) = (None, None);
    for ty in 0..game.map.height {
        for tx in 0..game.map.width {
            let p = tile_point(tx, ty);
            if p.0 >= SCREEN.0 - RAIL_W - 1.0 || p.1 >= SCREEN.1 {
                continue;
            }
            let g = hud.ghost(&game, &v, p.0, p.1).unwrap();
            assert_eq!(g.ok, game.can_place(0, plant, g.x, g.y).is_ok(), "ghost at {tx},{ty}");
            if g.ok {
                good.get_or_insert(p)
            } else {
                bad.get_or_insert(p)
            };
        }
    }
    let (good, bad) = (good.expect("somewhere fits"), bad.expect("somewhere doesn't"));

    // A click where it doesn't fit keeps it on the cursor and orders nothing.
    let orders = game.command_log().len();
    assert!(hud.click(&mut game, &v, bad, Button::Left, false).taken());
    assert_eq!(hud.placing, Some(plant));
    // A right click puts it back, still ready; picking it up again and clicking where it fits places it.
    assert!(hud.click(&mut game, &v, bad, Button::Right, false).taken());
    assert_eq!(hud.placing, None);
    hud.click(&mut game, &v, at, Button::Left, false);
    let g = hud.ghost(&game, &v, good.0, good.1).unwrap();
    assert!(hud.click(&mut game, &v, good, Button::Left, false).taken());
    game.step(1);
    assert_eq!(game.command_log().len(), orders + 1, "one place order, after the produce order");
    let placed = game.events.iter().any(
        |e| matches!(*e, Event::BuildingPlaced { kind, owner: 0, x, y, .. } if kind == plant && (x, y) == (g.x, g.y)),
    );
    println!("placed at {},{}: {placed}", g.x, g.y);
    assert!(placed);
    assert_eq!(hud.placing, None);
    assert_eq!(icon(&game, &hud, "power_plant").status.head, None, "nothing left in the yard");
}

#[test]
fn the_hud_draws_over_the_world_in_its_own_place() {
    let gpu = Gpu::headless().expect("a GPU adapter (a software one will do)");
    let (mut game, mut hud) = game();
    let pack = setting::load("generic").unwrap();
    let ramps = art::player_ramps(&pack, game.state.players.len());
    let mut batch = SpriteBatch::new(&gpu, OFFSCREEN_FORMAT);
    let art = Art::load(&gpu, &mut batch, &art::art_dirs(&pack), &ramps).unwrap();
    let font = Font::new(&gpu, &mut batch);
    let skin = Skin::load(&gpu, &mut batch, &[&Files::Dir(pack.dir.clone())]).unwrap();
    let v = view();
    let at = centre(icon(&game, &hud, "power_plant").rect);
    hud.click(&mut game, &v, at, Button::Left, true);
    game.step(60);

    let (w, h) = (SCREEN.0 as u32, SCREEN.1 as u32);
    let mut scene = Scene::default();
    scene.draw(&mut batch, &art, &game, &v.cam, SCREEN, 1.0);
    let world = batch.draw_to_image(&gpu, w, h, [0, 0, 0, 255]);
    scene.draw(&mut batch, &art, &game, &v.cam, SCREEN, 1.0);
    // The player's harvester selected, and a line in the feed.
    let harvester = game.kind("harvester").unwrap();
    let picked = game.state.entities.iter().find(|e| e.owner == 0 && e.kind == harvester).unwrap().id;
    hud.feed.say(&game, "low_power", None, classic_render::feed::Tone::Bad);
    hud.draw(&mut batch, &art, &skin, &game, &v, at, &[picked]);
    let image = batch.draw_to_image(&gpu, w, h, [0, 0, 0, 255]);
    let out = setting::root().join("target/hud-test.png");
    std::fs::write(&out, classic_tools::art::png::encode(w as usize, h as usize, &image)).unwrap();
    println!("wrote {}", out.display());

    let differs = |r: Rect| {
        let mut n = 0;
        for y in r.y as u32..(r.y + r.h) as u32 {
            for x in r.x as u32..(r.x + r.w) as u32 {
                let i = ((y * w + x) * 4) as usize;
                n += usize::from(image[i..i + 3] != world[i..i + 3]);
            }
        }
        n
    };
    let l = hud.layout(&game, SCREEN);
    let readout = differs(l.readout);
    let middle = differs(Rect::new(300.0, 300.0, 200.0, 150.0));
    assert_eq!(l.rail.x + l.rail.w, SCREEN.0, "the rail is on the right");
    println!("pixels changed: readout {readout}, middle of the world {middle}");
    assert!(readout > 1000, "the readout is drawn");
    let card = differs(l.card);
    let feed = differs(Rect::new(0.0, 0.0, 200.0, 24.0));
    println!("pixels changed: card {card}, feed {feed}");
    assert!(card > 1000, "the selection card is drawn");
    assert!(feed > 200, "the feed's line is drawn");
    // The card sits between the grid and the queue, inside the rail.
    assert!(l.icons.iter().all(|i| i.rect.y + i.rect.h <= l.card.y), "the grid stops above the card");
    assert!(l.card.y + l.card.h < l.minimap.y && l.rail.contains(l.card.x, l.card.y));
    assert_eq!(middle, 0, "the world away from the HUD is untouched");

    // The minimap shows the player's base in their colour, where the base is.
    let yard = game.state.entities.iter().find(|e| e.owner == 0 && e.kind == game.kind("construction_yard").unwrap());
    let t = yard.unwrap().tile();
    let m = l.minimap;
    let (bx, by) = (m.w / game.map.width as f32, m.h / game.map.height as f32);
    let (px, py) = ((m.x + (t.x as f32 + 1.0) * bx) as u32, (m.y + (t.y as f32 + 1.0) * by) as u32);
    let i = ((py * w + px) * 4) as usize;
    let want = art.owner_colour(0);
    println!("minimap at the yard: {:?}, player colour {want:?}", &image[i..i + 3]);
    assert_eq!(image[i..i + 3], want);

    // Text: one glyph lights exactly its own pixels.
    let font_px = {
        batch.fill(Rect::new(0.0, 0.0, 32.0, 32.0), [0, 0, 0, 255]);
        font.draw(&mut batch, "8", 4.0, 4.0, 2.0, [255; 4]);
        let img = batch.draw_to_image(&gpu, 32, 32, [0, 0, 0, 255]);
        img.chunks_exact(4).filter(|p| p[0] == 255).count()
    };
    // The 8 has 17 inked pixels, each drawn 2 by 2.
    assert_eq!(font_px, 17 * 4);
}

#[test]
fn the_minimap_moves_the_view_and_sends_orders_but_never_orders_itself() {
    let (mut game, mut hud) = game();
    let v = view();
    let l = hud.layout(&game, SCREEN);
    let m = l.minimap;
    println!("minimap {m:?} for a {}x{} map", game.map.width, game.map.height);
    assert!(l.rail.contains(m.x, m.y) && l.rail.contains(m.x + m.w - 1.0, m.y + m.h - 1.0), "inside the rail");
    assert!((m.w / m.h - game.map.width as f32 / game.map.height as f32).abs() < 0.01, "keeps the map's shape");
    // The minimap's centre is the map's centre; its corners are the map's corners.
    let mid = centre(m);
    let at = |x: f32, y: f32| hud.minimap_point(&l, &game, x, y).unwrap();
    let (cx, cy) = at(mid.0, mid.1);
    assert!((cx - game.map.width as f32 / 2.0).abs() < 0.5 && (cy - game.map.height as f32 / 2.0).abs() < 0.5);
    let (x0, y0) = at(m.x, m.y);
    assert!(x0 < 0.01 && y0 < 0.01);
    assert_eq!(hud.minimap_point(&l, &game, m.x - 1.0, m.y), None);
    // Left: centre there. Right: an order there, for the caller to give the selected units.
    let left = hud.click(&mut game, &v, mid, Button::Left, false);
    let right = hud.click(&mut game, &v, mid, Button::Right, false);
    println!("left {left:?}, right {right:?}");
    assert!(matches!(left, Click::Centre { x, y } if (x, y) == (cx, cy)));
    assert_eq!(right, Click::Order { x: cx.floor() as i32, y: cy.floor() as i32 });
    assert!(left.taken() && right.taken());
    assert!(game.command_log().is_empty(), "the minimap itself orders nothing");
}

#[test]
fn the_clock_shows_hours_minutes_and_seconds() {
    use classic_render::hud::clock_text;
    assert_eq!(clock_text(0), "0:00:00");
    assert_eq!(clock_text(252), "0:04:12");
    assert_eq!(clock_text(4500), "1:15:00");
    assert_eq!(clock_text(36_000 + 59), "10:00:59");
}
