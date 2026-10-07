//! The heads-up display over the world: the production rail on the left (a tab per kind of factory the player
//! owns, a grid of what it can build, and its queue), the economy readout at the top right, and the ghost of a
//! finished building being placed. Design: `plans/rts/ui.md`, sections 2 to 4.
//!
//! The HUD is client state only. It reads the game to draw and turns clicks into the same commands any player
//! sends (`Produce`, `Cancel`, `Place`); it never decides a rule itself, so whether a building fits is always the
//! simulation's `can_place`.

use std::collections::{BTreeMap, BTreeSet};

use classic_sim::production::has_ready;
use classic_sim::units::TICKS_PER_SECOND;
use classic_sim::{CommandOrder, EntryState, Game, Kind, ProduceError};

use crate::art::Art;
use crate::platform::{Font, Rect, SpriteBatch};
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
const READOUT_W: f32 = 240.0;
const READOUT_H: f32 = 58.0;
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
    names: BTreeMap<String, String>,
    unused: BTreeSet<String>,
}

impl Hud {
    /// A HUD for `player`, naming things as `pack` does and leaving out the entities it doesn't use.
    pub fn new(pack: &classic_data::Pack, game: &Game, player: u32) -> Hud {
        let ids = game.rules.kinds.iter().map(|k| k.id.clone());
        Hud {
            player,
            scale: 1.0,
            tab: None,
            placing: None,
            scroll: 0,
            names: ids.clone().map(|id| (id.clone(), pack.name(&id).to_string())).collect(),
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
        let rail = Rect::new(0.0, 0.0, RAIL_W * s, h);
        let readout = Rect::new(w - (READOUT_W + 6.0) * s, 6.0 * s, READOUT_W * s, READOUT_H * s);
        let factories = self.factories(game);
        let per_row = 4;
        let tabs: Vec<Tab> = factories
            .iter()
            .enumerate()
            .map(|(i, &factory)| {
                let (col, row) = ((i % per_row) as f32, (i / per_row) as f32);
                let rect =
                    Rect::new((5.0 + col * (TAB_W + 2.0)) * s, (6.0 + row * (TAB_H + 2.0)) * s, TAB_W * s, TAB_H * s);
                Tab { factory, rect }
            })
            .collect();
        let open = self.tab.filter(|t| factories.contains(t)).or(factories.first().copied());
        let grid_top = tabs.last().map_or(6.0 * s, |t| t.rect.y + t.rect.h + 8.0 * s);
        let queue_top = h - (QUEUE_H + 8.0) * s;
        let mut icons = Vec::new();
        let mut queue = Vec::new();
        if let Some(factory) = open {
            let rows_fit = (((queue_top - 6.0 * s - grid_top) / ((CELL_H + GAP) * s)).floor() as usize).max(1);
            let items = self.items(game, factory);
            let rows = items.len().div_ceil(2);
            let skip = self.scroll.min(rows.saturating_sub(rows_fit));
            for (i, &item) in items.iter().enumerate().skip(skip * 2).take(rows_fit * 2) {
                let (col, row) = ((i % 2) as f32, (i / 2 - skip) as f32);
                let rect = Rect::new(
                    (2.0 + col * (CELL_W + GAP)) * s,
                    grid_top + row * (CELL_H + GAP) * s,
                    CELL_W * s,
                    CELL_H * s,
                );
                icons.push(Icon { item, rect, status: self.status(game, item) });
            }
            // The queue of the primary factory, the one orders go to.
            if let Some(f) = game.state.entities.iter().find(|e| e.owner == self.player && e.kind == factory) {
                for (i, q) in f.queue.iter().enumerate() {
                    let rect = Rect::new((5.0 + i as f32 * (QUEUE_W + 2.0)) * s, queue_top, QUEUE_W * s, QUEUE_H * s);
                    queue.push((q.item, rect));
                }
            }
        }
        Layout { rail, tabs, open, icons, queue, readout }
    }

    /// Whether a screen point is on the HUD rather than the world.
    pub fn over(&self, game: &Game, screen: (f32, f32), x: f32, y: f32) -> bool {
        let l = self.layout(game, screen);
        l.rail.contains(x, y) || l.readout.contains(x, y)
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

    /// A mouse click at (x, y), with shift held or not. Returns whether the HUD took it; if not, it is the world's.
    ///
    /// On an icon a left click orders one (shift: up to five, as the queue has room), or picks up a ready building;
    /// a right click cancels the last one queued, with its refund. A click on a queue entry cancels that item.
    /// With a building on the cursor, a left click on the world places it if the simulation allows, and a right
    /// click puts it back.
    pub fn click(&mut self, game: &mut Game, view: &View, (x, y): (f32, f32), button: Button, shift: bool) -> bool {
        self.check_placing(game);
        let l = self.layout(game, view.screen);
        if l.rail.contains(x, y) || l.readout.contains(x, y) {
            if let Some(t) = l.tabs.iter().find(|t| t.rect.contains(x, y)) {
                self.tab = Some(t.factory);
                self.scroll = 0;
            } else if let Some(icon) = l.icons.iter().find(|i| i.rect.contains(x, y)) {
                self.click_icon(game, icon, button, shift);
            } else if let Some(&(item, _)) = l.queue.iter().find(|(_, r)| r.contains(x, y)) {
                game.order(self.player, &[], CommandOrder::Cancel { kind: item });
            }
            return true;
        }
        let Some(ghost) = self.ghost(game, view, x, y) else { return false };
        match button {
            Button::Left if ghost.ok => {
                game.order(self.player, &[], CommandOrder::Place { kind: ghost.kind, x: ghost.x, y: ghost.y });
                self.placing = None;
            }
            Button::Left => {}
            Button::Right => self.placing = None,
        }
        true
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

    /// The mouse wheel over the rail scrolls the grid by a row.
    pub fn wheel(&mut self, up: bool) {
        self.scroll = if up { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
    }

    /// Draw the HUD over the world, with the mouse at `mouse`.
    pub fn draw(
        &mut self,
        batch: &mut SpriteBatch,
        art: &Art,
        font: &Font,
        game: &Game,
        view: &View,
        mouse: (f32, f32),
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
        batch.fill(Rect::new(l.rail.x + l.rail.w - 1.0, 0.0, 1.0, l.rail.h), [70, 70, 80, 255]);
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

        self.draw_readout(batch, font, game, l.readout, text);
        if let Some(icon) = hovered {
            self.draw_tooltip(batch, font, game, icon, mouse, view.screen, text);
        }
    }

    /// An item's build icon fitted into `r`, or its sprite, or a plain box when the pack has neither.
    fn picture(&self, batch: &mut SpriteBatch, art: &Art, id: &str, r: Rect, tint: [u8; 4]) {
        let Some(strip) = art.icon(id, self.player).or_else(|| art.sprite(id, self.player)) else {
            batch.fill(Rect::new(r.x + r.w / 4.0, r.y + r.h / 4.0, r.w / 2.0, r.h / 2.0), [120, 120, 130, tint[3]]);
            return;
        };
        let fit = (r.w / strip.w).min(r.h / strip.h);
        let (w, h) = (strip.w * fit, strip.h * fit);
        batch.sprite(strip.tex, strip.frame(0), Rect::new(r.x + (r.w - w) / 2.0, r.y + (r.h - h) / 2.0, w, h), tint);
    }

    /// Credits, power in numbers and the game clock, and a power gauge: supply filled, demand marked.
    fn draw_readout(&self, batch: &mut SpriteBatch, font: &Font, game: &Game, r: Rect, text: f32) {
        let s = self.scale;
        batch.fill(r, PANEL);
        batch.outline(r, 1.0, [70, 70, 80, 255]);
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
        let secs = game.state.tick / TICKS_PER_SECOND;
        let clock = format!("{}:{:02}", secs / 60, secs % 60);
        font.draw(batch, &clock, r.x + r.w - 6.0 * s - Font::width(&clock, text), y, text, DIM);
        y += Font::height(text) + 4.0 * s;
        let bar = Rect::new(x, y, r.w - 12.0 * s, 8.0 * s);
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
        let x = (mouse.0 + 16.0 * s).min(screen.0 - w);
        let y = (mouse.1 + 8.0 * s).min(screen.1 - h);
        batch.fill(Rect::new(x, y, w, h), [10, 10, 12, 235]);
        batch.outline(Rect::new(x, y, w, h), 1.0, [90, 90, 100, 255]);
        for (i, (t, colour)) in lines.iter().enumerate() {
            font.draw(batch, t, x + 6.0 * s, y + 6.0 * s + i as f32 * line, text, *colour);
        }
    }
}
