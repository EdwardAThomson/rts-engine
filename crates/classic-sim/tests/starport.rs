//! The starport market (rules-economy-production.md, section 12): an order paid in full, a supply ship that lands
//! after the delivery time and sets the units down, prices that drift within bounds, stock per player, and refunds
//! when the starport is lost.
//!
//! The map is open rock with each player's starting base in a top corner; the tests' starport stands in the
//! middle. Each player starts with 2,000 credits... or whatever the rules say, read from the game.

use classic_sim::world::Event;
use classic_sim::{CommandOrder, Game, GameOptions, Kind, StarportError, Tile};

const OPEN: &str = "\
############################
#1######################2###
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
############################
";

fn game() -> Game {
    Game::new(GameOptions { map: OPEN, seed: 7, players: None, rules: None }).expect("map is valid")
}

fn kind(g: &Game, id: &str) -> Kind {
    g.kind(id).unwrap_or_else(|| panic!("no kind {id}"))
}

fn credits(g: &Game, p: u32) -> i64 {
    g.state.players[p as usize].credits
}

/// A game with a starport for player 0 in the middle of the map, and the market open.
fn with_starport() -> (Game, u32) {
    let mut g = game();
    let k = kind(&g, "starport");
    let port = g.spawn(k, 0, 12, 8);
    g.step(1);
    (g, port)
}

fn price(g: &Game, id: &str) -> i64 {
    classic_sim::starport::price(&g.state, &g.rules, kind(g, id)).expect("sold")
}

fn refused(g: &Game, reason: StarportError) -> bool {
    g.events.iter().any(|ev| matches!(*ev, Event::StarportRefused { reason: r, .. } if r == reason))
}

#[test]
fn the_market_opens_with_the_first_starport() {
    let mut g = game();
    g.step(10);
    assert!(g.state.market.is_none(), "no starport, no market");
    let (g, _) = with_starport();
    let m = g.state.market.as_ref().expect("open");
    assert!(m.percent.iter().all(|&p| p == 100), "prices start at base");
    assert_eq!(price(&g, "battle_tank"), 600);
}

#[test]
fn an_order_is_paid_whole_and_a_supply_ship_brings_it() {
    let (mut g, port) = with_starport();
    g.state.players[0].credits = 5000;
    let tank = kind(&g, "battle_tank");
    let quad = kind(&g, "quad");
    for k in [tank, tank, quad] {
        g.order(0, &[], CommandOrder::StarportAdd { kind: k });
    }
    g.step(1);
    assert_eq!(credits(&g, 0), 5000, "nothing paid while the order is open");
    assert_eq!(classic_sim::starport::stock(&g.state, &g.rules, 0, tank), 1, "two of three in the order");
    let cost = 2 * price(&g, "battle_tank") + price(&g, "quad");
    g.order(0, &[], CommandOrder::StarportConfirm);
    g.step(1);
    assert_eq!(credits(&g, 0), 5000 - cost, "paid in full at once");
    let placed = g.events.iter().find_map(|ev| match *ev {
        Event::StarportOrderPlaced { starport, cost: c, arrive, .. } if starport == port => Some((c, arrive)),
        _ => None,
    });
    let (paid, arrive) = placed.expect("order placed");
    assert_eq!(paid, cost);
    assert_eq!(arrive, g.state.tick - 1 + 600);
    // The ship comes in over the map and lands on time.
    let mut landed = None;
    let mut built = Vec::new();
    let mut left = false;
    while g.state.tick < 1000 && !left {
        g.step(1);
        for ev in &g.events {
            match *ev {
                Event::SupplyShipLanded { tick, .. } => landed = landed.or(Some(tick)),
                Event::UnitBuilt { factory, kind, tick, .. } if factory == port && !built.contains(&(tick, kind)) => {
                    built.push((tick, kind))
                }
                Event::SupplyShipLeft { .. } => left = true,
                _ => {}
            }
        }
    }
    assert_eq!(landed, Some(arrive), "lands when due");
    let kinds: Vec<Kind> = built.iter().map(|&(_, k)| k).collect();
    assert_eq!(kinds, [tank, tank, quad], "in order");
    assert_eq!(built[1].0 - built[0].0, 15, "one every 15 ticks");
    assert!(left, "the ship leaves the map");
    assert!(g.state.deliveries.is_empty());
    let ship = kind(&g, "supply_ship");
    assert!(g.state.entities.iter().all(|e| e.kind != ship));
    let mine = g.state.entities.iter().filter(|e| e.owner == 0 && (e.kind == tank || e.kind == quad)).count();
    assert_eq!(mine, 4, "three bought and the starting tank");
}

