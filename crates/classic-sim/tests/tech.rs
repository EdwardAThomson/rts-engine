//! Tech levels and factory upgrades (rules-economy-production.md sections 9 to 11). A game may have a tech level from
//! 1 to 8 that limits what anyone may build and how far each kind of factory may be upgraded; with none, everything
//! may be. An upgrade is an entry in the factory's own queue, built and paid like any other, that raises that one
//! building's level; items list the level of factory they need.
//!
//! Player 0's yard is at (1, 1); the tests add a power plant at (5, 1) and a heavy factory at (8, 1).

use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, ProduceError};

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

fn game() -> Game {
    Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: None }).expect("map is valid")
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

/// A power plant and a heavy factory for player 0, and `credits` to spend. Returns the factory's id.
fn base(g: &mut Game, credits: i64) -> u32 {
    g.spawn(kind(g, "power_plant"), 0, 5, 1);
    let factory = g.spawn(kind(g, "heavy_factory"), 0, 8, 1);
    g.state.players[0].credits = credits;
    factory
}

fn upgrade(g: &mut Game, ids: &[u32], id: &str, on: bool) {
    let k = kind(g, id);
    g.order(0, ids, CommandOrder::Upgrade { kind: k, on });
}

fn produce(g: &mut Game, ids: &[u32], id: &str) {
    let k = kind(g, id);
    g.order(0, ids, CommandOrder::Produce { kind: k });
}

fn rejections(g: &Game) -> Vec<ProduceError> {
    g.events
        .iter()
        .filter_map(|e| if let Event::ProductionRejected { reason, .. } = *e { Some(reason) } else { None })
        .collect()
}

fn level(g: &Game, id: u32) -> u32 {
    g.state.entity(id).unwrap().level
}

#[test]
fn the_tech_level_limits_what_anyone_may_build() {
    let mut g = game();
    base(&mut g, 10_000);
    g.spawn(kind(&g, "light_factory"), 0, 12, 1);
    let can = |g: &Game, id: &str| g.can_build(0, kind(g, id));
    // With no tech level, everything the player has the buildings for.
    assert_eq!(can(&g, "battle_tank"), Ok(()));
    assert_eq!(can(&g, "heavy_factory"), Ok(()));
    g.set_tech_level(Some(2));
    assert_eq!(can(&g, "power_plant"), Ok(()));
    assert_eq!(can(&g, "light_factory"), Ok(()));
    assert_eq!(can(&g, "heavy_factory"), Err(ProduceError::TechLevel));
    assert_eq!(can(&g, "battle_tank"), Err(ProduceError::TechLevel), "a factory already there doesn't help");
    // Asking anyway is refused and changes nothing.
    let before = g.state.players[0].credits;
    produce(&mut g, &[], "battle_tank");
    g.step(5);
    assert_eq!(rejections(&g), vec![ProduceError::TechLevel]);
    assert_eq!(g.state.players[0].credits, before);
    // The level is kept between 1 and 8.
    g.set_tech_level(Some(0));
    assert_eq!(g.state.tech_level, Some(1));
    g.set_tech_level(Some(20));
    assert_eq!(g.state.tech_level, Some(8));
    assert_eq!(can(&g, "heavy_factory"), Ok(()));
}

