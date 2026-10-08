//! The colours of the HUD and menus, from the setting pack's `theme/theme.css`. Design: `plans/rts/ui.md`.
//!
//! The file is a stylesheet so a pack's designer can preview it in a browser, but the player reads only its custom
//! properties (`--name: #rrggbb;` or `#rrggbbaa`). Any the pack leaves out keep the engine's own colours, and unknown
//! ones are warned about. Frames and button art (the UI skin) are not read yet.

use crate::platform::Files;

/// Where a pack keeps its theme, in its folder.
pub const THEME_FILE: &str = "theme/theme.css";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// The rail and menu panels (`--panel-bg`).
    pub panel: [u8; 4],
    /// Lines between and around panels (`--panel-edge`).
    pub edge: [u8; 4],
    /// Buttons, cells and tabs (`--button`).
    pub button: [u8; 4],
    /// A button under the mouse or an open tab (`--button-hover`).
    pub hover: [u8; 4],
    pub text: [u8; 4],
    /// Labels and less important text (`--text-dim`).
    pub dim: [u8; 4],
    /// Headings and highlights (`--accent`).
    pub accent: [u8; 4],
    pub good: [u8; 4],
    pub warn: [u8; 4],
    pub bad: [u8; 4],
}

impl Default for Theme {
    fn default() -> Theme {
        Theme {
            panel: [22, 22, 26, 240],
            edge: [70, 70, 80, 255],
            button: [44, 44, 52, 255],
            hover: [70, 70, 84, 255],
            text: [235, 235, 225, 255],
            dim: [150, 150, 150, 255],
            accent: [217, 180, 58, 255],
            good: [70, 200, 90, 255],
            warn: [235, 175, 40, 255],
            bad: [220, 60, 50, 255],
        }
    }
}

impl Theme {
    /// The pack's theme in `pack`, over the engine's colours, and a warning for each line it couldn't use.
    pub fn load(pack: &Files) -> (Theme, Vec<String>) {
        match pack.read_text(THEME_FILE) {
            Ok(css) => Theme::parse(&css, &pack.name(THEME_FILE)),
            Err(_) => (Theme::default(), Vec::new()),
        }
    }

    /// Read the custom properties in `css`; `at` names the file in warnings.
    pub fn parse(css: &str, at: &str) -> (Theme, Vec<String>) {
        let mut theme = Theme::default();
        let mut warnings = Vec::new();
        for decl in without_comments(css).split(';') {
            // Each declaration that matters starts with `--`, after any selector and brace before it.
            let Some(start) = decl.find("--") else { continue };
            let Some((name, value)) = decl[start + 2..].split_once(':') else { continue };
            let (name, value) = (name.trim(), value.trim().trim_end_matches('}').trim());
            let slot = match name {
                "panel-bg" => &mut theme.panel,
                "panel-edge" => &mut theme.edge,
                "button" => &mut theme.button,
                "button-hover" => &mut theme.hover,
                "text" => &mut theme.text,
                "text-dim" => &mut theme.dim,
                "accent" => &mut theme.accent,
                "good" => &mut theme.good,
                "warn" => &mut theme.warn,
                "bad" => &mut theme.bad,
                // Read by the browser preview only, or by the skin later.
                "font" | "panel-light" | "panel-dark" => continue,
                _ => {
                    warnings.push(format!("{at}: unknown colour --{name}"));
                    continue;
                }
            };
            match colour(value) {
                Some(c) => *slot = c,
                None => warnings.push(format!("{at}: --{name}: \"{value}\" is not #rrggbb or #rrggbbaa")),
            }
        }
        (theme, warnings)
    }
}

fn without_comments(css: &str) -> String {
    let mut out = String::new();
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = rest[start + 2..].find("*/").map_or("", |end| &rest[start + 2 + end + 2..]);
    }
    out.push_str(rest);
    out
}

fn colour(v: &str) -> Option<[u8; 4]> {
    let hex = v.strip_prefix('#').filter(|h| (h.len() == 6 || h.len() == 8) && h.is_ascii())?;
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?, if hex.len() == 8 { byte(6)? } else { 255 }])
}
