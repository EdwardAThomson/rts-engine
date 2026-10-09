//! Effects: muzzle flashes, shells and rockets in flight, smoke trails, explosions, sparks, and smoke and fire on
//! damaged things. They play from the game's events and state and never change either (the effects-on and -off
//! state hashes are the same because the renderer only ever holds `&Game`).
//!
//! Positions are kept in the simulation's units (256 per tile), as floats: effects drift and rise between tiles.
//! Time is the game's tick plus the fraction drawn between ticks, never the clock, so pausing pauses them and the
//! same game draws the same effects. Their randomness (where smoke rises, which corner of a building burns) comes
//! from a generator of their own that the simulation never reads.

use std::collections::{BTreeMap, BTreeSet};

use classic_sim::combat::facing_to;
use classic_sim::map::TILE;
use classic_sim::world::Event;
use classic_sim::{Entity, Game};

use crate::art::Art;
use crate::platform::{Rect, SpriteBatch};
use crate::scene::Camera;

/// One effect playing.
struct Effect {
    id: &'static str,
    /// Where, in simulation units, when it started, and how fast it drifts (units a tick).
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    /// The tick it starts on; it may be in the future, for the later blasts of a building going up.
    start: u32,
    facing: i64,
    /// Drawn this many times its art's size.
    size: f32,
    /// Drawn under the units (a muzzle flash the body hides).
    under: bool,
}

/// Ticks each frame of an effect stays on screen.
fn frame_ticks(id: &str) -> f32 {
    match id {
        "muzzle_flash_gun" | "muzzle_flash_rocket" | "hit_spark" => 1.0,
        "smoke_puff" => 3.0,
        _ => 2.0,
    }
}

/// Ticks an infantry soldier shows its firing pose after a shot.
pub const FIRE_TICKS: u32 = 3;

/// Where a rocket or shell leaves from, against where the simulation starts it (the firer's ground point).
struct Lift {
    dx: f32,
    dy: f32,
    /// How far it flies, to shrink the lift to nothing as it lands.
    distance: f32,
    to: (f32, f32),
}

#[derive(Default)]
pub struct Effects {
    playing: Vec<Effect>,
    /// Events since the last draw that need the art to place.
    pending: Vec<Event>,
    /// Rocket positions since the last draw, for their smoke trails: (projectile, x, y, tick).
    trail: Vec<(u32, f32, f32, u32)>,
    lift: BTreeMap<u32, Lift>,
    /// Projectile positions before the latest tick.
    prev: BTreeMap<u32, (i64, i64)>,
    /// The tick each unit last fired on.
    pub fired: BTreeMap<u32, u32>,
    /// The last tick damaged things have smoked for.
    smoked: u32,
    /// Units counting down to their own blast, which go up bigger than a vehicle's.
    fused: BTreeSet<u32>,
    rng: u32,
}

impl Effects {
    fn random(&mut self) -> f32 {
        if self.rng == 0 {
            self.rng = 0x9e37_79b9;
        }
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        (self.rng >> 8) as f32 / (1 << 24) as f32
    }

