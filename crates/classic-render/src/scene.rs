//! Drawing a Classic game: the map, buildings, units, shells, explosions, selection boxes and health bars. Reads
//! the game state and its events; never changes either. Anything only the picture needs (which way a unit's body
//! points, where it was last tick, explosions still playing) lives here, outside the simulation.

use std::collections::BTreeMap;

use classic_sim::combat::facing_to;
use classic_sim::map::{RESOURCE_PER_TILE, TILE};
use classic_sim::{Entity, Game, Ghost, Hazard, Terrain, vision};

use crate::art::{Art, Strip};
use crate::effects::{Effects, FIRE_TICKS};
use crate::fog;
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
/// How high an aircraft at its cruising height is drawn above its shadow, in tiles.
const CRUISE_LIFT: f32 = 0.75;
/// Ticks each frame of the hazard's ripple stays on screen.
const RIPPLE_TICKS: u32 = 3;
/// Frames in the hazard's strike, then its sink, played out over the ticks it stays surfaced (the studio's
/// `creatures/hazard` anims; a pack with shorter ones just holds their last frame).
const STRIKE_FRAMES: u32 = 12;
const SINK_FRAMES: u32 = 6;

#[derive(Default)]
pub struct Scene {
    /// Body facing per unit, 0 to 255 clockwise from north.
    facing: BTreeMap<u32, i64>,
    /// Positions before the latest tick, for drawing between ticks; hazards' too.
    prev: BTreeMap<u32, (i64, i64)>,
    /// Muzzle flashes, shots, explosions and smoke.
    pub fx: Effects,
    /// Members each squad showed when last drawn, so the ones it loses can fall.
    shown: BTreeMap<u32, usize>,
    fallen: Vec<Fallen>,
    /// How many of the game's events have been read.
    seen: usize,
    pub selected: Vec<u32>,
    /// Control groups: the units kept under each number key, 0 to 9.
    pub groups: [Vec<u32>; 10],
    /// The player whose fog of war is drawn: enemies out of their sight are hidden, enemy buildings show as they last
    /// saw them, and shroud and fog cover the map. `None`, or fog off in the rules, shows everything.
    pub viewer: Option<u32>,
}

