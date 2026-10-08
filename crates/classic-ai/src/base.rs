//! The base and the economy: what to build next, where it goes, which units the factories make, and harvesters
//! kept at work.
//!
//! **Build order.** `Settings::build_order` lists generic building ids with how many to have, in order. The first
//! entry it has fewer of (counting one already queued) is built next, so a lost building is rebuilt the same way.
//! Before anything that would leave power within `power_margin` of demand, a power plant comes first, and when power
//! runs short (a plant lost) a building less than half done is cancelled to make way for one. One building is queued
//! at a time.
//!
//! **Where it goes.** Every footprint position touching one of its buildings is checked with `Game::can_place`, the
//! same check a player's placement meets, then scored: near the yard for most, near a resource field for a
//! refinery, towards the enemy for anything armed. The best one that keeps every factory exit and refinery dock
//! reachable from the map edge wins (the "lanes" of the design doc, kept by a flood fill). A ready building with
//! nowhere to go is cancelled, which refunds it.

use std::collections::{BTreeSet, VecDeque};

use classic_sim::world::dock_at;
use classic_sim::{CommandOrder, EntryState, Game, Kind, Order, Task, Tile};
use rts_core::imath::isqrt;

use crate::{Ai, Orders, View, dist2};

/// Queue the next building, and place any that are ready.
pub(crate) fn think(ai: &Ai, game: &Game, view: &View, out: &mut Orders) {
    let es = &game.state.entities;
    let plant = game.kind("power_plant").filter(|&k| buildable(game, view, ai.player, k));
    let power = game.power(ai.player);
    let mut busy = false;
    for &i in &view.mine {
        let Some(head) = es[i].queue.first() else { continue };
        let k = game.rules.kind(head.item);
        if !k.building {
            continue;
        }
        busy = true;
        // Short of power, a building less than half done gives way to a power plant.
        if power.is_short()
            && plant.is_some_and(|p| p != head.item)
            && head.state != EntryState::Ready
            && head.progress * 2 < k.build_ticks * 100
        {
            out.push(vec![es[i].id], CommandOrder::Cancel { kind: head.item });
            continue;
        }
        if head.state == EntryState::Ready {
            match spot(game, view, ai.player, head.item) {
                Some(t) => out.push(Vec::new(), CommandOrder::Place { kind: head.item, x: t.x, y: t.y }),
                None => out.push(vec![es[i].id], CommandOrder::Cancel { kind: head.item }),
            }
        }
    }
    if busy {
        return;
    }
    let next = next_building(ai, game, view);
    let draw = next.map_or(0, |k| game.rules.kind(k).power.min(0));
    let next = match plant {
        Some(p) if power.supply - power.demand + draw < ai.settings.power_margin => Some(p),
        _ => next,
    };
    if let Some(k) = next {
        out.push(Vec::new(), CommandOrder::Produce { kind: k });
    }
}

/// The first entry of the build order it has fewer of than it wants and may build now.
fn next_building(ai: &Ai, game: &Game, view: &View) -> Option<Kind> {
    ai.settings.build_order.iter().find_map(|(id, want)| {
        let k = game.kind(id)?;
        (view.count(game, k) < *want && buildable(game, view, ai.player, k)).then_some(k)
    })
}

/// Whether it may build `kind` now and owns a building that makes it.
fn buildable(game: &Game, view: &View, player: u32, kind: Kind) -> bool {
    let maker = game.rules.kind(kind).built_at;
    game.can_build(player, kind).is_ok() && view.mine.iter().any(|&i| Some(game.state.entities[i].kind) == maker)
}

