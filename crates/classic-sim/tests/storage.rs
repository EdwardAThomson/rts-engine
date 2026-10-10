//! Storage (rules-economy-production.md, section 6): each refinery and silo adds to its owner's storage cap, a
//! delivery fills credits only up to the cap, and the rest is lost without jamming the dock.

use classic_sim::world::Event;
use classic_sim::{CommandOrder, EntryState, Game, GameOptions, Kind, QueueEntry, Task};

const MAP: &str = include_str!("../../../maps/test-01.txt");

fn game() -> Game {
    Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: None }).unwrap()
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

/// Player 0's harvester, unloading `cargo` from this tick on.
fn unloading(g: &mut Game, cargo: i64) -> u32 {
    let e = g.state.entities.iter_mut().find(|e| e.owner == 0 && e.cargo.is_some()).unwrap();
    e.cargo = Some(cargo);
    e.task = Some(Task::Unloading);
    e.path.clear();
    e.id
}

/// Step until player 0's harvester has emptied, and return how many ticks that took.
fn unload(g: &mut Game, h: u32) -> u32 {
    let start = g.state.tick;
    while g.state.entity(h).unwrap().cargo != Some(0) {
        g.step(1);
        assert!(g.state.tick - start < 1000, "the harvester empties");
    }
    g.state.tick - start
}

/// Player 0's storage events called `name`.
fn events(g: &Game, name: &str) -> usize {
    let mine = |e: &Event| matches!(e, Event::StorageFull { player: 0, .. } | Event::CreditsLost { player: 0, .. });
    g.events.iter().filter(|e| e.name() == name && mine(e)).count()
}

#[test]
fn a_refinery_and_a_silo_each_add_storage_and_the_yard_none() {
    let mut g = game();
    let start = g.storage(0);
    println!("the start base stores {start}");
    assert_eq!(start, 1000, "the start refinery only");
    let silo = g.spawn(kind(&g, "silo"), 0, 10, 12);
    assert_eq!(g.storage(0), 2000);
    assert_eq!(g.storage(1), 1000, "another player's silo adds nothing");
    assert_eq!(g.snapshot().storage, vec![2000, 1000]);
    g.state.entities.retain(|e| e.id != silo);
    assert_eq!(g.storage(0), 1000);
}

#[test]
fn a_delivery_over_the_cap_is_lost_at_the_normal_rate_and_said_once() {
    // The design doc's `storage-overflow` scenario: cap 1,000, credits 950, deliver 200.
    let mut g = game();
    g.state.players[0].credits = 950;
    let h = unloading(&mut g, 200);
    let full_rate = unload(&mut g, h);
    let p = &g.state.players[0];
    println!("credits {}, lost {}, delivered {}, {full_rate} ticks to empty", p.credits, p.lost, p.delivered);
    assert_eq!((p.credits, p.lost, p.delivered), (1000, 150, 200));
    assert_eq!(full_rate, 200 / 10, "it empties as fast as ever");
    assert_eq!(events(&g, "storage_full"), 1);
    assert_eq!(events(&g, "credits_lost"), 1, "one warning, not one a tick");
    assert!(g.events.iter().any(|e| matches!(e, Event::CreditsLost { player: 0, cap: 1000, .. })));
    // Credits never went above the cap on the way.
    assert!(g.events.iter().all(|e| !matches!(e, Event::Delivered { credits, .. } if *credits > 1000)));
}

#[test]
fn credits_above_the_cap_are_kept_but_every_delivery_is_lost() {
    // Starting credits (1,200) are above a lone refinery's 1,000. Nothing is taken away; nothing more is stored.
    let mut g = game();
    let h = unloading(&mut g, 200);
    unload(&mut g, h);
    let p = &g.state.players[0];
    assert_eq!((p.credits, p.lost), (1200, 200));
    assert_eq!(events(&g, "storage_full"), 0, "it was full before");
    // A silo makes room again, up to the new cap.
    g.spawn(kind(&g, "silo"), 0, 10, 12);
    let h = unloading(&mut g, 200);
    unload(&mut g, h);
    assert_eq!(g.state.players[0].credits, 1400);
}

#[test]
fn losing_a_silo_lowers_the_cap_but_takes_no_credits() {
    let mut g = game();
    let silo = g.spawn(kind(&g, "silo"), 0, 10, 12);
    g.state.players[0].credits = 1800;
    g.state.entities.retain(|e| e.id != silo);
    g.step(1);
    assert_eq!((g.storage(0), g.state.players[0].credits), (1000, 1800));
}

fn yard(g: &Game, player: u32) -> u32 {
    let k = kind(g, "construction_yard");
    g.state.entities.iter().find(|e| e.owner == player && e.kind == k).unwrap().id
}

#[test]
fn a_cancelled_entry_is_refunded_above_the_cap() {
    let mut g = game();
    g.state.players[0].credits = 1000;
    let (yard, silo) = (yard(&g, 0), kind(&g, "silo"));
    let y = g.state.entities.iter_mut().find(|e| e.id == yard).unwrap();
    y.queue.push(QueueEntry { state: EntryState::Building, paid: 100, ..QueueEntry::new(silo) });
    g.order(0, &[yard], CommandOrder::Cancel { kind: silo });
    g.step(1);
    assert_eq!(g.state.players[0].credits, 1100);
}

#[test]
fn a_silo_waiting_to_be_placed_adds_no_storage() {
    let mut g = game();
    let (yard, silo) = (yard(&g, 0), kind(&g, "silo"));
    let y = g.state.entities.iter_mut().find(|e| e.id == yard).unwrap();
    y.queue.push(QueueEntry { state: EntryState::Ready, paid: 150, ..QueueEntry::new(silo) });
    g.step(1);
    assert_eq!(g.storage(0), 1000);
}

#[test]
fn losses_are_said_at_most_once_per_warn_every_ticks_and_all_counted() {
    let mut g = game();
    let every = g.rules.storage.warn_every;
    let mut loads = 0;
    // Full from the start; keep unloading for a little over one warning window.
    while g.state.tick <= every + 40 {
        let h = unloading(&mut g, 200);
        unload(&mut g, h);
        loads += 1;
    }
    let p = &g.state.players[0];
    let warnings = events(&g, "credits_lost");
    println!("{loads} loads over {} ticks: lost {}, {warnings} warnings", g.state.tick, p.lost);
    assert_eq!(p.lost, 200 * loads);
    assert_eq!(warnings, 2, "one at the first loss, one a window later");
}
