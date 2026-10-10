//! The desktop player: a window onto a skirmish.
//!   cargo run --release --bin play -- [--setting generic] [--map maps/skirmish-01.txt] [--seed 1] [--player 0]
//!     [--ai 1 | --ai none] [--fog on | shroud | off] [--tech 1-8] [--start]
//!
//! It opens on the title screen, with the map waiting behind it: Start plays, and the opponents switch says whether
//! every other player is a computer opponent (`--ai` lists which ones, or says `none`), and the difficulty switch how
//! well they play (easy, normal or hard; `--difficulty` sets it). `--start` skips the title. When someone wins, or
//! you lose your last building, the end screen shows the score and offers another game or the title. `--tech` sets
//! the game's tech level, which limits what anyone may build and how far factories may be upgraded (8, everything,
//! when it is left out).
//!
//! The settings screen (from the title or the pause menu) sets the volume of each sound bus, the scroll speed and the
//! keys; they are kept between runs, in the user's settings folder or the browser's storage for the page. The pause
//! menu saves the game and loads it again, one save per setting pack; loading plays the game forward to the saved
//! tick and checks its state hash (`classic_render::save`).
//!
//! Arrow keys or WASD (or the mouse at a screen edge) scroll, the wheel zooms, a left click or drag selects your
//! units, and a right click sends them: onto an enemy to attack it, anywhere else to move there. A click on a
//! building or an enemy shows it on the selection card. Ctrl and a number key keeps the selected units as a group;
//! the number selects them again, and a second press centres the view on them. H centres on your base.
//!
//! A right click with infantry selected on an enemy building hurt badly enough sends them in to capture it (the
//! cursor shows `enter`), and with damaged vehicles selected on your own repair pad sends them to be mended. Z sells
//! the buildings you have selected, and C turns their repair on (or off, when they all have it on already); with
//! no building of yours selected, Z or C waits for a click on one instead (right click or Escape to stop waiting).
//!
//! The rail on the right builds: pick a factory's tab, left-click an item to queue one (shift: five), right-click to
//! cancel one with a refund; Tab and shift-Tab change tabs. When a building is ready, click it and then a spot on
//! the map; the ghost shows green where it fits. The minimap at the rail's foot moves the view (click or drag), and
//! a right click on it orders the selected units there. Escape puts the building back, then clears the selection,
//! then opens the pause menu (resume, restart, back to the title, quit). Space pauses without the menu, M mutes the
//! sound (or start with `--mute`). `--frames N` quits after N frames, for smoke tests.
//!
//! Fog of war is on when the pack turns it on (the generic pack does): the map starts black, units uncover it, and
//! enemies out of sight are hidden, their buildings shown as last seen. `--fog` overrides the pack: `shroud` keeps
//! what has been uncovered in view, as the original did, and `off` shows the whole map.
//!
//! The same program runs in the browser (`web/play/`, see the README), drawing with WebGPU or WebGL2 into the
//! page's canvas. There the options come from the page address instead (`?setting=generic&seed=3&ai=none&mute&start`), the
//! files are fetched from the server first, because the browser has no file system and can't wait for the GPU, and
//! sound starts with the first click or key press.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use classic_ai::{Ai, Difficulty};
use classic_render::art::{self, Art};
use classic_render::hud::{self, Button, Click, RAIL_W};
use classic_render::lines::Moment;
use classic_render::menu::{Action, Menu, Screen};
use classic_render::platform::{Files, Gpu, Instant, Mixer, Rect, SpriteBatch};
use classic_render::prefs::{Bind, Prefs};
use classic_render::save::{Save, SaveInfo, map_hash};
use classic_render::score::Score;
use classic_render::skin::{self, Mode, Pointer, Skin, SkinFiles};
use classic_render::sound::Cue;
use classic_render::store::Store;
use classic_render::{Camera, Hud, Listener, Scene, SoundBoard, View, feed, scene};
use classic_sim::map::TILE;
use classic_sim::units::TICKS_PER_SECOND;
use classic_sim::{CommandOrder, Game, GameOptions, Rules};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

/// The simulation's fixed rate.
const TICK: Duration = Duration::from_micros(1_000_000 / 15);
/// Pixels per second the camera scrolls at zoom 1.
const SCROLL: f32 = 600.0;
/// How close to a screen edge the mouse scrolls the view.
const EDGE: f32 = 8.0;

/// The number a digit key stands for, 0 to 9.
fn digit(code: KeyCode) -> Option<usize> {
    const KEYS: [KeyCode; 10] = [
        KeyCode::Digit0,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::Digit4,
        KeyCode::Digit5,
        KeyCode::Digit6,
        KeyCode::Digit7,
        KeyCode::Digit8,
        KeyCode::Digit9,
    ];
    KEYS.iter().position(|&k| k == code)
}

/// An option: `--name value` on the command line, or `?name=value` in the browser.
fn arg(name: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    return classic_render::platform::web::query(name);
    #[cfg(not(target_arch = "wasm32"))]
    {
        let args: Vec<String> = std::env::args().collect();
        args.iter().position(|a| a == &format!("--{name}")).and_then(|i| args.get(i + 1).cloned())
    }
}

/// A switch: `--name` on the command line, or `?name` in the browser.
fn flag(name: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    return classic_render::platform::web::query(name).is_some();
    #[cfg(not(target_arch = "wasm32"))]
    std::env::args().any(|a| a == format!("--{name}"))
}

/// A line for the person playing: the terminal on the desktop, the console in the browser.
fn say(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    classic_render::platform::web::log(msg);
    #[cfg(not(target_arch = "wasm32"))]
    println!("{msg}");
}

