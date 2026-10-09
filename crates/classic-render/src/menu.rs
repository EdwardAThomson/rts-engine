//! The screens around a game: the title screen before it, the pause menu during it (Escape), the settings and keys
//! screens from either, and the end screen with the score after it. Design: `plans/rts/ui.md`, section 9.
//!
//! Like the HUD, menus are client state only. They decide which screen shows, change the player's settings
//! (`Prefs`) and turn clicks into actions for the player program (start, save, load, resume, restart, back to the
//! title, quit); they never touch the game. Their places come from `layout`, from the screen size alone, so tests
//! can find and click every button.

use classic_ai::{Difficulty, defeated, winner};
use classic_sim::Game;
use classic_sim::units::TICKS_PER_SECOND;

use crate::platform::{Rect, SpriteBatch};
use crate::prefs::{BUSES, Bind, Prefs, key_label};
use crate::score::Score;
use crate::skin::{ButtonState, Skin, Style};
use crate::theme::Theme;

const BUTTON_W: f32 = 280.0;
const BUTTON_H: f32 = 36.0;
const GAP: f32 = 10.0;

const SHADE: [u8; 4] = [0, 0, 0, 170];
/// The score table's width and row height, at UI scale 1.
const TABLE_W: f32 = 540.0;
const TABLE_ROW: f32 = 20.0;

/// Which screen shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// Before a game: the pack's title, start and options.
    Title,
    /// The game runs and takes the input.
    Playing,
    /// Escape during a game: the game stops until the player goes back to it.
    Paused,
    /// Someone won: `won` is whether it was the local player.
    Over { won: bool },
    /// The volumes and scroll speed, from the title or the pause menu.
    Settings,
    /// The keys, from the settings screen.
    Keys,
}

/// What a button asks the player program to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Start a new game with the options shown.
    Start,
    /// Flip the computer opponents on or off for the next game.
    Opponents,
    /// The next map for the next game.
    Map,
    /// The next faction for the local player in the next game.
    Faction,
    /// The next difficulty for the computer opponents in the next game.
    Difficulty,
    /// Save the game being played.
    Save,
    /// Load the saved game.
    Load,
    /// The settings screen.
    Settings,
    /// The keys screen.
    Keys,
    /// A bus's volume changed (by index into `prefs::BUSES`).
    Volume(usize),
    /// The scroll speed changed.
    Scroll,
    /// Waiting for a key for this bind.
    Bind(Bind),
    /// Every key back to its default.
    ResetKeys,
    /// Back from the settings or keys screen.
    Back,
    Resume,
    /// The same map and options again, from the start.
    Restart,
    ToTitle,
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub action: Action,
    pub label: String,
    pub rect: Rect,
}

pub struct Menu {
    pub screen: Screen,
    /// Whether the next game has computer opponents for every other player.
    pub opponents: bool,
    /// The names of the maps on offer, and the one picked for the next game. With one map there is no choice.
    pub maps: Vec<String>,
    pub map: usize,
    /// The names of the pack's factions, and the local player's for the next game.
    pub factions: Vec<String>,
    pub faction: usize,
    /// UI scale: 1 at 100%.
    pub scale: f32,
    /// Whether a Quit button makes sense (not in the browser, where the page stays).
    pub can_quit: bool,
    /// The pack's colours.
    pub theme: Theme,
    /// The player's settings, which the settings and keys screens change; the player program keeps them.
    pub prefs: Prefs,
    /// Whether there is a saved game to load.
    pub has_save: bool,
    /// A line under the heading, such as whether a save worked; cleared when the screen changes.
    pub notice: Option<String>,
    /// The bind waiting for its new key on the keys screen.
    pub waiting: Option<Bind>,
    /// The game's score so far, counted after every tick and shown on the end screen.
    pub score: Score,
    /// The local player, for the score's "you".
    pub local: u32,
    /// Where the settings screen goes back to: the title or the pause menu.
    back: Screen,
    /// The setting pack's title, shown on the title screen.
    title: String,
}

impl Menu {
    pub fn new(title: &str) -> Menu {
        Menu {
            screen: Screen::Title,
            opponents: true,
            maps: Vec::new(),
            map: 0,
            factions: Vec::new(),
            faction: 0,
            scale: 1.0,
            can_quit: true,
            theme: Theme::default(),
            prefs: Prefs::default(),
            has_save: false,
            notice: None,
            waiting: None,
            score: Score::default(),
            local: 0,
            back: Screen::Title,
            title: title.to_string(),
        }
    }

    /// Whether the game should run and take the input.
    pub fn playing(&self) -> bool {
        self.screen == Screen::Playing
    }