#[test]
fn an_upgrade_is_built_and_paid_like_any_entry_and_unlocks_its_items() {
    let mut g = game();
    let f = base(&mut g, 10_000);
    let (cost, ticks) = {
        let k = g.rules.kind(kind(&g, "heavy_factory"));
        (k.upgrade_cost, k.upgrade_ticks)
    };
    assert_eq!(g.can_build(0, kind(&g, "siege_tank")), Err(ProduceError::FactoryLevel { level: 1 }));
    produce(&mut g, &[], "siege_tank");
    upgrade(&mut g, &[], "heavy_factory", true);
    // A tank queued behind the upgrade waits for it: while it runs, the factory makes nothing else.
    produce(&mut g, &[], "battle_tank");
    g.step(1);
    assert_eq!(rejections(&g), vec![ProduceError::FactoryLevel { level: 1 }]);
    let queue: Vec<bool> = g.state.entity(f).unwrap().queue.iter().map(|q| q.upgrade).collect();
    assert_eq!(queue, vec![true, false]);
    let credits = g.state.players[0].credits;
    g.step(ticks as u32 / 2);
    let paid = credits - g.state.players[0].credits;
    println!("half way through the upgrade: paid {paid} of {cost}");
    assert!(paid > 0 && paid < cost, "paid as it builds");
    assert_eq!(level(&g, f), 0);
    g.step(ticks as u32);
    let done: Vec<(u32, u32)> = g
        .events
        .iter()
        .filter_map(
            |e| if let Event::UpgradeCompleted { factory, level, .. } = *e { Some((factory, level)) } else { None },
        )
        .collect();
    assert_eq!(done, vec![(f, 1)]);
    assert_eq!(level(&g, f), 1);
    assert!(g.state.entity(f).unwrap().queue.iter().all(|q| !q.upgrade), "the upgrade left the queue");
    assert_eq!(g.can_build(0, kind(&g, "siege_tank")), Ok(()));
    // The default heavy factory goes no higher than level 1.
    upgrade(&mut g, &[], "heavy_factory", true);
    g.step(1);
    assert_eq!(rejections(&g).last(), Some(&ProduceError::MaxLevel));
}

#[test]
fn taking_an_upgrade_back_refunds_what_it_paid_and_cancel_leaves_it_alone() {
    let mut g = game();
    let f = base(&mut g, 10_000);
    let start = g.state.players[0].credits;
    upgrade(&mut g, &[], "heavy_factory", true);
    g.step(100);
    assert!(g.state.players[0].credits < start);
    // Cancelling the factory's kind is about building one, not upgrading this one.
    let hf = kind(&g, "heavy_factory");
    g.order(0, &[], CommandOrder::Cancel { kind: hf });
    g.step(1);
    assert_eq!(g.state.entity(f).unwrap().queue.len(), 1);
    // Nor does hold touch it.
    g.order(0, &[], CommandOrder::Hold { kind: hf, on: true });
    g.step(1);
    assert!(g.state.entity(f).unwrap().queue[0].state != classic_sim::EntryState::Held);
    upgrade(&mut g, &[], "heavy_factory", false);
    g.step(1);
    assert!(g.state.entity(f).unwrap().queue.is_empty());
    assert_eq!(g.state.players[0].credits, start, "every credit came back");
    assert_eq!(level(&g, f), 0);
    // With none queued, there is nothing to take back.
    upgrade(&mut g, &[], "heavy_factory", false);
    g.step(1);
    assert_eq!(rejections(&g).last(), Some(&ProduceError::NotQueued));
}

#[test]
fn an_upgrade_belongs_to_its_building_and_orders_find_the_one_that_can_make_them() {
    let mut g = game();
    let first = base(&mut g, 20_000);
    let second = g.spawn(kind(&g, "heavy_factory"), 0, 8, 5);
    upgrade(&mut g, &[second], "heavy_factory", true);
    g.step(700);
    assert_eq!((level(&g, first), level(&g, second)), (0, 1), "only the one upgraded went up");
    // The first is still primary, but only the second can make a siege tank, so it gets the order.
    assert_eq!(
        classic_sim::production::primary(&g.state, 0, kind(&g, "heavy_factory")).map(|i| g.state.entities[i].id),
        Some(first)
    );
    produce(&mut g, &[], "siege_tank");
    produce(&mut g, &[], "battle_tank");
    g.step(1);
    let tank = kind(&g, "siege_tank");
    assert!(g.state.entity(second).unwrap().queue.iter().any(|q| q.item == tank));
    assert!(
        g.state.entity(first).unwrap().queue.iter().all(|q| q.item != tank),
        "battle tanks still go to the primary"
    );
    // Naming the first for a siege tank is refused.
    produce(&mut g, &[first], "siege_tank");
    g.step(1);
    assert_eq!(rejections(&g).last(), Some(&ProduceError::FactoryLevel { level: 1 }));
    // A new heavy factory starts at level 0.
    let third = g.spawn(kind(&g, "heavy_factory"), 0, 12, 5);
    assert_eq!(level(&g, third), 0);
}

