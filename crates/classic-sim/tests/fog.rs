//! Fog of war (rules-world.md, sections 2 and 3): off unless a pack turns it on; each player sees round its own
//! entities, from a building's edge; explored ground stays explored; fog hides enemy units and shows enemy buildings
//! as last seen; targets must be in sight; firing shows the shooter; the counts never drift.
//!
//! The map: each player's base on a rock shelf at a side and open ground between them.

use classic_data::{RulesTable, json};
use classic_sim::world::{Event, Order};
use classic_sim::{CommandOrder, Game, GameOptions, Rules, TileView, vision};

const MAP: &str = "\
##########......................##########
#1########......................########2#
##########......................##########
##########......................##########
##########......................##########
##########......................##########
##########......................##########
##########......................##########
##########......................##########
##########......................##########
##########......................##########
##########......................##########
";

/// The engine's rules with fog on, as a pack's `features` would turn it on, then `tuning`.
fn rules(tuning: &str) -> Rules {
    let mut t = RulesTable::builtin();
    t.modules.get_mut("fog").unwrap().numbers.get_mut("on").unwrap().value = 1;
    let errors = t.apply_tuning(&json::parse(tuning).unwrap());
    assert!(errors.is_empty(), "{errors:?}");
    Rules::from_table(&t).unwrap()
}

fn game(rules: &Rules) -> Game {
    Game::new(GameOptions { map: MAP, seed: 7, players: None, rules: Some(rules) }).expect("map is valid")
}

fn spawn(g: &mut Game, id: &str, owner: u32, x: i32, y: i32) -> u32 {
    let k = g.kind(id).unwrap();
    g.spawn(k, owner, x, y)
}

/// Clear every starting unit away, so only the starting buildings and what a test adds give sight.
fn bases_only(g: &mut Game) {
    let rules = g.rules.clone();
    g.state.entities.retain(|e| rules.kind(e.kind).building);
    vision::tick(&mut g.state, &rules);
}

fn assert_counts_hold(g: &Game) {
    let fresh = vision::recount(&g.state, &g.rules).unwrap();
    let now: Vec<Vec<u16>> = g.state.vision.as_ref().unwrap().players.iter().map(|p| p.seen.clone()).collect();
    assert_eq!(now, fresh, "running counts drifted from a fresh count at tick {}", g.state.tick);
}

#[test]
fn off_unless_a_pack_turns_it_on() {
    let g = Game::new(GameOptions { map: MAP, seed: 7, players: None, rules: None }).unwrap();
    assert!(g.state.vision.is_none());
    assert_eq!(g.tile_view(0, 38, 1), TileView::Visible);
    let far = g.state.entities.iter().find(|e| e.owner == 1).unwrap().id;
    // Fog off hashes as before: the golden hashes (classic-tools' golden.rs) pin that.
    assert!(g.visible(0, far));
}

#[test]
fn a_player_starts_seeing_its_own_base_only() {
    let r = rules("{}");
    let g = game(&r);
    // Its own yard is in sight; the other base, across the open ground, is shroud.
    assert_eq!(g.tile_view(0, 1, 1), TileView::Visible);
    assert_eq!(g.tile_view(0, 40, 1), TileView::Shroud);
    assert_eq!(g.tile_view(1, 40, 1), TileView::Visible);
    assert_eq!(g.tile_view(1, 1, 1), TileView::Shroud);
    let theirs = g.state.entities.iter().find(|e| e.owner == 1).unwrap().id;
    let mine = g.state.entities.iter().find(|e| e.owner == 0).unwrap().id;
    assert!(!g.visible(0, theirs) && !g.known(0, theirs));
    assert!(g.visible(0, mine));
    assert_counts_hold(&g);
}

#[test]
fn sight_is_a_rounded_disc_from_a_buildings_edge() {
    let r = rules("{}");
    let mut g = game(&r);
    bases_only(&mut g);
    // A radar (vision 6) standing alone in the middle, footprint (20, 4) to (21, 5).
    spawn(&mut g, "radar", 0, 20, 4);
    let v = |x, y| g.tile_view(0, x, y);
    // Six tiles straight out from each edge, and no further.
    assert_eq!(v(27, 4), TileView::Visible);
    assert_eq!(v(28, 4), TileView::Shroud);
    assert_eq!(v(14, 5), TileView::Visible);
    assert_eq!(v(13, 5), TileView::Shroud);
    assert_eq!(v(20, 11), TileView::Visible);
    // dx*dx + dy*dy <= r*r + r: (4, 5) from the corner is in (41 <= 42), (5, 5) is out (50).
    assert_eq!(v(25, 10), TileView::Visible);
    assert_eq!(v(26, 10), TileView::Shroud);
    assert_counts_hold(&g);
}

