//! The heads-up display over the world: the production rail on the right (the economy readout at its top, then a
//! tab per kind of factory the player owns, a grid of what it can build, the selection card, the open factory's
//! queue, and the minimap at the bottom), the ghost of a finished building being placed, and the message feed at the
//! world's top left. Design: `plans/rts/ui.md`, sections 2 to 4 and 10.
//!
//! The HUD is client state only. It reads the game to draw and turns clicks into the same commands any player
//! sends (`Produce`, `Cancel`, `Place`); it never decides a rule itself, so whether a building fits is always the
//! simulation's `can_place`.

use std::collections::{BTreeMap, BTreeSet};

use classic_sim::production::has_ready;
use classic_sim::units::TICKS_PER_SECOND;
use classic_sim::world::{Entity, Order, Task};
use classic_sim::{CommandOrder, EntryState, Game, Kind, ProduceError, Terrain};

use crate::art::Art;
use crate::feed::{Feed, Tone};
use crate::platform::{Files, Font, Rect, SpriteBatch};
use crate::scene::Camera;

/// The rail's width, at UI scale 1.
pub const RAIL_W: f32 = 200.0;
const TAB_W: f32 = 46.0;
const TAB_H: f32 = 36.0;
const CELL_W: f32 = 96.0;
const CELL_H: f32 = 72.0;
const GAP: f32 = 4.0;
const QUEUE_W: f32 = 36.0;
const QUEUE_H: f32 = 27.0;
const READOUT_H: f32 = 62.0;
/// The minimap's square, at UI scale 1; the map is fitted inside it.
const MINIMAP: f32 = 192.0;
/// The selection card's height, above the queue.
const CARD_H: f32 = 64.0;
/// Units shown one by one on the card for a group; the rest are counted.
const CHIPS: usize = 10;
/// The most a shift-click queues at once.
const SHIFT_COUNT: usize = 5;

const PANEL: [u8; 4] = [22, 22, 26, 240];
const CELL: [u8; 4] = [44, 44, 52, 255];
const TEXT: [u8; 4] = [235, 235, 225, 255];
const DIM: [u8; 4] = [150, 150, 150, 255];
const GOOD: [u8; 4] = [70, 200, 90, 255];
const WARN: [u8; 4] = [235, 175, 40, 255];
const BAD: [u8; 4] = [220, 60, 50, 255];

/// Where the world is on screen: the camera, the screen size in pixels and the art's pixels per tile.
#[derive(Clone, Copy, Debug)]
pub struct View {
    pub cam: Camera,
    pub screen: (f32, f32),
    pub tile: f32,
}

impl View {
    /// The tile under a screen point.
    pub fn tile_at(&self, sx: f32, sy: f32) -> (i32, i32) {
        let (wx, wy) = self.cam.to_world(sx, sy);
        ((wx / self.tile).floor() as i32, (wy / self.tile).floor() as i32)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
}

/// What a click did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Click {
    /// Nothing: the click is the world's.
    World,
    /// The HUD took it.
    Taken,
    /// A left click on the minimap: centre the view on this point, in tiles.
    Centre { x: f32, y: f32 },
    /// A right click on the minimap: give the selected units their default order at this tile.
    Order { x: i32, y: i32 },
}

impl Click {
    /// Whether the HUD took the click, so the world should ignore it.
    pub fn taken(self) -> bool {
        self != Click::World
    }
}

/// Where one item stands for the player, across every factory of the kind that makes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Status {
    /// A building the player still needs before this can be ordered.
    pub needs: Option<Kind>,
    /// Entries for it in the queues.
    pub queued: usize,
    /// The state of the first entry for it that heads a queue, and how far along it is (0 to 1).
    pub head: Option<(EntryState, f32)>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tab {
    /// The kind of factory the tab shows.
    pub factory: Kind,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Icon {
    pub item: Kind,
    pub rect: Rect,
    pub status: Status,
}

/// Every widget's place on screen this frame, worked out from the game and the screen size alone, so tests and
/// agents can find and click any of them.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub rail: Rect,
    pub tabs: Vec<Tab>,
    /// The factory kind whose items the grid shows.
    pub open: Option<Kind>,
    pub icons: Vec<Icon>,
    /// The open tab's primary factory queue, in order.
    pub queue: Vec<(Kind, Rect)>,
    pub readout: Rect,
    /// The selection card: what is selected, its health and what it is doing.
    pub card: Rect,
    /// Where the map is drawn on the minimap: one block per tile, fitted into the square at the rail's foot.
    pub minimap: Rect,
}

/// A building being placed: where its top-left tile would go, and whether the simulation would take it there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ghost {
    pub kind: Kind,
    pub x: i32,
    pub y: i32,
    pub ok: bool,
}

pub struct Hud {
    pub player: u32,
    /// UI scale: 1 at 100%.
    pub scale: f32,
    /// The tab the player picked; the first tab when it is gone.
    pub tab: Option<Kind>,
    /// A ready building on the cursor, waiting to be placed.
    pub placing: Option<Kind>,
    /// Grid rows scrolled past.
    pub scroll: usize,
    /// What the local player has been told lately.
    pub feed: Feed,
    names: BTreeMap<String, String>,
    unused: BTreeSet<String>,
}