#[test]
fn the_tech_level_caps_how_far_factories_may_be_upgraded() {
    let mut g = game();
    let f = base(&mut g, 10_000);
    let yard = g.state.entities.iter().find(|e| g.rules.kind(e.kind).id == "construction_yard").unwrap().id;
    g.spawn(kind(&g, "radar"), 0, 12, 1);
    g.set_tech_level(Some(4));
    // At tech 4 the yard may go to level 1 but the heavy factory may not.
    assert_eq!(g.level_cap(kind(&g, "construction_yard")), 1);
    assert_eq!(g.level_cap(kind(&g, "heavy_factory")), 0);
    upgrade(&mut g, &[f], "heavy_factory", true);
    upgrade(&mut g, &[yard], "construction_yard", true);
    g.step(500);
    assert_eq!(rejections(&g), vec![ProduceError::MaxLevel]);
    assert_eq!((level(&g, yard), level(&g, f)), (1, 0));
    assert_eq!(g.can_build(0, kind(&g, "rocket_turret")), Ok(()));
    assert_eq!(g.can_build(0, kind(&g, "slab_large")), Err(ProduceError::NotBuildable), "slabs only with decay on");
}

#[test]
fn upgrades_and_tech_levels_replay_from_the_command_log() {
    let setup = |g: &mut Game| {
        g.set_tech_level(Some(6));
        base(g, 10_000)
    };
    let mut live = game();
    let f = setup(&mut live);
    upgrade(&mut live, &[], "heavy_factory", true);
    produce(&mut live, &[], "battle_tank");
    live.step(300);
    upgrade(&mut live, &[], "heavy_factory", false);
    upgrade(&mut live, &[], "heavy_factory", true);
    live.step(1500);
    produce(&mut live, &[], "siege_tank");
    live.step(800);
    let log = live.command_log().to_vec();
    let mut replay = game();
    setup(&mut replay);
    for t in 0..2600 {
        for c in log.iter().filter(|c| c.tick == t) {
            replay.order(c.command.player, &c.command.ids, c.command.order);
        }
        replay.step(1);
    }
    println!("live {} replay {}, refused {:?}", live.hash(), replay.hash(), rejections(&live));
    assert_eq!(level(&live, f), 1);
    let siege = kind(&live, "siege_tank");
    assert!(live.events.iter().any(|e| matches!(*e, Event::UnitBuilt { kind: k, .. } if k == siege)));
    assert_eq!(replay.hash(), live.hash());
}

#[test]
fn the_starport_sells_nothing_above_the_tech_level() {
    let mut g = game();
    base(&mut g, 10_000);
    g.spawn(kind(&g, "starport"), 0, 1, 4);
    g.set_tech_level(Some(4));
    let siege = kind(&g, "siege_tank");
    g.order(0, &[], CommandOrder::StarportAdd { kind: siege });
    g.step(1);
    let refused = |g: &Game| {
        g.events
            .iter()
            .filter_map(|e| if let Event::StarportRefused { reason, .. } = *e { Some(reason) } else { None })
            .collect::<Vec<_>>()
    };
    assert_eq!(refused(&g), vec![classic_sim::StarportError::NotSold]);
    g.set_tech_level(Some(5));
    g.order(0, &[], CommandOrder::StarportAdd { kind: siege });
    g.step(1);
    assert_eq!(refused(&g).len(), 1, "sold at tech 5");
    assert!(g.state.deliveries.iter().any(|d| d.items.iter().any(|&(k, _)| k == siege)));
}
