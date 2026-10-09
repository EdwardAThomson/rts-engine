//! Sound, with no sound card: the sound board turns a real game's events into cues, the mixer renders them into a
//! buffer, and the game never notices. Each test prints what it heard.

use std::collections::BTreeMap;

use classic_render::platform::{Bus, Mixer};
use classic_render::sound::{Cue, EVENT_NAMES, Tables};
use classic_render::{Listener, SoundBoard};
use classic_sim::{Game, GameOptions, Rules};
use classic_tools::setting;

const MAP: &str = include_str!("../../../maps/test-01.txt");

fn game() -> Game {
    let pack = setting::load("generic").unwrap();
    let rules = Rules::from_table(&pack.rules).unwrap();
    Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap()
}

fn board(mixer: &mut Mixer) -> SoundBoard {
    let generic = setting::root().join("settings/generic");
    SoundBoard::load(&generic, &generic, 0, 1, mixer)
}

/// The whole test map in view.
fn everything() -> Listener {
    Listener { x0: 0.0, y0: 0.0, x1: 64.0 * 256.0, y1: 64.0 * 256.0 }
}

/// Two lines of tanks facing each other.
fn battle(game: &mut Game, per_side: i32) {
    let tank = game.kind("battle_tank").unwrap();
    for i in 0..per_side {
        game.spawn(tank, 0, 10 + i % 2, 6 + i / 2);
        game.spawn(tank, 1, 16 - i % 2, 6 + i / 2);
    }
}

#[test]
fn every_sound_the_engine_plays_has_a_generic_file() {
    let mut mixer = Mixer::new(48_000);
    let b = board(&mut mixer);
    assert!(b.warnings.is_empty(), "{:?}", b.warnings);
    for d in &b.tables.defs {
        assert!(!d.clips.is_empty(), "{} has no file in the generic pack", d.id);
        for &c in &d.clips {
            let clip = mixer.clip(c);
            assert!(clip.seconds() > 0.02 && clip.seconds() < 2.5, "{}: {} s", d.id, clip.seconds());
        }
    }
    let t = Tables::builtin();
    println!("{} sound ids, {} played by events, {} events silent", t.defs.len(), t.played_ids().len(), t.silent.len());
    assert!(t.silent.len() < EVENT_NAMES.len());
}

#[test]
fn a_battle_sounds_like_one_and_the_game_never_notices() {
    let mut heard = game();
    let mut quiet = game();
    battle(&mut heard, 4);
    battle(&mut quiet, 4);
    let mut mixer = Mixer::new(48_000);
    let mut b = board(&mut mixer);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut buf = vec![0.0f32; 2 * 48_000 / 15];
    let (mut loud, mut limited, mut samples) = (0.0f32, 0usize, 0usize);
    for _ in 0..300 {
        heard.step(1);
        quiet.step(1);
        for Cue { id, sound } in b.after_step(&heard, &everything()) {
            *counts.entry(id).or_default() += 1;
            mixer.play(sound);
        }
        mixer.render(&mut buf, 2);
        loud = buf.iter().fold(loud, |m, s| m.max(s.abs()));
        limited += buf.iter().filter(|s| s.abs() > 0.8).count();
        samples += buf.len();
    }
    let limited = limited as f32 * 100.0 / samples as f32;
    println!("heard {counts:?}, loudest sample {loud:.2}, {limited:.2}% limited, hash {}", heard.hash());
    assert_eq!(heard.hash(), quiet.hash(), "listening changed the game");
    for id in ["sfx_cannon", "sfx_impact", "sfx_explode_small"] {
        assert!(counts.contains_key(id), "no {id}");
    }
    assert!(loud > 0.05 && loud <= 1.0);
    // A fight this size shouldn't lean on the limiter.
    assert!(limited < 1.0);
}