    /// How many effects are playing or waiting to start, by id: for tests.
    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut n = BTreeMap::new();
        for e in &self.playing {
            *n.entry(e.id).or_default() += 1;
        }
        n
    }

    pub fn before_step(&mut self, game: &Game) {
        self.prev = game.state.projectiles.iter().map(|p| (p.id, (p.x, p.y))).collect();
    }

    /// Note this tick's events and rockets; they are placed on the next draw, which has the art.
    pub fn after_step(&mut self, game: &Game, events: &[Event]) {
        for ev in events {
            if let Event::Fired { tick, unit, .. } = *ev {
                self.fired.insert(unit, tick);
            }
            if let Event::SelfDestructStarted { unit, .. } = *ev {
                self.fused.insert(unit);
            }
            if matches!(
                ev,
                Event::Fired { .. }
                    | Event::ProjectileSpawned { .. }
                    | Event::ProjectileHit { .. }
                    | Event::Hit { .. }
                    | Event::Destroyed { .. }
                    | Event::BeamFired { .. }
            ) {
                self.pending.push(ev.clone());
            }
        }
        let tick = game.state.tick;
        // A puff where each rocket is and two on the way from where it was, so the trail joins up.
        for p in &game.state.projectiles {
            if rocket(game, p.weapon) {
                let (ox, oy) = self.prev.get(&p.id).copied().unwrap_or((p.x, p.y));
                for t in [1.0 / 3.0, 2.0 / 3.0, 1.0] {
                    let (x, y) = (ox as f32 + (p.x - ox) as f32 * t, oy as f32 + (p.y - oy) as f32 * t);
                    self.trail.push((p.id, x, y, tick));
                }
            }
        }
        self.fired.retain(|id, t| tick.saturating_sub(*t) < 60 && game.state.entity(*id).is_some());
        self.lift.retain(|id, _| game.state.projectiles.iter().any(|p| p.id == *id));
    }

    /// Turn what happened since the last draw into effects, and smoke what is damaged.
    pub fn update(&mut self, game: &Game, art: &Art, facing: &BTreeMap<u32, i64>) {
        for ev in std::mem::take(&mut self.pending) {
            match ev {
                Event::Fired { tick, unit, weapon, .. } => {
                    let Some(e) = game.state.entity(unit) else { continue };
                    let id = if rocket(game, weapon) { "muzzle_flash_rocket" } else { "muzzle_flash_gun" };
                    for (x, y, hidden) in muzzles(game, art, e, facing) {
                        self.playing.push(Effect {
                            id,
                            x,
                            y,
                            vx: 0.0,
                            vy: 0.0,
                            start: tick,
                            facing: e.facing,
                            size: 1.0,
                            under: hidden,
                        });
                    }
                }
                Event::ProjectileSpawned { projectile, x, y, to_x, to_y, .. } => {
                    let Some(p) = game.state.projectiles.iter().find(|p| p.id == projectile) else { continue };
                    let Some(e) = game.state.entity(p.firer) else { continue };
                    let Some(&(mx, my, _)) = muzzles(game, art, e, facing).first() else { continue };
                    let distance = ((to_x - x) as f32).hypot((to_y - y) as f32).max(1.0);
                    self.lift.insert(
                        projectile,
                        Lift { dx: mx - x as f32, dy: my - y as f32, distance, to: (to_x as f32, to_y as f32) },
                    );
                }
                Event::ProjectileHit { tick, weapon, x, y, .. } => {
                    let id = if rocket(game, weapon) { "explosion_medium" } else { "explosion_small" };
                    let size = if rocket(game, weapon) { 0.7 } else { 1.0 };
                    self.burst(id, x as f32, y as f32, tick, size);
                }
                Event::Hit { tick, target, weapon, .. } => {
                    // Shots that land at once leave no burst of their own, so show the hit on armour.
                    if game.rules.weapon(weapon).speed != 0 {
                        continue;
                    }
                    let Some(e) = game.state.entity(target) else { continue };
                    let (jx, jy) = (self.random() - 0.5, self.random() - 0.5);
                    let (x, y) = centre(game, e);
                    self.burst("hit_spark", x + jx * 96.0, y - 48.0 + jy * 64.0, tick, 1.0);
                }
                // A beam ripples out along its line, a spark each half tile, a tick apart.
                Event::BeamFired { tick, x1, y1, x2, y2, .. } => {
                    let steps = (((x2 - x1).pow(2) + (y2 - y1).pow(2)) as f32).sqrt() / (TILE as f32 / 2.0);
                    for i in 1..=steps as u32 {
                        let t = i as f32 / steps;
                        let (x, y) = (x1 as f32 + (x2 - x1) as f32 * t, y1 as f32 + (y2 - y1) as f32 * t);
                        self.burst("hit_spark", x, y - 32.0, tick + i / 2, 1.4);
                    }
                }
                Event::Destroyed { tick, entity, x, y, .. } if self.fused.remove(&entity) => {
                    self.burst("explosion_large", x as f32, y as f32, tick, 1.6);
                    for i in 0..4 {
                        let (fx, fy) = (self.random() - 0.5, self.random() - 0.5);
                        self.burst(
                            "explosion_medium",
                            x as f32 + fx * 640.0,
                            y as f32 + fy * 640.0,
                            tick + 2 + i * 2,
                            1.0,
                        );
                    }
                }
                Event::Destroyed { tick, kind, x, y, .. } => {
                    let k = game.rules.kind(kind);
                    if art.squad(&k.id).is_some() {
                        continue; // its soldiers fall instead
                    }
                    if !k.building {
                        self.burst("explosion_medium", x as f32, y as f32, tick, 1.0);
                        continue;
                    }
                    // A building goes up in a few blasts over its footprint, then a big one in the middle.
                    let (tx, ty) = (x.div_euclid(TILE) * TILE, y.div_euclid(TILE) * TILE);
                    let (w, h) = ((k.width as i64 * TILE) as f32, (k.height as i64 * TILE) as f32);
                    for i in 0..k.width.max(k.height) as u32 + 1 {
                        let (fx, fy) = (self.random(), self.random());
                        let at = (tx as f32 + w * (0.15 + 0.7 * fx), ty as f32 + h * (0.2 + 0.7 * fy));
                        self.burst("explosion_medium", at.0, at.1, tick + i * 3, 0.9);
                    }
                    self.burst("explosion_large", tx as f32 + w / 2.0, ty as f32 + h * 0.6, tick + 5, 1.0);
                }
                _ => {}
            }
        }
        // Rockets leave puffs of smoke where they were each tick.
        for (id, x, y, tick) in std::mem::take(&mut self.trail) {
            let (lx, ly) = self.lift.get(&id).map_or((0.0, 0.0), |l| lift_at(l, x, y));
            let drift = self.random() - 0.5;
            self.playing.push(Effect {
                id: "smoke_puff",
                x: x + lx,
                y: y + ly,
                vx: drift * 3.0,
                vy: -2.0,
                start: tick,
                facing: 0,
                size: 0.7,
                under: false,
            });
        }
        // Damaged buildings and vehicles smoke, a puff every few ticks from a spot of their own.
        let tick = game.state.tick;
        let from = self.smoked.max(tick.saturating_sub(30)) + 1;
        for t in from..=tick {
            for e in &game.state.entities {
                let k = game.rules.kind(e.kind);
                if e.health * 2 >= k.max_health || art.squad(&k.id).is_some() {
                    continue;
                }
                let every = if k.building { 4 } else { 6 };
                if (t + e.id * 3) % every != 0 {
                    continue;
                }
                let (x, y) = smoke_spot(game, e, t / every);
                let (r1, r2) = (self.random(), self.random());
                self.playing.push(Effect {
                    id: "smoke_puff",
                    x: x + (r1 - 0.5) * 24.0,
                    y,
                    vx: 2.0 + r2 * 2.0,
                    vy: -9.0 - r2 * 3.0,
                    start: t,
                    facing: 0,
                    size: if k.building { 1.0 + 0.15 * k.width.min(k.height) as f32 } else { 0.9 },
                    under: false,
                });
            }
        }
        self.smoked = tick;
    }

    fn burst(&mut self, id: &'static str, x: f32, y: f32, start: u32, size: f32) {
        self.playing.push(Effect { id, x, y, vx: 0.0, vy: 0.0, start, facing: 0, size, under: false });
    }

    /// Draw the effects that go under the units (hidden muzzle flashes), or over them, dropping finished ones.
    pub fn draw(&mut self, batch: &mut SpriteBatch, art: &Art, game: &Game, cam: &Camera, alpha: f32, under: bool) {
        let px = art.tile / TILE as f32;
        let now = game.state.tick as f32 + alpha;
        self.playing.retain(|fx| {
            if fx.under != under {
                return true;
            }
            let age = now - fx.start as f32;
            if age < 0.0 {
                return true;
            }
            let (x, y) = (fx.x + fx.vx * age, fx.y + fx.vy * age);
            let (sx, sy) = cam.to_screen(x * px, y * px);
            let frame = (age / frame_ticks(fx.id)) as u32;
            draw_effect(batch, art, cam.zoom, fx.id, (sx, sy), fx.facing, frame, fx.size, 255)
        });
        if under {
            return;
        }
        // Badly damaged things burn.
        for e in &game.state.entities {
            let k = game.rules.kind(e.kind);
            if e.health * 4 >= k.max_health || art.squad(&k.id).is_some() {
                continue;
            }
            let spots = if k.building { 2 } else { 1 };
            for n in 0..spots {
                let (x, y) = smoke_spot(game, e, n + 7);
                let (sx, sy) = cam.to_screen(x * px, (y + 40.0) * px);
                let length = effect_length(art, "fire").max(1);
                let frame = (game.state.tick / 2 + e.id + n * 3) % length;
                draw_effect(batch, art, cam.zoom, "fire", (sx, sy), 0, frame, if k.building { 1.0 } else { 0.7 }, 255);
            }
        }
    }

    /// Draw the shells and rockets in flight, from their barrel tips down to where they land.
    pub fn draw_shots(&self, batch: &mut SpriteBatch, art: &Art, game: &Game, cam: &Camera, alpha: f32) {
        let px = art.tile / TILE as f32;
        for p in &game.state.projectiles {
            let (ox, oy) = self.prev.get(&p.id).copied().unwrap_or((p.x, p.y));
            let x = ox as f32 + (p.x - ox) as f32 * alpha;
            let y = oy as f32 + (p.y - oy) as f32 * alpha;
            let (lx, ly) = self.lift.get(&p.id).map_or((0.0, 0.0), |l| lift_at(l, x, y));
            let (sx, sy) = cam.to_screen((x + lx) * px, (y + ly) * px);
            let facing = facing_to(p.to_x - p.x, p.to_y - p.y);
            let id = if rocket(game, p.weapon) { "rocket" } else { "shell" };
            let frame = game.state.tick % effect_length(art, id).max(1);
            if !draw_effect(batch, art, cam.zoom, id, (sx, sy), facing, frame, 1.0, 255) {
                batch.fill(Rect::new(sx - 2.0, sy - 2.0, 4.0, 4.0), [255, 230, 120, 255]);
            }
        }
    }
}