#[test]
fn an_order_it_cannot_pay_for_is_refused_whole() {
    let (mut g, _) = with_starport();
    g.state.players[0].credits = 700;
    let tank = kind(&g, "battle_tank");
    g.order(0, &[], CommandOrder::StarportAdd { kind: tank });
    g.order(0, &[], CommandOrder::StarportAdd { kind: tank });
    g.order(0, &[], CommandOrder::StarportConfirm);
    g.step(1);
    assert!(refused(&g, StarportError::Funds));
    assert_eq!(credits(&g, 0), 700, "nothing taken");
    assert_eq!(g.state.deliveries[0].items.len(), 2, "the order stays open to change");
    g.order(0, &[], CommandOrder::StarportRemove { kind: tank });
    g.order(0, &[], CommandOrder::StarportConfirm);
    g.step(1);
    assert_eq!(credits(&g, 0), 700 - price(&g, "battle_tank"));
}

#[test]
fn stock_runs_out_per_player_and_grows_back() {
    let (mut g, _) = with_starport();
    let k = kind(&g, "starport");
    g.spawn(k, 1, 18, 8);
    g.state.players[0].credits = 10_000;
    let tank = kind(&g, "battle_tank");
    for _ in 0..4 {
        g.order(0, &[], CommandOrder::StarportAdd { kind: tank });
    }
    g.step(1);
    assert!(refused(&g, StarportError::OutOfStock), "three to a player");
    assert_eq!(g.state.deliveries[0].items.len(), 3);
    assert_eq!(classic_sim::starport::stock(&g.state, &g.rules, 1, tank), 3, "the other player's stock is their own");
    g.order(0, &[], CommandOrder::StarportConfirm);
    g.step(1);
    let bought = g.state.tick - 1;
    while g.state.tick <= bought + 1800 {
        g.step(1);
    }
    assert_eq!(classic_sim::starport::stock(&g.state, &g.rules, 0, tank), 1, "one back after 1,800 ticks");
    while g.state.tick <= bought + 3600 {
        g.step(1);
    }
    assert_eq!(classic_sim::starport::stock(&g.state, &g.rules, 0, tank), 2);
}

#[test]
fn an_order_holds_five_and_one_order_is_in_flight_at_a_time() {
    let (mut g, _) = with_starport();
    g.state.players[0].credits = 10_000;
    let (bike, quad, tank) = (kind(&g, "scout_bike"), kind(&g, "quad"), kind(&g, "battle_tank"));
    for k in [bike, bike, quad, quad, tank, tank] {
        g.order(0, &[], CommandOrder::StarportAdd { kind: k });
    }
    g.step(1);
    assert!(refused(&g, StarportError::Full));
    assert_eq!(g.state.deliveries[0].items.len(), 5);
    g.order(0, &[], CommandOrder::StarportConfirm);
    g.step(1);
    g.order(0, &[], CommandOrder::StarportAdd { kind: tank });
    g.step(1);
    assert!(refused(&g, StarportError::Busy));
}