#[test]
fn a_big_battle_stays_within_the_voice_limits() {
    let mut g = game();
    battle(&mut g, 20);
    let mut mixer = Mixer::new(48_000);
    let mut b = board(&mut mixer);
    let cannon = b.tables.defs.iter().position(|d| d.id == "sfx_cannon").unwrap() as u32;
    let (mut most, mut most_cannon, mut cues) = (0, 0, 0);
    let mut buf = vec![0.0f32; 2 * 48_000 / 15];
    for _ in 0..200 {
        g.step(1);
        for c in b.after_step(&g, &everything()) {
            cues += 1;
            mixer.play(c.sound);
            most = most.max(mixer.playing());
            most_cannon = most_cannon.max(mixer.playing_key(cannon));
        }
        mixer.render(&mut buf, 2);
    }
    println!("{cues} cues; at most {most} voices, {most_cannon} cannons at once");
    assert!(cues > 40);
    assert!(most <= mixer.max_voices && most_cannon <= 3);
}

#[test]
fn far_away_fights_are_quiet_and_off_to_the_side() {
    let mut g = game();
    battle(&mut g, 2);
    let mut mixer = Mixer::new(48_000);
    let mut b = board(&mut mixer);
    // A view a little to the left of the fight, then one far away.
    let near = Listener { x0: 0.0, y0: 0.0, x1: 8.0 * 256.0, y1: 12.0 * 256.0 };
    let far = Listener { x0: 40.0 * 256.0, y0: 40.0 * 256.0, x1: 48.0 * 256.0, y1: 46.0 * 256.0 };
    let (mut near_cues, mut far_cues) = (Vec::new(), Vec::new());
    for t in 0..200 {
        g.step(1);
        let l = if t % 2 == 0 { &near } else { &far };
        let cues = b.after_step(&g, l);
        if t % 2 == 0 { near_cues.extend(cues) } else { far_cues.extend(cues) }
    }
    let sfx = |c: &&Cue| c.sound.bus == Bus::Sfx;
    let near_sfx: Vec<_> = near_cues.iter().filter(sfx).collect();
    println!("near: {} sfx cues, far: {} sfx cues", near_sfx.len(), far_cues.iter().filter(sfx).count());
    assert!(!near_sfx.is_empty());
    assert!(near_sfx.iter().all(|c| c.sound.pan > 0.0), "the fight is to the right of the view");
    assert!(far_cues.iter().filter(sfx).count() == 0, "too far to hear");
}

#[test]
fn only_the_local_player_hears_their_base() {
    let mut g = game();
    let mut mixer = Mixer::new(48_000);
    let mut ours = board(&mut mixer);
    let generic = setting::root().join("settings/generic");
    let mut theirs = SoundBoard::load(&generic, &generic, 1, 1, &mut mixer);
    // Player 0's harvester delivers within the first minutes on the test map.
    let (mut a, mut b) = (0, 0);
    for _ in 0..3000 {
        g.step(1);
        a += ours.after_step(&g, &everything()).iter().filter(|c| c.id == "ui_credits").count();
        b += theirs.after_step(&g, &everything()).iter().filter(|c| c.id == "ui_credits").count();
    }
    let delivered = |p| {
        g.events.iter().filter(|e| matches!(e, classic_sim::Event::Delivered { player, .. } if *player == p)).count()
    };
    println!("player 0 delivered {} times, heard {a}; player 1 delivered {}, heard {b}", delivered(0), delivered(1));
    assert_eq!(a, delivered(0));
    assert_eq!(b, delivered(1));
}

#[test]
fn interface_sounds_play_by_id() {
    let mut mixer = Mixer::new(48_000);
    let mut b = board(&mut mixer);
    for id in ["ui_select", "ui_order", "ui_error"] {
        let c = b.ui(id).unwrap();
        assert_eq!(c.sound.bus, Bus::Ui);
        assert_eq!(c.sound.pan, 0.0);
    }
    assert!(b.ui("no_such_sound").is_none());
}