impl Scene {
    /// A scene drawn as `player` sees it, through their fog of war.
    pub fn for_player(player: u32) -> Scene {
        Scene { viewer: Some(player), ..Scene::default() }
    }

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
        self.prev.extend(hazards(game).iter().map(|z| (z.id, (z.x, z.y))));
        self.fx.before_step(game);
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
        self.fx.after_step(game, &game.events[self.seen..]);
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
        self.fx.update(game, art, &self.facing);
        let viewer = self.viewer.filter(|_| game.state.vision.is_some());
        let shows = |e: &Entity| viewer.is_none_or(|p| vision::visible(&game.state, &game.rules, p, e));
        let ghosts: &[Ghost] = match (viewer, &game.state.vision) {
            (Some(p), Some(v)) => v.players.get(p as usize).map_or(&[], |s| &s.ghosts),
            _ => &[],
        };
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
        // Concrete slabs on the ground, in their owner's colours, under the buildings.
        if let Some(slabs) = &game.state.slabs {
            for ty in y0..y1 {
                for tx in x0..x1 {
                    let o = slabs.tiles[(ty * game.map.width + tx) as usize];
                    if o == 0 {
                        continue;
                    }
                    let (sx, sy) = cam.to_screen(tx as f32 * tile, ty as f32 * tile);
                    let dst = Rect::new(sx, sy, tile * cam.zoom, tile * cam.zoom);
                    match art.sprite("slab", o - 1) {
                        Some(s) => batch.sprite(s.tex, s.facing_frame(0, 0), dst, [255; 4]),
                        None => batch.fill(dst, [150, 150, 150, 255]),
                    }
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
            if !shows(e) {
                continue;
            }
            if !k.building {
                units.push(e);
                continue;
            }
            let t = e.tile();
            let (sx, sy) = cam.to_screen(t.x as f32 * tile, t.y as f32 * tile);
            let dst = Rect::new(sx, sy, k.width as f32 * tile * cam.zoom, k.height as f32 * tile * cam.zoom);
            let anims: &[&str] = if e.health * 2 < k.max_health { &["damaged"] } else { &["idle"] };
            let pose =
                Pose { facing: e.facing, turret: e.facing, anims, step: tick / OVERLAY_TICKS, alpha: 255, lift: 0.0 };
            draw_look(batch, art, cam.zoom, &k.id, e.owner, (sx, sy), dst, &pose);
        }
        // Enemy buildings out of sight, as the viewer last saw them; gone or not.
        for g in ghosts.iter().filter(|g| !game.state.entity(g.id).is_some_and(&shows)) {
            let k = game.rules.kind(g.kind);
            let (sx, sy) = cam.to_screen(g.x as f32 * tile, g.y as f32 * tile);
            let dst = Rect::new(sx, sy, k.width as f32 * tile * cam.zoom, k.height as f32 * tile * cam.zoom);
            let anims: &[&str] = if g.health * 2 < k.max_health { &["damaged"] } else { &["idle"] };
            let pose = Pose { facing: 0, turret: 0, anims, step: 0, alpha: 255, lift: 0.0 };
            draw_look(batch, art, cam.zoom, &k.id, g.owner, (sx, sy), dst, &pose);
        }
        units.sort_by_key(|e| (e.y, e.id));
        // Aircraft, and what they carry, go over everything on the ground (rules-movement.md section 9); a carried
        // unit just before its carrier, so it hangs under it.
        let (mut flying, units): (Vec<&Entity>, Vec<&Entity>) =
            units.into_iter().partition(|e| game.rules.kind(e.kind).air || e.carried_by.is_some());
        let carrier_of = |e: &Entity| e.carried_by.and_then(|c| game.state.entity(c));
        flying.sort_by_key(|e| {
            (carrier_of(e).map_or(e.y, |c| c.y), carrier_of(e).map_or(e.id, |c| c.id), e.carried_by.is_none())
        });
        // The viewer sees a hazard only where they see the ground it is on.
        let hazard_shows = |z: &Hazard| {
            let t = classic_sim::Tile { x: z.x.div_euclid(TILE) as i32, y: z.y.div_euclid(TILE) as i32 };
            viewer.is_none_or(|p| game.state.vision.as_ref().is_some_and(|v| v.shows(p, t.x, t.y)))
        };
        // Hazards under the sand: a ripple, under the units.
        let surface = game.rules.hazard.as_ref().map_or(1, |h| h.surface_ticks.max(1));
        for z in hazards(game).iter().filter(|z| z.surfaced == 0 && hazard_shows(z)) {
            let (ox, oy) = self.prev.get(&z.id).copied().unwrap_or((z.x, z.y));
            let x = (ox as f32 + (z.x - ox) as f32 * alpha) * px;
            let y = (oy as f32 + (z.y - oy) as f32 * alpha) * px;
            // A full one fades as it goes deep.
            let fade = z.leaving.map_or(255, |left| (255 * left.min(30) / 30) as u8);
            let pose =
                Pose { facing: 0, turret: 0, anims: &["ripple"], step: tick / RIPPLE_TICKS, alpha: fade, lift: 0.0 };
            draw_hazard(batch, art, cam, tile, (x, y), &pose);
        }
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
            let pose = Pose { facing: f.facing, turret: f.facing, anims: &[die], step, alpha, lift: 0.0 };
            draw_look(batch, art, cam.zoom, &f.member, f.owner, (sx, sy), cell, &pose);
            true
        });
        // Muzzle flashes the bodies hide, under them.
        self.fx.draw(batch, art, game, cam, alpha, true);
        let (ground, total) = (units.len(), units.len() + flying.len());
        for (n, e) in units.into_iter().chain(flying).enumerate() {
            // Surfaced hazards over everything on the ground, under the aircraft.
            if n == ground {
                draw_strikes(batch, art, game, cam, tile, surface, &hazard_shows);
            }
            let k = game.rules.kind(e.kind);
            let carrier = e.carried_by.and_then(|c| game.state.entity(c));
            let (wx, wy) = at(carrier.unwrap_or(e));
            let up = lift(game, e) * tile * cam.zoom;
            // A soldier who just fired holds the firing pose.
            let firing = self.fx.fired.get(&e.id).map(|&t| tick.saturating_sub(t)).filter(|&age| age < FIRE_TICKS);
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
                    let (anims, step): (&[&str], u32) = match firing {
                        Some(age) => (&["fire"], age),
                        None if moving => (&["walk", "move"], squad.step(i, walk, cycle)),
                        None => (&["idle"], 0),
                    };
                    // A soldier fires the way the unit aims.
                    let body = if firing.is_some() { e.facing } else { facing };
                    let pose = Pose { facing: body, turret: body, anims, step, alpha: 255, lift: 0.0 };
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
            // Aircraft loop their idle frames (rotors, wings) while they are up.
            let pose = Pose {
                facing,
                turret,
                anims: if moving && !k.air { &["walk", "move"] } else { &["idle"] },
                step: if k.air && e.altitude > 0 { tick / WALK_TICKS } else { walk },
                alpha: 255,
                lift: up,
            };
            draw_look(batch, art, cam.zoom, &k.id, e.owner, (sx, sy), cell, &pose);
        }
        if ground == total {
            draw_strikes(batch, art, game, cam, tile, surface, &hazard_shows);
        }
        self.fx.draw_shots(batch, art, game, cam, alpha);
        self.fx.draw(batch, art, game, cam, alpha, false);
        // Shroud and fog over everything on the ground.
        if let Some(p) = viewer {
            fog::draw(batch, art.fog, game, p, cam, tile, (x0, y0, x1, y1));
        }
        // Selection boxes, and health bars on whatever is selected or hurt and in sight.
        for e in game.state.entities.iter().filter(|e| shows(e)) {
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
                let (wx, wy) = at(e.carried_by.and_then(|c| game.state.entity(c)).unwrap_or(e));
                let (sx, sy) = cam.to_screen(wx - tile / 2.0, wy - tile / 2.0);
                Rect::new(sx, sy - lift(game, e) * tile * cam.zoom, tile * cam.zoom, tile * cam.zoom)
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

fn hazards(game: &Game) -> &[Hazard] {
    game.state.hazards.as_ref().map_or(&[], |h| &h.list)
}

/// A hazard at world pixel `at`: the studio's creature where the pack has one, else its placeholder (a warning
/// sign in the generic pack) on its tile.
fn draw_hazard(batch: &mut SpriteBatch, art: &Art, cam: &Camera, tile: f32, at: (f32, f32), pose: &Pose) {
    let (sx, sy) = cam.to_screen(at.0, at.1);
    let cell = Rect::new(sx - tile * cam.zoom / 2.0, sy - tile * cam.zoom / 2.0, tile * cam.zoom, tile * cam.zoom);
    draw_look(batch, art, cam.zoom, "hazard", 0, (sx, sy), cell, pose);
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
    /// Screen pixels the sprite is drawn above its ground point (aircraft); its shadow stays on the ground.
    lift: f32,
}

/// How far above its shadow `e` is drawn, in tiles: an aircraft by its altitude, a unit being carried just under its
/// carrier, anything else not at all. Drawing and picking with the mouse both use it.
pub fn lift(game: &Game, e: &Entity) -> f32 {
    let cruise = game.rules.air.cruise_altitude.max(1) as f32;
    let up = |e: &Entity| e.altitude as f32 / cruise * CRUISE_LIFT;
    match e.carried_by.and_then(|c| game.state.entity(c)) {
        Some(c) => (up(c) - 0.25).max(0.0),
        None => up(e),
    }
}

/// Hazards above the sand: the strike, then the sink, over everything on the ground.
fn draw_strikes(
    batch: &mut SpriteBatch,
    art: &Art,
    game: &Game,
    cam: &Camera,
    tile: f32,
    surface: u32,
    shows: &dyn Fn(&Hazard) -> bool,
) {
    let px = tile / TILE as f32;
    for z in hazards(game).iter().filter(|z| z.surfaced > 0 && shows(z)) {
        let up = surface.saturating_sub(z.surfaced);
        let frame = up * (STRIKE_FRAMES + SINK_FRAMES) / surface;
        let (anims, step): (&[&str], u32) = if frame < STRIKE_FRAMES {
            (&["strike"], frame)
        } else {
            (&["sink"], (frame - STRIKE_FRAMES).min(SINK_FRAMES - 1))
        };
        let pose = Pose { facing: 0, turret: 0, anims, step, alpha: 255, lift: 0.0 };
        draw_hazard(batch, art, cam, tile, (z.x as f32 * px, z.y as f32 * px), &pose);
    }
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
        // A placeholder strip has no shadow of its own: a lifted one gets a dark patch on the ground.
        let up = Rect::new(cell.x, cell.y - pose.lift, cell.w, cell.h);
        if pose.lift > 0.0 {
            let shadow = Rect::new(cell.x + cell.w * 0.2, cell.y + cell.h * 0.4, cell.w * 0.6, cell.h * 0.3);
            batch.fill(shadow, [0, 0, 0, (u32::from(SHADOW_ALPHA) * u32::from(pose.alpha) / 255) as u8]);
        }
        match art.sprite(id, owner) {
            Some(s) => batch.sprite(s.tex, s.facing_frame(pose.facing, pose.step), up, [255, 255, 255, pose.alpha]),
            None => draw_sprite(batch, None, 0, up, owner),
        }
        return;
    };
    let k = zoom * art.tile / 32.0 / sprite.scale;
    // The shadow on the ground point, the rest lifted above it.
    let put = |batch: &mut SpriteBatch, f: &Frame, tint: [u8; 4], lift: f32| {
        let dst = Rect::new(ground.0 - f.pivot.0 * k, ground.1 - lift - f.pivot.1 * k, f.src.w * k, f.src.h * k);
        batch.sprite(tex, f.src, dst, tint);
    };
    let white = [255, 255, 255, pose.alpha];
    if let Some(a) = sprite.body().and_then(|p| p.anim(pose.anims)) {
        let i = a.index(pose.facing, pose.step);
        if let Some(sh) = a.shadow.get(i) {
            put(batch, sh, [255, 255, 255, (u32::from(SHADOW_ALPHA) * u32::from(pose.alpha) / 255) as u8], 0.0);
        }
        if let Some(f) = a.frames.get(i) {
            put(batch, f, white, pose.lift);
        }
    }
    if let Some(a) = sprite.turret().and_then(|p| p.anim(&["idle"]))
        && let Some(f) = a.frames.get(a.index(pose.turret, 0))
    {
        put(batch, f, white, pose.lift);
    }
    for part in sprite.overlays() {
        if let Some(a) = part.anim(&["idle"])
            && let Some(f) = a.frames.get(a.index(0, pose.step))
        {
            put(batch, f, white, pose.lift);
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
