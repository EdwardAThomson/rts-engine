//! The title, pause, settings, keys and end screens: which screen shows, what each button asks for, and that the
//! menus never touch the game. One test draws them (it needs a GPU adapter, a software one will do).

use classic_render::Skin;
use classic_render::menu::{Action, Menu, Screen};
use classic_render::platform::{Files, Gpu, Rect, SpriteBatch, gpu::OFFSCREEN_FORMAT};
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
    assert_eq!(labels, ["START", "OPPONENTS: COMPUTER", "DIFFICULTY: NORMAL", "SETTINGS", "QUIT"]);
    // The buttons are centred and don't overlap.
    for w in items.windows(2) {
        assert!(w[0].rect.y + w[0].rect.h < w[1].rect.y);
    }
    assert!((centre(items[0].rect).0 - SCREEN.0 / 2.0).abs() < 0.5);

    assert_eq!(menu.click(SCREEN, centre(button(&menu, Action::Opponents))), Some(Action::Opponents));
    assert!(!menu.opponents);
    assert_eq!(menu.layout(SCREEN)[1].label, "OPPONENTS: NONE");
    assert!(menu.layout(SCREEN).iter().all(|i| i.action != Action::Difficulty), "no difficulty without opponents");
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
    assert_eq!(labels, ["RESUME", "SAVE GAME", "SETTINGS", "RESTART", "QUIT TO TITLE", "QUIT"]);
    menu.has_save = true;
    assert_eq!(menu.layout(SCREEN)[2].label, "LOAD GAME", "a saved game can be loaded from the pause menu");
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
    let generic = Files::Dir(setting::root().join("settings/generic"));
    let skin = Skin::load(&gpu, &mut batch, &[&generic]).unwrap();
    let game = game();
    let (w, h) = (SCREEN.0 as u32, SCREEN.1 as u32);
    let grey = [90, 90, 90, 255];
    let lit = |batch: &mut SpriteBatch, menu: &Menu| {
        batch.fill(Rect::new(0.0, 0.0, SCREEN.0, SCREEN.1), grey);
        menu.draw(batch, &skin, &game, SCREEN, (0.0, 0.0));
        let img = batch.draw_to_image(&gpu, w, h, [0, 0, 0, 255]);
        img.chunks_exact(4).filter(|p| p[..3] != grey[..3]).count()
    };
    let mut menu = Menu::new("Generic");
    let title = lit(&mut batch, &menu);
    // The title screen in the generic skin, for a person to look at.
    batch.fill(Rect::new(0.0, 0.0, SCREEN.0, SCREEN.1), grey);
    menu.draw(&mut batch, &skin, &game, SCREEN, centre(menu.layout(SCREEN)[0].rect));
    let image = batch.draw_to_image(&gpu, w, h, [0, 0, 0, 255]);
    let out = setting::root().join("target/menu-test.png");
    std::fs::write(&out, classic_tools::art::png::encode(w as usize, h as usize, &image)).unwrap();
    println!("wrote {}", out.display());
    menu.screen = Screen::Playing;
    let playing = lit(&mut batch, &menu);
    menu.screen = Screen::Over { won: true };
    let plain = lit(&mut batch, &menu);
    // The end screen with a score table, and the settings and keys screens, for a person to look at.
    menu.after_step(&game, 0);
    let mut over = 0;
    for (screen, name) in [(Screen::Over { won: true }, "over"), (Screen::Settings, "settings"), (Screen::Keys, "keys")]
    {
        menu.screen = screen;
        over = over.max(lit(&mut batch, &menu));
        batch.fill(Rect::new(0.0, 0.0, SCREEN.0, SCREEN.1), grey);
        menu.draw(&mut batch, &skin, &game, SCREEN, (0.0, 0.0));
        let image = batch.draw_to_image(&gpu, w, h, [0, 0, 0, 255]);
        let out = setting::root().join(format!("target/menu-test-{name}.png"));
        std::fs::write(&out, classic_tools::art::png::encode(w as usize, h as usize, &image)).unwrap();
    }
    println!("pixels changed: title {title}, playing {playing}, victory {plain}");
    assert_eq!(plain, (w * h) as usize);
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
    let want = ["START", "MAP: OPEN SANDS", "FACTION: FACTION A", "OPPONENTS: COMPUTER", "DIFFICULTY: NORMAL"];
    assert_eq!(labels, [&want[..], &["SETTINGS", "QUIT"]].concat());
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