#[test]
fn sounds_fetched_into_memory_load_as_they_do_from_the_pack_folder() {
    // The browser build fetches the files `audio/sounds.json` names, then loads them from memory.
    let generic = setting::root().join("settings/generic");
    let index = std::fs::read_to_string(generic.join(classic_render::sound::SOUND_INDEX)).unwrap();
    let mut files = BTreeMap::new();
    for f in classic_render::sound::files_named(&index).into_iter().chain([classic_render::sound::SOUND_INDEX.into()]) {
        files.insert(f.clone(), std::fs::read(generic.join(&f)).unwrap());
    }
    let fetched = [classic_render::platform::Files::Memory { label: "fetched".into(), files }];
    let (mut a, mut b) = (Mixer::new(48_000), Mixer::new(48_000));
    let from_folder = board(&mut a);
    let from_memory = SoundBoard::from_files(&fetched, 0, 1, &mut b);
    assert!(from_memory.warnings.is_empty(), "{:?}", from_memory.warnings);
    for (x, y) in from_folder.tables.defs.iter().zip(&from_memory.tables.defs) {
        assert_eq!(x.clips.len(), y.clips.len(), "{}", x.id);
        for (&cx, &cy) in x.clips.iter().zip(&y.clips) {
            assert_eq!(a.clip(cx).seconds(), b.clip(cy).seconds(), "{}", x.id);
        }
    }
}

/// A pack in memory: the generic pack's sound index, and a voice index whose takes are generic sounds of known
/// lengths, so each test can tell which take played.
fn voiced(mixer: &mut Mixer) -> SoundBoard {
    let generic = setting::root().join("settings/generic");
    let read = |f: &str| std::fs::read(generic.join(f)).unwrap();
    let index = r#"{ "voices": { "faction_a": {
        "advisor": { "low_power": ["audio/sfx/cannon_1.wav", "audio/sfx/explode_large_1.wav"] },
        "vehicle": { "select": ["audio/ui/select_1.wav", "audio/ui/order_1.wav"] } } } }"#;
    let mut files = BTreeMap::new();
    files.insert(classic_render::sound::VOICE_INDEX.to_string(), index.as_bytes().to_vec());
    for f in classic_render::sound::voice_files_named(index) {
        files.insert(f.clone(), read(&f));
    }
    let pack = classic_render::platform::Files::Memory { label: "voiced".into(), files };
    let b = SoundBoard::from_files(&[classic_render::platform::Files::Dir(generic), pack], 0, 1, mixer);
    assert!(b.warnings.is_empty(), "{:?}", b.warnings);
    b
}

#[test]
fn spoken_lines_play_the_take_the_screen_shows_and_no_reply_talks_over_the_advisor() {
    use classic_render::lines::Speech;
    use classic_render::platform::Played;
    let mut mixer = Mixer::new(48_000);
    let mut b = voiced(&mut mixer);
    let say = |who: &'static str, key: &str, variant| Speech { who, key: key.into(), variant };
    let advisor = b.speak("faction_a", &say("advisor", "low_power", 1), &mixer).unwrap();
    let generic = setting::root().join("settings/generic");
    let decode = |f: &str| classic_render::platform::wav::decode(&std::fs::read(generic.join(f)).unwrap()).unwrap();
    let take = mixer.clip(advisor.sound.clip).seconds();
    assert_eq!(take, decode("audio/sfx/explode_large_1.wav").seconds(), "the second take for the second line");
    assert_eq!(advisor.sound.bus, Bus::Voice);
    // No voice for another faction, a line the pack didn't voice, or a variant past its takes.
    assert!(b.speak("faction_b", &say("advisor", "low_power", 0), &mixer).is_none());
    assert!(b.speak("faction_a", &say("advisor", "base_attacked", 0), &mixer).is_none());
    assert!(b.speak("faction_a", &say("vehicle", "select", 2), &mixer).is_none());

    // While the advisor speaks, units keep quiet; a second advisor line waits its turn too.
    assert_eq!(mixer.play(advisor.sound), Played::Started);
    assert!(b.speak("faction_a", &say("vehicle", "select", 0), &mixer).is_none());
    let mut out = vec![0.0; 2 * 4800];
    mixer.render(&mut out, 2);
    assert_eq!(mixer.play(advisor.sound), Played::Dropped, "one advisor line at a time");

    // The effects drop while anyone speaks, and come back after.
    let level = mixer.bus_gain[Bus::Sfx as usize];
    b.duck(&mut mixer);
    assert!(mixer.bus_gain[Bus::Sfx as usize] < level * 0.6, "held down 6 dB");
    b.duck(&mut mixer);
    mixer.stop_all();
    b.duck(&mut mixer);
    assert_eq!(mixer.bus_gain[Bus::Sfx as usize], level);
    let reply = b.speak("faction_a", &say("vehicle", "select", 0), &mixer).unwrap();
    assert_eq!(mixer.play(reply.sound), Played::Started, "once the advisor is done, units answer");
}