#[test]
fn prices_drift_within_their_bounds_the_same_way_every_run() {
    let run = || {
        let (mut g, _) = with_starport();
        let mut seen = Vec::new();
        for _ in 0..20 {
            g.step(900);
            seen.push(g.state.market.as_ref().unwrap().percent.clone());
        }
        (seen, g.events.iter().filter(|ev| matches!(ev, Event::MarketPricesChanged { .. })).count())
    };
    let (seen, drifts) = run();
    assert_eq!(drifts, 20, "every 900 ticks");
    assert!(seen.iter().flatten().all(|p| (75..=150).contains(p)));
    assert!(seen.windows(2).any(|w| w[0] != w[1]), "they move");
    assert!(seen.iter().flatten().any(|&p| p != 100));
    assert_eq!(run().0, seen, "the game's own generator: the same every run");
}

#[test]
fn a_starport_lost_before_the_ship_lands_refunds_the_order() {
    let (mut g, port) = with_starport();
    g.state.players[0].credits = 3000;
    let tank = kind(&g, "battle_tank");
    g.order(0, &[], CommandOrder::StarportAdd { kind: tank });
    g.order(0, &[], CommandOrder::StarportConfirm);
    g.step(100);
    assert_eq!(credits(&g, 0), 3000 - price(&g, "battle_tank"));
    let i = g.state.entities.iter().position(|e| e.id == port).unwrap();
    g.state.entities[i].health = 0;
    g.step(2);
    assert!(g.state.entity(port).is_none());
    assert!(g.events.iter().any(|ev| matches!(*ev, Event::StarportOrderRefunded { refund: 600, .. })));
    assert_eq!(credits(&g, 0), 3000, "a full refund");
    g.step(1000);
    assert!(g.state.deliveries.is_empty());
    let ship = kind(&g, "supply_ship");
    assert!(g.state.entities.iter().all(|e| e.kind != ship), "any ship on its way turns back and goes");
}

#[test]
fn an_order_paid_while_short_of_power_takes_twice_as_long() {
    let (mut g, _) = with_starport();
    g.state.players[0].credits = 3000;
    // Enough starports to draw more than the starting plant gives.
    let k = kind(&g, "starport");
    for x in [4, 8, 16, 20] {
        g.spawn(k, 0, x, 12);
    }
    assert!(g.power(0).is_short());
    g.order(0, &[], CommandOrder::StarportAdd { kind: kind(&g, "quad") });
    g.order(0, &[], CommandOrder::StarportConfirm);
    g.step(1);
    let arrive = g.events.iter().find_map(|ev| match *ev {
        Event::StarportOrderPlaced { arrive, .. } => Some(arrive),
        _ => None,
    });
    assert_eq!(arrive, Some(g.state.tick - 1 + 1200));
}

#[test]
fn the_supply_ship_takes_no_orders() {
    let (mut g, _) = with_starport();
    g.state.players[0].credits = 3000;
    g.order(0, &[], CommandOrder::StarportAdd { kind: kind(&g, "quad") });
    g.order(0, &[], CommandOrder::StarportConfirm);
    let ship_kind = kind(&g, "supply_ship");
    while !g.state.entities.iter().any(|e| e.kind == ship_kind) {
        g.step(1);
    }
    let ship = g.state.entities.iter().find(|e| e.kind == ship_kind).unwrap().id;
    let goal = *g.state.entity(ship).unwrap().path.back().unwrap();
    g.order(0, &[ship], CommandOrder::Move { x: 1, y: 14 });
    g.step(1);
    assert_eq!(g.state.entity(ship).unwrap().path.back(), Some(&goal));
    assert_eq!(goal, Tile { x: 13, y: 9 }, "the middle of the starport");
}

#[test]
fn a_game_with_a_starport_replays_to_the_same_hash() {
    let run = || {
        let (mut g, _) = with_starport();
        g.state.players[0].credits = 5000;
        for id in ["quad", "battle_tank", "harvester"] {
            g.order(0, &[], CommandOrder::StarportAdd { kind: kind(&g, id) });
        }
        g.order(0, &[], CommandOrder::StarportConfirm);
        g.step(2500);
        g.hash()
    };
    assert_eq!(run(), run());
}