#[test]
fn the_settings_screen_steps_volumes_and_scroll_speed_and_goes_back_where_it_came_from() {
    let mut menu = Menu::new("Generic");
    assert_eq!(menu.click(SCREEN, centre(button(&menu, Action::Settings))), Some(Action::Settings));
    assert_eq!(menu.screen, Screen::Settings);
    let labels: Vec<String> = menu.layout(SCREEN).into_iter().map(|i| i.label).collect();
    println!("settings buttons {labels:?}");
    assert_eq!(
        labels,
        ["EFFECTS: 100%", "INTERFACE: 100%", "VOICES: 100%", "MUSIC: 100%", "SCROLL SPEED: 100%", "KEYS", "BACK"]
    );
    // A left click steps up, wrapping from 100% to 0; a right click steps down.
    let music = centre(button(&menu, Action::Volume(3)));
    assert_eq!(menu.click(SCREEN, music), Some(Action::Volume(3)));
    assert_eq!(menu.prefs.volume[3], 0);
    menu.click_with(SCREEN, music, true);
    assert_eq!(menu.prefs.volume[3], 100);
    menu.click_with(SCREEN, music, true);
    assert_eq!(menu.prefs.volume, [100, 100, 100, 90]);
    let scroll = centre(button(&menu, Action::Scroll));
    menu.click(SCREEN, scroll);
    assert_eq!(menu.prefs.scroll, 125);
    for _ in 0..3 {
        menu.click_with(SCREEN, scroll, true);
    }
    assert_eq!(menu.prefs.scroll, 50);
    menu.click_with(SCREEN, scroll, true);
    assert_eq!(menu.prefs.scroll, 200, "round to the fastest");
    assert!(menu.escape(), "Escape goes back");
    assert_eq!(menu.screen, Screen::Title);

    // From the pause menu it goes back to the pause menu.
    menu.screen = Screen::Paused;
    menu.click(SCREEN, centre(button(&menu, Action::Settings)));
    menu.click(SCREEN, centre(button(&menu, Action::Back)));
    assert_eq!(menu.screen, Screen::Paused);
    // The difficulty switch on the title steps round easy, normal and hard.
    menu.screen = Screen::Title;
    let mut seen = Vec::new();
    for _ in 0..3 {
        menu.click(SCREEN, centre(button(&menu, Action::Difficulty)));
        seen.push(menu.difficulty());
    }
    use classic_ai::Difficulty::*;
    assert_eq!(seen, [Hard, Easy, Normal]);
}

#[test]
fn the_keys_screen_rebinds_a_key_and_swaps_a_clash() {
    use classic_render::prefs::Bind;
    let mut menu = Menu::new("Generic");
    menu.screen = Screen::Settings;
    menu.click(SCREEN, centre(button(&menu, Action::Keys)));
    assert_eq!(menu.screen, Screen::Keys);
    let labels: Vec<String> = menu.layout(SCREEN).into_iter().map(|i| i.label).collect();
    println!("keys buttons {labels:?}");
    assert_eq!(labels[0], "SCROLL UP: W");
    assert_eq!(labels[4], "PAUSE: SPACE");
    // Every button fits on the screen, even this long one.
    assert!(menu.layout(SCREEN).iter().all(|i| i.rect.y >= 0.0 && i.rect.y + i.rect.h <= SCREEN.1));
    assert!(menu.layout((800.0, 480.0)).iter().all(|i| i.rect.y + i.rect.h <= 480.0), "and in a short window");

    assert!(!menu.key("KeyP"), "no key is wanted yet");
    menu.click(SCREEN, centre(button(&menu, Action::Bind(Bind::Pause))));
    assert_eq!(menu.layout(SCREEN)[4].label, "PAUSE: PRESS A KEY");
    assert!(menu.key("KeyP"));
    assert_eq!(menu.prefs.key(Bind::Pause), "KeyP");
    // Mute is on M; putting pause there moves mute to pause's old key.
    menu.click(SCREEN, centre(button(&menu, Action::Bind(Bind::Pause))));
    assert!(menu.key("KeyM"));
    assert_eq!((menu.prefs.key(Bind::Pause), menu.prefs.key(Bind::Mute)), ("KeyM", "KeyP"));
    // Escape gives up waiting and keeps the key.
    menu.click(SCREEN, centre(button(&menu, Action::Bind(Bind::Base))));
    assert!(menu.escape());
    assert_eq!(menu.screen, Screen::Keys, "the first Escape only stops waiting");
    assert_eq!(menu.prefs.key(Bind::Base), "KeyH");
    menu.click(SCREEN, centre(button(&menu, Action::ResetKeys)));
    assert_eq!(menu.prefs.keys, classic_render::prefs::Prefs::default().keys);
    assert!(menu.escape());
    assert_eq!(menu.screen, Screen::Settings);
}

#[test]
fn the_end_screen_leaves_room_for_the_score() {
    let mut menu = Menu::new("Generic");
    let g = game();
    menu.screen = Screen::Playing;
    menu.after_step(&g, 0);
    assert_eq!(menu.score.lines().len(), 2, "a line for each player");
    menu.screen = Screen::Over { won: true };
    let first = menu.layout(SCREEN)[0].rect.y;
    let mut bare = Menu::new("Generic");
    bare.screen = Screen::Over { won: true };
    assert!(first > bare.layout(SCREEN)[0].rect.y + 30.0, "the buttons move down under the table");
}