    /// The buttons of the screen showing, centred on a screen of this size; none while playing.
    pub fn layout(&self, screen: (f32, f32)) -> Vec<Item> {
        let mut quit = self.can_quit;
        let mut items: Vec<(Action, String)> = match self.screen {
            Screen::Playing => return Vec::new(),
            Screen::Title => {
                let mut items = vec![(Action::Start, "START".to_string())];
                if self.has_save {
                    items.push((Action::Load, "LOAD GAME".to_string()));
                }
                if self.maps.len() > 1 {
                    items.push((Action::Map, format!("MAP: {}", self.maps[self.map].to_uppercase())));
                }
                if self.factions.len() > 1 {
                    items.push((Action::Faction, format!("FACTION: {}", self.factions[self.faction].to_uppercase())));
                }
                let opponents = if self.opponents { "COMPUTER" } else { "NONE" };
                items.push((Action::Opponents, format!("OPPONENTS: {opponents}")));
                if self.opponents {
                    let d = self.prefs.difficulty.id().to_uppercase();
                    items.push((Action::Difficulty, format!("DIFFICULTY: {d}")));
                }
                items.push((Action::Settings, "SETTINGS".to_string()));
                items
            }
            Screen::Paused => {
                let mut items = vec![(Action::Resume, "RESUME".to_string()), (Action::Save, "SAVE GAME".to_string())];
                if self.has_save {
                    items.push((Action::Load, "LOAD GAME".to_string()));
                }
                items.extend([
                    (Action::Settings, "SETTINGS".to_string()),
                    (Action::Restart, "RESTART".to_string()),
                    (Action::ToTitle, "QUIT TO TITLE".to_string()),
                ]);
                items
            }
            Screen::Over { .. } => {
                vec![(Action::Restart, "PLAY AGAIN".to_string()), (Action::ToTitle, "TITLE SCREEN".to_string())]
            }
            Screen::Settings => {
                quit = false;
                let mut items: Vec<(Action, String)> = BUSES
                    .iter()
                    .enumerate()
                    .map(|(i, (_, name))| (Action::Volume(i), format!("{name}: {}%", self.prefs.volume[i])))
                    .collect();
                items.push((Action::Scroll, format!("SCROLL SPEED: {}%", self.prefs.scroll)));
                items.push((Action::Keys, "KEYS".to_string()));
                items.push((Action::Back, "BACK".to_string()));
                items
            }
            Screen::Keys => {
                quit = false;
                let mut items: Vec<(Action, String)> = Bind::ALL
                    .iter()
                    .map(|&b| {
                        let key = if self.waiting == Some(b) {
                            "PRESS A KEY".to_string()
                        } else {
                            key_label(self.prefs.key(b))
                        };
                        (Action::Bind(b), format!("{}: {key}", b.label()))
                    })
                    .collect();
                items.push((Action::ResetKeys, "RESET KEYS".to_string()));
                items.push((Action::Back, "BACK".to_string()));
                items
            }
        };
        if quit {
            items.push((Action::Quit, "QUIT".to_string()));
        }
        let s = self.scale;
        let n = items.len() as f32;
        // Long screens (the keys) close up to fit a short window.
        let room = screen.1 - 160.0 * s - self.table_height();
        let gap = if n * (BUTTON_H + GAP) * s - GAP * s > room { 4.0 * s } else { GAP * s };
        let h = (BUTTON_H * s).min(((room + gap) / n - gap).max(18.0 * s));
        let w = BUTTON_W * s;
        let total = n * (h + gap) - gap;
        let top = screen.1 / 2.0 - total / 2.0 + 30.0 * s + self.table_height() / 2.0;
        items
            .into_iter()
            .enumerate()
            .map(|(i, (action, label))| Item {
                action,
                label,
                rect: Rect::new((screen.0 - w) / 2.0, top + i as f32 * (h + gap), w, h),
            })
            .collect()
    }

    /// The height of the score table on the end screen, and nothing on the others.
    fn table_height(&self) -> f32 {
        match self.screen {
            Screen::Over { .. } if !self.score.lines().is_empty() => {
                (self.score.lines().len() as f32 + 1.0) * TABLE_ROW * self.scale + 16.0 * self.scale
            }
            _ => 0.0,
        }
    }

    /// A left click at (x, y): the action of the button under it, if any. The switches (map, faction, opponents,
    /// difficulty, volumes, scroll speed) change here, and the settings and keys screens open and close here.
    pub fn click(&mut self, screen: (f32, f32), at: (f32, f32)) -> Option<Action> {
        self.click_with(screen, at, false)
    }

