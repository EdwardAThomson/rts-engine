//! The player's settings, kept between runs: the volume of each sound bus, how fast the view scrolls, the keys,
//! and the opponent's difficulty last picked. Design: `plans/rts/ui.md`, section 9.
//!
//! They are client state, like the menus: nothing here reaches the game. They are stored as a small text file of
//! `name = value` lines (`store.rs` says where), so a person can read and edit it; a line it doesn't know is skipped
//! with a warning, and a missing one keeps its default.

use classic_ai::Difficulty;

/// The sound buses in the mixer's order (`rts_platform::audio::Bus`), as named in the settings file and the menu.
pub const BUSES: [(&str, &str); 4] = [("sfx", "EFFECTS"), ("ui", "INTERFACE"), ("voice", "VOICES"), ("music", "MUSIC")];

/// Volume steps on the settings screen, in percent.
pub const VOLUME_STEP: u32 = 10;
/// Scroll speeds on offer, in percent of the normal speed.
pub const SCROLL_MIN: u32 = 50;
pub const SCROLL_MAX: u32 = 200;
pub const SCROLL_STEP: u32 = 25;

/// What a key the player can change does. The arrow keys always scroll as well, Escape always opens the menu, and
/// the number keys always pick groups.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bind {
    ScrollUp,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    Pause,
    Mute,
    Base,
    NextTab,
    Sell,
    Repair,
    /// Make the selected factory the primary one of its kind.
    Primary,
}

impl Bind {
    pub const ALL: [Bind; 11] = [
        Bind::ScrollUp,
        Bind::ScrollDown,
        Bind::ScrollLeft,
        Bind::ScrollRight,
        Bind::Pause,
        Bind::Mute,
        Bind::Base,
        Bind::NextTab,
        Bind::Sell,
        Bind::Repair,
        Bind::Primary,
    ];

    /// Its name in the settings file.
    pub fn id(self) -> &'static str {
        match self {
            Bind::ScrollUp => "scroll_up",
            Bind::ScrollDown => "scroll_down",
            Bind::ScrollLeft => "scroll_left",
            Bind::ScrollRight => "scroll_right",
            Bind::Pause => "pause",
            Bind::Mute => "mute",
            Bind::Base => "centre_on_base",
            Bind::NextTab => "next_tab",
            Bind::Sell => "sell",
            Bind::Repair => "repair",
            Bind::Primary => "primary",
        }
    }

    /// Its name on the keys screen.
    pub fn label(self) -> &'static str {
        match self {
            Bind::ScrollUp => "SCROLL UP",
            Bind::ScrollDown => "SCROLL DOWN",
            Bind::ScrollLeft => "SCROLL LEFT",
            Bind::ScrollRight => "SCROLL RIGHT",
            Bind::Pause => "PAUSE",
            Bind::Mute => "MUTE",
            Bind::Base => "CENTRE ON BASE",
            Bind::NextTab => "NEXT TAB",
            Bind::Sell => "SELL",
            Bind::Repair => "REPAIR",
            Bind::Primary => "PRIMARY FACTORY",
        }
    }

    /// The key it starts on, by the window system's name for it (winit's `KeyCode`, as `Debug` prints it).
    pub fn default_key(self) -> &'static str {
        match self {
            Bind::ScrollUp => "KeyW",
            Bind::ScrollDown => "KeyS",
            Bind::ScrollLeft => "KeyA",
            Bind::ScrollRight => "KeyD",
            Bind::Pause => "Space",
            Bind::Mute => "KeyM",
            Bind::Base => "KeyH",
            Bind::NextTab => "Tab",
            Bind::Sell => "KeyZ",
            Bind::Repair => "KeyC",
            Bind::Primary => "KeyP",
        }
    }
}