impl Hud {
    /// A HUD for `player`, naming things as `pack` does and leaving out the entities it doesn't use. `pack_files` is
    /// where the pack's own files are, for its wording of the message feed.
    pub fn new(pack: &classic_data::Pack, pack_files: &Files, game: &Game, player: u32) -> Hud {
        let ids = game.rules.kinds.iter().map(|k| k.id.clone());
        let names: BTreeMap<String, String> = ids.clone().map(|id| (id.clone(), pack.name(&id).to_string())).collect();
        Hud {
            player,
            scale: 1.0,
            tab: None,
            placing: None,
            scroll: 0,
            feed: Feed::new(pack_files, names.clone(), player),
            names,
            unused: ids.filter(|id| !pack.uses(id)).collect(),
        }
    }

    fn name(&self, game: &Game, kind: Kind) -> String {
        let id = &game.rules.kind(kind).id;
        self.names.get(id).cloned().unwrap_or_else(|| id.clone())
    }

    /// Where `item` stands for this player.
    pub fn status(&self, game: &Game, item: Kind) -> Status {
        let needs = match game.can_build(self.player, item) {
            Err(ProduceError::Requires { kind }) => Some(kind),
            _ => None,
        };
        let mut queued = 0;
        let mut head = None;
        for e in game.state.entities.iter().filter(|e| e.owner == self.player) {
            queued += e.queue.iter().filter(|q| q.item == item).count();
            if let Some(q) = e.queue.first().filter(|q| q.item == item)
                && head.is_none()
            {
                let total = (game.rules.kind(item).build_ticks * 100).max(1);
                head = Some((q.state, (q.progress as f32 / total as f32).min(1.0)));
            }
        }
        Status { needs, queued, head }
    }

    /// The factory kinds this player owns that make something the pack uses, in kind order.
    fn factories(&self, game: &Game) -> Vec<Kind> {
        let makers: BTreeSet<u16> = game
            .rules
            .kinds
            .iter()
            .filter(|k| !self.unused.contains(&k.id))
            .filter_map(|k| k.built_at.map(|b| b.0))
            .collect();
        let owned: BTreeSet<u16> =
            game.state.entities.iter().filter(|e| e.owner == self.player).map(|e| e.kind.0).collect();
        makers.intersection(&owned).map(|&k| Kind(k)).collect()
    }

    /// What the grid offers for `factory`: whatever the player can order now, and the next step of the tech tree
    /// (items whose missing buildings can themselves be ordered now), shown locked. The rest stays hidden.
    fn items(&self, game: &Game, factory: Kind) -> Vec<Kind> {
        (0..game.rules.kinds.len() as u16)
            .map(Kind)
            .filter(|&k| {
                let r = game.rules.kind(k);
                r.built_at == Some(factory) && !self.unused.contains(&r.id)
            })
            .filter(|&k| match game.can_build(self.player, k) {
                Ok(()) => true,
                Err(ProduceError::Requires { .. }) => game.rules.kind(k).requires.iter().all(|&r| {
                    game.state.entities.iter().any(|e| e.owner == self.player && e.kind == r)
                        || game.can_build(self.player, r).is_ok()
                }),
                Err(_) => false,
            })
            .collect()
    }