    /// A click, with `down` for the right button: on the volumes and the scroll speed it steps down instead of up.
    pub fn click_with(&mut self, screen: (f32, f32), (x, y): (f32, f32), down: bool) -> Option<Action> {
        let action = self.layout(screen).into_iter().find(|i| i.rect.contains(x, y))?.action;
        if self.waiting.take().is_some() && action != Action::Back {
            // A click while waiting for a key gives up on it; clicking another bind waits for that one instead.
            if !matches!(action, Action::Bind(_)) {
                return Some(action);
            }
        }
        match action {
            Action::Opponents => self.opponents = !self.opponents,
            Action::Map => self.map = (self.map + 1) % self.maps.len().max(1),
            Action::Faction => self.faction = (self.faction + 1) % self.factions.len().max(1),
            Action::Difficulty => self.prefs.difficulty = self.prefs.difficulty.next(),
            Action::Volume(bus) => self.prefs.step_volume(bus, down),
            Action::Scroll => self.prefs.step_scroll(down),
            Action::Settings => {
                self.back = self.screen;
                self.show(Screen::Settings);
            }
            Action::Keys => self.show(Screen::Keys),
            Action::Bind(b) => self.waiting = Some(b),
            Action::ResetKeys => self.prefs.keys = Prefs::default().keys,
            Action::Back => self.go_back(),
            _ => {}
        }
        Some(action)
    }

    /// A key press while a bind waits for one, by the window system's name for the key: puts the bind on it and
    /// returns true. Escape gives up instead.
    pub fn key(&mut self, name: &str) -> bool {
        let Some(bind) = self.waiting.take() else { return false };
        if name != "Escape" {
            self.prefs.set_key(bind, name);
        }
        true
    }

    /// Show a screen, clearing the notice.
    pub fn show(&mut self, screen: Screen) {
        self.screen = screen;
        self.notice = None;
        self.waiting = None;
    }

    fn go_back(&mut self) {
        match self.screen {
            Screen::Keys => self.show(Screen::Settings),
            Screen::Settings => self.show(self.back),
            _ => {}
        }
    }

    /// The difficulty of the computer opponents in the next game.
    pub fn difficulty(&self) -> Difficulty {
        self.prefs.difficulty
    }

    /// Escape: from the game to the pause menu, and from the pause menu back to the game. Returns whether the menu
    /// took the key; on the title and end screens it doesn't, so the program may quit.
    pub fn escape(&mut self) -> bool {
        match self.screen {
            Screen::Playing => self.show(Screen::Paused),
            Screen::Paused => self.show(Screen::Playing),
            Screen::Keys if self.waiting.is_some() => self.waiting = None,
            Screen::Settings | Screen::Keys => self.go_back(),
            Screen::Title | Screen::Over { .. } => return false,
        }
        true
    }

    /// Call after each tick: it counts the score, and when the game has a winner, or the local player has lost,
    /// shows the end screen.
    pub fn after_step(&mut self, game: &Game, local: u32) {
        self.score.after_step(game);
        self.local = local;
        if self.screen != Screen::Playing {
            return;
        }
        let won = match winner(game) {
            Some(w) => w == local,
            None if defeated(game, local) => false,
            None => return,
        };
        self.show(Screen::Over { won });
    }

