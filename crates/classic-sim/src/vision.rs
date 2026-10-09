//! Fog of war (playbooks `plans/rts/rules-world.md`, sections 2 and 3; the `fog` module, off unless a setting pack
//! turns it on). Each player has two per-tile arrays:
//!
//! - `explored`: set once a tile has ever been in sight. Unexplored tiles are **shroud**: black, and nothing on them
//!   is known.
//! - `seen`: how many of the player's sight sources cover the tile now. Explored with none is **fog**.
//!
//! With the module's `hide` number on, fog hides enemy units, and each player keeps a **ghost** record per enemy
//! building it has seen (type, owner, footprint and health as last seen), dropped once it looks again and finds the
//! building gone. With `hide` off it is shroud only, as in the original: explored ground shows everything on it.
//!
//! Sight is counted by reference, never recomputed (performance.md, section 6, from the reference-count fog of
//! OpenRA and 0 A.D.): a source adds 1 over its disc when it appears or changes tile, and takes it away when it
//! leaves or dies. The disc is every tile with `dx*dx + dy*dy <= r*r + r` from the source's footprint, so a building
//! sees from its edges. Adds and subtracts commute, so the counts don't depend on order; sources are kept in id order
//! anyway. Firing reveals the shooter: the player it fired at sees a disc of radius 1 round it for `reveal_ticks`.
//!
//! Visibility is game state, not drawing: targets must be in their owner's sight, and the computer opponent reads the
//! same arrays. It is hashed while the module is on.

use rts_core::hash::{Canon, CanonHasher};

use crate::map::Tile;
use crate::units::{Kind, Rules};
use crate::world::{Entity, GameState};

/// What a player knows of a tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TileView {
    /// Never seen.
    Shroud,
    /// Seen before, not now.
    Fog,
    /// In sight now.
    Visible,
}

/// An enemy building as a player last saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ghost {
    pub id: u32,
    pub kind: Kind,
    pub owner: u32,
    /// The footprint's top-left tile.
    pub x: i32,
    pub y: i32,
    pub health: i64,
}

/// One player's view of the map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sight {
    pub explored: Vec<bool>,
    pub seen: Vec<u16>,
    /// Enemy buildings as last seen, in id order. Kept only while fog hides.
    pub ghosts: Vec<Ghost>,
}

/// A disc of sight as it was last added: a footprint, inclusive, and a radius.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Source {
    id: u32,
    owner: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    r: i32,
}

/// A shooter shown to the player it fired at, until a tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reveal {
    pub player: u32,
    pub x: i32,
    pub y: i32,
    pub until: u32,
    /// Its disc has been added to `seen`.
    added: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vision {
    pub width: i32,
    pub height: i32,
    pub hide: bool,
    /// Each player's view, in player order.
    pub players: Vec<Sight>,
    /// Every entity's disc as last added, in id order.
    sources: Vec<Source>,
    /// Shooters shown to those they fired at, oldest first.
    pub reveals: Vec<Reveal>,
}

/// The radius a firing reveal shows round the shooter.
const REVEAL_RADIUS: i32 = 1;

impl Vision {
    pub fn new(width: i32, height: i32, players: usize, rules: &Rules) -> Option<Vision> {
        let fog = rules.fog.as_ref()?;
        let tiles = (width * height) as usize;
        let sight = Sight { explored: vec![fog.start_explored; tiles], seen: vec![0; tiles], ghosts: Vec::new() };
        Some(Vision {
            width,
            height,
            hide: fog.hide,
            players: vec![sight; players],
            sources: Vec::new(),
            reveals: Vec::new(),
        })
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        ((0..self.width).contains(&x) && (0..self.height).contains(&y)).then_some((y * self.width + x) as usize)
    }

    /// What `player` knows of tile (x, y). Off the map, or for a player the game doesn't have, it is shroud.
    pub fn tile(&self, player: u32, x: i32, y: i32) -> TileView {
        let (Some(p), Some(i)) = (self.players.get(player as usize), self.index(x, y)) else {
            return TileView::Shroud;
        };
        if p.seen[i] > 0 {
            TileView::Visible
        } else if p.explored[i] {
            TileView::Fog
        } else {
            TileView::Shroud
        }
    }

    /// Whether `player` can see what stands on (x, y) now: in sight while fog hides, explored while it doesn't.
    pub fn shows(&self, player: u32, x: i32, y: i32) -> bool {
        match self.tile(player, x, y) {
            TileView::Visible => true,
            TileView::Fog => !self.hide,
            TileView::Shroud => false,
        }
    }

    /// `player`'s ghost of building `id`, if it keeps one.
    pub fn ghost(&self, player: u32, id: u32) -> Option<&Ghost> {
        let p = self.players.get(player as usize)?;
        p.ghosts.binary_search_by_key(&id, |g| g.id).ok().map(|i| &p.ghosts[i])
    }