    pub fn layout(&self, game: &Game, screen: (f32, f32)) -> Layout {
        let s = self.scale;
        let (w, h) = screen;
        // On the right, as most players expect (Ed, 2026-10-07; ui.md had picked the left).
        let rail = Rect::new(w - RAIL_W * s, 0.0, RAIL_W * s, h);
        let x0 = rail.x;
        let readout = Rect::new(x0, 0.0, RAIL_W * s, READOUT_H * s);
        let factories = self.factories(game);
        let per_row = 4;
        let tabs: Vec<Tab> = factories
            .iter()
            .enumerate()
            .map(|(i, &factory)| {
                let (col, row) = ((i % per_row) as f32, (i / per_row) as f32);
                let rect = Rect::new(
                    x0 + (5.0 + col * (TAB_W + 2.0)) * s,
                    readout.h + (6.0 + row * (TAB_H + 2.0)) * s,
                    TAB_W * s,
                    TAB_H * s,
                );
                Tab { factory, rect }
            })
            .collect();
        let open = self.tab.filter(|t| factories.contains(t)).or(factories.first().copied());
        let grid_top = tabs.last().map_or(readout.h + 6.0 * s, |t| t.rect.y + t.rect.h + 8.0 * s);
        // The minimap, the map's shape fitted and centred in a square at the bottom of the rail.
        // The minimap: the map's shape fitted into a square at the bottom of the rail, sitting on its foot.
        let side = MINIMAP * s;
        let block = side / game.map.width.max(game.map.height).max(1) as f32;
        let (mw, mh) = (game.map.width as f32 * block, game.map.height as f32 * block);
        let minimap = Rect::new(x0 + (rail.w - mw) / 2.0, h - 6.0 * s - mh, mw, mh);
        let queue_top = minimap.y - (QUEUE_H + 10.0) * s;
        // The card sits above the queue's label.
        let card = Rect::new(x0 + 4.0 * s, queue_top - (12.0 + CARD_H) * s, rail.w - 8.0 * s, CARD_H * s);
        let mut icons = Vec::new();
        let mut queue = Vec::new();
        if let Some(factory) = open {
            let rows_fit = (((card.y - 6.0 * s - grid_top) / ((CELL_H + GAP) * s)).floor() as usize).max(1);
            let items = self.items(game, factory);
            let rows = items.len().div_ceil(2);
            let skip = self.scroll.min(rows.saturating_sub(rows_fit));
            for (i, &item) in items.iter().enumerate().skip(skip * 2).take(rows_fit * 2) {
                let (col, row) = ((i % 2) as f32, (i / 2 - skip) as f32);
                let rect = Rect::new(
                    x0 + (2.0 + col * (CELL_W + GAP)) * s,
                    grid_top + row * (CELL_H + GAP) * s,
                    CELL_W * s,
                    CELL_H * s,
                );
                icons.push(Icon { item, rect, status: self.status(game, item) });
            }
            // The queue of the primary factory, the one orders go to.
            if let Some(f) = game.state.entities.iter().find(|e| e.owner == self.player && e.kind == factory) {
                for (i, q) in f.queue.iter().enumerate() {
                    let rect =
                        Rect::new(x0 + (5.0 + i as f32 * (QUEUE_W + 2.0)) * s, queue_top, QUEUE_W * s, QUEUE_H * s);
                    queue.push((q.item, rect));
                }
            }
        }
        Layout { rail, tabs, open, icons, queue, readout, card, minimap }
    }

    /// Whether a screen point is on the HUD rather than the world.
    pub fn over(&self, game: &Game, screen: (f32, f32), x: f32, y: f32) -> bool {
        let l = self.layout(game, screen);
        l.rail.contains(x, y) || l.readout.contains(x, y)
    }

    /// The map point under a screen point on the minimap, in tiles.
    pub fn minimap_point(&self, l: &Layout, game: &Game, x: f32, y: f32) -> Option<(f32, f32)> {
        let m = l.minimap;
        m.contains(x, y).then(|| ((x - m.x) / m.w * game.map.width as f32, (y - m.y) / m.h * game.map.height as f32))
    }

    /// Where the ready building on the cursor would go with the cursor at (x, y).
    pub fn ghost(&self, game: &Game, view: &View, x: f32, y: f32) -> Option<Ghost> {
        let kind = self.placing?;
        let k = game.rules.kind(kind);
        let (tx, ty) = view.tile_at(x, y);
        let (gx, gy) = (tx - (k.width - 1) / 2, ty - (k.height - 1) / 2);
        Some(Ghost { kind, x: gx, y: gy, ok: game.can_place(self.player, kind, gx, gy).is_ok() })
    }

    /// Drop the building on the cursor if it is no longer ready (cancelled, or placed another way).
    fn check_placing(&mut self, game: &Game) {
        if self.placing.is_some_and(|k| !has_ready(&game.state, self.player, k)) {
            self.placing = None;
        }
    }

    /// A mouse click at (x, y), with shift held or not.
    ///
    /// On the minimap a left click asks to centre the view there and a right click asks for an order there. On an icon a left click orders one (shift: up to five, as the queue has room), or picks up a ready building;
    /// a right click cancels the last one queued, with its refund. A click on a queue entry cancels that item.
    /// With a building on the cursor, a left click on the world places it if the simulation allows, and a right
    /// click puts it back.
    pub fn click(&mut self, game: &mut Game, view: &View, (x, y): (f32, f32), button: Button, shift: bool) -> Click {
        self.check_placing(game);
        let l = self.layout(game, view.screen);
        if l.rail.contains(x, y) || l.readout.contains(x, y) {
            if let Some((mx, my)) = self.minimap_point(&l, game, x, y) {
                return match button {
                    Button::Left => Click::Centre { x: mx, y: my },
                    Button::Right => Click::Order { x: mx.floor() as i32, y: my.floor() as i32 },
                };
            }
            if let Some(t) = l.tabs.iter().find(|t| t.rect.contains(x, y)) {
                self.tab = Some(t.factory);
                self.scroll = 0;
            } else if let Some(icon) = l.icons.iter().find(|i| i.rect.contains(x, y)) {
                self.click_icon(game, icon, button, shift);
            } else if let Some(&(item, _)) = l.queue.iter().find(|(_, r)| r.contains(x, y)) {
                game.order(self.player, &[], CommandOrder::Cancel { kind: item });
            }
            return Click::Taken;
        }
        let Some(ghost) = self.ghost(game, view, x, y) else { return Click::World };
        match button {
            Button::Left if ghost.ok => {
                game.order(self.player, &[], CommandOrder::Place { kind: ghost.kind, x: ghost.x, y: ghost.y });
                self.placing = None;
            }
            Button::Left => {}
            Button::Right => self.placing = None,
        }
        Click::Taken
    }

