//! The screens around a game: the title screen before it, the pause menu during it (Escape) and the end screen
//! after it. Design: `plans/rts/ui.md`, section 9.
//!
//! Like the HUD, menus are client state only. They decide which screen shows and turn clicks into actions for the
//! player program (start, resume, restart, back to the title, quit); they never touch the game. Their places come
//! from `layout`, from the screen size alone, so tests can find and click every button.

use classic_ai::{defeated, winner};
use classic_sim::Game;
use classic_sim::units::TICKS_PER_SECOND;

use crate::platform::{Font, Rect, SpriteBatch};
use crate::theme::Theme;

const BUTTON_W: f32 = 280.0;
const BUTTON_H: f32 = 36.0;
const GAP: f32 = 10.0;

const SHADE: [u8; 4] = [0, 0, 0, 170];

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
            title: title.to_string(),
        }
    }

    /// Whether the game should run and take the input.
    pub fn playing(&self) -> bool {
        self.screen == Screen::Playing
    }

    /// The buttons of the screen showing, centred on a screen of this size; none while playing.
    pub fn layout(&self, screen: (f32, f32)) -> Vec<Item> {
        let mut items: Vec<(Action, String)> = match self.screen {
            Screen::Playing => return Vec::new(),
            Screen::Title => {
                let mut items = vec![(Action::Start, "START".to_string())];
                if self.maps.len() > 1 {
                    items.push((Action::Map, format!("MAP: {}", self.maps[self.map].to_uppercase())));
                }
                if self.factions.len() > 1 {
                    items.push((Action::Faction, format!("FACTION: {}", self.factions[self.faction].to_uppercase())));
                }
                let opponents = if self.opponents { "COMPUTER" } else { "NONE" };
                items.push((Action::Opponents, format!("OPPONENTS: {opponents}")));
                items
            }
            Screen::Paused => vec![
                (Action::Resume, "RESUME".to_string()),
                (Action::Restart, "RESTART".to_string()),
                (Action::ToTitle, "QUIT TO TITLE".to_string()),
            ],
            Screen::Over { .. } => {
                vec![(Action::Restart, "PLAY AGAIN".to_string()), (Action::ToTitle, "TITLE SCREEN".to_string())]
            }
        };
        if self.can_quit {
            items.push((Action::Quit, "QUIT".to_string()));
        }
        let s = self.scale;
        let (w, h) = (BUTTON_W * s, BUTTON_H * s);
        let total = items.len() as f32 * (h + GAP * s) - GAP * s;
        let top = screen.1 / 2.0 - total / 2.0 + 30.0 * s;
        items
            .into_iter()
            .enumerate()
            .map(|(i, (action, label))| Item {
                action,
                label,
                rect: Rect::new((screen.0 - w) / 2.0, top + i as f32 * (h + GAP * s), w, h),
            })
            .collect()
    }

    /// A left click at (x, y): the action of the button under it, if any. The switches (map, faction, opponents)
    /// change here.
    pub fn click(&mut self, screen: (f32, f32), (x, y): (f32, f32)) -> Option<Action> {
        let action = self.layout(screen).into_iter().find(|i| i.rect.contains(x, y))?.action;
        match action {
            Action::Opponents => self.opponents = !self.opponents,
            Action::Map => self.map = (self.map + 1) % self.maps.len().max(1),
            Action::Faction => self.faction = (self.faction + 1) % self.factions.len().max(1),
            _ => {}
        }
        Some(action)
    }

    /// Escape: from the game to the pause menu, and from the pause menu back to the game. Returns whether the menu
    /// took the key; on the title and end screens it doesn't, so the program may quit.
    pub fn escape(&mut self) -> bool {
        match self.screen {
            Screen::Playing => self.screen = Screen::Paused,
            Screen::Paused => self.screen = Screen::Playing,
            Screen::Title | Screen::Over { .. } => return false,
        }
        true
    }

    /// Call after each tick: when the game has a winner, or the local player has lost, show the end screen.
    pub fn after_step(&mut self, game: &Game, local: u32) {
        if self.screen != Screen::Playing {
            return;
        }
        let won = match winner(game) {
            Some(w) => w == local,
            None if defeated(game, local) => false,
            None => return,
        };
        self.screen = Screen::Over { won };
    }

    /// Draw the screen showing over whatever is behind it (the map, dimmed), with the mouse at `mouse`.
    pub fn draw(&self, batch: &mut SpriteBatch, font: &Font, game: &Game, screen: (f32, f32), mouse: (f32, f32)) {
        if self.screen == Screen::Playing {
            return;
        }
        let s = self.scale;
        let big = (4.0 * s).round().max(1.0);
        let text = (2.0 * s).round().max(1.0);
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
            Screen::Playing => return,
        };
        // The heading shrinks to fit a narrow window.
        let size = if Font::width(&heading, big) + 40.0 * s <= screen.0 { big } else { text };
        let panel_w = (BUTTON_W * s + 60.0 * s).max(Font::width(&heading, size) + 40.0 * s).min(screen.0);
        let head_h = Font::height(size) + Font::height(text) + 34.0 * s;
        let panel =
            Rect::new((screen.0 - panel_w) / 2.0, first - head_h - 20.0 * s, panel_w, last - first + head_h + 40.0 * s);
        batch.fill(panel, self.theme.panel);
        batch.outline(panel, 1.0, self.theme.edge);
        let hx = (screen.0 - Font::width(&heading, size)) / 2.0;
        font.draw(batch, &heading, hx, panel.y + 16.0 * s, size, colour);
        let sx = (screen.0 - Font::width(&sub, text)) / 2.0;
        font.draw(batch, &sub, sx, panel.y + 24.0 * s + Font::height(size), text, self.theme.dim);
        for i in &items {
            let over = i.rect.contains(mouse.0, mouse.1);
            batch.fill(i.rect, if over { self.theme.hover } else { self.theme.button });
            if over {
                batch.outline(i.rect, 1.0, self.theme.text);
            }
            let lx = i.rect.x + (i.rect.w - Font::width(&i.label, text)) / 2.0;
            font.draw(batch, &i.label, lx, i.rect.y + (i.rect.h - Font::height(text)) / 2.0, text, self.theme.text);
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
