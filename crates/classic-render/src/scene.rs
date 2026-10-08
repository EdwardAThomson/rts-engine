//! Drawing a Classic game: the map, buildings, units, shells, explosions, selection boxes and health bars. Reads
//! the game state and its events; never changes either. Anything only the picture needs (which way a unit's body
//! points, where it was last tick, explosions still playing) lives here, outside the simulation.

use std::collections::BTreeMap;

use classic_sim::combat::facing_to;
use classic_sim::map::{RESOURCE_PER_TILE, TILE};
use classic_sim::world::Event;
use classic_sim::{Entity, Game, Terrain};

use crate::art::{Art, Strip};
use crate::platform::{Rect, SpriteBatch, TexId};
use crate::studio::{Frame, SHADOW_ALPHA};
use crate::tiles;

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
    /// Which of the two deaths it plays.
    which: usize,
}

/// Ticks each frame of a walk cycle stays on screen.
const WALK_TICKS: u32 = 3;
/// Ticks a fallen squad member stays on screen: its death, then a fade.
const FALL_TICKS: u32 = 48;
/// Ticks each frame of a building's overlay (a pump, a turning dish) stays on screen.
const OVERLAY_TICKS: u32 = 4;

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
        if let Some((set, tex)) = &art.tileset {
            draw_tiles(batch, set, *tex, game, cam, tile, (x0, y0, x1, y1));
        }
        // Without a tile set, the art index's plain tiles, one per map tile, and the resource faded over them.
        let plain = if art.tileset.is_some() { 0..0 } else { y0..y1 };
        for ty in plain {
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
            let anims: &[&str] = if e.health * 2 < k.max_health { &["damaged"] } else { &["idle"] };
            let pose = Pose { facing: e.facing, turret: e.facing, anims, step: tick / OVERLAY_TICKS, alpha: 255 };
            draw_look(batch, art, cam.zoom, &k.id, e.owner, (sx, sy), dst, &pose);
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
                self.fallen.push(Fallen {
                    member: squad.member.clone(),
                    owner: e.owner,
                    x,
                    y,
                    facing,
                    start: tick,
                    which: i % 2,
                });
            }
        }
        self.fallen.retain(|f| {
            let age = tick.saturating_sub(f.start);
            if age >= FALL_TICKS {
                return false;
            }
            // A death anim plays out, then the body fades; one without plays nothing and fades the standing frame.
            let alpha = if age < FALL_TICKS / 2 { 255 } else { (255 * (FALL_TICKS - age) / (FALL_TICKS / 2)) as u8 };
            let die = if f.which == 0 { "die-1" } else { "die-2" };
            let length =
                art.studio.get(&f.member, f.owner).and_then(|(s, _)| s.body()?.anims.get(die)).map(|a| a.length);
            let step = length.map_or(0, |n| (age / WALK_TICKS).min(n.saturating_sub(1)));
            let (sx, sy) = cam.to_screen(f.x, f.y);
            let cell =
                Rect::new(sx - tile * cam.zoom / 2.0, sy - tile * cam.zoom / 2.0, tile * cam.zoom, tile * cam.zoom);
            let pose = Pose { facing: f.facing, turret: f.facing, anims: &[die], step, alpha };
            draw_look(batch, art, cam.zoom, &f.member, f.owner, (sx, sy), cell, &pose);
            true
        });
        for e in units {
            let k = game.rules.kind(e.kind);
            let (wx, wy) = at(e);
            let facing = *self.facing.get(&e.id).unwrap_or(&e.facing);
            let moving = self.prev.get(&e.id).is_some_and(|&p| p != (e.x, e.y));
            let walk = if moving { tick / WALK_TICKS } else { 0 };
            if let Some(squad) = art.squad(&k.id) {
                let shown = squad.shown(e.health, k.max_health);
                let place = |i: usize| {
                    let (dx, dy) = squad.place(i, facing);
                    (wx + dx * tile, wy + dy * tile)
                };
                // Members further down the screen go on top; each starts its walk at its own point in the cycle.
                let mut order: Vec<usize> = (0..shown).collect();
                order.sort_by(|&a, &b| place(a).1.total_cmp(&place(b).1));
                let cycle = member_cycle(art, &squad.member, e.owner);
                for i in order {
                    let (x, y) = place(i);
                    let (sx, sy) = cam.to_screen(x, y);
                    let step = if moving { squad.step(i, walk, cycle) } else { 0 };
                    let pose = Pose {
                        facing,
                        turret: facing,
                        anims: if moving { &["walk", "move"] } else { &["idle"] },
                        step,
                        alpha: 255,
                    };
                    let cell = Rect::new(
                        sx - tile * cam.zoom / 2.0,
                        sy - tile * cam.zoom / 2.0,
                        tile * cam.zoom,
                        tile * cam.zoom,
                    );
                    draw_look(batch, art, cam.zoom, &squad.member, e.owner, (sx, sy), cell, &pose);
                }
                continue;
            }
            let (sx, sy) = cam.to_screen(wx, wy);
            let cell =
                Rect::new(sx - tile * cam.zoom / 2.0, sy - tile * cam.zoom / 2.0, tile * cam.zoom, tile * cam.zoom);
            let turret = if e.target.is_some() || k.weapon.is_none() { e.facing } else { facing };
            let pose = Pose {
                facing,
                turret,
                anims: if moving { &["walk", "move"] } else { &["idle"] },
                step: walk,
                alpha: 255,
            };
            draw_look(batch, art, cam.zoom, &k.id, e.owner, (sx, sy), cell, &pose);
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

/// How to draw one thing: which way its body and turret face, the anims to try in order, how far into the cycle,
/// and how opaque.
struct Pose<'a> {
    facing: i64,
    turret: i64,
    anims: &'a [&'a str],
    step: u32,
    alpha: u8,
}