    fn click_icon(&mut self, game: &mut Game, icon: &Icon, button: Button, shift: bool) {
        let item = icon.item;
        match button {
            Button::Right if self.placing == Some(item) => self.placing = None,
            Button::Right => game.order(self.player, &[], CommandOrder::Cancel { kind: item }),
            Button::Left if icon.status.needs.is_some() => {}
            Button::Left
                if game.rules.kind(item).building && matches!(icon.status.head, Some((EntryState::Ready, _))) =>
            {
                self.placing = Some(item);
            }
            Button::Left => {
                let n = if shift {
                    let maker = game.rules.kind(item).built_at;
                    let used = game
                        .state
                        .entities
                        .iter()
                        .find(|e| e.owner == self.player && Some(e.kind) == maker)
                        .map_or(0, |f| f.queue.len());
                    SHIFT_COUNT.min(game.rules.production.queue_size.saturating_sub(used)).max(1)
                } else {
                    1
                };
                for _ in 0..n {
                    game.order(self.player, &[], CommandOrder::Produce { kind: item });
                }
            }
        }
    }

    /// Escape: put back the building on the cursor. Returns whether there was one.
    pub fn cancel(&mut self) -> bool {
        self.placing.take().is_some()
    }

    /// Tab (or shift-tab): open the next (or previous) factory's tab.
    pub fn next_tab(&mut self, game: &Game, back: bool) {
        let factories = self.factories(game);
        if factories.is_empty() {
            return;
        }
        let at = self.layout(game, (0.0, 0.0)).open.and_then(|o| factories.iter().position(|&f| f == o)).unwrap_or(0);
        let n = factories.len();
        self.tab = Some(factories[if back { (at + n - 1) % n } else { (at + 1) % n }]);
        self.scroll = 0;
    }

    /// Call after each tick: the feed reads what happened.
    pub fn after_step(&mut self, game: &Game) {
        self.feed.after_step(game);
    }