#[test]
fn explored_ground_stays_explored_after_a_unit_leaves() {
    let r = rules("{}");
    let mut g = game(&r);
    bases_only(&mut g);
    let bike = spawn(&mut g, "scout_bike", 0, 12, 6);
    g.order(0, &[bike], CommandOrder::Move { x: 28, y: 6 });
    g.step(200);
    assert_eq!(g.state.entity(bike).unwrap().tile().x, 28);
    assert_eq!(g.tile_view(0, 28, 6), TileView::Visible);
    // Where it passed is fog now: explored, out of sight.
    assert_eq!(g.tile_view(0, 16, 6), TileView::Fog);
    // Beside its road, past its sight, is still shroud.
    assert_eq!(g.tile_view(0, 16, 11), TileView::Shroud);
    assert_counts_hold(&g);
}

#[test]
fn fog_hides_enemy_units_and_shroud_only_shows_them() {
    for hide in [1, 0] {
        let r = rules(&format!(r#"{{ "modules": {{ "fog": {{ "hide": {hide} }} }} }}"#));
        let mut g = game(&r);
        bases_only(&mut g);
        // Player 0's bike explores the middle, then goes home; an enemy harvester then parks where it was.
        let bike = spawn(&mut g, "scout_bike", 0, 12, 6);
        g.order(0, &[bike], CommandOrder::Move { x: 20, y: 6 });
        g.step(120);
        g.order(0, &[bike], CommandOrder::Move { x: 12, y: 6 });
        g.step(120);
        let theirs = spawn(&mut g, "harvester", 1, 20, 6);
        g.state.entities.iter_mut().find(|e| e.id == theirs).unwrap().order = Order::Idle;
        g.step(1);
        assert_eq!(g.tile_view(0, 20, 6), TileView::Fog);
        assert_eq!(g.visible(0, theirs), hide == 0, "hide {hide}");
        // In sight, it shows either way.
        g.order(0, &[bike], CommandOrder::Move { x: 18, y: 6 });
        g.step(60);
        assert!(g.visible(0, theirs), "hide {hide}");
        assert_counts_hold(&g);
    }
}

#[test]
fn an_enemy_building_seen_once_stays_as_a_ghost_until_seen_gone() {
    let r = rules("{}");
    let mut g = game(&r);
    bases_only(&mut g);
    let silo = spawn(&mut g, "silo", 1, 22, 9);
    assert!(!g.known(0, silo));
    // A bike goes to look at it, then back out of sight.
    let bike = spawn(&mut g, "scout_bike", 0, 14, 4);
    g.order(0, &[bike], CommandOrder::Move { x: 20, y: 6 });
    g.step(60);
    let ghost = g.state.vision.as_ref().unwrap().ghost(0, silo).cloned().expect("seen, so remembered");
    assert_eq!((ghost.x, ghost.y, ghost.owner), (22, 9, 1));
    g.order(0, &[bike], CommandOrder::Move { x: 12, y: 1 });
    g.step(90);
    assert!(!g.visible(0, silo) && g.known(0, silo), "out of sight, kept as a ghost");
    // It is destroyed while nobody of player 0's looks: the ghost stays.
    g.state.entities.iter_mut().find(|e| e.id == silo).unwrap().health = 0;
    g.step(2);
    assert!(g.state.entity(silo).is_none());
    assert!(g.state.vision.as_ref().unwrap().ghost(0, silo).is_some(), "not seen gone yet");
    // Seeing the empty ground drops it.
    g.order(0, &[bike], CommandOrder::Move { x: 19, y: 8 });
    g.step(90);
    assert!(g.state.vision.as_ref().unwrap().ghost(0, silo).is_none());
    assert_counts_hold(&g);
}

#[test]
fn attack_orders_need_a_target_in_sight_or_a_ghost() {
    let r = rules("{}");
    let mut g = game(&r);
    bases_only(&mut g);
    let tank = spawn(&mut g, "battle_tank", 0, 12, 6);
    let hidden = spawn(&mut g, "harvester", 1, 30, 10);
    g.state.entities.iter_mut().find(|e| e.id == hidden).unwrap().order = Order::Idle;
    g.order(0, &[tank], CommandOrder::Attack { target: hidden });
    g.step(1);
    assert_eq!(g.state.entity(tank).unwrap().order, Order::Idle, "an unseen unit can't be ordered on");
    // An enemy building out of sight now but seen before can.
    let yard = g.state.entities.iter().find(|e| e.owner == 1).unwrap().id;
    let scout = spawn(&mut g, "scout_bike", 0, 36, 1);
    g.step(1);
    assert!(g.visible(0, yard));
    g.state.entities.retain(|e| e.id != scout);
    vision::tick(&mut g.state, &r);
    assert!(!g.visible(0, yard) && g.known(0, yard));
    g.order(0, &[tank], CommandOrder::Attack { target: yard });
    g.step(1);
    let t = g.state.entity(tank).unwrap();
    assert_eq!((t.order, t.target), (Order::Attack, Some(yard)));
}

#[test]
fn units_only_pick_targets_their_side_can_see() {
    let r = rules("{}");
    let mut g = game(&r);
    bases_only(&mut g);
    // Tanks look for targets 5 tiles out but see 3: four tiles apart, neither side sees the other.
    let a = spawn(&mut g, "battle_tank", 0, 16, 6);
    let b = spawn(&mut g, "battle_tank", 1, 20, 6);
    g.step(60);
    let fired = |g: &Game| g.events.iter().any(|e| matches!(e, Event::Fired { .. }));
    assert!(!fired(&g), "nobody sees anybody");
    assert_eq!(g.state.entity(a).unwrap().target, None);
    // A bike of player 0's comes to look: now its tank fires; player 1 sees nothing until it is shot.
    spawn(&mut g, "scout_bike", 0, 18, 9);
    g.step(30);
    assert_eq!(g.state.entity(a).unwrap().target, Some(b));
    assert!(fired(&g));
    assert_counts_hold(&g);
}

#[test]
fn firing_shows_the_shooter_to_the_player_it_fires_at() {
    let r = rules("{}");
    let mut g = game(&r);
    bases_only(&mut g);
    // A bike (vision 4, guns reach 3) and a harvester (vision 2) three tiles apart: the bike sees it, not back.
    let harvester = spawn(&mut g, "harvester", 0, 16, 6);
    g.state.entities.iter_mut().find(|e| e.id == harvester).unwrap().order = Order::Idle;
    let bike = spawn(&mut g, "scout_bike", 1, 19, 6);
    assert!(g.visible(1, harvester) && !g.visible(0, bike));
    let shot = (0..60).find(|_| {
        g.step(1);
        g.events.iter().any(|e| matches!(e, Event::Fired { unit, .. } if *unit == bike))
    });
    assert!(shot.is_some(), "the bike opens fire");
    assert!(g.visible(0, bike), "shown by its firing");
    assert_counts_hold(&g);
    // Once it stops firing it fades from view after the reveal's ticks.
    g.order(1, &[bike], CommandOrder::Move { x: 19, y: 6 });
    g.state.entities.iter_mut().find(|e| e.id == harvester).unwrap().health = 0;
    g.step(r.fog.as_ref().unwrap().reveal_ticks + 10);
    assert!(!g.visible(0, bike));
    assert_counts_hold(&g);
}

#[test]
fn start_explored_test_switch_lifts_the_shroud() {
    let r = rules(r#"{ "modules": { "fog": { "start_explored": 1 } } }"#);
    let g = game(&r);
    assert_eq!(g.tile_view(0, 40, 6), TileView::Fog);
    assert_eq!(g.tile_view(0, 1, 1), TileView::Visible);
}

#[test]
fn same_game_same_hash_and_the_counts_never_drift() {
    let r = rules("{}");
    let run = || {
        let mut g = game(&r);
        let mine: Vec<u32> = g.state.entities.iter().filter(|e| e.owner == 0).map(|e| e.id).collect();
        let theirs: Vec<u32> = g.state.entities.iter().filter(|e| e.owner == 1).map(|e| e.id).collect();
        g.order(0, &mine, CommandOrder::Move { x: 30, y: 6 });
        g.order(1, &theirs, CommandOrder::Move { x: 11, y: 6 });
        let mut hashes = Vec::new();
        for _ in 0..12 {
            g.step(50);
            assert_counts_hold(&g);
            hashes.push(g.hash());
        }
        hashes
    };
    assert_eq!(run(), run());
}