#[test]
fn the_feed_hands_what_it_said_to_the_sound_board() {
    use classic_render::Hud;
    use classic_render::lines::Moment;
    use classic_render::platform::Files;
    let mut game = game();
    let pack = setting::load("generic").unwrap();
    let dir = setting::root().join("target/voiced-lines-pack");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("lines.json"),
        r#"{ "factions": { "faction_a": { "advisor": { "low_power": ["One", "Two"] },
             "acks": { "vehicle": { "select": ["A.", "B."] } } } } }"#,
    )
    .unwrap();
    let mut hud = Hud::new(&pack, &Files::Dir(dir), &game, 0, 0);
    assert!(hud.feed.warnings.is_empty(), "{:?}", hud.feed.warnings);
    let tank = game.kind("battle_tank").unwrap();
    let mine = game.spawn(tank, 0, 6, 8);
    hud.feed.reply(&game, Moment::Select, &[mine]);
    let radar = game.kind("radar").unwrap();
    for i in 0..5 {
        game.spawn(radar, 0, 4 + 2 * i, 10);
    }
    game.step(1);
    hud.after_step(&game);
    let said = std::mem::take(&mut hud.feed.spoken);
    println!("{said:?}");
    assert_eq!(said.len(), 2, "the reply and the advisor's line, nothing for words the advisor has none of");
    let reply = hud.feed.reply.clone().unwrap();
    assert_eq!(["A.", "B."][said[0].variant], reply.text, "the voice says what the subtitle shows");
    let line = hud.feed.lines.iter().find(|l| l.id == "low_power").unwrap();
    assert_eq!((said[1].who, said[1].key.as_str()), ("advisor", "low_power"));
    assert_eq!(["One", "Two"][said[1].variant], line.text);
}

#[test]
fn the_private_packs_voices_load_and_cover_their_lines_when_they_are_cloned_in() {
    let packs = setting::private_packs();
    if packs.is_empty() {
        eprintln!("settings-private/ is not cloned here; skipping");
        return;
    }
    let generic = setting::root().join("settings/generic");
    for dir in &packs {
        let mut mixer = Mixer::new(48_000);
        let b = SoundBoard::load(dir, &generic, 0, 1, &mut mixer);
        assert!(b.warnings.is_empty(), "{}: {:?}", dir.display(), b.warnings);
        let pack = setting::load(dir.to_str().unwrap()).unwrap();
        let factions: Vec<String> = pack.factions.iter().map(|f| f.id.clone()).collect();
        if b.voices.is_empty() {
            continue;
        }
        // Every line the pack writes for a faction has a take, and every take speaks a line.
        for f in &factions {
            let lines = classic_render::lines::Lines::load(
                &classic_render::platform::Files::Dir(dir.clone()),
                &factions,
                Some(f),
            );
            for (id, said) in &lines.advisor {
                let takes = &b.voices[&(f.clone(), "advisor".to_string(), id.clone())];
                assert_eq!(takes.len(), said.len(), "{f} advisor {id}");
            }
            for ((set, moment), said) in &lines.acks {
                let key = (f.clone(), set.to_string(), moment.id().to_string());
                assert_eq!(b.voices.get(&key).map_or(0, Vec::len), said.len(), "{key:?}");
            }
        }
        println!("{}: {} voiced lines", dir.display(), b.voices.len());
    }
}