    /// The mouse wheel over the rail scrolls the grid by a row.
    pub fn wheel(&mut self, up: bool) {
        self.scroll = if up { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
    }

    /// Draw the HUD over the world, with the mouse at `mouse` and `selected` on the card.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &mut self,
        batch: &mut SpriteBatch,
        art: &Art,
        font: &Font,
        game: &Game,
        view: &View,
        mouse: (f32, f32),
        selected: &[u32],
    ) {
        self.check_placing(game);
        let s = self.scale;
        let text = (2.0 * s).round().max(1.0);
        let small = s.round().max(1.0);
        let tick = game.state.tick;
        let pulse = (tick / 4).is_multiple_of(2);
        let over = self.over(game, view.screen, mouse.0, mouse.1);

        // The ghost of a building being placed, under the HUD.
        if !over && let Some(g) = self.ghost(game, view, mouse.0, mouse.1) {
            let k = game.rules.kind(g.kind);
            let (sx, sy) = view.cam.to_screen(g.x as f32 * view.tile, g.y as f32 * view.tile);
            let dst = Rect::new(
                sx,
                sy,
                k.width as f32 * view.tile * view.cam.zoom,
                k.height as f32 * view.tile * view.cam.zoom,
            );
            if let Some(strip) = art.sprite(&k.id, self.player) {
                batch.sprite(strip.tex, strip.frame(0), dst, [255, 255, 255, 150]);
            }
            let tint = if g.ok { [GOOD[0], GOOD[1], GOOD[2], 90] } else { [BAD[0], BAD[1], BAD[2], 110] };
            batch.fill(dst, tint);
            batch.outline(dst, 1.0, if g.ok { GOOD } else { BAD });
        }

        let l = self.layout(game, view.screen);
        batch.fill(l.rail, PANEL);
        batch.fill(Rect::new(l.rail.x, 0.0, 1.0, l.rail.h), [70, 70, 80, 255]);
        for t in &l.tabs {
            let open = l.open == Some(t.factory);
            batch.fill(t.rect, if open { [70, 70, 84, 255] } else { CELL });
            let id = &game.rules.kind(t.factory).id;
            let inner = Rect::new(t.rect.x + 3.0 * s, t.rect.y + 2.0 * s, t.rect.w - 6.0 * s, t.rect.h - 8.0 * s);
            self.picture(batch, art, id, inner, [255; 4]);
            // Progress of whatever this kind of factory is making, and a pulse when something is ready.
            let mut best: Option<(EntryState, f32)> = None;
            for e in game.state.entities.iter().filter(|e| e.owner == self.player && e.kind == t.factory) {
                if let Some(q) = e.queue.first() {
                    let total = (game.rules.kind(q.item).build_ticks * 100).max(1);
                    let share = (q.progress as f32 / total as f32).min(1.0);
                    if best.is_none_or(|(st, _)| st != EntryState::Ready) {
                        best = Some((q.state, share));
                    }
                }
            }
            match best {
                Some((EntryState::Ready, _)) if pulse => batch.outline(t.rect, 2.0 * s, WARN),
                Some((_, share)) => {
                    let bar = Rect::new(t.rect.x + 3.0 * s, t.rect.y + t.rect.h - 5.0 * s, t.rect.w - 6.0 * s, 3.0 * s);
                    batch.fill(bar, [0, 0, 0, 255]);
                    batch.fill(Rect::new(bar.x, bar.y, bar.w * share, bar.h), GOOD);
                }
                None => {}
            }
            if open {
                batch.outline(t.rect, 1.0, TEXT);
            }
        }

        let mut hovered = None;
        for icon in &l.icons {
            let r = icon.rect;
            batch.fill(r, CELL);
            let st = icon.status;
            let id = &game.rules.kind(icon.item).id;
            let tint = if st.needs.is_some() { [90, 90, 90, 255] } else { [255; 4] };
            self.picture(batch, art, id, r, tint);
            match st.head {
                Some((state @ (EntryState::Building | EntryState::Paused), share)) => {
                    batch.fill(Rect::new(r.x, r.y, r.w, r.h * (1.0 - share)), [0, 0, 0, 140]);
                    let bar = Rect::new(r.x, r.y + r.h - 4.0 * s, r.w, 4.0 * s);
                    batch.fill(bar, [0, 0, 0, 255]);
                    let colour = if state == EntryState::Paused { WARN } else { GOOD };
                    batch.fill(Rect::new(bar.x, bar.y, bar.w * share, bar.h), colour);
                    if state == EntryState::Paused {
                        let (cx, cy) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
                        batch.fill(Rect::new(cx - 7.0 * s, cy - 9.0 * s, 5.0 * s, 18.0 * s), TEXT);
                        batch.fill(Rect::new(cx + 2.0 * s, cy - 9.0 * s, 5.0 * s, 18.0 * s), TEXT);
                    }
                }
                Some((EntryState::Ready, _)) => {
                    batch.outline(r, 2.0 * s, if pulse { [255, 230, 120, 255] } else { WARN });
                    let w = Font::width("READY", small);
                    font.draw(batch, "READY", r.x + (r.w - w) / 2.0, r.y + r.h - 12.0 * s, small, TEXT);
                }
                Some((EntryState::Blocked, _)) => batch.outline(r, 2.0 * s, WARN),
                _ => {}
            }
            if st.needs.is_some() {
                let w = Font::width("LOCKED", small);
                font.draw(batch, "LOCKED", r.x + (r.w - w) / 2.0, r.y + r.h / 2.0 - 3.0 * s, small, DIM);
            }
            if st.queued > 1 {
                let n = st.queued.to_string();
                let w = Font::width(&n, text);
                let badge =
                    Rect::new(r.x + r.w - w - 6.0 * s, r.y + 2.0 * s, w + 4.0 * s, Font::height(text) + 4.0 * s);
                batch.fill(badge, [0, 0, 0, 200]);
                font.draw(batch, &n, badge.x + 2.0 * s, badge.y + 2.0 * s, text, TEXT);
            }
            if r.contains(mouse.0, mouse.1) {
                batch.outline(r, 1.0, TEXT);
                hovered = Some(icon);
            }
        }

        // The open factory's queue along the bottom of the rail.
        if let Some(&(_, first)) = l.queue.first() {
            font.draw(batch, "QUEUE", first.x, first.y - 9.0 * s, small, DIM);
        }
        for (i, &(item, r)) in l.queue.iter().enumerate() {
            batch.fill(r, CELL);
            self.picture(
                batch,
                art,
                &game.rules.kind(item).id,
                r,
                if i == 0 { [255; 4] } else { [170, 170, 170, 255] },
            );
            if r.contains(mouse.0, mouse.1) {
                batch.outline(r, 1.0, BAD);
            }
        }

        self.draw_card(batch, art, font, game, selected, l.card);
        self.draw_minimap(batch, art, game, view, l.minimap);
        self.draw_readout(batch, font, game, l.readout, text);
        self.draw_feed(batch, font, game, text);
        if let Some(icon) = hovered {
            self.draw_tooltip(batch, font, game, icon, mouse, view.screen, text);
        }
    }

    /// An item's build icon fitted into `r`, or its sprite, or a plain box when the pack has neither.
    fn picture(&self, batch: &mut SpriteBatch, art: &Art, id: &str, r: Rect, tint: [u8; 4]) {
        picture(batch, art, id, self.player, r, tint);
    }