/// Is weapon `w` a rocket? By its id, until weapons say how they look.
fn rocket(game: &Game, w: classic_sim::units::WeaponId) -> bool {
    game.state.weapon_ids.get(w.0 as usize).is_some_and(|s| s.contains("rocket") || s.contains("missile"))
}

/// The middle of an entity, in simulation units.
fn centre(game: &Game, e: &Entity) -> (f32, f32) {
    let k = game.rules.kind(e.kind);
    if !k.building {
        return (e.x as f32, e.y as f32);
    }
    let t = e.tile();
    let w = (k.width as i64 * TILE) as f32;
    let h = (k.height as i64 * TILE) as f32;
    ((t.x as i64 * TILE) as f32 + w / 2.0, (t.y as i64 * TILE) as f32 + h / 2.0)
}

/// Where damage smoke rises from: on a building, one of a few spots over its roof, chosen by `n`; on a unit, its
/// back deck.
fn smoke_spot(game: &Game, e: &Entity, n: u32) -> (f32, f32) {
    let k = game.rules.kind(e.kind);
    if !k.building {
        return (e.x as f32, e.y as f32 - 48.0);
    }
    let t = e.tile();
    let h = (e.id.wrapping_mul(2_654_435_761) ^ (n % 3).wrapping_mul(40_503)) >> 8;
    let fx = (h % 1000) as f32 / 1000.0;
    let fy = ((h / 1000) % 1000) as f32 / 1000.0;
    let w = (k.width as i64 * TILE) as f32;
    let d = (k.height as i64 * TILE) as f32;
    ((t.x as i64 * TILE) as f32 + w * (0.2 + 0.6 * fx), (t.y as i64 * TILE) as f32 + d * (0.1 + 0.5 * fy))
}

