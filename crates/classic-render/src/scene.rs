//! Drawing a Classic game: the map, buildings, units, shells, explosions, selection boxes and health bars. Reads
//! the game state and its events; never changes either. Anything only the picture needs (which way a unit's body
//! points, where it was last tick, explosions still playing) lives here, outside the simulation.

use std::collections::BTreeMap;

use classic_sim::combat::facing_to;
use classic_sim::map::{RESOURCE_PER_TILE, TILE};
use classic_sim::world::Event;
use classic_sim::{Entity, Game, Terrain};

use crate::art::{Art, Strip};
use crate::platform::{Rect, SpriteBatch};

/// What part of the world the screen shows: the world pixel at the screen's top left, and the scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
}

impl Camera {
    pub fn to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        ((wx - self.x) * self.zoom, (wy - self.y) * self.zoom)
    }

    pub fn to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        (sx / self.zoom + self.x, sy / self.zoom + self.y)
    }
}

/// An explosion or spark playing out where something happened.
struct Effect {
    id: &'static str,
    x: i64,
    y: i64,
    start: u32,
}

/// Ticks each frame of an effect stays on screen.
const EFFECT_TICKS: u32 = 2;

/// A squad member lost to damage, fading where it fell.
struct Fallen {
    member: String,
    owner: u32,
    /// World pixels of the middle of its sprite.
    x: f32,
    y: f32,
    facing: i64,
    start: u32,
}

/// Ticks each frame of a walk cycle stays on screen.
const WALK_TICKS: u32 = 3;
/// Ticks a fallen squad member takes to fade.
const FALL_TICKS: u32 = 24;

#[derive(Default)]
pub struct Scene {
    /// Body facing per unit, 0 to 255 clockwise from north.
    facing: BTreeMap<u32, i64>,
    /// Positions before the latest tick, for drawing between ticks.
    prev: BTreeMap<u32, (i64, i64)>,
    effects: Vec<Effect>,
    /// Members each squad showed when last drawn, so the ones it loses can fall.
    shown: BTreeMap<u32, usize>,
    fallen: Vec<Fallen>,
    /// How many of the game's events have been read.
    seen: usize,
    pub selected: Vec<u32>,
    /// Control groups: the units kept under each number key, 0 to 9.
    pub groups: [Vec<u32>; 10],
}

impl Scene {
    /// Ctrl and a number: keep `player`'s selected units under group `n`, replacing what it held. Buildings and
    /// other players' entities stay out of groups.
    pub fn set_group(&mut self, game: &Game, player: u32, n: usize) {
        let mine =
            |id: &u32| game.state.entity(*id).is_some_and(|e| e.owner == player && !game.rules.kind(e.kind).building);
        self.groups[n] = self.selected.iter().copied().filter(mine).collect();
    }

    /// A number: select group `n`. Returns true when the group was already the selection (a second press), so the
    /// caller can centre the view on it; an empty group leaves the selection alone.
    pub fn recall_group(&mut self, n: usize) -> bool {
        if self.groups[n].is_empty() {
            return false;
        }
        let again = self.selected == self.groups[n];
        self.selected = self.groups[n].clone();
        again
    }

    /// The middle of the selected entities, in tiles.
    pub fn selection_centre(&self, game: &Game) -> Option<(f32, f32)> {
        let found: Vec<&Entity> = self.selected.iter().filter_map(|&id| game.state.entity(id)).collect();
        if found.is_empty() {
            return None;
        }
        let n = found.len() as f32;
        let x = found.iter().map(|e| e.x as f32).sum::<f32>() / n / TILE as f32;
        let y = found.iter().map(|e| e.y as f32).sum::<f32>() / n / TILE as f32;
        Some((x, y))
    }

    /// Call before each tick: remember where everything is.
    pub fn before_step(&mut self, game: &Game) {
        self.prev = game.state.entities.iter().map(|e| (e.id, (e.x, e.y))).collect();
    }