    /// The selection card. One thing selected: its picture in its owner's colours, its name, a health bar with the
    /// numbers, and what it is doing. A group: how many, and a chip with a health bar for each of the first few.
    #[allow(clippy::too_many_arguments)]
    fn draw_card(&self, batch: &mut SpriteBatch, art: &Art, font: &Font, game: &Game, selected: &[u32], r: Rect) {
        let s = self.scale;
        let small = s.round().max(1.0);
        let text = (2.0 * s).round().max(1.0);
        let chosen: Vec<&Entity> = selected.iter().filter_map(|&id| game.state.entity(id)).collect();
        let Some(&first) = chosen.first() else { return };
        batch.fill(r, CELL);
        batch.outline(r, 1.0, [70, 70, 80, 255]);
        let pad = 4.0 * s;
        if chosen.len() == 1 {
            let e = first;
            let k = game.rules.kind(e.kind);
            let pic = Rect::new(r.x + pad, r.y + pad, 56.0 * s, r.h - 2.0 * pad);
            batch.fill(pic, [0, 0, 0, 120]);
            picture(batch, art, &k.id, e.owner, pic, [255; 4]);
            let x = pic.x + pic.w + pad;
            let w = r.x + r.w - pad - x;
            let name = self.name(game, e.kind);
            // The name in large letters when it fits, else small.
            let size = if Font::width(&name, text) <= w { text } else { small };
            let mut y = r.y + pad;
            font.draw(batch, &name, x, y, size, if e.owner == self.player { TEXT } else { BAD });
            y += Font::height(text) + 3.0 * s;
            let share = (e.health.max(0) as f32 / k.max_health.max(1) as f32).min(1.0);
            let bar = Rect::new(x, y, w, 5.0 * s);
            batch.fill(bar, [0, 0, 0, 255]);
            batch.fill(Rect::new(bar.x, bar.y, bar.w * share, bar.h), health_colour(share));
            y += bar.h + 3.0 * s;
            font.draw(batch, &format!("{}/{}", e.health.max(0), k.max_health), x, y, small, DIM);
            y += Font::height(small) + 4.0 * s;
            let (doing, colour) = self.doing(game, e);
            font.draw(batch, &doing, x, y, small, colour);
            return;
        }
        let head = format!("{} SELECTED", chosen.len());
        font.draw(batch, &head, r.x + pad, r.y + pad, small, TEXT);
        let (cols, gap) = (5, 2.0 * s);
        let top = r.y + pad + Font::height(small) + 3.0 * s;
        let cw = (r.w - 2.0 * pad - gap * (cols - 1) as f32) / cols as f32;
        let ch = (r.y + r.h - pad - top - gap) / 2.0;
        for (i, e) in chosen.iter().take(CHIPS).enumerate() {
            let k = game.rules.kind(e.kind);
            let (col, row) = ((i % cols) as f32, (i / cols) as f32);
            let chip = Rect::new(r.x + pad + col * (cw + gap), top + row * (ch + gap), cw, ch);
            batch.fill(chip, [0, 0, 0, 120]);
            picture(batch, art, &k.id, e.owner, Rect::new(chip.x, chip.y, chip.w, chip.h - 3.0 * s), [255; 4]);
            let share = (e.health.max(0) as f32 / k.max_health.max(1) as f32).min(1.0);
            let bar = Rect::new(chip.x, chip.y + chip.h - 2.0 * s, chip.w, 2.0 * s);
            batch.fill(bar, [0, 0, 0, 255]);
            batch.fill(Rect::new(bar.x, bar.y, bar.w * share, bar.h), health_colour(share));
        }
        if chosen.len() > CHIPS {
            let more = format!("+{}", chosen.len() - CHIPS);
            let w = Font::width(&more, small);
            font.draw(batch, &more, r.x + r.w - pad - w, r.y + pad, small, DIM);
        }
    }

    /// What an entity is doing, in a few words, for the card.
    fn doing(&self, game: &Game, e: &Entity) -> (String, [u8; 4]) {
        let k = game.rules.kind(e.kind);
        if e.owner != self.player {
            return ("ENEMY".to_string(), BAD);
        }
        if k.building {
            if let Some(q) = e.queue.first() {
                let total = (game.rules.kind(q.item).build_ticks * 100).max(1);
                let share = (q.progress * 100 / total).min(100);
                let name = self.name(game, q.item);
                return match q.state {
                    EntryState::Ready => (format!("{name} READY"), GOOD),
                    EntryState::Paused => (format!("{name} ON HOLD"), WARN),
                    EntryState::Blocked => (format!("{name}: EXIT BLOCKED"), WARN),
                    EntryState::Building | EntryState::Waiting => (format!("{name} {share}%"), GOOD),
                };
            }
            return match k.power {
                p if p > 0 => (format!("POWER +{p}"), GOOD),
                p if p < 0 => (format!("POWER {p}"), DIM),
                _ => (String::new(), DIM),
            };
        }
        let cargo = |c: i64| {
            let cap = k.harvester.as_ref().map_or(1, |h| h.capacity).max(1);
            format!("{}%", (c * 100 / cap).min(100))
        };
        match (e.order, e.task) {
            (Order::Harvest, Some(Task::Stuck)) => ("HARVESTER STUCK".to_string(), WARN),
            (Order::Harvest, Some(Task::Mining)) => (format!("MINING {}", cargo(e.cargo.unwrap_or(0))), GOOD),
            (Order::Harvest, Some(Task::ToRefinery | Task::Unloading)) => {
                (format!("RETURNING {}", cargo(e.cargo.unwrap_or(0))), GOOD)
            }
            (Order::Harvest, _) => ("HARVESTING".to_string(), GOOD),
            (Order::Move, _) => ("MOVING".to_string(), TEXT),
            (Order::Attack, _) => ("ATTACKING".to_string(), WARN),
            (Order::Idle, _) if e.target.is_some() => ("FIRING".to_string(), WARN),
            (Order::Idle, _) => ("GUARDING".to_string(), DIM),
        }
    }