    /// Draw the screen showing over whatever is behind it (the map, dimmed), with the mouse at `mouse`.
    pub fn draw(&self, batch: &mut SpriteBatch, skin: &Skin, game: &Game, screen: (f32, f32), mouse: (f32, f32)) {
        if self.screen == Screen::Playing {
            return;
        }
        let s = self.scale;
        batch.fill(Rect::new(0.0, 0.0, screen.0, screen.1), SHADE);
        let items = self.layout(screen);
        let first = items.first().map_or(screen.1 / 2.0, |i| i.rect.y);
        let last = items.last().map_or(screen.1 / 2.0, |i| i.rect.y + i.rect.h);
        let (heading, colour, sub) = match self.screen {
            Screen::Title => (self.title.to_uppercase(), self.theme.accent, "SKIRMISH".to_string()),
            Screen::Paused => ("PAUSED".to_string(), self.theme.text, clock(game)),
            Screen::Over { won: true } => ("VICTORY".to_string(), self.theme.good, format!("WON IN {}", clock(game))),
            Screen::Over { won: false } => {
                ("DEFEAT".to_string(), self.theme.bad, format!("LOST AFTER {}", clock(game)))
            }
            Screen::Settings => ("SETTINGS".to_string(), self.theme.text, "RIGHT CLICK STEPS DOWN".to_string()),
            Screen::Keys => ("KEYS".to_string(), self.theme.text, "CLICK ONE, THEN PRESS ITS NEW KEY".to_string()),
            Screen::Playing => return,
        };
        let sub = self.notice.clone().unwrap_or(sub);
        // The heading shrinks to fit a narrow window.
        let size =
            if skin.width(Style::Title, &heading, s) + 40.0 * s <= screen.0 { Style::Title } else { Style::Heading };
        let table = self.table_height();
        let panel_w = (BUTTON_W * s + 60.0 * s)
            .max(skin.width(size, &heading, s) + 40.0 * s)
            .max(if table > 0.0 { TABLE_W * s + 40.0 * s } else { 0.0 })
            .min(screen.0);
        let head_h = skin.line(size, s) + skin.line(Style::Body, s) + 34.0 * s + table;
        let panel =
            Rect::new((screen.0 - panel_w) / 2.0, first - head_h - 20.0 * s, panel_w, last - first + head_h + 40.0 * s);
        skin.frame(batch, "panel", 0, panel, s, self.theme.panel);
        let hx = (screen.0 - skin.width(size, &heading, s)) / 2.0;
        skin.text(batch, size, &heading, hx, panel.y + 16.0 * s, s, colour);
        let sx = (screen.0 - skin.width(Style::Body, &sub, s)) / 2.0;
        let sub_y = panel.y + 24.0 * s + skin.line(size, s);
        skin.text(batch, Style::Body, &sub, sx, sub_y, s, self.theme.dim);
        if table > 0.0 {
            self.draw_score(batch, skin, screen, sub_y + skin.line(Style::Body, s) + 12.0 * s);
        }
        for i in &items {
            let over = i.rect.contains(mouse.0, mouse.1);
            let state = if over { ButtonState::Hover } else { ButtonState::Normal };
            skin.button(batch, i.rect, state, s);
            let lx = i.rect.x + (i.rect.w - skin.width(Style::Body, &i.label, s)) / 2.0;
            let ly = i.rect.y + (i.rect.h - skin.line(Style::Body, s)) / 2.0;
            skin.text(batch, Style::Body, &i.label, lx, ly, s, self.theme.text);
        }
    }
}

impl Menu {
    /// The score table: a row for each player under a row of column names, centred, its top at `y`.
    fn draw_score(&self, batch: &mut SpriteBatch, skin: &Skin, screen: (f32, f32), y: f32) {
        let s = self.scale;
        let left = (screen.0 - TABLE_W * s) / 2.0;
        let cols = ["UNITS", "BUILDINGS", "KILLS", "LOSSES", "HARVESTED"];
        // Numbers are right-aligned in their columns, after the players' names.
        let name_w = 120.0 * s;
        let col_w = (TABLE_W * s - name_w) / cols.len() as f32;
        let row = |i: usize| y + i as f32 * TABLE_ROW * s;
        let right = |batch: &mut SpriteBatch, text: &str, c: usize, y: f32, colour| {
            let x = left + name_w + (c + 1) as f32 * col_w - skin.width(Style::Small, text, s);
            skin.text(batch, Style::Small, text, x, y, s, colour);
        };
        for (c, name) in cols.iter().enumerate() {
            right(batch, name, c, row(0), self.theme.dim);
        }
        for (i, l) in self.score.lines().iter().enumerate() {
            let you = l.player == self.local;
            let colour = if you { self.theme.accent } else { self.theme.text };
            let name = if you { "YOU".to_string() } else { format!("PLAYER {}", l.player + 1) };
            skin.text(batch, Style::Small, &name, left, row(i + 1), s, colour);
            let cells = [
                l.units_built.to_string(),
                l.buildings_built.to_string(),
                l.kills.to_string(),
                (l.units_lost + l.buildings_lost).to_string(),
                l.harvested.to_string(),
            ];
            for (c, text) in cells.iter().enumerate() {
                right(batch, text, c, row(i + 1), colour);
            }
        }
    }
}

/// The game clock as minutes and seconds.
fn clock(game: &Game) -> String {
    let secs = game.state.tick / TICKS_PER_SECOND;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// A map's name for the menu: its first comment line up to a colon (`; Skirmish map 1: two players ...`), else its
/// file name without the folder or `.txt`.
pub fn map_name(path: &str, text: &str) -> String {
    let heading = text.lines().next().and_then(|l| l.strip_prefix(';')).map(|l| l.split(':').next().unwrap_or(l));
    match heading.map(str::trim).filter(|h| !h.is_empty()) {
        Some(h) => h.to_string(),
        None => {
            let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
            file.strip_suffix(".txt").unwrap_or(file).to_string()
        }
    }
}
