//! The title, pause and end screens: which screen shows, what each button asks for, and that the menus never
//! touch the game. The last test draws one (it needs a GPU adapter, a software one will do).

use classic_render::menu::{Action, Menu, Screen};
use classic_render::platform::{Font, Gpu, Rect, SpriteBatch, gpu::OFFSCREEN_FORMAT};
use classic_sim::{Game, GameOptions, Rules};
use classic_tools::setting;

const MAP: &str = include_str!("../../../maps/skirmish-01.txt");
const SCREEN: (f32, f32) = (1024.0, 768.0);

fn game() -> Game {
    let pack = setting::load("generic").unwrap();
    let rules = Rules::from_table(&pack.rules).unwrap();
    Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap()
}

fn centre(r: Rect) -> (f32, f32) {
    (r.x + r.w / 2.0, r.y + r.h / 2.0)
}

fn button(menu: &Menu, action: Action) -> Rect {
    menu.layout(SCREEN).into_iter().find(|i| i.action == action).unwrap_or_else(|| panic!("no {action:?}")).rect
}

#[test]
fn the_title_starts_a_game_and_switches_the_opponents() {
    let mut menu = Menu::new("Generic");
    assert_eq!(menu.screen, Screen::Title);
    assert!(!menu.playing());
    let items = menu.layout(SCREEN);
    let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
    println!("title buttons {labels:?}");
    assert_eq!(labels, ["START", "OPPONENTS: COMPUTER", "QUIT"]);
    // The buttons are centred and don't overlap.
    for w in items.windows(2) {
        assert!(w[0].rect.y + w[0].rect.h < w[1].rect.y);
    }
    assert!((centre(items[0].rect).0 - SCREEN.0 / 2.0).abs() < 0.5);

    assert_eq!(menu.click(SCREEN, centre(button(&menu, Action::Opponents))), Some(Action::Opponents));
    assert!(!menu.opponents);
    assert_eq!(menu.layout(SCREEN)[1].label, "OPPONENTS: NONE");
    assert_eq!(menu.click(SCREEN, (5.0, 5.0)), None, "a click off the buttons does nothing");
    assert_eq!(menu.click(SCREEN, centre(button(&menu, Action::Start))), Some(Action::Start));

    // In the browser the page stays, so there is no Quit.
    menu.can_quit = false;
    assert!(menu.layout(SCREEN).iter().all(|i| i.action != Action::Quit));
}

#[test]
fn escape_pauses_and_resumes_but_not_on_the_title() {
    let mut menu = Menu::new("Generic");
    assert!(!menu.escape(), "the title leaves Escape to the program");
    menu.screen = Screen::Playing;
    assert!(menu.layout(SCREEN).is_empty(), "no buttons while playing");
    assert!(menu.escape());
    assert_eq!(menu.screen, Screen::Paused);
    let labels: Vec<String> = menu.layout(SCREEN).into_iter().map(|i| i.label).collect();
    assert_eq!(labels, ["RESUME", "RESTART", "QUIT TO TITLE", "QUIT"]);
    assert!(menu.escape());
    assert!(menu.playing());
}

#[test]
fn the_end_screen_comes_when_someone_wins_or_the_player_loses() {
    let mut menu = Menu::new("Generic");
    let mut won = game();
    won.state.entities.retain(|e| e.owner != 1);
    menu.after_step(&won, 0);
    assert_eq!(menu.screen, Screen::Title, "the title screen doesn't end");

    menu.screen = Screen::Playing;
    let hash = game().hash();
    let fresh = game();
    menu.after_step(&fresh, 0);
    assert!(menu.playing(), "nobody has won yet");
    assert_eq!(fresh.hash(), hash, "the menu reads the game and changes nothing");

    menu.after_step(&won, 0);
    assert_eq!(menu.screen, Screen::Over { won: true });
    let labels: Vec<String> = menu.layout(SCREEN).into_iter().map(|i| i.label).collect();
    assert_eq!(labels, ["PLAY AGAIN", "TITLE SCREEN", "QUIT"]);
    assert_eq!(menu.click(SCREEN, centre(button(&menu, Action::Restart))), Some(Action::Restart));

    let mut lost = game();
    lost.state.entities.retain(|e| e.owner != 0);
    menu.screen = Screen::Playing;
    menu.after_step(&lost, 0);
    assert_eq!(menu.screen, Screen::Over { won: false });
}