    /// The feed's lines at the top left of the world, newest at the bottom, fading in their last second.
    fn draw_feed(&self, batch: &mut SpriteBatch, font: &Font, game: &Game, text: f32) {
        let s = self.scale;
        let line = Font::height(text) + 6.0 * s;
        let now = game.state.tick;
        for (i, l) in self.feed.lines.iter().enumerate() {
            let left = crate::feed::LIFE.saturating_sub(now.saturating_sub(l.tick));
            let fade = (left as f32 / TICKS_PER_SECOND as f32).min(1.0);
            let mut colour = match l.tone {
                Tone::Info => TEXT,
                Tone::Good => GOOD,
                Tone::Warn => WARN,
                Tone::Bad => BAD,
            };
            colour[3] = (255.0 * fade) as u8;
            let (x, y) = (8.0 * s, 8.0 * s + i as f32 * line);
            let w = Font::width(&l.text, text);
            batch.fill(
                Rect::new(x - 3.0 * s, y - 3.0 * s, w + 6.0 * s, line - 2.0 * s),
                [0, 0, 0, (150.0 * fade) as u8],
            );
            font.draw(batch, &l.text, x, y, text, colour);
        }
    }

    /// The whole map, one block per tile: terrain and resource in the art's own average colours, buildings and
    /// units in their owners' colours, and the part of the map the view shows as a white box.
    fn draw_minimap(&self, batch: &mut SpriteBatch, art: &Art, game: &Game, view: &View, m: Rect) {
        let s = self.scale;
        batch.fill(Rect::new(m.x - 2.0 * s, m.y - 2.0 * s, m.w + 4.0 * s, m.h + 4.0 * s), [0, 0, 0, 255]);
        let (bw, bh) = (m.w / game.map.width as f32, m.h / game.map.height as f32);
        let ground = |id: &str, fallback: [u8; 3]| {
            let [r, g, b] = art.terrain(id).map_or(fallback, |t| t.colour);
            [r, g, b, 255]
        };
        let open = ground("open", [180, 150, 100]);
        let rock = ground("rock", [120, 110, 100]);
        // A field's tile is mostly the ground it lies on, so take what stands out in it and push it further from
        // the open ground, so a field still reads at a few pixels a tile.
        let accent = art.terrain("resource").map_or([220, 150, 40], |t| t.accent);
        let [r, g, b] = [0, 1, 2].map(|c| (3 * i32::from(accent[c]) - 2 * i32::from(open[c])).clamp(0, 255) as u8);
        let resource = [r, g, b, 255];
        for ty in 0..game.map.height {
            for tx in 0..game.map.width {
                let i = (ty * game.map.width + tx) as usize;
                let colour = if game.state.resource[i] > 0 {
                    resource
                } else {
                    match game.map.terrain[i] {
                        Terrain::Open => open,
                        Terrain::Rock => rock,
                        Terrain::Cliff => [46, 40, 36, 255],
                    }
                };
                batch.fill(Rect::new(m.x + tx as f32 * bw, m.y + ty as f32 * bh, bw, bh), colour);
            }
        }
        for e in &game.state.entities {
            let k = game.rules.kind(e.kind);
            let t = e.tile();
            let [r, g, b] = art.owner_colour(e.owner);
            let (w, h) = if k.building { (k.width as f32, k.height as f32) } else { (1.0, 1.0) };
            // Units a little larger than a tile, so they stay visible.
            let grow = if k.building { 0.0 } else { 0.5 };
            let rect = Rect::new(
                m.x + (t.x as f32 - grow / 2.0) * bw,
                m.y + (t.y as f32 - grow / 2.0) * bh,
                (w + grow) * bw,
                (h + grow) * bh,
            );
            batch.fill(rect, [r, g, b, 255]);
        }
        // The view, clipped to the map.
        let (wx0, wy0) = view.cam.to_world(0.0, 0.0);
        let (wx1, wy1) = view.cam.to_world(view.screen.0 - RAIL_W * s, view.screen.1);
        let to_mini = |wx: f32, wy: f32| {
            let fx = (wx / view.tile).clamp(0.0, game.map.width as f32);
            let fy = (wy / view.tile).clamp(0.0, game.map.height as f32);
            (m.x + fx * bw, m.y + fy * bh)
        };
        let (ax, ay) = to_mini(wx0, wy0);
        let (bx, by) = to_mini(wx1, wy1);
        batch.outline(Rect::new(ax, ay, (bx - ax).max(2.0), (by - ay).max(2.0)), s.max(1.0), TEXT);
    }