/// Draw `id` in `owner`'s colours: the studio's sprite with its pivot on the screen point `ground`, where the pack
/// has one (shadow, body, turret, overlays), else its placeholder strip filling `cell`, else a box in a player colour.
#[allow(clippy::too_many_arguments)]
fn draw_look(
    batch: &mut SpriteBatch,
    art: &Art,
    zoom: f32,
    id: &str,
    owner: u32,
    ground: (f32, f32),
    cell: Rect,
    pose: &Pose,
) {
    let Some((sprite, tex)) = art.studio.get(id, owner) else {
        match art.sprite(id, owner) {
            Some(s) => batch.sprite(s.tex, s.facing_frame(pose.facing, pose.step), cell, [255, 255, 255, pose.alpha]),
            None => draw_sprite(batch, None, 0, cell, owner),
        }
        return;
    };
    let k = zoom * art.tile / 32.0 / sprite.scale;
    let put = |batch: &mut SpriteBatch, f: &Frame, tint: [u8; 4]| {
        let dst = Rect::new(ground.0 - f.pivot.0 * k, ground.1 - f.pivot.1 * k, f.src.w * k, f.src.h * k);
        batch.sprite(tex, f.src, dst, tint);
    };
    let white = [255, 255, 255, pose.alpha];
    if let Some(a) = sprite.body().and_then(|p| p.anim(pose.anims)) {
        let i = a.index(pose.facing, pose.step);
        if let Some(sh) = a.shadow.get(i) {
            put(batch, sh, [255, 255, 255, (u32::from(SHADOW_ALPHA) * u32::from(pose.alpha) / 255) as u8]);
        }
        if let Some(f) = a.frames.get(i) {
            put(batch, f, white);
        }
    }
    if let Some(a) = sprite.turret().and_then(|p| p.anim(&["idle"]))
        && let Some(f) = a.frames.get(a.index(pose.turret, 0))
    {
        put(batch, f, white);
    }
    for part in sprite.overlays() {
        if let Some(a) = part.anim(&["idle"])
            && let Some(f) = a.frames.get(a.index(0, pose.step))
        {
            put(batch, f, white);
        }
    }
}

/// Frames in the walk cycle `member` is drawn with: the studio's walk if it has one, else its strip's.
fn member_cycle(art: &Art, member: &str, owner: u32) -> u32 {
    match art.studio.get(member, owner) {
        Some((s, _)) => s.body().and_then(|p| p.anim(&["walk", "move"])).map_or(1, |a| a.length),
        None => art.sprite(member, owner).map_or(1, |s| s.cycle()),
    }
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

/// The ground from a tile set, layer by layer, for the map tiles from (x0, y0) up to (x1, y1): each drawn tile sits
/// half a tile up and left of its map tile, so the four map tiles round it are its corners (`tiles`). Drawn tiles
/// that hang over the map's edge are cut to it.
fn draw_tiles(
    batch: &mut SpriteBatch,
    set: &tiles::Tileset,
    tex: TexId,
    game: &Game,
    cam: &Camera,
    tile: f32,
    (x0, y0, x1, y1): (i32, i32, i32, i32),
) {
    let (map_w, map_h) = (game.map.width as f32 * tile, game.map.height as f32 * tile);
    for layer in &set.layers {
        for y in y0..=y1 {
            for x in x0..=x1 {
                let case = tiles::corner_case(layer, game, x, y);
                let Some(src) = set.source(layer, case, x, y) else { continue };
                // The drawn tile in world pixels, cut to the map.
                let (wx, wy) = ((x as f32 - 0.5) * tile, (y as f32 - 0.5) * tile);
                let (cx0, cy0) = (wx.max(0.0), wy.max(0.0));
                let (cx1, cy1) = ((wx + tile).min(map_w), (wy + tile).min(map_h));
                if cx1 <= cx0 || cy1 <= cy0 {
                    continue;
                }
                let k = src.w / tile;
                let src = Rect::new(src.x + (cx0 - wx) * k, src.y + (cy0 - wy) * k, (cx1 - cx0) * k, (cy1 - cy0) * k);
                let (sx, sy) = cam.to_screen(cx0, cy0);
                let dst = Rect::new(sx, sy, (cx1 - cx0) * cam.zoom, (cy1 - cy0) * cam.zoom);
                batch.sprite(tex, src, dst, [255; 4]);
            }
        }
    }
}