#[test]
fn a_menu_draws_over_the_game_and_nothing_while_playing() {
    let gpu = Gpu::headless().expect("a GPU adapter (a software one will do)");
    let mut batch = SpriteBatch::new(&gpu, OFFSCREEN_FORMAT);
    let font = Font::new(&gpu, &mut batch);
    let game = game();
    let (w, h) = (SCREEN.0 as u32, SCREEN.1 as u32);
    let grey = [90, 90, 90, 255];
    let lit = |batch: &mut SpriteBatch, menu: &Menu| {
        batch.fill(Rect::new(0.0, 0.0, SCREEN.0, SCREEN.1), grey);
        menu.draw(batch, &font, &game, SCREEN, (0.0, 0.0));
        let img = batch.draw_to_image(&gpu, w, h, [0, 0, 0, 255]);
        img.chunks_exact(4).filter(|p| p[..3] != grey[..3]).count()
    };
    let mut menu = Menu::new("Generic");
    let title = lit(&mut batch, &menu);
    menu.screen = Screen::Playing;
    let playing = lit(&mut batch, &menu);
    menu.screen = Screen::Over { won: true };
    let over = lit(&mut batch, &menu);
    println!("pixels changed: title {title}, playing {playing}, victory {over}");
    assert_eq!(playing, 0);
    assert_eq!(title, (w * h) as usize, "the whole screen is shaded behind the title");
    assert_eq!(over, (w * h) as usize);
}

#[test]
fn the_title_offers_the_maps_and_factions_when_there_is_a_choice() {
    let mut menu = Menu::new("Pack");
    menu.maps = vec!["Open sands".into(), "Twin ridges".into()];
    menu.factions = vec!["Faction A".into(), "Faction B".into(), "Faction C".into()];
    let labels: Vec<String> = menu.layout(SCREEN).into_iter().map(|i| i.label).collect();
    println!("title buttons {labels:?}");
    assert_eq!(labels, ["START", "MAP: OPEN SANDS", "FACTION: FACTION A", "OPPONENTS: COMPUTER", "QUIT"]);
    assert_eq!(menu.click(SCREEN, centre(button(&menu, Action::Map))), Some(Action::Map));
    assert_eq!(menu.map, 1);
    assert_eq!(menu.click(SCREEN, centre(button(&menu, Action::Map))), Some(Action::Map));
    assert_eq!(menu.map, 0, "round to the first again");
    for want in [1, 2, 0] {
        menu.click(SCREEN, centre(button(&menu, Action::Faction)));
        assert_eq!(menu.faction, want);
    }
    // One map and one faction: nothing to choose, so no buttons.
    menu.maps.truncate(1);
    menu.factions.truncate(1);
    assert!(menu.layout(SCREEN).iter().all(|i| i.action != Action::Map && i.action != Action::Faction));

    assert_eq!(classic_render::menu::map_name("maps/skirmish-01.txt", MAP), "Skirmish map 1");
    assert_eq!(classic_render::menu::map_name("packs/x/maps/dunes.txt", "....\n"), "dunes");
}

#[test]
fn the_local_player_gets_the_picked_faction_and_the_others_the_rest() {
    use classic_render::art::player_factions;
    assert_eq!(player_factions(3, 2, 0, 0), [0, 1]);
    assert_eq!(player_factions(3, 2, 0, 2), [2, 0]);
    assert_eq!(player_factions(3, 3, 1, 0), [1, 0, 2]);
    assert_eq!(player_factions(2, 4, 0, 1), [1, 0, 0, 1], "more players than factions: round again");
    assert_eq!(player_factions(1, 2, 0, 0), [0, 0]);
    assert_eq!(player_factions(3, 2, 0, 7), [1, 0], "a faction number past the end wraps");
}

#[test]
fn a_pack_theme_recolours_the_menus_and_the_generic_one_matches_the_engine() {
    use classic_render::theme::{THEME_FILE, Theme};
    let css = "/* a comment with --text: #000000; inside */\n:root {\n  --panel-bg: #10203080;\n  --text: #fafafa;\n  \
               --accent: orange;\n  --glow: #ffffff;\n  --font: serif;\n}\n";
    let (theme, warnings) = Theme::parse(css, "theme.css");
    println!("{theme:?}\n{warnings:?}");
    assert_eq!(theme.panel, [0x10, 0x20, 0x30, 0x80]);
    assert_eq!(theme.text, [0xfa, 0xfa, 0xfa, 255]);
    assert_eq!(theme.accent, Theme::default().accent, "a colour it can't read keeps the engine's");
    assert_eq!(theme.button, Theme::default().button, "one it leaves out too");
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("--accent") && warnings[1].contains("--glow"));

    let generic = classic_render::platform::Files::Dir(setting::root().join("settings/generic"));
    let css = generic.read_text(THEME_FILE).unwrap();
    assert_eq!(Theme::parse(&css, THEME_FILE), (Theme::default(), Vec::new()), "the generic look is the engine's");
}