    /// Add one to (`add`) or take one from `seen` over a disc of radius `r` round a footprint.
    fn disc(&mut self, owner: u32, (x0, y0, x1, y1): (i32, i32, i32, i32), r: i32, add: bool) {
        let (w, h) = (self.width, self.height);
        let Some(p) = self.players.get_mut(owner as usize) else { return };
        let reach = r * r + r;
        for y in (y0 - r).max(0)..=(y1 + r).min(h - 1) {
            let dy = (y0 - y).max(y - y1).max(0);
            for x in (x0 - r).max(0)..=(x1 + r).min(w - 1) {
                let dx = (x0 - x).max(x - x1).max(0);
                if dx * dx + dy * dy > reach {
                    continue;
                }
                let i = (y * w + x) as usize;
                if add {
                    p.seen[i] += 1;
                    p.explored[i] = true;
                } else {
                    p.seen[i] -= 1;
                }
            }
        }
    }

    fn apply(&mut self, s: &Source, add: bool) {
        self.disc(s.owner, (s.x0, s.y0, s.x1, s.y1), s.r, add);
    }

    /// Show the shooter at `tile` to `player` for `ticks` from `tick`, or keep showing it longer if it is already.
    pub fn reveal(&mut self, player: u32, tile: Tile, tick: u32, ticks: u32) {
        if ticks == 0 || player as usize >= self.players.len() {
            return;
        }
        let until = tick + ticks;
        match self.reveals.iter_mut().find(|r| r.player == player && (r.x, r.y) == (tile.x, tile.y)) {
            Some(r) => r.until = r.until.max(until),
            None => self.reveals.push(Reveal { player, x: tile.x, y: tile.y, until, added: false }),
        }
    }
}

/// An entity's disc of sight, if it has one.
fn source(rules: &Rules, e: &Entity) -> Option<Source> {
    let k = rules.kind(e.kind);
    let t = e.tile();
    (k.vision > 0).then_some(Source {
        id: e.id,
        owner: e.owner,
        x0: t.x,
        y0: t.y,
        x1: t.x + k.width - 1,
        y1: t.y + k.height - 1,
        r: k.vision,
    })
}

/// Bring every player's sight up to date with the entities as they stand, at the end of a tick (and once when the
/// game starts). Does nothing while the fog module is off.
pub fn tick(state: &mut GameState, rules: &Rules) {
    let Some(mut v) = state.vision.take() else { return };
    // Entities that appeared, moved to another tile or died: walk the old discs and the new side by side, in id order.
    let now: Vec<Source> = state.entities.iter().filter_map(|e| source(rules, e)).collect();
    let old = std::mem::take(&mut v.sources);
    let (mut a, mut b) = (0, 0);
    while a < old.len() || b < now.len() {
        match (old.get(a), now.get(b)) {
            (Some(o), Some(n)) if o.id == n.id => {
                if o != n {
                    v.apply(o, false);
                    v.apply(n, true);
                }
                a += 1;
                b += 1;
            }
            (Some(o), Some(n)) if o.id < n.id => {
                v.apply(o, false);
                a += 1;
            }
            (Some(o), None) => {
                v.apply(o, false);
                a += 1;
            }
            (_, Some(n)) => {
                v.apply(n, true);
                b += 1;
            }
            (None, None) => break,
        }
    }
    v.sources = now;
    // Shooters shown to those they fired at: new ones appear, expired ones go.
    let tick = state.tick;
    let mut reveals = std::mem::take(&mut v.reveals);
    for r in &mut reveals {
        let at = (r.x, r.y, r.x, r.y);
        if !r.added {
            v.disc(r.player, at, REVEAL_RADIUS, true);
            r.added = true;
        }
        if r.until <= tick {
            v.disc(r.player, at, REVEAL_RADIUS, false);
        }
    }
    reveals.retain(|r| r.until > tick);
    v.reveals = reveals;
    if v.hide {
        ghosts(&mut v, state, rules);
    }
    state.vision = Some(v);
}

