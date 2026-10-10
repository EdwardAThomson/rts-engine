//! The generic pack's UI skin loads whole, and the cursor follows what a click would do. No GPU needed.

use classic_render::platform::Files;
use classic_render::skin::{self, Pointer, SkinFiles, Style};
use classic_sim::{CommandOrder, Game, GameOptions, Rules, Terrain};
use classic_tools::setting;

const MAP: &str = include_str!("../../../maps/skirmish-01.txt");

fn generic() -> Files {
    Files::Dir(setting::root().join("settings/generic"))
}

/// The cursor modes of `plans/rts/ui.md` section 9.
const CURSORS: [&str; 23] = [
    "default",
    "select",
    "move",
    "attack",
    "no",
    "harvest",
    "enter",
    "repair_pad",
    "carry",
    "deploy",
    "place_ok",
    "place_bad",
    "sell",
    "repair",
    "target",
    "scroll_n",
    "scroll_ne",
    "scroll_e",
    "scroll_se",
    "scroll_s",
    "scroll_sw",
    "scroll_w",
    "scroll_nw",
];

#[test]
fn the_generic_skin_has_every_frame_cursor_emblem_and_font() {
    let files = generic();
    let theme = SkinFiles::load(&[&files]).unwrap();
    for id in ["rail", "panel", "inset", "tooltip", "tab", "button"] {
        let f = theme.frames.get(id).unwrap_or_else(|| panic!("no {id} frame"));
        assert_eq!(f.image.w % f.frame.0, 0, "{id}: the strip is whole frames");
    }
    assert_eq!(theme.frames["button"].image.w / theme.frames["button"].frame.0, 4, "buttons have four states");
    for id in CURSORS {
        let c = theme.cursors.get(id).unwrap_or_else(|| panic!("no {id} cursor"));
        assert_eq!((c.image.w, c.image.h), (32, 32), "{id}");
        let big = c.image_2x.as_ref().unwrap();
        assert_eq!((big.w, big.h), (64, 64), "{id} at 2x");
    }
    let pack = setting::load("generic").unwrap();
    for faction in &pack.factions {
        assert!(theme.emblems.contains_key(&faction.id), "no emblem for {}", faction.id);
    }
    for style in Style::ALL {
        for scale in 1..=3 {
            let atlas = theme.fonts.get(&(style, scale)).unwrap_or_else(|| panic!("no {style:?} font at {scale}x"));
            for c in (32u8..127).map(char::from) {
                assert!(atlas.glyphs.contains_key(&c), "{style:?} {scale}x has no {c:?}");
            }
            assert!(atlas.line > atlas.ascent && atlas.ascent > 0.0);
        }
    }
    // Text grows with the scale, and the display face is wider than the body face at its size.
    let w = |s: Style, k: u32| {
        let a = &theme.fonts[&(s, k)];
        "CREDITS 1250".chars().map(|c| a.glyphs[&c].advance).sum::<f32>()
    };
    assert!(w(Style::Body, 2) > 1.8 * w(Style::Body, 1));
    assert!(w(Style::Small, 1) < w(Style::Body, 1) && w(Style::Body, 1) < w(Style::Title, 1));
}

#[test]
fn every_file_the_theme_names_exists_for_the_browser() {
    let dir = setting::root().join("settings/generic");
    let index = std::fs::read_to_string(dir.join(skin::THEME_INDEX)).unwrap();
    let names = skin::theme_files(&index);
    assert!(names.len() > 50);
    for n in &names {
        assert!(dir.join(n).is_file(), "{n} is missing");
    }
    let fonts = names.iter().find(|n| n.ends_with("fonts.json")).expect("the theme names its fonts");
    for n in skin::font_files(&std::fs::read_to_string(dir.join(fonts)).unwrap()) {
        assert!(dir.join(&n).is_file(), "{n} is missing");
    }
}