    /// Call after each tick: turn bodies the way they moved, start effects for what happened and forget the dead.
    pub fn after_step(&mut self, game: &Game) {
        for e in &game.state.entities {
            if game.rules.kind(e.kind).building {
                continue;
            }
            let (px, py) = self.prev.get(&e.id).copied().unwrap_or((e.x, e.y));
            let f = if (e.x, e.y) != (px, py) {
                facing_to(e.x - px, e.y - py)
            } else if e.target.is_some() {
                e.facing
            } else {
                *self.facing.get(&e.id).unwrap_or(&e.facing)
            };
            self.facing.insert(e.id, f);
        }
        self.facing.retain(|id, _| game.state.entity(*id).is_some());
        self.shown.retain(|id, _| game.state.entity(*id).is_some());
        self.selected.retain(|id| game.state.entity(*id).is_some());
        for g in &mut self.groups {
            g.retain(|id| game.state.entity(*id).is_some());
        }
        if game.events.len() < self.seen {
            self.seen = 0;
        }
        for ev in &game.events[self.seen..] {
            match *ev {
                Event::ProjectileHit { tick, x, y, .. } => {
                    self.effects.push(Effect { id: "explosion_small", x, y, start: tick })
                }
                Event::Destroyed { tick, kind, x, y, .. } => {
                    let id = if game.rules.kind(kind).building { "explosion_large" } else { "explosion_medium" };
                    self.effects.push(Effect { id, x, y, start: tick });
                }
                _ => {}
            }
        }
        self.seen = game.events.len();
    }