/// A window, its surface and the GPU that draws to it, once the GPU has opened.
type Opened = (Arc<Window>, wgpu::Surface<'static>, Gpu);

struct Running {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: Gpu,
    batch: SpriteBatch,
    art: Art,
    /// The faction ramp of each player the art was coloured for, in owner order.
    ramps: Vec<String>,
    skin: Skin,
    /// The mouse cursors made so far, by id, and the one showing.
    cursors: BTreeMap<&'static str, winit::window::CustomCursor>,
    cursor: &'static str,
}

struct App {
    game: Game,
    /// The title, pause and end screens.
    menu: Menu,
    /// The maps on offer, as (name, text); the menu picks one.
    maps: Vec<(String, String)>,
    /// The pack's own files, for its wording of the message feed, its lines and its theme.
    pack_files: Files,
    scene: Scene,
    hud: Hud,
    pack: classic_data::Pack,
    /// The art's files: the pack folder on the desktop, fetched copies in the browser.
    art_files: Vec<Files>,
    /// The UI skin's files: the pack's own, then the generic pack's.
    skin_files: Vec<Files>,
    /// In the browser the GPU opens in the background and lands here; see `resumed`.
    #[cfg(target_arch = "wasm32")]
    opened: std::rc::Rc<std::cell::RefCell<Option<Opened>>>,
    player: u32,
    /// The computer opponents, which order through the same command queue as the player.
    ais: Vec<Ai>,
    cam: Camera,
    run: Option<Running>,
    last: Instant,
    owed: Duration,
    paused: bool,
    keys: BTreeSet<KeyCode>,
    mouse: (f32, f32),
    /// Where a left-button drag started, in screen pixels.
    drag: Option<(f32, f32)>,
    /// The left button went down on the minimap: moving the mouse keeps moving the view.
    minimap_drag: bool,
    /// Z or C pressed with no own building selected: the next left click on one sells it or toggles its repair.
    mode: Option<Mode>,
    frames: u64,
    max_frames: Option<u64>,
    sound: SoundBoard,
    /// Shared with the sound card's thread, which pulls the mix from it.
    mixer: Arc<Mutex<Mixer>>,
    #[cfg(feature = "device")]
    speaker: Option<classic_render::platform::Speaker>,
    speaker_tried: bool,
    /// Where the settings and the saved game are kept.
    store: Store,
    /// The game's seed and the player's fog override (`--seed`, `--fog`), kept with a save.
    seed: i32,
    fog: Option<String>,
    /// The mixer's starting bus levels, which the volume settings scale.
    default_gain: [f32; 4],
}

/// The settings file's name in the store.
const PREFS: &str = "settings.txt";

impl App {
    /// A player for a game of `pack` on one of `maps` (name and text, the first unless the title screen picks
    /// another), with the pack's art from `art_files` (the pack's own first, over the generic pack's) and its sounds
    /// from `sound_files` (the generic pack's first, then the pack's own over them). The sound card opens separately, in `open_speaker`. It opens on the title screen, with
    /// the game waiting behind it, unless `start` is given.
    fn new(
        pack: classic_data::Pack,
        maps: Vec<(String, String)>,
        art_files: Vec<Files>,
        sound_files: &[Files],
        pack_files: Files,
        skin_files: Vec<Files>,
    ) -> App {
        let seed = arg("seed").and_then(|s| s.parse::<i32>().ok()).unwrap_or(1);
        let fog = arg("fog");
        let mut game = new_game(&pack, &maps[0].1, seed, fog.as_deref());
        let player = arg("player").and_then(|s| s.parse().ok()).unwrap_or(0);
        let mut mixer = Mixer::new(48_000);
        mixer.muted = flag("mute");
        let mut sound = SoundBoard::from_files(sound_files, player, seed as u64, &mut mixer);
        for w in &sound.warnings {
            say(&format!("sound: {w}"));
        }
        let faction = arg("faction").and_then(|s| s.parse().ok()).unwrap_or(0) % pack.factions.len().max(1);
        give_factions(&mut game, &pack, player, faction);
        let hud = Hud::new(&pack, &pack_files, &game, player, faction);
        for w in &hud.feed.warnings {
            say(&format!("messages and lines: {w}"));
        }
        for w in classic_render::theme::Theme::load(&pack_files).1 {
            say(&format!("theme: {w}"));
        }
        let mut menu = Menu::new(&pack.title);
        menu.theme = hud.theme.clone();
        menu.maps = maps.iter().map(|(name, _)| name.clone()).collect();
        menu.factions = pack.factions.iter().map(|f| f.name.clone()).collect();
        menu.faction = faction;
        menu.opponents = arg("ai").as_deref() != Some("none");
        menu.can_quit = cfg!(not(target_arch = "wasm32"));
        let store = Store::user();
        if let Some(text) = store.read(PREFS) {
            let (prefs, warnings) = Prefs::parse(&text);
            for w in warnings {
                say(&format!("settings ({}): {w}", store.describe()));
            }
            menu.prefs = prefs;
        }
        if let Some(d) = arg("difficulty") {
            match Difficulty::from_id(&d) {
                Some(d) => menu.prefs.difficulty = d,
                None => say(&format!("--difficulty {d}: use easy, normal or hard")),
            }
        }
        menu.has_save = store.read(&save_name(&pack)).is_some();
        menu.local = player;
        let default_gain = mixer.bus_gain;
        sound.set_levels(&mut mixer, menu.prefs.bus_gain(default_gain));
        if flag("start") {
            menu.screen = Screen::Playing;
        }
        let ais = opponents(&game, player, menu.opponents, menu.difficulty());
        App {
            game,
            menu,
            maps,
            pack_files,
            scene: Scene::for_player(player),
            hud,
            pack,
            art_files,
            skin_files,
            #[cfg(target_arch = "wasm32")]
            opened: Default::default(),
            player,
            ais,
            cam: Camera { x: 0.0, y: 0.0, zoom: 2.0 },
            run: None,
            last: Instant::now(),
            owed: Duration::ZERO,
            paused: false,
            keys: BTreeSet::new(),
            mouse: (-1.0e4, -1.0e4),
            drag: None,
            minimap_drag: false,
            mode: None,
            frames: 0,
            max_frames: arg("frames").and_then(|s| s.parse().ok()),
            sound,
            mixer: Arc::new(Mutex::new(mixer)),
            #[cfg(feature = "device")]
            speaker: None,
            speaker_tried: false,
            store,
            seed,
            fog,
            default_gain,
        }
    }

    /// Start a game on the map, faction and opponents the menu shows, and play it.
    fn restart(&mut self) {
        let mut game = new_game(&self.pack, &self.maps[self.menu.map].1, self.seed, self.fog.as_deref());
        give_factions(&mut game, &self.pack, self.player, self.menu.faction);
        let ais = opponents(&game, self.player, self.menu.opponents, self.menu.difficulty());
        self.begin(game, ais, Score::default());
    }

    /// Play `game` against `ais`, with the score counted so far: a new game, or a loaded one.
    fn begin(&mut self, game: Game, ais: Vec<Ai>, score: Score) {
        self.game = game;
        // A new faction or map can change who is drawn in which colours.
        let ramps = self.ramps();
        if let Some(run) = self.run.as_mut().filter(|r| r.ramps != ramps) {
            run.art = Art::from_files(&run.gpu, &mut run.batch, &self.art_files, &ramps).expect("the pack's art loads");
            run.ramps = ramps;
        }
        self.ais = ais;
        let scale = self.hud.scale;
        self.hud = Hud::new(&self.pack, &self.pack_files, &self.game, self.player, self.menu.faction);
        self.hud.scale = scale;
        self.scene = Scene::for_player(self.player);
        self.owed = Duration::ZERO;
        self.paused = false;
        self.centre_on_base();
        self.menu.score = score;
        self.menu.local = self.player;
        self.menu.show(Screen::Playing);
    }

    /// Save the game being played, over the last save.
    fn save_game(&mut self) -> Result<(), String> {
        let (map, text) = &self.maps[self.menu.map];
        let info = SaveInfo {
            setting: self.pack.id.clone(),
            map: map.clone(),
            map_hash: map_hash(text),
            seed: self.seed,
            fog: self.fog.clone().unwrap_or_else(|| "pack".into()),
            player: self.player,
            faction: self.menu.faction,
        };
        // Each opponent's difficulty is the preset its settings came from.
        let ais: Vec<(u32, Difficulty)> = self
            .ais
            .iter()
            .map(|a| (a.player, Difficulty::ALL.into_iter().find(|d| d.settings() == a.settings).unwrap_or_default()))
            .collect();
        let save = Save::of(&self.game, &ais, info);
        self.store.write(&save_name(&self.pack), &save.to_text(&self.game.rules))?;
        say(&format!("saved at tick {} in {}", save.tick, self.store.describe()));
        Ok(())
    }

    /// Load the saved game: start it again and play it forward to the tick it was saved on.
    fn load_game(&mut self) -> Result<(), String> {
        let text = self.store.read(&save_name(&self.pack)).ok_or("there is no saved game")?;
        let save = Save::parse(&text, &self.game.rules)?;
        if save.setting != self.pack.id {
            return Err(format!("the save is for the {} setting", save.setting));
        }
        let map = self
            .maps
            .iter()
            .position(|(name, text)| *name == save.map && map_hash(text) == save.map_hash)
            .ok_or_else(|| format!("the save's map, {}, isn't on offer", save.map))?;
        if save.faction >= self.pack.factions.len().max(1) {
            return Err("the save's faction isn't in this pack".into());
        }
        let fog = Some(save.fog.clone()).filter(|f| f != "pack");
        let fresh = new_game(&self.pack, &self.maps[map].1, save.seed, fog.as_deref());
        let mut score = Score::default();
        let started = Instant::now();
        let (game, ais) = save.replay(fresh, |g| score.after_step(g))?;
        say(&format!("loaded tick {} in {:.1} s", save.tick, (Instant::now() - started).as_secs_f32()));
        (self.seed, self.fog, self.player) = (save.seed, fog, save.player);
        self.menu.map = map;
        self.menu.faction = save.faction;
        self.menu.opponents = !ais.is_empty();
        self.begin(game, ais, score);
        Ok(())
    }

    /// Keep the settings and put the volumes into the mixer.
    fn keep_prefs(&mut self) {
        if let Ok(mut m) = self.mixer.lock() {
            self.sound.set_levels(&mut m, self.menu.prefs.bus_gain(self.default_gain));
        }
        if let Err(e) = self.store.write(PREFS, &self.menu.prefs.to_text()) {
            say(&format!("couldn't keep the settings: {e}"));
        }
    }

    /// Whether a key is held: the `bind`'s key, or `also` (an arrow key).
    fn held(&self, bind: Bind, also: KeyCode) -> bool {
        self.keys.contains(&also) || self.keys.iter().any(|k| key_name(*k) == self.menu.prefs.key(bind))
    }

    /// Do what a menu button asks.
    fn menu_action(&mut self, action: Action, event_loop: &ActiveEventLoop) {
        match action {
            Action::Start | Action::Restart => self.restart(),
            Action::Resume => self.menu.show(Screen::Playing),
            Action::ToTitle => self.menu.show(Screen::Title),
            Action::Quit => event_loop.exit(),
            Action::Save => {
                let notice = match self.save_game() {
                    Ok(()) => {
                        self.menu.has_save = true;
                        format!("SAVED AT {}", hud::clock_text(self.game.state.tick / TICKS_PER_SECOND))
                    }
                    Err(e) => {
                        say(&format!("couldn't save: {e}"));
                        "COULDN'T SAVE THE GAME".to_string()
                    }
                };
                self.menu.notice = Some(notice);
            }
            Action::Load => {
                if let Err(e) = self.load_game() {
                    say(&format!("couldn't load: {e}"));
                    self.menu.notice = Some(format!("COULDN'T LOAD: {}", e.to_uppercase()));
                }
            }
            Action::Difficulty | Action::Volume(_) | Action::Scroll | Action::ResetKeys => self.keep_prefs(),
            Action::Opponents | Action::Map | Action::Faction => {}
            Action::Settings | Action::Keys | Action::Bind(_) | Action::Back => {}
        }
        self.ui_sound("ui_select");
    }

    /// Open the sound card, once. No sound card (a server, a CI runner) just means a silent game. Browsers only let
    /// a page make sound after the player has clicked or pressed a key, so there it opens on the first one.
    fn open_speaker(&mut self) {
        if std::mem::replace(&mut self.speaker_tried, true) {
            return;
        }
        #[cfg(feature = "device")]
        match classic_render::platform::Speaker::open(self.mixer.clone()) {
            Ok(s) => {
                say(&format!("sound on {}", s.describe));
                self.speaker = Some(s);
            }
            Err(e) => say(&format!("playing without sound: {e}")),
        }
    }

    fn view(&self) -> View {
        let screen = self.run.as_ref().map_or((1280.0, 800.0), |r| (r.config.width as f32, r.config.height as f32));
        View { cam: self.cam, screen, tile: TILE as f32 * self.world_px() }
    }

    fn shift(&self) -> bool {
        self.keys.contains(&KeyCode::ShiftLeft) || self.keys.contains(&KeyCode::ShiftRight)
    }

    /// Give a click to the HUD first; returns whether it took it.
    fn hud_click(&mut self, button: Button) -> bool {
        let view = self.view();
        let shift = self.shift();
        let ctrl = self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight);
        let click = self.hud.click_with(&mut self.game, &view, self.mouse, button, shift, ctrl);
        let taken = click.taken();
        match click {
            Click::Centre { x, y } => {
                self.minimap_drag = true;
                self.centre_on(x, y);
            }
            Click::Order { x, y } => self.order_at(None, (x, y)),
            Click::Mode(mode) => self.building_key(mode),
            Click::Taken | Click::World => {}
        }
        taken
    }

    /// Centre the world's part of the screen on a map point, in tiles.
    fn centre_on(&mut self, x: f32, y: f32) {
        let view = self.view();
        let world_w = view.screen.0 - RAIL_W * self.hud.scale;
        self.cam.x = x * view.tile - world_w / 2.0 / self.cam.zoom;
        self.cam.y = y * view.tile - view.screen.1 / 2.0 / self.cam.zoom;
    }

    /// Z (sell) or C (repair): act on the own buildings selected, or with none, wait for a click on one. Repair
    /// goes on for any of them that is damaged and not yet repairing, else off for all of them.
    fn building_key(&mut self, mode: Mode) {
        let own: Vec<u32> = self
            .scene
            .selected
            .iter()
            .copied()
            .filter(|&id| {
                self.game
                    .state
                    .entity(id)
                    .is_some_and(|e| e.owner == self.player && self.game.rules.kind(e.kind).building)
            })
            .collect();
        if own.is_empty() {
            self.mode = if self.mode == Some(mode) { None } else { Some(mode) };
            return;
        }
        self.mode = None;
        self.building_order(mode, &own);
    }

    fn building_order(&mut self, mode: Mode, ids: &[u32]) {
        let order = match mode {
            Mode::Sell => CommandOrder::Sell,
            Mode::Repair => {
                let rules = &self.game.rules;
                let start = ids
                    .iter()
                    .filter_map(|&id| self.game.state.entity(id))
                    .any(|e| !e.repairing && e.selling == 0 && e.health < rules.kind(e.kind).max_health);
                CommandOrder::Repair { on: start }
            }
        };
        self.game.order(self.player, ids, order);
        self.ui_sound("ui_order");
    }

    /// A left click while waiting with Z or C: an own building takes the order; anything else just ends the wait.
    fn mode_click(&mut self, mode: Mode) {
        self.mode = None;
        let target = self.pick(self.mouse.0, self.mouse.1).filter(|&id| {
            self.game.state.entity(id).is_some_and(|e| e.owner == self.player && self.game.rules.kind(e.kind).building)
        });
        if let Some(id) = target {
            self.building_order(mode, &[id]);
        }
    }

    /// A right click goes to the HUD, else orders the selected units.
    fn right_click(&mut self) {
        if self.mode.take().is_some() {
            return;
        }
        if !self.hud_click(Button::Right) {
            let (wx, wy) = self.cam.to_world(self.mouse.0, self.mouse.1);
            let tile = TILE as f32 * self.world_px();
            let target = self.pick(self.mouse.0, self.mouse.1);
            self.order_at(target, ((wx / tile).floor() as i32, (wy / tile).floor() as i32));
        }
    }

    /// A number key: with ctrl held, keep the selection as that group; without, select the group, and centre on it
    /// when it was already selected.
    fn group_key(&mut self, n: usize) {
        if self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight) {
            self.scene.set_group(&self.game, self.player, n);
        } else if self.scene.recall_group(n) {
            if let Some((x, y)) = self.scene.selection_centre(&self.game) {
                self.centre_on(x, y);
            }
        } else if !self.scene.groups[n].is_empty() {
            self.ui_sound("ui_select");
            self.hud.feed.reply(&self.game, Moment::Select, &self.scene.selected);
            self.speak();
        }
    }

    /// H: centre the view on the player's base, the first building they own that builds other buildings (or any
    /// building when none does).
    fn centre_on_base(&mut self) {
        let rules = &self.game.rules;
        let makes_buildings = |k| rules.kinds.iter().any(|b| b.building && b.built_at == Some(k));
        let own = || self.game.state.entities.iter().filter(|e| e.owner == self.player && rules.kind(e.kind).building);
        let Some(e) = own().find(|e| makes_buildings(e.kind)).or_else(|| own().next()) else { return };
        let k = rules.kind(e.kind);
        let t = e.tile();
        let (x, y) = (t.x as f32 + k.width as f32 / 2.0, t.y as f32 + k.height as f32 / 2.0);
        self.centre_on(x, y);
    }

    fn toggle_mute(&mut self) {
        if let Ok(mut m) = self.mixer.lock() {
            m.muted = !m.muted;
            m.stop_all();
        }
    }

    fn hear(&self, cues: Vec<Cue>) {
        if let Ok(mut m) = self.mixer.lock() {
            for c in cues {
                m.play(c.sound);
            }
        }
    }

    /// Voice what the advisor and the units just said, in the local player's faction's voices if the pack has them.
    fn speak(&mut self) {
        let said = std::mem::take(&mut self.hud.feed.spoken);
        let Some(faction) = self.pack.factions.get(self.menu.faction).map(|f| f.id.clone()) else { return };
        let Ok(mut m) = self.mixer.lock() else { return };
        for s in &said {
            if let Some(c) = self.sound.speak(&faction, s, &m) {
                m.play(c.sound);
            }
        }
    }

    fn ui_sound(&mut self, id: &str) {
        let cue = self.sound.ui(id);
        self.hear(cue.into_iter().collect());
    }

    fn world_px(&self) -> f32 {
        self.run.as_ref().map_or(32.0, |r| r.art.tile) / TILE as f32
    }

    /// The entity under a screen point: a unit within half a tile of its centre, or a building whose footprint
    /// holds the point. Units win over buildings. Under fog, only what the player can see, or a building they keep
    /// a ghost of.
    fn pick(&self, sx: f32, sy: f32) -> Option<u32> {
        let (wx, wy) = self.cam.to_world(sx, sy);
        let px = self.world_px();
        let half = TILE as f32 * px / 2.0;
        let mut found = None;
        for e in &self.game.state.entities {
            let k = self.game.rules.kind(e.kind);
            if !self.game.known(self.player, e.id) {
                continue;
            }
            if k.building {
                let t = e.tile();
                let tile = TILE as f32 * px;
                let inside = wx >= t.x as f32 * tile
                    && wy >= t.y as f32 * tile
                    && wx < (t.x + k.width) as f32 * tile
                    && wy < (t.y + k.height) as f32 * tile;
                if inside && found.is_none() {
                    found = Some(e.id);
                }
            } else if self.game.visible(self.player, e.id)
                && (e.x as f32 * px - wx).abs() <= half
                // An aircraft is picked where it is drawn, above its shadow.
                && (e.y as f32 * px - scene::lift(&self.game, e) * 2.0 * half - wy).abs() <= half
            {
                return Some(e.id);
            }
        }
        found
    }

    fn select(&mut self, from: (f32, f32), to: (f32, f32)) {
        let mine = |id: u32| {
            self.game.state.entity(id).is_some_and(|e| e.owner == self.player && !self.game.rules.kind(e.kind).building)
        };
        if (from.0 - to.0).abs() < 4.0 && (from.1 - to.1).abs() < 4.0 {
            // One click takes anything, so the card can show a building or an enemy; only own units take orders.
            self.scene.selected = self.pick(to.0, to.1).into_iter().collect();
            if !self.scene.selected.is_empty() {
                self.ui_sound("ui_select");
                self.hud.feed.reply(&self.game, Moment::Select, &self.scene.selected);
                self.speak();
            }
            return;
        }
        let (a, b) = (self.cam.to_world(from.0, from.1), self.cam.to_world(to.0, to.1));
        let (x0, x1) = (a.0.min(b.0), a.0.max(b.0));
        let (y0, y1) = (a.1.min(b.1), a.1.max(b.1));
        let px = self.world_px();
        self.scene.selected = self
            .game
            .state
            .entities
            .iter()
            .filter(|e| mine(e.id))
            .filter(|e| {
                let (x, y) = (e.x as f32 * px, e.y as f32 * px);
                x >= x0 && x <= x1 && y >= y0 && y <= y1
            })
            .map(|e| e.id)
            .collect();
        if !self.scene.selected.is_empty() {
            self.ui_sound("ui_select");
            self.hud.feed.reply(&self.game, Moment::Select, &self.scene.selected);
            self.speak();
        }
    }

    /// The selected units' default order: attack `target` if it is an enemy, otherwise move to `tile`.
    fn order_at(&mut self, target: Option<u32>, (x, y): (i32, i32)) {
        let ids: Vec<u32> = self
            .scene
            .selected
            .iter()
            .copied()
            .filter(|&id| {
                self.game
                    .state
                    .entity(id)
                    .is_some_and(|e| e.owner == self.player && !self.game.rules.kind(e.kind).building)
            })
            .collect();
        if ids.is_empty() {
            return;
        }
        // Capturers into a building that can be taken, damaged vehicles to a repair pad, a unit that deploys clicked
        // itself deploys; the rest as usual.
        let (special, ids) = skin::special_orders(&self.game, self.player, &ids, target);
        for (units, order) in &special {
            self.game.order(self.player, units, *order);
            self.hud.feed.reply(&self.game, Moment::Move, units);
        }
        if ids.is_empty() {
            self.ui_sound("ui_order");
            self.speak();
            return;
        }
        let enemy = target.filter(|&id| self.game.state.entity(id).is_some_and(|e| e.owner != self.player));
        let (order, moment) = match enemy {
            Some(target) => (CommandOrder::Attack { target }, Moment::Attack),
            // The units still go as near as they can; they only say so.
            None if !feed::reachable(&self.game, &ids, (x, y)) => (CommandOrder::Move { x, y }, Moment::Cant),
            None => (CommandOrder::Move { x, y }, Moment::Move),
        };
        self.game.order(self.player, &ids, order);
        self.ui_sound("ui_order");
        self.hud.feed.reply(&self.game, moment, &ids);
        self.speak();
    }

    /// Advance the game by the ticks owed since the last frame, at most a few at once so a stall doesn't snowball.
    fn advance(&mut self) -> f32 {
        if let Ok(mut m) = self.mixer.lock() {
            self.sound.duck(&mut m);
        }
        let now = Instant::now();
        let dt = now - self.last;
        self.last = now;
        if self.paused || !self.menu.playing() {
            return 1.0;
        }
        self.owed = (self.owed + dt).min(TICK * 5);
        let screen = self.run.as_ref().map_or((1280.0, 800.0), |r| (r.config.width as f32, r.config.height as f32));
        let listener = Listener::from_camera(&self.cam, screen, self.world_px());
        while self.owed >= TICK {
            self.owed -= TICK;
            self.scene.before_step(&self.game);
            for ai in &mut self.ais {
                ai.tick(&mut self.game);
            }
            self.game.step(1);
            self.scene.after_step(&self.game);
            self.hud.after_step(&self.game);
            self.speak();
            let cues = self.sound.after_step(&self.game, &listener);
            self.hear(cues);
            self.menu.after_step(&self.game, self.player);
            if let Screen::Over { won } = self.menu.screen {
                self.hud.feed.over(&self.game, won);
                self.speak();
            }
            if !self.menu.playing() {
                self.owed = Duration::ZERO;
                break;
            }
        }
        // Events have been turned into effects; don't let them pile up.
        if self.game.events.len() > 10_000 {
            self.game.events.clear();
        }
        self.owed.as_secs_f32() / TICK.as_secs_f32()
    }

    fn scroll(&mut self, dt: f32, w: f32, h: f32) {
        let (mut dx, mut dy) = (0.0, 0.0);
        if self.held(Bind::ScrollLeft, KeyCode::ArrowLeft) || self.mouse.0 < EDGE {
            dx -= 1.0;
        }
        if self.held(Bind::ScrollRight, KeyCode::ArrowRight) || self.mouse.0 > w - EDGE {
            dx += 1.0;
        }
        if self.held(Bind::ScrollUp, KeyCode::ArrowUp) || self.mouse.1 < EDGE {
            dy -= 1.0;
        }
        if self.held(Bind::ScrollDown, KeyCode::ArrowDown) || self.mouse.1 > h - EDGE {
            dy += 1.0;
        }
        let speed = SCROLL * self.menu.prefs.scroll as f32 / 100.0 * dt / self.cam.zoom;
        let tile = TILE as f32 * self.world_px();
        let (mw, mh) = (self.game.map.width as f32 * tile, self.game.map.height as f32 * tile);
        let (vw, vh) = (w / self.cam.zoom, h / self.cam.zoom);
        // The rail covers the right of the screen, so the map may scroll out from under it.
        let right = mw - vw + RAIL_W * self.hud.scale / self.cam.zoom;
        self.cam.x = (self.cam.x + dx * speed).clamp(0.0_f32.min(right), right.max(0.0));
        self.cam.y = (self.cam.y + dy * speed).clamp(0.0_f32.min(mh - vh), (mh - vh).max(0.0));
    }

    fn title(&self) -> String {
        let p = self.game.state.players.iter().find(|p| p.id == self.player);
        let power = self.game.power(self.player);
        format!(
            "{} | tick {} | credits {} | power {}/{}{}",
            self.pack.title,
            self.game.state.tick,
            p.map_or(0, |p| p.credits),
            power.supply,
            power.demand,
            match (classic_ai::winner(&self.game), self.paused, self.menu.screen) {
                (Some(w), _, _) if w == self.player => " | you won".to_string(),
                (Some(w), _, _) => format!(" | player {w} won"),
                (None, _, Screen::Over { .. }) => " | you lost".to_string(),
                (None, true, _) | (None, _, Screen::Paused) => " | paused".to_string(),
                (None, _, Screen::Title) => " | title".to_string(),
                (None, false, _) => String::new(),
            }
        )
    }

    fn redraw(&mut self) {
        let alpha = self.advance();
        let title = self.title();
        let Some(run) = &self.run else { return };
        let (w, h) = (run.config.width as f32, run.config.height as f32);
        let frame_dt = 1.0 / 60.0;
        self.scroll(frame_dt, w, h);
        let Some(run) = &mut self.run else { return };
        let texture = match run.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                run.surface.configure(&run.gpu.device, &run.config);
                return;
            }
            _ => return,
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.scene.draw(&mut run.batch, &run.art, &self.game, &self.cam, (w, h), alpha);
        let world = View { cam: self.cam, screen: (w, h), tile: run.art.tile };
        self.hud.mode = self.mode;
        self.hud.draw(&mut run.batch, &run.art, &run.skin, &self.game, &world, self.mouse, &self.scene.selected);
        self.menu.draw(&mut run.batch, &run.skin, &self.game, (w, h), self.mouse);
        if let Some(from) = self.drag {
            let (x0, y0) = (from.0.min(self.mouse.0), from.1.min(self.mouse.1));
            let r = Rect::new(x0, y0, (from.0 - self.mouse.0).abs(), (from.1 - self.mouse.1).abs());
            run.batch.outline(r, 1.0, [240, 240, 240, 255]);
        }
        run.batch.draw(&run.gpu, &view, run.config.width, run.config.height, [0, 0, 0, 255]);
        run.gpu.queue.present(texture);
        run.window.set_title(&title);
        #[cfg(target_arch = "wasm32")]
        classic_render::platform::web::set_title(&title);
        self.frames += 1;
    }

    /// Show the cursor for what a click would do here (`plans/rts/ui.md` section 9), as the window's own cursor so
    /// it moves at the speed of the mouse. Each cursor is made once, the first time it is wanted.
    fn show_cursor(&mut self, event_loop: &ActiveEventLoop) {
        let Some(run) = &self.run else { return };
        let (w, h) = (run.config.width as f32, run.config.height as f32);
        let id = if self.menu.playing() {
            let view = View { cam: self.cam, screen: (w, h), tile: run.art.tile };
            let (mx, my) = self.mouse;
            let axis = |v: f32, max: f32| {
                if v < EDGE {
                    -1
                } else if v > max - EDGE {
                    1
                } else {
                    0
                }
            };
            let p = Pointer {
                over_hud: self.hud.over(&self.game, (w, h), mx, my),
                placing: self.hud.ghost(&self.game, &view, mx, my).map(|g| g.ok),
                edge: (axis(mx, w), axis(my, h)),
                hovered: self.pick(mx, my),
                tile: view.tile_at(mx, my),
                mode: self.mode,
            };
            skin::choose_cursor(&self.game, self.player, &self.scene.selected, &p)
        } else {
            "default"
        };
        let Some(run) = &mut self.run else { return };
        if run.cursor == id {
            return;
        }
        if !run.cursors.contains_key(id) {
            let Some((img, (hx, hy))) = run.skin.cursor(id, self.hud.scale) else { return };
            match winit::window::CustomCursor::from_rgba(
                img.rgba.clone(),
                img.w as u16,
                img.h as u16,
                hx as u16,
                hy as u16,
            ) {
                Ok(source) => {
                    run.cursors.insert(id, event_loop.create_custom_cursor(source));
                }
                Err(e) => {
                    say(&format!("cursor {id}: {e}"));
                    return;
                }
            }
        }
        run.window.set_cursor(run.cursors[id].clone());
        run.cursor = id;
    }

    /// The faction ramp of each player in this game, with the local player on the faction the menu shows.
    fn ramps(&self) -> Vec<String> {
        art::faction_ramps(&self.pack, self.game.state.players.len(), self.player as usize, self.menu.faction)
    }

    /// Finish setting up once the GPU is open: the surface, the sprite batcher and the art.
    fn start(&mut self, (window, surface, gpu): Opened) {
        say(&format!("drawing with {}", gpu.describe()));
        // In the browser the page's loading message goes away.
        #[cfg(target_arch = "wasm32")]
        classic_render::platform::web::status("");
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .expect("the surface works with this adapter");
        // A plain (not sRGB) format, so the art's pixels reach the screen unchanged.
        let caps = surface.get_capabilities(&gpu.adapter);
        if let Some(&f) = caps.formats.iter().find(|f| !f.is_srgb()) {
            config.format = f;
        }
        surface.configure(&gpu.device, &config);
        let mut batch = SpriteBatch::new(&gpu, config.format);
        let ramps = self.ramps();
        let art = Art::from_files(&gpu, &mut batch, &self.art_files, &ramps).expect("the pack's art loads");
        let packs: Vec<&Files> = self.skin_files.iter().collect();
        let skin = Skin::load(&gpu, &mut batch, &packs).unwrap_or_else(|e| {
            say(&format!("skin: {e}"));
            Skin::new(&gpu, &mut batch, SkinFiles::default())
        });
        self.hud.scale = window.scale_factor().round().max(1.0) as f32;
        self.menu.scale = self.hud.scale;
        // Start over the player's own base.
        if let Some(e) = self.game.state.entities.iter().find(|e| e.owner == self.player) {
            let t = e.tile();
            self.cam.x = (t.x as f32 * art.tile - 320.0).max(0.0);
            self.cam.y = (t.y as f32 * art.tile - 240.0).max(0.0);
        }
        let cursors = BTreeMap::new();
        self.run = Some(Running { window, surface, config, gpu, batch, art, ramps, skin, cursors, cursor: "" });
        self.last = Instant::now();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.run.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title(self.title());
        #[cfg(not(target_arch = "wasm32"))]
        {
            let window = Arc::new(
                event_loop
                    .create_window(attrs.with_inner_size(winit::dpi::PhysicalSize::new(1280, 800)))
                    .expect("a window"),
            );
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
                event_loop.owned_display_handle(),
            )));
            let surface = instance.create_surface(window.clone()).expect("a surface for the window");
            let gpu =
                pollster::block_on(Gpu::open(instance, Some(&surface))).expect("a GPU that can draw to the window");
            self.start((window, surface, gpu));
        }
        // The browser can't wait for the GPU, so it opens in the background and the first redraw after it lands
        // finishes the set-up. The canvas is the page's `#game`, sized by the page's style.
        #[cfg(target_arch = "wasm32")]
        {
            use classic_render::platform::web;
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = web::canvas("game");
            let window = Arc::new(event_loop.create_window(attrs.with_canvas(canvas)).expect("a canvas"));
            let opened = self.opened.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let instance = Gpu::browser_instance().await;
                let surface = instance.create_surface(window.clone()).expect("a surface for the canvas");
                match Gpu::open(instance, Some(&surface)).await {
                    Ok(gpu) => {
                        window.request_redraw();
                        *opened.borrow_mut() = Some((window, surface, gpu));
                    }
                    Err(e) => web::status(&format!("this browser can't draw the game: {e}")),
                }
            });
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Browsers let a page make sound only after a click or key press, so the sound card opens on the first one.
        #[cfg(target_arch = "wasm32")]
        if matches!(
            event,
            WindowEvent::KeyboardInput { .. } | WindowEvent::MouseInput { state: ElementState::Pressed, .. }
        ) {
            self.open_speaker();
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(run) = &mut self.run
                    && size.width > 0
                    && size.height > 0
                {
                    run.config.width = size.width;
                    run.config.height = size.height;
                    run.surface.configure(&run.gpu.device, &run.config);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                if event.state == ElementState::Pressed {
                    self.keys.insert(code);
                    if self.menu.waiting.is_some() && !event.repeat {
                        // The keys screen waits for a new key.
                        if self.menu.key(&key_name(code)) {
                            self.keep_prefs();
                            self.ui_sound("ui_select");
                        }
                        return;
                    }
                    let bind = self.menu.prefs.bound(&key_name(code));
                    if !self.menu.playing() {
                        // On a menu only Escape (back, or quit from the title) and mute work.
                        match code {
                            KeyCode::Escape if !event.repeat && !self.menu.escape() && self.menu.can_quit => {
                                if self.menu.screen == Screen::Title {
                                    event_loop.exit();
                                }
                            }
                            _ if !event.repeat && bind == Some(Bind::Mute) => self.toggle_mute(),
                            _ => {}
                        }
                        return;
                    }
                    match code {
                        KeyCode::Escape if !event.repeat => {
                            // Escape ends a sell or repair wait, puts back a building, then clears the selection,
                            // then opens the pause menu.
                            if self.mode.take().is_none() {
                                if !self.hud.cancel() && self.scene.selected.is_empty() {
                                    self.menu.escape();
                                }
                                self.scene.selected.clear();
                            }
                        }
                        // Ctrl+X: the selected units that can blow themselves up start their countdown.
                        KeyCode::KeyX
                            if !event.repeat
                                && (self.keys.contains(&KeyCode::ControlLeft)
                                    || self.keys.contains(&KeyCode::ControlRight)) =>
                        {
                            let ids = self.scene.selected.clone();
                            self.game.order(self.player, &ids, CommandOrder::SelfDestruct);
                        }
                        _ if event.repeat => {}
                        _ if digit(code).is_some() => self.group_key(digit(code).unwrap_or(0)),
                        _ => match bind {
                            Some(Bind::Pause) => self.paused = !self.paused,
                            Some(Bind::NextTab) => self.hud.next_tab(&self.game, self.shift()),
                            Some(Bind::Base) => self.centre_on_base(),
                            Some(Bind::Mute) => self.toggle_mute(),
                            Some(Bind::Sell) => self.building_key(Mode::Sell),
                            Some(Bind::Repair) => self.building_key(Mode::Repair),
                            // The first selected factory becomes the one of its kind that takes orders.
                            Some(Bind::Primary) => {
                                let ids = self.scene.selected.clone();
                                self.game.order(self.player, &ids, CommandOrder::Primary);
                            }
                            // F (a fixed key, unless rebound to another action): aim the charged palace power
                            // (or put it back).
                            None if code == KeyCode::KeyF => self.hud.aim(&self.game),
                            _ => {}
                        },
                    }
                } else {
                    self.keys.remove(&code);
                }
            }
            WindowEvent::CursorMoved { position: PhysicalPosition { x, y }, .. } => {
                self.mouse = (x as f32, y as f32);
                if self.minimap_drag {
                    let l = self.hud.layout(&self.game, self.view().screen);
                    // Follow the mouse along the minimap's edge when it leaves it.
                    let (mx, my) = (
                        self.mouse.0.clamp(l.minimap.x, l.minimap.x + l.minimap.w - 0.01),
                        self.mouse.1.clamp(l.minimap.y, l.minimap.y + l.minimap.h - 0.01),
                    );
                    if let Some((tx, ty)) = self.hud.minimap_point(&l, &self.game, mx, my) {
                        self.centre_on(tx, ty);
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => self.mouse = (-1.0e4, -1.0e4),
            WindowEvent::MouseInput { state, button, .. } => match (button, state) {
                (MouseButton::Left | MouseButton::Right, ElementState::Pressed) if !self.menu.playing() => {
                    let screen = self.view().screen;
                    if let Some(action) = self.menu.click_with(screen, self.mouse, button == MouseButton::Right) {
                        self.menu_action(action, event_loop);
                    }
                }
                (_, _) if !self.menu.playing() => {}
                (MouseButton::Left, ElementState::Pressed) => {
                    if let Some(mode) = self.mode
                        && !self.hud.over(&self.game, self.view().screen, self.mouse.0, self.mouse.1)
                    {
                        self.mode_click(mode);
                    } else if !self.hud_click(Button::Left) {
                        self.drag = Some(self.mouse);
                    }
                }
                (MouseButton::Left, ElementState::Released) => {
                    self.minimap_drag = false;
                    if let Some(from) = self.drag.take() {
                        self.select(from, self.mouse);
                    }
                }
                (MouseButton::Right, ElementState::Pressed) => self.right_click(),
                _ => {}
            },
            WindowEvent::MouseWheel { delta, .. } => {
                let up = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y > 0.0,
                    MouseScrollDelta::PixelDelta(p) => p.y > 0.0,
                };
                if !self.menu.playing() {
                    return;
                }
                if self.hud.over(&self.game, self.view().screen, self.mouse.0, self.mouse.1) {
                    self.hud.wheel(up);
                    return;
                }
                // Whole-number zooms keep every art pixel the same size; zoom about the mouse.
                let zoom = if up { (self.cam.zoom + 1.0).min(4.0) } else { (self.cam.zoom - 1.0).max(1.0) };
                let (wx, wy) = self.cam.to_world(self.mouse.0, self.mouse.1);
                self.cam.zoom = zoom;
                self.cam.x = wx - self.mouse.0 / zoom;
                self.cam.y = wy - self.mouse.1 / zoom;
            }
            WindowEvent::RedrawRequested => {
                #[cfg(target_arch = "wasm32")]
                if self.run.is_none() {
                    let opened = self.opened.borrow_mut().take();
                    match opened {
                        Some(o) => self.start(o),
                        None => return,
                    }
                }
                self.redraw();
                // Every frame, as the selection and what is under the mouse change without the mouse moving.
                self.show_cursor(event_loop);
                if self.max_frames.is_some_and(|m| self.frames >= m) {
                    event_loop.exit();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(run) = &self.run {
            run.window.request_redraw();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use classic_tools::setting;
    let setting_name = arg("setting").or_else(|| std::env::var("SETTING").ok()).unwrap_or_else(|| "generic".into());
    let pack = setting::load(&setting_name).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    // `--map` plays that one map; else the pack's own maps, else the engine's.
    let paths: Vec<std::path::PathBuf> = match arg("map") {
        Some(m) => vec![m.into()],
        None if !pack.maps.is_empty() => pack.maps.iter().map(|m| pack.dir.join(m)).collect(),
        None => vec![setting::root().join(MAP)],
    };
    let maps = paths
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            (classic_render::menu::map_name(&p.display().to_string(), &text), text)
        })
        .collect();
    let art_files = art::art_dirs(&pack).into_iter().map(Files::Dir).collect();
    let generic = setting::root().join("settings/generic");
    let mut sound_files = vec![Files::Dir(generic.clone())];
    if pack.dir.canonicalize().ok() != generic.canonicalize().ok() {
        sound_files.push(Files::Dir(pack.dir.clone()));
    }
    let pack_files = Files::Dir(pack.dir.clone());
    let skin_files = vec![Files::Dir(pack.dir.clone()), Files::Dir(generic)];
    let mut app = App::new(pack, maps, art_files, &sound_files, pack_files, skin_files);
    app.open_speaker();
    let event_loop = EventLoop::new().expect("an event loop (is there a display?)");
    event_loop.run_app(&mut app).expect("the event loop runs");
    let ai_orders =
        app.game.command_log().iter().filter(|c| app.ais.iter().any(|a| a.player == c.command.player)).count();
    println!(
        "quit at tick {} after {} frames, {ai_orders} computer orders, hash {}",
        app.game.state.tick,
        app.frames,
        app.game.hash()
    );
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use classic_render::platform::web;
    use winit::platform::web::EventLoopExtWebSys;
    web::report_panics();
    web::status("loading");
    wasm_bindgen_futures::spawn_local(async {
        let setting_name = arg("setting").unwrap_or_else(|| "generic".into());
        let map = arg("map");
        let loaded = match classic_render::web::load(&setting_name, map.as_deref().unwrap_or(MAP), map.is_some()).await
        {
            Ok(l) => l,
            Err(e) => return web::status(&e),
        };
        let app = App::new(loaded.pack, loaded.maps, loaded.art, &loaded.sounds, loaded.pack_files, loaded.skin);
        EventLoop::new().expect("an event loop").spawn_app(app);
    });
}

/// The map played when none is given, from the repository root.
const MAP: &str = "maps/skirmish-01.txt";

/// The name a key is kept under in the settings: winit's own name for it.
fn key_name(code: KeyCode) -> String {
    format!("{code:?}")
}

/// The name of a pack's saved game in the store.
fn save_name(pack: &classic_data::Pack) -> String {
    format!("save-{}.txt", pack.id)
}

/// The computer opponents at `difficulty`: none when `on` is false, else the players `--ai` lists, else every player
/// but `player`.
fn opponents(game: &Game, player: u32, on: bool, difficulty: Difficulty) -> Vec<Ai> {
    match arg("ai").as_deref() {
        _ if !on => Vec::new(),
        Some("none") | None => game
            .state
            .players
            .iter()
            .map(|p| p.id)
            .filter(|&p| p != player)
            .map(|p| Ai::new(p, difficulty.settings()))
            .collect(),
        Some(list) => list
            .split(',')
            .map(|p| Ai::new(p.trim().parse().expect("a player number"), difficulty.settings()))
            .collect(),
    }
}

/// Tell the game which of the pack's factions each player has, the local one on `chosen`, as the colours are dealt,
/// so each builds its own faction's specials.
fn give_factions(game: &mut Game, pack: &classic_data::Pack, local: u32, chosen: usize) {
    let players = game.state.players.len();
    let ids: Vec<&str> = art::player_factions(pack.factions.len(), players, local as usize, chosen)
        .into_iter()
        .filter_map(|f| pack.factions.get(f).map(|f| f.id.as_str()))
        .collect();
    game.set_factions(&ids);
}

/// A game of `pack` on `map`. `fog` (`--fog`) is `on`, `shroud` (shroud only, nothing hidden once explored) or `off`
/// to override the pack's fog of war.
fn new_game(pack: &classic_data::Pack, map: &str, seed: i32, fog: Option<&str>) -> Game {
    let mut table = pack.rules.clone();
    if let Some(fog) = fog
        && let Some(m) = table.modules.get_mut("fog")
    {
        let (on, hide) = match fog {
            "off" => (0, 1),
            "shroud" => (1, 0),
            _ => (1, 1),
        };
        for (name, value) in [("on", on), ("hide", hide)] {
            if let Some(n) = m.numbers.get_mut(name) {
                n.value = value;
            }
        }
    }
    let rules = Rules::from_table(&table).expect("pack rules match the simulation");
    let mut game = Game::new(GameOptions { map, seed, players: None, rules: Some(&rules) }).expect("valid map");
    game.set_tech_level(arg("tech").and_then(|t| t.parse().ok()));
    game
}