/// Each player's record of enemy buildings: those in sight now are written as they stand; a record whose footprint
/// is in sight with the building gone is dropped.
fn ghosts(v: &mut Vision, state: &GameState, rules: &Rules) {
    let (w, h) = (v.width, v.height);
    for (player, sight) in v.players.iter_mut().enumerate() {
        let player = player as u32;
        let in_sight = |x: i32, y: i32, kind: Kind| {
            let k = rules.kind(kind);
            let seen = |tx: i32, ty: i32| {
                (0..w).contains(&tx) && (0..h).contains(&ty) && sight.seen[(ty * w + tx) as usize] > 0
            };
            (y..y + k.height).any(|ty| (x..x + k.width).any(|tx| seen(tx, ty)))
        };
        let mut now: Vec<Ghost> = Vec::new();
        let mut kept = sight.ghosts.iter().peekable();
        for e in state.entities.iter().filter(|e| e.owner != player && rules.kind(e.kind).building) {
            // Records of buildings with lower ids that are gone: kept unless their ground is in sight.
            while let Some(g) = kept.next_if(|g| g.id < e.id) {
                if !in_sight(g.x, g.y, g.kind) {
                    now.push(g.clone());
                }
            }
            let t = e.tile();
            let old = kept.next_if(|g| g.id == e.id);
            if in_sight(t.x, t.y, e.kind) {
                now.push(Ghost { id: e.id, kind: e.kind, owner: e.owner, x: t.x, y: t.y, health: e.health });
            } else if let Some(g) = old {
                now.push(g.clone());
            }
        }
        now.extend(kept.filter(|g| !in_sight(g.x, g.y, g.kind)).cloned());
        sight.ghosts = now;
    }
}

/// Whether `player` can see entity `e` now: its own always; another's when any tile it stands on shows
/// (`Vision::shows`). Everything shows while the fog module is off.
pub fn visible(state: &GameState, rules: &Rules, player: u32, e: &Entity) -> bool {
    let Some(v) = &state.vision else { return true };
    if e.owner == player {
        return true;
    }
    let (k, t) = (rules.kind(e.kind), e.tile());
    (t.y..t.y + k.height).any(|y| (t.x..t.x + k.width).any(|x| v.shows(player, x, y)))
}

/// Whether `player` knows entity `e` is there: it can see it, or keeps a ghost of it.
pub fn known(state: &GameState, rules: &Rules, player: u32, e: &Entity) -> bool {
    visible(state, rules, player, e) || state.vision.as_ref().is_some_and(|v| v.ghost(player, e.id).is_some())
}

/// Fog as the state hash writes it, with kinds spelt as their generic ids.
pub(crate) struct VisionCanon<'a>(pub &'a Vision, pub &'a [String]);

struct GhostCanon<'a>(&'a Ghost, &'a [String]);

struct SightCanon<'a>(&'a Sight, &'a [String]);

impl Canon for GhostCanon<'_> {
    fn canon(&self, w: &mut CanonHasher) {
        let g = self.0;
        w.object()
            .field("health", &g.health)
            .field("id", &g.id)
            .field("owner", &g.owner)
            .field("type", self.1[g.kind.0 as usize].as_str())
            .field("x", &g.x)
            .field("y", &g.y)
            .end();
    }
}

impl Canon for SightCanon<'_> {
    fn canon(&self, w: &mut CanonHasher) {
        let ghosts: Vec<GhostCanon> = self.0.ghosts.iter().map(|g| GhostCanon(g, self.1)).collect();
        // Explored tiles as hex digits, four tiles to a digit, to keep the hash quick.
        let explored: String = self
            .0
            .explored
            .chunks(4)
            .map(|c| {
                let n = c.iter().enumerate().fold(0u32, |n, (i, &b)| n | (u32::from(b) << i));
                char::from_digit(n, 16).unwrap_or('0')
            })
            .collect();
        w.object()
            .field("explored", explored.as_str())
            .opt("ghosts", (!ghosts.is_empty()).then_some(&ghosts))
            .array("seen", &self.0.seen)
            .end();
    }
}

impl Canon for Reveal {
    fn canon(&self, w: &mut CanonHasher) {
        w.object()
            .field("player", &self.player)
            .field("until", &self.until)
            .field("x", &self.x)
            .field("y", &self.y)
            .end();
    }
}

impl Canon for VisionCanon<'_> {
    fn canon(&self, w: &mut CanonHasher) {
        let v = self.0;
        let players: Vec<SightCanon> = v.players.iter().map(|p| SightCanon(p, self.1)).collect();
        w.object()
            .field("hide", &v.hide)
            .field("players", &players)
            .opt("reveals", (!v.reveals.is_empty()).then_some(&v.reveals))
            .end();
    }
}

/// Every player's `seen` counted again from scratch: what the running counts must always equal. For tests.
pub fn recount(state: &GameState, rules: &Rules) -> Option<Vec<Vec<u16>>> {
    let v = state.vision.as_ref()?;
    let mut fresh = Vision { players: v.players.clone(), sources: Vec::new(), reveals: Vec::new(), ..v.clone() };
    for p in &mut fresh.players {
        p.seen.iter_mut().for_each(|s| *s = 0);
    }
    for s in state.entities.iter().filter_map(|e| source(rules, e)) {
        fresh.apply(&s, true);
    }
    for r in &v.reveals {
        fresh.disc(r.player, (r.x, r.y, r.x, r.y), REVEAL_RADIUS, true);
    }
    Some(fresh.players.into_iter().map(|p| p.seen).collect())
}