    /// Draw the game as it stands on a `screen` of (width, height) pixels, `alpha` (0 to 1) of the way from the
    /// previous tick to the latest.
    pub fn draw(
        &mut self,
        batch: &mut SpriteBatch,
        art: &Art,
        game: &Game,
        cam: &Camera,
        screen: (f32, f32),
        alpha: f32,
    ) {
        let (w, h) = screen;
        let tile = art.tile;
        let px = tile / TILE as f32;
        let tick = game.state.tick;
        // The map, only the tiles on screen.
        let (wx0, wy0) = cam.to_world(0.0, 0.0);
        let (wx1, wy1) = cam.to_world(w, h);
        let x0 = ((wx0 / tile).floor() as i32).max(0);
        let y0 = ((wy0 / tile).floor() as i32).max(0);
        let x1 = ((wx1 / tile).ceil() as i32).min(game.map.width);
        let y1 = ((wy1 / tile).ceil() as i32).min(game.map.height);
        for ty in y0..y1 {
            for tx in x0..x1 {
                let i = (ty * game.map.width + tx) as usize;
                let (sx, sy) = cam.to_screen(tx as f32 * tile, ty as f32 * tile);
                let dst = Rect::new(sx, sy, tile * cam.zoom, tile * cam.zoom);
                let variant = (tx * 7 + ty * 13) as u32;
                let ground = match game.map.terrain[i] {
                    Terrain::Open => art.terrain("open"),
                    Terrain::Rock => art.terrain("rock"),
                    Terrain::Cliff => None,
                };
                match ground {
                    Some(s) => batch.sprite(s.tex, s.frame(variant), dst, [255; 4]),
                    None => batch.fill(dst, [46, 40, 36, 255]),
                }
                let amount = game.state.resource[i];
                if amount > 0
                    && let Some(s) = art.terrain("resource")
                {
                    let a = (96 + 159 * amount.min(RESOURCE_PER_TILE) / RESOURCE_PER_TILE) as u8;
                    batch.sprite(s.tex, s.frame(variant), dst, [255, 255, 255, a]);
                }
            }
        }
        // Buildings, then units in screen order (higher up first), then shells.
        let at = |e: &Entity| -> (f32, f32) {
            let (ox, oy) = self.prev.get(&e.id).copied().unwrap_or((e.x, e.y));
            let x = ox as f32 + (e.x - ox) as f32 * alpha;
            let y = oy as f32 + (e.y - oy) as f32 * alpha;
            (x * px, y * px)
        };
        let mut units: Vec<&Entity> = Vec::new();
        for e in &game.state.entities {
            let k = game.rules.kind(e.kind);
            if !k.building {
                units.push(e);
                continue;
            }
            let t = e.tile();
            let (sx, sy) = cam.to_screen(t.x as f32 * tile, t.y as f32 * tile);
            let dst = Rect::new(sx, sy, k.width as f32 * tile * cam.zoom, k.height as f32 * tile * cam.zoom);
            let frame = if k.weapon.is_some() { facing_frame(e.facing) } else { 0 };
            draw_sprite(batch, art.sprite(&k.id, e.owner), frame, dst, e.owner);
        }
        units.sort_by_key(|e| (e.y, e.id));
        // Squad members lost since the last frame fall where they stood, fading under the units still standing.
        for e in &units {
            let k = game.rules.kind(e.kind);
            let Some(squad) = art.squad(&k.id) else { continue };
            let shown = squad.shown(e.health, k.max_health);
            let before = self.shown.insert(e.id, shown).unwrap_or(shown);
            let (wx, wy) = at(e);
            let facing = *self.facing.get(&e.id).unwrap_or(&e.facing);
            for i in shown..before {
                let (dx, dy) = squad.place(i, facing);
                let (x, y) = (wx + dx * tile, wy + dy * tile);
                self.fallen.push(Fallen { member: squad.member.clone(), owner: e.owner, x, y, facing, start: tick });
            }
        }
        self.fallen.retain(|f| {
            let age = tick.saturating_sub(f.start);
            let Some(s) = art.sprite(&f.member, f.owner).filter(|_| age < FALL_TICKS) else { return false };
            let (sx, sy) = cam.to_screen(f.x - tile / 2.0, f.y - tile / 2.0);
            let a = (255 * (FALL_TICKS - age) / FALL_TICKS) as u8;
            batch.sprite(
                s.tex,
                s.facing_frame(f.facing, 0),
                Rect::new(sx, sy, tile * cam.zoom, tile * cam.zoom),
                [255, 255, 255, a],
            );
            true
        });
        for e in units {
            let k = game.rules.kind(e.kind);
            let (wx, wy) = at(e);
            let facing = *self.facing.get(&e.id).unwrap_or(&e.facing);
            let moving = self.prev.get(&e.id).is_some_and(|&p| p != (e.x, e.y));
            let walk = if moving { tick / WALK_TICKS } else { 0 };
            if let Some(squad) = art.squad(&k.id) {
                let member = art.sprite(&squad.member, e.owner).expect("art.squad checks the member");
                let shown = squad.shown(e.health, k.max_health);
                let place = |i: usize| {
                    let (dx, dy) = squad.place(i, facing);
                    (wx + dx * tile, wy + dy * tile)
                };
                // Members further down the screen go on top; each starts its walk at its own point in the cycle.
                let mut order: Vec<usize> = (0..shown).collect();
                order.sort_by(|&a, &b| place(a).1.total_cmp(&place(b).1));
                for i in order {
                    let (x, y) = place(i);
                    let (sx, sy) = cam.to_screen(x - tile / 2.0, y - tile / 2.0);
                    let step = if moving { squad.step(i, walk, member.cycle()) } else { 0 };
                    let dst = Rect::new(sx, sy, tile * cam.zoom, tile * cam.zoom);
                    batch.sprite(member.tex, member.facing_frame(facing, step), dst, [255; 4]);
                }
                continue;
            }
            let (sx, sy) = cam.to_screen(wx - tile / 2.0, wy - tile / 2.0);
            let dst = Rect::new(sx, sy, tile * cam.zoom, tile * cam.zoom);
            match art.sprite(&k.id, e.owner) {
                Some(s) => batch.sprite(s.tex, s.facing_frame(facing, walk), dst, [255; 4]),
                None => draw_sprite(batch, None, 0, dst, e.owner),
            }
        }
        for p in &game.state.projectiles {
            let name = game.state.weapon_ids.get(p.weapon.0 as usize).map_or("", |s| s.as_str());
            let effect = if name.contains("rocket") { "rocket" } else { "shell" };
            let (sx, sy) = cam.to_screen(p.x as f32 * px, p.y as f32 * px);
            match art.effect(effect) {
                Some(s) => {
                    let (fw, fh) = (s.w * cam.zoom, s.h * cam.zoom);
                    batch.sprite(s.tex, s.frame(0), Rect::new(sx - fw / 2.0, sy - fh / 2.0, fw, fh), [255; 4]);
                }
                None => batch.fill(Rect::new(sx - 2.0, sy - 2.0, 4.0, 4.0), [255, 230, 120, 255]),
            }
        }
        // Explosions, dropping those that have finished.
        self.effects.retain(|fx| {
            let Some(s) = art.effect(fx.id) else { return false };
            let frame = tick.saturating_sub(fx.start) / EFFECT_TICKS;
            if frame >= s.frames {
                return false;
            }
            let (sx, sy) = cam.to_screen(fx.x as f32 * px, fx.y as f32 * px);
            let (fw, fh) = (s.w * cam.zoom, s.h * cam.zoom);
            batch.sprite(s.tex, s.frame(frame), Rect::new(sx - fw / 2.0, sy - fh / 2.0, fw, fh), [255; 4]);
            true
        });
        // Selection boxes, and health bars on whatever is selected or hurt.
        for e in &game.state.entities {
            let k = game.rules.kind(e.kind);
            let selected = self.selected.contains(&e.id);
            if !selected && e.health >= k.max_health {
                continue;
            }
            let r = if k.building {
                let t = e.tile();
                let (sx, sy) = cam.to_screen(t.x as f32 * tile, t.y as f32 * tile);
                Rect::new(sx, sy, k.width as f32 * tile * cam.zoom, k.height as f32 * tile * cam.zoom)
            } else {
                let (wx, wy) = at(e);
                let (sx, sy) = cam.to_screen(wx - tile / 2.0, wy - tile / 2.0);
                Rect::new(sx, sy, tile * cam.zoom, tile * cam.zoom)
            };
            if selected {
                batch.outline(r, 1.0, [240, 240, 240, 255]);
            }
            let share = (e.health.max(0) as f32 / k.max_health.max(1) as f32).min(1.0);
            let colour = if share > 0.5 {
                [60, 200, 70, 255]
            } else if share > 0.25 {
                [230, 200, 40, 255]
            } else {
                [220, 50, 40, 255]
            };
            batch.fill(Rect::new(r.x, r.y - 4.0, r.w, 3.0), [0, 0, 0, 200]);
            batch.fill(Rect::new(r.x, r.y - 4.0, r.w * share, 3.0), colour);
        }
    }
}

/// The frame of an eight-facing strip for a facing of 0 to 255 clockwise from north.
pub fn facing_frame(facing: i64) -> u32 {
    (((facing + 16).rem_euclid(256)) / 32) as u32
}

/// A sprite, or a plain box in a player colour when the pack has no art for it.
fn draw_sprite(batch: &mut SpriteBatch, strip: Option<&Strip>, frame: u32, dst: Rect, owner: u32) {
    match strip {
        Some(s) => batch.sprite(s.tex, s.frame(frame), dst, [255; 4]),
        None => {
            const COLOURS: [[u8; 4]; 4] =
                [[60, 120, 208, 255], [208, 64, 48, 255], [60, 160, 72, 255], [208, 170, 32, 255]];
            batch.fill(dst, COLOURS[owner as usize % COLOURS.len()]);
        }
    }
}