/// Where unit `e`'s shots leave from, in simulation units, and whether its body hides them: one point per soldier
/// standing in a squad, else its barrel tip. A unit with no measured tip shoots from its front edge.
fn muzzles(game: &Game, art: &Art, e: &Entity, facing: &BTreeMap<u32, i64>) -> Vec<(f32, f32, bool)> {
    let k = game.rules.kind(e.kind);
    // Atlas pixels to simulation units for a sprite drawn at `scale`.
    let units = |scale: f32| TILE as f32 / 32.0 / scale;
    let aim = e.facing;
    if let Some(squad) = art.squad(&k.id) {
        let body = *facing.get(&e.id).unwrap_or(&e.facing);
        let member = art.studio.get(&squad.member, e.owner).map(|(s, _)| s);
        return (0..squad.shown(e.health, k.max_health))
            .map(|i| {
                let (dx, dy) = squad.place(i, body);
                let (gx, gy) = (e.x as f32 + dx * TILE as f32, e.y as f32 + dy * TILE as f32);
                match member.and_then(|s| Some((s.muzzle(&["fire"], aim, aim, 0)?, s.scale))) {
                    Some((m, scale)) => (gx + m.x * units(scale), gy + m.y * units(scale), m.hidden),
                    None => (gx, gy - TILE as f32 / 4.0, false),
                }
            })
            .collect();
    }
    let ground = if k.building {
        let t = e.tile();
        ((t.x as i64 * TILE) as f32, (t.y as i64 * TILE) as f32)
    } else {
        (e.x as f32, e.y as f32)
    };
    if let Some((s, _)) = art.studio.get(&k.id, e.owner)
        && let Some(m) = s.muzzle(&["fire"], aim, aim, 0)
    {
        return vec![(ground.0 + m.x * units(s.scale), ground.1 + m.y * units(s.scale), m.hidden)];
    }
    let (cx, cy) = centre(game, e);
    let a = aim as f32 * std::f32::consts::TAU / 256.0;
    let reach = TILE as f32 * 0.45 * k.width.max(1) as f32;
    vec![(cx + a.sin() * reach, cy - a.cos() * reach, false)]
}