/// Keep each factory's queue topped up: harvesters until every refinery has its share, then combat units.
pub(crate) fn produce(ai: &Ai, game: &Game, view: &View, out: &mut Orders) {
    let es = &game.state.entities;
    let rules = &game.rules;
    let units: Vec<Kind> = (0..rules.kinds.len() as u16).map(Kind).filter(|&k| !rules.kind(k).building).collect();
    let refineries = view.mine.iter().filter(|&&i| rules.kind(es[i].kind).refinery).count();
    let is_harvester = |k: Kind| rules.kind(k).harvester.is_some();
    let queued_harvesters = view.mine.iter().flat_map(|&i| es[i].queue.iter()).filter(|q| is_harvester(q.item)).count();
    let mut harvesters = view.mine.iter().filter(|&&i| is_harvester(es[i].kind)).count() + queued_harvesters;
    let want = (refineries * ai.settings.harvesters_per_refinery).min(ai.settings.max_harvesters);
    let credits = game.state.players.iter().find(|p| p.id == ai.player).map_or(0, |p| p.credits);
    for &i in &view.mine {
        let f = &es[i];
        if f.queue.len() >= ai.settings.factory_queue {
            continue;
        }
        let made_here: Vec<Kind> = units
            .iter()
            .copied()
            .filter(|&k| rules.kind(k).built_at == Some(f.kind) && game.can_build(ai.player, k).is_ok())
            .collect();
        if made_here.is_empty() {
            continue;
        }
        if harvesters < want
            && let Some(&h) = made_here.iter().find(|&&k| is_harvester(k))
        {
            harvesters += 1;
            out.push(vec![f.id], CommandOrder::Produce { kind: h });
            continue;
        }
        // The strongest armed unit it makes, by cost.
        let fighter =
            made_here.iter().filter(|&&k| rules.kind(k).weapon.is_some()).max_by_key(|&&k| rules.kind(k).cost);
        if let Some(&k) = fighter
            && credits >= ai.settings.unit_reserve
        {
            out.push(vec![f.id], CommandOrder::Produce { kind: k });
        }
    }
}

/// Send any harvester that has stopped (stuck, or left standing after a move) back to work.
pub(crate) fn harvesters(game: &Game, view: &View, out: &mut Orders) {
    let es = &game.state.entities;
    let idle: Vec<u32> = view
        .mine
        .iter()
        .map(|&i| &es[i])
        .filter(|e| game.rules.kind(e.kind).harvester.is_some())
        .filter(|e| e.task == Some(Task::Stuck) || (e.order != Order::Harvest && e.path.is_empty()))
        .map(|e| e.id)
        .collect();
    if !idle.is_empty() {
        out.push(idle, CommandOrder::Harvest);
    }
}

/// The best place for a ready building, or `None` if there is nowhere it may go.
pub(crate) fn spot(game: &Game, view: &View, player: u32, kind: Kind) -> Option<Tile> {
    let rules = &game.rules;
    let es = &game.state.entities;
    let k = rules.kind(kind);
    let gap = rules.placement.max_gap;
    let home = view.home?;
    // Every top-left tile whose footprint lies within the gap of one of its buildings.
    let mut tried = BTreeSet::new();
    for &i in &view.mine {
        let bk = rules.kind(es[i].kind);
        if !bk.building || bk.wall {
            continue;
        }
        let t = es[i].tile();
        for y in t.y - k.height - gap..=t.y + bk.height + gap {
            for x in t.x - k.width - gap..=t.x + bk.width + gap {
                tried.insert((y, x));
            }
        }
    }
    let field = resource_tiles(game);
    // Armed buildings face the enemy: aim for a point a few tiles out from home towards it.
    let front = view.enemy_home.map_or(home, |e| toward(home, e, 5));
    let mut scored: Vec<(i64, i32, i32)> = tried
        .into_iter()
        .filter(|&(y, x)| game.can_place(player, kind, x, y).is_ok())
        .map(|(y, x)| {
            let mid = Tile { x: x + (k.width - 1) / 2, y: y + (k.height - 1) / 2 };
            let mut score = if k.weapon.is_some() { dist2(mid, front) * 4 } else { dist2(mid, home) * 4 };
            if k.refinery {
                let dock = dock_at(k, x, y);
                score += field.iter().map(|&f| dist2(dock, f)).min().unwrap_or(0) * 8;
            }
            if x == 0 || y == 0 || x + k.width >= game.map.width || y + k.height >= game.map.height {
                score += 50;
            }
            (score, y, x)
        })
        .collect();
    scored.sort_unstable();
    scored.into_iter().map(|(_, y, x)| Tile { x, y }).find(|&t| keeps_lanes(game, view, kind, t))
}

