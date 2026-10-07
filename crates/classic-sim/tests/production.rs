//! Production (rules-economy-production.md, sections 7, 8 and 10): queues, paying while building, the power factor,
//! pausing for funds, cancelling, the yard's ready-and-place flow, unit exits and prerequisites.
//!
//! The map below is all rock with no resource, so no delivery ever changes a player's credits. Player 0's 3x2
//! refinery stands at (1, 1) and its tank at (4, 3). The tests spawn a power plant at (5, 1), a heavy factory at
//! (8, 1) (exit tile (9, 3)) and a construction yard at (12, 1). A battle tank costs 600 and takes 450 ticks.

use classic_data::{RulesTable, json};
use classic_sim::world::Event;
use classic_sim::{CommandOrder, EntryState, Game, GameOptions, Kind, ProduceError, Rules, Tile};

const MAP: &str = "\
################
#1##############
################
################
################
################
################
################
";

fn game(rules: Option<&Rules>) -> Game {
    Game::new(GameOptions { map: MAP, seed: 1, players: None, rules }).expect("map is valid")
}

fn tuned(tuning: &str) -> Rules {
    let mut t = RulesTable::builtin();
    let errors = t.apply_tuning(&json::parse(tuning).unwrap());
    assert!(errors.is_empty(), "{errors:?}");
    Rules::from_table(&t).unwrap()
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

/// Player 0's base with a power plant, a heavy factory and a yard, and `credits` to spend. Returns the factory's id.
fn base(g: &mut Game, credits: i64, plant: bool) -> u32 {
    if plant {
        g.spawn(kind(g, "power_plant"), 0, 5, 1);
    }
    let factory = g.spawn(kind(g, "heavy_factory"), 0, 8, 1);
    g.spawn(kind(g, "construction_yard"), 0, 12, 1);
    g.state.players[0].credits = credits;
    factory
}

fn credits(g: &Game) -> i64 {
    g.state.players[0].credits
}

fn queue(g: &Game, id: u32) -> Vec<(String, EntryState, i64, i64)> {
    let e = g.state.entity(id).unwrap();
    e.queue.iter().map(|q| (g.rules.kind(q.item).id.clone(), q.state, q.progress, q.paid)).collect()
}

fn produce(g: &mut Game, id: &str) {
    let k = kind(g, id);
    g.order(0, &[], CommandOrder::Produce { kind: k });
}

fn rejections(g: &Game) -> Vec<ProduceError> {
    g.events
        .iter()
        .filter_map(|e| if let Event::ProductionRejected { reason, .. } = *e { Some(reason) } else { None })
        .collect()
}

fn built_at(g: &Game) -> Vec<(u32, Tile)> {
    g.events
        .iter()
        .filter_map(|e| match *e {
            Event::UnitBuilt { entity, tick, .. } => Some((tick, g.state.entity(entity).unwrap().tile())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_tank_is_paid_for_steadily_and_leaves_by_the_exit_when_done() {
    let mut g = game(None);
    let factory = base(&mut g, 600, true);
    assert_eq!(g.power(0).factor(&g.rules), 100);
    produce(&mut g, "battle_tank");
    g.step(225);
    println!("tick 225: {} credits, {:?}", credits(&g), queue(&g, factory));
    assert_eq!(credits(&g), 300);
    assert_eq!(queue(&g, factory), [("battle_tank".into(), EntryState::Building, 22_500, 300)]);
    g.step(225);
    assert_eq!(credits(&g), 0, "paid in full, exactly");
    assert!(queue(&g, factory).is_empty());
    assert_eq!(built_at(&g), [(449, Tile { x: 9, y: 3 })], "on the 450th tick, at the exit below the middle column");
}

#[test]
fn a_shortfall_slows_production_to_the_power_factor() {
    let mut g = game(None);
    let factory = base(&mut g, 600, false);
    // A refinery (30) and a heavy factory (35) with no supply: the factor sits at its 25% floor.
    assert_eq!(g.power(0).factor(&g.rules), 25);
    produce(&mut g, "battle_tank");
    g.step(450);
    assert_eq!(queue(&g, factory), [("battle_tank".into(), EntryState::Building, 11_250, 150)]);
    g.step(1350);
    assert_eq!((credits(&g), built_at(&g).len()), (0, 1), "four times as long, the same price");
}

#[test]
fn production_pauses_for_funds_without_losing_progress_and_resumes_by_itself() {
    let mut g = game(None);
    let factory = base(&mut g, 100, true);
    produce(&mut g, "battle_tank");
    g.step(200);
    // 100 credits buy 75 ticks of a 600-credit, 450-tick tank (7,500 hundredths owes exactly 100).
    let paused: Vec<u32> =
        g.events.iter().filter(|e| matches!(e, Event::ProductionPaused { .. })).map(|e| e.tick()).collect();
    println!("paused at {paused:?}, {:?}", queue(&g, factory));
    assert_eq!(paused, [75], "reported once");
    assert_eq!(queue(&g, factory), [("battle_tank".into(), EntryState::Paused, 7_500, 100)]);
    assert_eq!(credits(&g), 0);
    g.state.players[0].credits = 1000;
    g.step(375);
    assert_eq!((credits(&g), built_at(&g).len()), (500, 1), "the rest, 500, paid as it finished");
}

#[test]
fn cancelling_refunds_exactly_what_was_paid() {
    let mut g = game(None);
    let factory = base(&mut g, 600, true);
    produce(&mut g, "battle_tank");
    g.step(180);
    assert_eq!(queue(&g, factory)[0].3, 240, "40% built, 40% paid");
    let tank = kind(&g, "battle_tank");
    g.order(0, &[], CommandOrder::Cancel { kind: tank });
    g.step(1);
    assert_eq!(credits(&g), 600);
    assert!(queue(&g, factory).is_empty());
    assert!(g.events.iter().any(|e| matches!(e, Event::ProductionCancelled { refund: 240, .. })));
}

#[test]
fn a_queue_holds_five_and_builds_them_in_order() {
    let rules = tuned(r#"{ "modules": { "production": { "instant_build": 1 } } }"#);
    let mut g = game(Some(&rules));
    let factory = base(&mut g, 100_000, true);
    for id in ["battle_tank", "harvester", "battle_tank", "battle_tank", "harvester", "battle_tank"] {
        produce(&mut g, id);
    }
    g.step(1);
    assert_eq!(rejections(&g), [ProduceError::QueueFull], "the sixth is refused");
    // Instant build finishes the head on the tick it gets there; the next one starts the tick after.
    g.step(4);
    let made: Vec<String> = g
        .events
        .iter()
        .filter_map(|e| if let Event::UnitBuilt { kind, .. } = *e { Some(g.rules.kind(kind).id.clone()) } else { None })
        .collect();
    assert_eq!(made, ["battle_tank", "harvester", "battle_tank", "battle_tank", "harvester"]);
    assert!(queue(&g, factory).is_empty());
    assert_eq!(credits(&g), 100_000 - 3 * 600 - 2 * 300);
    let harvester = g.state.entities.iter().rev().find(|e| g.rules.kind(e.kind).id == "harvester").unwrap();
    assert_eq!(harvester.order, classic_sim::Order::Harvest, "a new harvester goes to work");
}

#[test]
fn what_a_player_can_build_depends_on_what_they_own() {
    let mut g = game(None);
    assert_eq!(g.can_build(0, kind(&g, "construction_yard")), Err(ProduceError::NotBuildable));
    let plant = kind(&g, "power_plant");
    assert_eq!(g.can_build(0, kind(&g, "barracks")), Err(ProduceError::Requires { kind: plant }));
    assert_eq!(g.can_build(0, kind(&g, "power_plant")), Ok(()));
    produce(&mut g, "power_plant");
    g.step(1);
    assert_eq!(rejections(&g), [ProduceError::NoFactory], "no yard yet");
    base(&mut g, 1000, true);
    assert_eq!(g.can_build(0, kind(&g, "barracks")), Ok(()), "now there is a plant");
}

#[test]
fn a_finished_building_waits_at_the_yard_until_placed() {
    let mut g = game(None);
    base(&mut g, 1000, true);
    let yard = g.state.entities.iter().find(|e| g.rules.kind(e.kind).id == "construction_yard").unwrap().id;
    produce(&mut g, "power_plant");
    produce(&mut g, "wall");
    g.step(450);
    assert!(g.events.iter().any(|e| matches!(e, Event::BuildingReady { tick: 449, .. })));
    g.step(100);
    let q = queue(&g, yard);
    assert_eq!((q[0].1, q[1].1, q[1].2), (EntryState::Ready, EntryState::Waiting, 0), "the wall waits its turn");
    let plant = kind(&g, "power_plant");
    g.order(0, &[], CommandOrder::Place { kind: plant, x: 5, y: 3 });
    g.step(1);
    assert!(g.events.iter().any(|e| matches!(e, Event::BuildingPlaced { x: 5, y: 3, .. })));
    let q = queue(&g, yard);
    assert_eq!((q[0].1, q[0].2, q[0].3), (EntryState::Building, 100, 0), "the yard moves on to the wall");
    assert_eq!(credits(&g), 700, "the plant paid; one tick of a 25-credit wall rounds down to nothing yet");
}

#[test]
fn a_unit_with_no_free_exit_waits_until_one_frees_up() {
    let rules = tuned(r#"{ "modules": { "production": { "instant_build": 1 } } }"#);
    let mut g = game(Some(&rules));
    let factory = base(&mut g, 1000, true);
    // The exit (9, 3) and the open tiles round it; the row above is the factory itself.
    let tank = kind(&g, "battle_tank");
    let blockers: Vec<u32> =
        [(8, 3), (9, 3), (10, 3), (8, 4), (9, 4), (10, 4)].iter().map(|&(x, y)| g.spawn(tank, 0, x, y)).collect();
    produce(&mut g, "battle_tank");
    g.step(5);
    assert_eq!(queue(&g, factory)[0].1, EntryState::Blocked);
    assert!(built_at(&g).is_empty());
    g.order(0, &[blockers[3]], CommandOrder::Move { x: 8, y: 7 });
    g.step(30);
    let out = built_at(&g);
    println!("left at {out:?}");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].1, Tile { x: 8, y: 4 }, "the first free tile in row order");
}

#[test]
fn orders_go_to_the_first_factory_unless_one_is_named() {
    let mut g = game(None);
    let first = base(&mut g, 10_000, true);
    let second = g.spawn(kind(&g, "heavy_factory"), 0, 8, 5);
    produce(&mut g, "battle_tank");
    let tank = kind(&g, "battle_tank");
    g.order(0, &[second], CommandOrder::Produce { kind: tank });
    g.step(1);
    assert_eq!((queue(&g, first).len(), queue(&g, second).len()), (1, 1), "two factories build two at once");
}

#[test]
fn production_replays_from_the_command_log() {
    let setup = |g: &mut Game| {
        base(g, 5000, true);
    };
    let mut live = game(None);
    setup(&mut live);
    produce(&mut live, "battle_tank");
    produce(&mut live, "power_plant");
    live.step(200);
    let tank = kind(&live, "battle_tank");
    live.order(0, &[], CommandOrder::Cancel { kind: tank });
    produce(&mut live, "harvester");
    live.step(700);
    let plant = kind(&live, "power_plant");
    live.order(0, &[], CommandOrder::Place { kind: plant, x: 5, y: 3 });
    live.step(100);
    let log = live.command_log().to_vec();
    let mut replay = game(None);
    setup(&mut replay);
    for t in 0..1000 {
        for c in log.iter().filter(|c| c.tick == t) {
            replay.order(c.command.player, &c.command.ids, c.command.order);
        }
        replay.step(1);
    }
    println!("live {} replay {}, credits {}", live.hash(), replay.hash(), credits(&live));
    assert!(live.events.iter().any(|e| matches!(e, Event::BuildingPlaced { .. })));
    assert!(live.events.iter().any(|e| matches!(e, Event::UnitBuilt { .. })));
    assert_eq!(replay.hash(), live.hash());
}