#[test]
fn a_pack_without_a_theme_falls_back_and_its_own_entries_win() {
    let empty = Files::Memory { label: "empty".into(), files: Default::default() };
    assert!(SkinFiles::load(&[&empty]).unwrap().frames.is_empty(), "no theme: plain fills and the pixel font");
    // A pack that brings only its own panel keeps the generic everything else.
    let generic = generic();
    let panel = generic.read("theme/inset.png").unwrap();
    let mut files = std::collections::BTreeMap::new();
    files.insert(
        "theme/theme.json".to_string(),
        br#"{ "frames": { "panel": { "file": "theme/mine.png", "frame": [12, 12], "slice": 4 } } }"#.to_vec(),
    );
    files.insert("theme/mine.png".to_string(), panel);
    let own = Files::Memory { label: "own".into(), files };
    let theme = SkinFiles::load(&[&own, &generic]).unwrap();
    assert_eq!(theme.frames["panel"].frame, (12, 12), "the pack's own panel wins");
    assert_eq!(theme.frames["rail"].frame, (32, 32), "the rest is the generic skin");
    assert!(!theme.fonts.is_empty() && theme.cursors.contains_key("attack"));
}

#[test]
fn the_cursor_shows_what_a_click_would_do() {
    let pack = setting::load("generic").unwrap();
    let rules = Rules::from_table(&pack.rules).unwrap();
    let mut game = Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    // A base builder beside player 0's tank, spawned before the closures below borrow the game.
    let tank_at = game.state.entities.iter().find(|e| e.owner == 0 && game.rules.kind(e.kind).weapon.is_some());
    let tank_at = tank_at.map(|e| e.tile()).unwrap();
    let mcv_kind = game.kind("mcv").unwrap();
    let mcv = game.spawn(mcv_kind, 0, tank_at.x, tank_at.y + 1);
    let game = game;
    let mine = |building: bool, armed: bool, owner: u32| {
        game.state
            .entities
            .iter()
            .find(|e| {
                let k = game.rules.kind(e.kind);
                e.owner == owner && k.building == building && (building || k.weapon.is_some() == armed)
            })
            .map(|e| e.id)
    };
    let tank = mine(false, true, 0).expect("player 0 starts with an armed unit");
    let enemy = mine(true, false, 1).expect("player 1 has a building");
    let open = (0..game.map.height)
        .flat_map(|y| (0..game.map.width).map(move |x| (x, y)))
        .find(|&(x, y)| game.map.terrain[(y * game.map.width + x) as usize] == Terrain::Open)
        .unwrap();
    let cliff = (0..game.map.height)
        .flat_map(|y| (0..game.map.width).map(move |x| (x, y)))
        .find(|&(x, y)| game.map.terrain[(y * game.map.width + x) as usize] == Terrain::Cliff);
    let at = |hovered: Option<u32>, tile: (i32, i32)| Pointer { hovered, tile, ..Pointer::default() };
    let pick = |selected: &[u32], p: Pointer| skin::choose_cursor(&game, 0, selected, &p);

    assert_eq!(pick(&[], at(None, open)), "default");
    assert_eq!(pick(&[], at(Some(enemy), open)), "select", "nothing selected: a click selects");
    assert_eq!(pick(&[tank], at(None, open)), "move");
    assert_eq!(pick(&[tank], at(Some(enemy), open)), "attack");
    assert_eq!(pick(&[tank], at(Some(tank), open)), "select");
    if let Some(c) = cliff {
        assert_eq!(pick(&[tank], at(None, c)), "no", "units can't go onto a cliff");
    }
    assert_eq!(pick(&[tank], at(None, (-1, 0))), "no", "off the map");
    assert_eq!(pick(&[tank], Pointer { over_hud: true, ..at(Some(enemy), open) }), "default");
    assert_eq!(pick(&[], Pointer { placing: Some(true), ..at(None, open) }), "place_ok");
    assert_eq!(pick(&[], Pointer { placing: Some(false), ..at(None, open) }), "place_bad");
    assert_eq!(pick(&[tank], Pointer { edge: (1, -1), ..at(Some(enemy), open) }), "scroll_ne", "edge scroll wins");
    assert_eq!(pick(&[], Pointer { edge: (0, 1), over_hud: true, ..at(None, open) }), "scroll_s");
    // An enemy's unit selected gives no orders.
    assert_eq!(pick(&[enemy], at(None, open)), "default");
    // A selected unit that deploys, pointed at itself, deploys; pointed at by another unit's selection, selects.
    assert_eq!(pick(&[tank, mcv], at(Some(mcv), open)), "deploy");
    assert_eq!(pick(&[tank], at(Some(mcv), open)), "select");
    let (special, rest) = skin::special_orders(&game, 0, &[tank, mcv], Some(mcv));
    assert_eq!((special, rest), (vec![(vec![mcv], CommandOrder::Deploy)], vec![tank]));
}
