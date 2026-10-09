//! The palace power (ai-opponent.md, "Superpowers"). Once its power is charged the opponent scores the tiles of the
//! enemies it knows of and uses the power on the best, if that is worth it, so it doesn't spend a missile on a lone
//! scout. Only known enemies count, so under fog it aims only at what it has seen, which is always explored ground.
//!
//! - Missile: the cost of what stands within its blast, weighed by the damage of each ring, less twice that of its
//!   own side's; at least `MISSILE_WORTH`.
//! - Guerrillas: the cost of the enemy units (not buildings) within `GUERRILLA_REACH` tiles; at least
//!   `GUERRILLAS_WORTH`.
//! - Saboteur: the dearest enemy building it knows of that isn't a wall and that its blast would destroy (failing
//!   that, the dearest), costing at least `SABOTEUR_WORTH`.
//!
//! The design scores a coarse grid; scoring the enemies' own tiles gives the same targets without aiming at ground
//! nobody has explored (a simplification, noted in the design doc).

use classic_sim::superpower;
use classic_sim::{CommandOrder, Entity, Game, Superpower, Tile};

use crate::{Ai, Orders, View};

const MISSILE_WORTH: i64 = 2000;
const GUERRILLAS_WORTH: i64 = 600;
const GUERRILLA_REACH: i32 = 4;
const SABOTEUR_WORTH: i64 = 400;

/// The tile offset of `e` from tile `t`: the larger of x and y, to a building's nearest tile.
fn ring(game: &Game, e: &Entity, t: Tile) -> i32 {
    let k = game.rules.kind(e.kind);
    let at = e.tile();
    let (w, h) = if k.building { (k.width, k.height) } else { (1, 1) };
    let dx = (at.x - t.x).max(t.x - (at.x + w - 1)).max(0);
    let dy = (at.y - t.y).max(t.y - (at.y + h - 1)).max(0);
    dx.max(dy)
}

/// The middle tile of `e`.
fn middle(game: &Game, e: &Entity) -> Tile {
    let k = game.rules.kind(e.kind);
    let t = e.tile();
    if k.building { Tile { x: t.x + (k.width - 1) / 2, y: t.y + (k.height - 1) / 2 } } else { t }
}

pub(crate) fn think(ai: &Ai, game: &Game, view: &View, out: &mut Orders) {
    let (state, rules) = (&game.state, &game.rules);
    if !superpower::ready(state, rules, ai.player) {
        return;
    }
    let (Some(power), Some(sp)) = (superpower::power(state, rules, ai.player), rules.superpowers.as_ref()) else {
        return;
    };
    let es = &state.entities;
    let cost = |e: &Entity| rules.kind(e.kind).cost;
    let enemies: Vec<&Entity> =
        view.enemies.iter().map(|&i| &es[i]).filter(|e| rules.kind(e.kind).targetable).collect();
    let mut candidates: Vec<Tile> = enemies.iter().map(|e| middle(game, e)).collect();
    candidates.sort_by_key(|t| (t.y, t.x));
    candidates.dedup();
    let best =
        |score: &dyn Fn(Tile) -> i64| candidates.iter().map(|&t| (score(t), t)).max_by_key(|&(s, t)| (s, -t.y, -t.x));
    let pick = match power {
        Superpower::Missile => {
            let top = sp.missile_damage[0].max(1);
            let hurt = |e: &Entity, t: Tile| {
                let r = ring(game, e, t) as usize;
                sp.missile_damage.get(r).map_or(0, |d| cost(e) * d / top)
            };
            best(&|t| {
                let theirs: i64 = enemies.iter().map(|e| hurt(e, t)).sum();
                let ours: i64 = view.mine.iter().map(|&i| hurt(&es[i], t)).sum();
                theirs - 2 * ours
            })
            .filter(|&(s, _)| s >= MISSILE_WORTH)
        }
        Superpower::Guerrillas => best(&|t| {
            enemies
                .iter()
                .filter(|e| !rules.kind(e.kind).building && ring(game, e, t) <= GUERRILLA_REACH)
                .map(|e| cost(e))
                .sum()
        })
        .filter(|&(s, _)| s >= GUERRILLAS_WORTH),
        Superpower::Saboteur => {
            // One it can destroy outright first, then the dearest.
            let blast = rules.kind(sp.saboteur).weapon.map_or(0, |w| rules.weapon(w).damage);
            enemies
                .iter()
                .filter(|e| rules.kind(e.kind).building && !rules.kind(e.kind).wall)
                .map(|e| (e.health <= blast, cost(e), middle(game, e)))
                .max_by_key(|&(kills, s, t)| (kills, s, -t.y, -t.x))
                .map(|(_, s, t)| (s, t))
                .filter(|&(s, _)| s >= SABOTEUR_WORTH)
        }
    };
    if let Some((_, t)) = pick {
        out.push(Vec::new(), CommandOrder::Superpower { x: t.x, y: t.y });
    }
}