/// The lift of a shot at (x, y): all of it at the start, none where it lands.
fn lift_at(l: &Lift, x: f32, y: f32) -> (f32, f32) {
    let left = ((l.to.0 - x).hypot(l.to.1 - y) / l.distance).clamp(0.0, 1.0);
    (l.dx * left, l.dy * left)
}

/// Frames in effect `id`'s cycle, from its detailed art or its placeholder strip.
pub fn effect_length(art: &Art, id: &str) -> u32 {
    if let Some((s, _)) = art.fx(id) {
        return s.body().and_then(|p| p.anim(&["idle"])).map_or(0, |a| a.length);
    }
    art.effect(id).map_or(0, |s| s.cycle())
}

/// Draw frame `frame` of effect `id` facing `facing`, with its pivot on the screen point `at`, `size` times its
/// art's size. False when the effect has no such frame (it has finished) or no art.
#[allow(clippy::too_many_arguments)]
pub fn draw_effect(
    batch: &mut SpriteBatch,
    art: &Art,
    zoom: f32,
    id: &str,
    at: (f32, f32),
    facing: i64,
    frame: u32,
    size: f32,
    alpha: u8,
) -> bool {
    if let Some((s, tex)) = art.fx(id) {
        let Some(a) = s.body().and_then(|p| p.anim(&["idle"])) else { return false };
        if frame >= a.length {
            return false;
        }
        let Some(f) = a.frames.get(a.index(facing, frame)) else { return false };
        let k = zoom * art.tile / 32.0 / s.scale * size;
        let dst = Rect::new(at.0 - f.pivot.0 * k, at.1 - f.pivot.1 * k, f.src.w * k, f.src.h * k);
        batch.sprite(tex, f.src, dst, [255, 255, 255, alpha]);
        return true;
    }
    let Some(s) = art.effect(id) else { return false };
    if frame >= s.cycle() {
        return false;
    }
    let (fw, fh) = (s.w * zoom * size, s.h * zoom * size);
    batch.sprite(
        s.tex,
        s.facing_frame(facing, frame),
        Rect::new(at.0 - fw / 2.0, at.1 - fh / 2.0, fw, fh),
        [255, 255, 255, alpha],
    );
    true
}