/// A key's name for the screen: `KeyW` shows as `W`, `Digit5` as `5`, `ArrowUp` as `ARROW UP`.
pub fn key_label(name: &str) -> String {
    let short = name.strip_prefix("Key").or_else(|| name.strip_prefix("Digit")).unwrap_or(name);
    let mut out = String::new();
    let mut prev: Option<char> = None;
    for c in short.chars() {
        if c.is_uppercase() && prev.is_some_and(|p| !p.is_uppercase()) {
            out.push(' ');
        }
        out.extend(c.to_uppercase());
        prev = Some(c);
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    /// Each bus's volume in percent of its starting level, in `BUSES` order.
    pub volume: [u32; 4],
    /// Scroll speed in percent of the normal speed.
    pub scroll: u32,
    /// The key for each `Bind`, in `Bind::ALL` order.
    pub keys: [String; Bind::ALL.len()],
    /// The opponent's strength for the next game.
    pub difficulty: Difficulty,
}

impl Default for Prefs {
    fn default() -> Prefs {
        Prefs {
            volume: [100; 4],
            scroll: 100,
            keys: Bind::ALL.map(|b| b.default_key().to_string()),
            difficulty: Difficulty::Normal,
        }
    }
}

impl Prefs {
    pub fn key(&self, bind: Bind) -> &str {
        &self.keys[bind as usize]
    }

    /// Put `bind` on the key named `key`. A bind already on that key takes this one's old key, so no key does two
    /// things and none is lost.
    pub fn set_key(&mut self, bind: Bind, key: &str) {
        let old = self.keys[bind as usize].clone();
        for k in &mut self.keys {
            if k == key {
                *k = old.clone();
            }
        }
        self.keys[bind as usize] = key.to_string();
    }

    /// Which bind the key named `key` is on, if any.
    pub fn bound(&self, key: &str) -> Option<Bind> {
        Bind::ALL.into_iter().find(|&b| self.key(b) == key)
    }

    /// The next volume step for a bus, up or (with `down`) down, wrapping between 0 and 100.
    pub fn step_volume(&mut self, bus: usize, down: bool) {
        let v = self.volume[bus];
        self.volume[bus] = match (down, v) {
            (false, 100..) => 0,
            (false, _) => (v + VOLUME_STEP).min(100),
            (true, 0) => 100,
            (true, _) => v.saturating_sub(VOLUME_STEP),
        };
    }

    /// The next scroll speed, up or down, wrapping between the slowest and the fastest.
    pub fn step_scroll(&mut self, down: bool) {
        let s = self.scroll;
        self.scroll = match down {
            false if s >= SCROLL_MAX => SCROLL_MIN,
            false => (s + SCROLL_STEP).min(SCROLL_MAX),
            true if s <= SCROLL_MIN => SCROLL_MAX,
            true => (s - SCROLL_STEP).max(SCROLL_MIN),
        };
    }

    /// The mixer's gain for each bus: its starting level (`defaults`) scaled by the volume.
    pub fn bus_gain(&self, defaults: [f32; 4]) -> [f32; 4] {
        std::array::from_fn(|i| defaults[i] * self.volume[i] as f32 / 100.0)
    }

    /// The settings file's text.
    pub fn to_text(&self) -> String {
        let mut out = String::from(
            "# The player's settings. Volumes and the scroll speed are percent; keys are window-system key names.\n",
        );
        for (i, (id, _)) in BUSES.iter().enumerate() {
            out += &format!("volume.{id} = {}\n", self.volume[i]);
        }
        out += &format!("scroll = {}\n", self.scroll);
        out += &format!("difficulty = {}\n", self.difficulty.id());
        for b in Bind::ALL {
            out += &format!("key.{} = {}\n", b.id(), self.key(b));
        }
        out
    }

    /// Settings from a file's text, and a warning for each line that couldn't be used. Anything missing or wrong
    /// keeps its default.
    pub fn parse(text: &str) -> (Prefs, Vec<String>) {
        let mut p = Prefs::default();
        let mut warnings = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((name, value)) = line.split_once('=').map(|(a, b)| (a.trim(), b.trim())) else {
                warnings.push(format!("line {}: no `=` in {line:?}", n + 1));
                continue;
            };
            let percent = |max: u32| value.parse::<u32>().ok().filter(|&v| v <= max);
            let ok = if let Some(bus) = name.strip_prefix("volume.") {
                match (BUSES.iter().position(|(id, _)| *id == bus), percent(100)) {
                    (Some(i), Some(v)) => {
                        p.volume[i] = v;
                        true
                    }
                    _ => false,
                }
            } else if let Some(bind) = name.strip_prefix("key.") {
                match Bind::ALL.into_iter().find(|b| b.id() == bind) {
                    Some(b) if !value.is_empty() && !value.contains(char::is_whitespace) => {
                        p.set_key(b, value);
                        true
                    }
                    _ => false,
                }
            } else if name == "scroll" {
                match percent(SCROLL_MAX).filter(|&v| v >= SCROLL_MIN) {
                    Some(v) => {
                        p.scroll = v;
                        true
                    }
                    None => false,
                }
            } else if name == "difficulty" {
                match Difficulty::from_id(value) {
                    Some(d) => {
                        p.difficulty = d;
                        true
                    }
                    None => false,
                }
            } else {
                false
            };
            if !ok {
                warnings.push(format!("line {}: can't use {line:?}", n + 1));
            }
        }
        (p, warnings)
    }
}
