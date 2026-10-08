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

const BUTTON_W: f32 = 280.0;
const BUTTON_H: f32 = 36.0;
const GAP: f32 = 10.0;

const SHADE: [u8; 4] = [0, 0, 0, 170];
const PANEL: [u8; 4] = [22, 22, 26, 240];
const BUTTON: [u8; 4] = [44, 44, 52, 255];
const HOVER: [u8; 4] = [70, 70, 84, 255];
const TEXT: [u8; 4] = [235, 235, 225, 255];
const DIM: [u8; 4] = [150, 150, 150, 255];
const GOOD: [u8; 4] = [70, 200, 90, 255];
const BAD: [u8; 4] = [220, 60, 50, 255];

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
    /// UI scale: 1 at 100%.
    pub scale: f32,
    /// Whether a Quit button makes sense (not in the browser, where the page stays).
    pub can_quit: bool,
    /// The setting pack's title, shown on the title screen.
    title: String,
}

impl Menu {
    pub fn new(title: &str) -> Menu {
        Menu { screen: Screen::Title, opponents: true, scale: 1.0, can_quit: true, title: title.to_string() }
    }

    /// Whether the game should run and take the input.
    pub fn playing(&self) -> bool {
        self.screen == Screen::Playing
    }

    /// The buttons of the screen showing, centred on a screen of this size; none while playing.
    pub fn layout(&self, screen: (f32, f32)) -> Vec<Item> {
        let mut items: Vec<(Action, String)> = match self.screen {
            Screen::Playing => return Vec::new(),
            Screen::Title => vec![
                (Action::Start, "START".to_string()),
                (Action::Opponents, format!("OPPONENTS: {}", if self.opponents { "COMPUTER" } else { "NONE" })),
            ],
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

    /// A left click at (x, y): the action of the button under it, if any. The opponents switch flips here.
    pub fn click(&mut self, screen: (f32, f32), (x, y): (f32, f32)) -> Option<Action> {
        let action = self.layout(screen).into_iter().find(|i| i.rect.contains(x, y))?.action;
        if action == Action::Opponents {
            self.opponents = !self.opponents;
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
            Screen::Title => (self.title.to_uppercase(), TEXT, "SKIRMISH".to_string()),
            Screen::Paused => ("PAUSED".to_string(), TEXT, clock(game)),
            Screen::Over { won: true } => ("VICTORY".to_string(), GOOD, format!("WON IN {}", clock(game))),
            Screen::Over { won: false } => ("DEFEAT".to_string(), BAD, format!("LOST AFTER {}", clock(game))),
            Screen::Playing => return,
        };
        // The heading shrinks to fit a narrow window.
        let size = if Font::width(&heading, big) + 40.0 * s <= screen.0 { big } else { text };
        let panel_w = (BUTTON_W * s + 60.0 * s).max(Font::width(&heading, size) + 40.0 * s).min(screen.0);
        let head_h = Font::height(size) + Font::height(text) + 34.0 * s;
        let panel =
            Rect::new((screen.0 - panel_w) / 2.0, first - head_h - 20.0 * s, panel_w, last - first + head_h + 40.0 * s);
        batch.fill(panel, PANEL);
        batch.outline(panel, 1.0, [90, 90, 100, 255]);
        let hx = (screen.0 - Font::width(&heading, size)) / 2.0;
        font.draw(batch, &heading, hx, panel.y + 16.0 * s, size, colour);
        let sx = (screen.0 - Font::width(&sub, text)) / 2.0;
        font.draw(batch, &sub, sx, panel.y + 24.0 * s + Font::height(size), text, DIM);
        for i in &items {
            let over = i.rect.contains(mouse.0, mouse.1);
            batch.fill(i.rect, if over { HOVER } else { BUTTON });
            if over {
                batch.outline(i.rect, 1.0, TEXT);
            }
            let lx = i.rect.x + (i.rect.w - Font::width(&i.label, text)) / 2.0;
            font.draw(batch, &i.label, lx, i.rect.y + (i.rect.h - Font::height(text)) / 2.0, text, TEXT);
        }
    }
}

/// The game clock as minutes and seconds.
fn clock(game: &Game) -> String {
    let secs = game.state.tick / TICKS_PER_SECOND;
    format!("{}:{:02}", secs / 60, secs % 60)
}