/// Whether, with `kind` placed at `at`, every factory exit and refinery dock it owns, the new building's included,
/// keeps the tiles beside and below it free of buildings and can still be reached from the edge of the map. The free
/// ring gives a unit leaving room to pass one arriving.
fn keeps_lanes(game: &Game, view: &View, kind: Kind, at: Tile) -> bool {
    let rules = &game.rules;
    let es = &game.state.entities;
    let map = &game.map;
    let (w, h) = (map.width, map.height);
    let mut blocked: Vec<bool> = (0..w * h).map(|i| !map.passable(i % w, i / w)).collect();
    let mut fill = |x0: i32, y0: i32, kw: i32, kh: i32| {
        for y in y0..y0 + kh {
            for x in x0..x0 + kw {
                if map.in_bounds(x, y) {
                    blocked[map.index(x, y)] = true;
                }
            }
        }
    };
    for e in es {
        let k = rules.kind(e.kind);
        if k.building {
            let t = e.tile();
            fill(t.x, t.y, k.width, k.height);
        }
    }
    let new = rules.kind(kind);
    fill(at.x, at.y, new.width, new.height);
    let has_exit = |k: Kind| {
        let kr = rules.kind(k);
        kr.refinery || rules.kinds.iter().any(|u| !u.building && u.built_at == Some(k))
    };
    let mut exits: Vec<Tile> = view
        .mine
        .iter()
        .filter(|&&i| has_exit(es[i].kind))
        .map(|&i| dock_at(rules.kind(es[i].kind), es[i].tile().x, es[i].tile().y))
        .collect();
    if has_exit(kind) {
        exits.push(dock_at(new, at.x, at.y));
    }
    let ring_free = |t: &Tile| {
        (0..=1)
            .all(|dy| (-1..=1).all(|dx| !map.in_bounds(t.x + dx, t.y + dy) || !blocked[map.index(t.x + dx, t.y + dy)]))
    };
    if !exits.iter().all(ring_free) {
        return false;
    }
    let mut seen = vec![false; (w * h) as usize];
    let mut queue = VecDeque::new();
    for i in 0..w * h {
        let (x, y) = (i % w, i / w);
        if (x == 0 || y == 0 || x == w - 1 || y == h - 1) && !blocked[i as usize] {
            seen[i as usize] = true;
            queue.push_back(Tile { x, y });
        }
    }
    if queue.is_empty() {
        return true;
    }
    while let Some(t) = queue.pop_front() {
        for (dx, dy) in [(0, -1), (1, 0), (0, 1), (-1, 0)] {
            let (x, y) = (t.x + dx, t.y + dy);
            if map.in_bounds(x, y) {
                let n = map.index(x, y);
                if !seen[n] && !blocked[n] {
                    seen[n] = true;
                    queue.push_back(Tile { x, y });
                }
            }
        }
    }
    exits.iter().all(|t| map.in_bounds(t.x, t.y) && seen[map.index(t.x, t.y)])
}

/// Every tile with resource on it now.
fn resource_tiles(game: &Game) -> Vec<Tile> {
    (0..game.state.resource.len()).filter(|&i| game.state.resource[i] > 0).map(|i| game.map.tile_at(i)).collect()
}

/// The point `n` tiles from `from` along the line to `to` (or `to` itself if it is nearer).
pub(crate) fn toward(from: Tile, to: Tile, n: i32) -> Tile {
    let d = isqrt(dist2(from, to) as u64) as i64;
    if d <= n as i64 {
        return to;
    }
    let (dx, dy) = ((to.x - from.x) as i64, (to.y - from.y) as i64);
    Tile { x: from.x + (dx * n as i64 / d) as i32, y: from.y + (dy * n as i64 / d) as i32 }
}