    /// Credits, power in numbers and the game clock, and a power gauge: supply filled, demand marked.
    fn draw_readout(&self, batch: &mut SpriteBatch, font: &Font, game: &Game, r: Rect, text: f32) {
        let s = self.scale;
        batch.fill(Rect::new(r.x, r.y + r.h - 1.0, r.w, 1.0), [70, 70, 80, 255]);
        let credits = game.state.players.iter().find(|p| p.id == self.player).map_or(0, |p| p.credits);
        let (x, mut y) = (r.x + 6.0 * s, r.y + 6.0 * s);
        font.draw(batch, &format!("CREDITS {credits}"), x, y, text, TEXT);
        y += Font::height(text) + 4.0 * s;
        let power = game.power(self.player);
        let short = power.is_short();
        font.draw(
            batch,
            &format!("POWER {}/{}", power.supply, power.demand),
            x,
            y,
            text,
            if short { BAD } else { TEXT },
        );
        y += Font::height(text) + 4.0 * s;
        let secs = game.state.tick / TICKS_PER_SECOND;
        let clock = format!("{}:{:02}", secs / 60, secs % 60);
        let clock_w = Font::width(&clock, text);
        font.draw(batch, &clock, r.x + r.w - 6.0 * s - clock_w, y, text, DIM);
        let bar = Rect::new(x, y + (Font::height(text) - 8.0 * s) / 2.0, r.w - 18.0 * s - clock_w, 8.0 * s);
        batch.fill(bar, [0, 0, 0, 255]);
        let top = power.supply.max(power.demand).max(1) as f32;
        let fill = Rect::new(bar.x, bar.y, bar.w * power.supply as f32 / top, bar.h);
        batch.fill(fill, if short { BAD } else { GOOD });
        if short {
            // Stripes as well as colour, so a shortfall shows without telling red from green.
            let mut sx = bar.x;
            while sx < bar.x + bar.w {
                batch.fill(Rect::new(sx, bar.y, 3.0 * s, bar.h), [0, 0, 0, 140]);
                sx += 8.0 * s;
            }
        }
        let mx = bar.x + bar.w * power.demand as f32 / top;
        batch.fill(Rect::new((mx - s).min(bar.x + bar.w - 2.0 * s), bar.y - 2.0 * s, 2.0 * s, bar.h + 4.0 * s), TEXT);
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_tooltip(
        &self,
        batch: &mut SpriteBatch,
        font: &Font,
        game: &Game,
        icon: &Icon,
        mouse: (f32, f32),
        screen: (f32, f32),
        text: f32,
    ) {
        let s = self.scale;
        let k = game.rules.kind(icon.item);
        let st = icon.status;
        let mut lines = vec![(self.name(game, icon.item), TEXT), (format!("COST {}", k.cost), DIM)];
        let state = match (st.needs, st.head) {
            (Some(need), _) => Some((format!("NEEDS {}", self.name(game, need)), WARN)),
            (_, Some((EntryState::Building, share))) => Some((format!("BUILDING {}%", (share * 100.0) as i32), GOOD)),
            (_, Some((EntryState::Paused, _))) => Some(("PAUSED: NOT ENOUGH CREDITS".to_string(), WARN)),
            (_, Some((EntryState::Ready, _))) => Some(("READY: CLICK TO PLACE".to_string(), GOOD)),
            (_, Some((EntryState::Blocked, _))) => Some(("EXIT BLOCKED".to_string(), WARN)),
            _ => None,
        };
        lines.extend(state);
        let line = Font::height(text) + 4.0 * s;
        let w = lines.iter().map(|(t, _)| Font::width(t, text)).fold(0.0, f32::max) + 12.0 * s;
        let h = lines.len() as f32 * line + 8.0 * s;
        // Beside the cursor, on the world's side of the rail.
        let x = (mouse.0 - 16.0 * s - w).max(0.0);
        let y = (mouse.1 + 8.0 * s).min(screen.1 - h);
        batch.fill(Rect::new(x, y, w, h), [10, 10, 12, 235]);
        batch.outline(Rect::new(x, y, w, h), 1.0, [90, 90, 100, 255]);
        for (i, (t, colour)) in lines.iter().enumerate() {
            font.draw(batch, t, x + 6.0 * s, y + 6.0 * s + i as f32 * line, text, *colour);
        }
    }
}

fn health_colour(share: f32) -> [u8; 4] {
    if share > 0.5 {
        [60, 200, 70, 255]
    } else if share > 0.25 {
        [230, 200, 40, 255]
    } else {
        [220, 50, 40, 255]
    }
}

/// An entity's build icon in `owner`'s colours fitted into `r`, or its sprite, or a plain box when the pack has
/// neither.
fn picture(batch: &mut SpriteBatch, art: &Art, id: &str, owner: u32, r: Rect, tint: [u8; 4]) {
    let Some(strip) = art.icon(id, owner).or_else(|| art.sprite(id, owner)) else {
        batch.fill(Rect::new(r.x + r.w / 4.0, r.y + r.h / 4.0, r.w / 2.0, r.h / 2.0), [120, 120, 130, tint[3]]);
        return;
    };
    let fit = (r.w / strip.w).min(r.h / strip.h);
    let (w, h) = (strip.w * fit, strip.h * fit);
    batch.sprite(strip.tex, strip.frame(0), Rect::new(r.x + (r.w - w) / 2.0, r.y + (r.h - h) / 2.0, w, h), tint);
}
