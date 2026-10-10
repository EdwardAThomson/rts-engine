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
    let mut pack = setting::load("generic").unwrap();
    // The battles here are about sound: fog off, so both sides see each other across the whole field.
    pack.rules.modules.get_mut("fog").unwrap().numbers.get_mut("on").unwrap().value = 0;
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
fn guns_infantry_deaths_and_the_hazard_eating_each_have_a_sound() {
    use classic_sim::world::Event;
    let mut game = game();
    let quad = game.kind("quad").unwrap();
    let squad = game.kind("infantry_squad").unwrap();
    // A quad (machine guns) against an infantry squad, close enough to fight at once.
    game.spawn(quad, 0, 10, 6);
    game.spawn(squad, 1, 12, 6);
    let mut mixer = Mixer::new(48_000);
    let mut b = board(&mut mixer);
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for _ in 0..900 {
        game.step(1);
        for Cue { id, .. } in b.after_step(&game, &everything()) {
            *counts.entry(id).or_default() += 1;
        }
    }
    let hazard_ate = Event::HazardAte { tick: game.state.tick, hazard: 1, unit: 1, kind: quad, owner: 0, x: 0, y: 0 };
    game.events.push(hazard_ate);
    for Cue { id, .. } in b.after_step(&game, &everything()) {
        *counts.entry(id).or_default() += 1;
    }
    println!("heard {counts:?}");
    for id in ["sfx_gun", "sfx_infantry_die", "sfx_hazard_eat"] {
        assert!(counts.contains_key(id), "no {id}");
    }
    assert!(!counts.contains_key("sfx_explode_small"), "the squad falls without a blast, and the quad lives");
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
    let say = |who: &str, key: &str, variant| Speech { who: who.into(), key: key.into(), variant, engine: false };
    let advisor = b.speak("faction_a", &say("advisor", "low_power", 1), &mixer, 0.0).unwrap();
    let generic = setting::root().join("settings/generic");
    let decode = |f: &str| classic_render::platform::wav::decode(&std::fs::read(generic.join(f)).unwrap()).unwrap();
    let take = mixer.clip(advisor.sound.clip).seconds();
    assert_eq!(take, decode("audio/sfx/explode_large_1.wav").seconds(), "the second take for the second line");
    assert_eq!(advisor.sound.bus, Bus::Voice);
    // No voice for another faction, a line the pack didn't voice, or a variant past its takes.
    assert!(b.speak("faction_b", &say("advisor", "low_power", 0), &mixer, 0.0).is_none());
    assert!(b.speak("faction_a", &say("advisor", "base_attacked", 0), &mixer, 0.0).is_none());
    assert!(b.speak("faction_a", &say("vehicle", "select", 2), &mixer, 0.0).is_none());

    // While the advisor speaks, units keep quiet; a second advisor line waits its turn too.
    assert_eq!(mixer.play(advisor.sound), Played::Started);
    assert!(b.speak("faction_a", &say("vehicle", "select", 0), &mixer, 0.0).is_none());
    let mut out = vec![0.0; 2 * 4800];
    mixer.render(&mut out, 2);
    assert!(b.speak("faction_a", &say("advisor", "low_power", 0), &mixer, 0.1).is_none(), "it waits");
    assert_eq!(b.waiting().len(), 1);
    assert_eq!(mixer.play(advisor.sound), Played::Dropped, "one advisor line at a time");

    // The effects drop while anyone speaks, and come back after.
    let level = mixer.bus_gain[Bus::Sfx as usize];
    b.duck(&mut mixer);
    assert!(mixer.bus_gain[Bus::Sfx as usize] < level * 0.6, "held down 6 dB");
    b.duck(&mut mixer);
    mixer.stop_all();
    b.duck(&mut mixer);
    assert_eq!(mixer.bus_gain[Bus::Sfx as usize], level);
    // The waiting line has gone stale by now, so it is dropped rather than said.
    assert!(b.next_line(&mixer, 10.0).is_none());
    assert!(b.waiting().is_empty());
    let reply = b.speak("faction_a", &say("vehicle", "select", 0), &mixer, 10.0).unwrap();
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
    assert_eq!((said[1].who.as_str(), said[1].key.as_str()), ("advisor", "low_power"));
    assert_eq!(["One", "Two"][said[1].variant], line.text);
}

#[test]
fn the_generic_voices_speak_every_engine_line_but_only_in_the_engines_words() {
    use classic_render::lines::{Lines, Moment, Speech, VOICES};
    use classic_render::platform::Files;
    let generic = setting::root().join("settings/generic");
    let mut mixer = Mixer::new(48_000);
    let b = SoundBoard::from_files(&[Files::Dir(generic.clone())], 0, 1, &mut mixer);
    assert!(b.warnings.is_empty(), "{:?}", b.warnings);
    assert!(b.engine_voices);
    let provenance = std::fs::read_to_string(generic.join("audio/voices/provenance.jsonl")).unwrap();
    let engine = Lines::engine();
    let pack = setting::load("generic").unwrap();
    for f in &pack.factions {
        for id in classic_render::feed::default_words().keys() {
            let key = (f.id.clone(), "advisor".to_string(), id.clone());
            assert_eq!(b.voices.get(&key).map_or(0, Vec::len), 1, "{key:?}");
        }
        for voice in VOICES {
            for m in Moment::ALL {
                let key = (f.id.clone(), voice.to_string(), m.id().to_string());
                let lines = engine.acks[&(voice.to_string(), m)].len();
                assert_eq!(b.voices.get(&key).map_or(0, Vec::len), lines, "{key:?}");
            }
        }
    }
    let index = std::fs::read_to_string(generic.join(classic_render::sound::VOICE_INDEX)).unwrap();
    for file in classic_render::sound::voice_files_named(&index) {
        assert!(provenance.contains(&format!("\"file\": \"{file}\"")), "{file} has no provenance line");
    }
    // The engine's words take the generic voices; a pack's own words don't.
    let mut b = b;
    let say = |engine| Speech { who: "advisor".into(), key: "low_power".into(), variant: 0, engine };
    assert!(b.speak("faction_a", &say(true), &mixer, 0.0).is_some());
    assert!(b.speak("faction_a", &say(false), &mixer, 0.0).is_none());
    // A pack with voices of its own replaces the generic cast whole.
    let own = voiced(&mut mixer);
    assert!(!own.engine_voices);
    assert!(!own.voices.contains_key(&("faction_a".to_string(), "advisor".to_string(), "base_attacked".to_string())));
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
            let units: Vec<String> = pack.rules.entities.keys().cloned().collect();
            let lines = classic_render::lines::Lines::load(
                &classic_render::platform::Files::Dir(dir.clone()),
                &factions,
                &units,
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

#[test]
fn under_fog_a_player_hears_only_the_fights_they_can_see() {
    let pack = setting::load("generic").unwrap();
    let rules = Rules::from_table(&pack.rules).unwrap();
    assert!(rules.fog.is_some(), "the generic pack turns fog on");
    let mut game = Game::new(GameOptions { map: MAP, seed: 1, players: None, rules: Some(&rules) }).unwrap();
    // Player 1's tanks shelling a third side's, far out in player 0's shroud.
    let tank = game.kind("battle_tank").unwrap();
    for i in 0..3 {
        game.spawn(tank, 1, 19, 3 + i);
        game.spawn(tank, 2, 21, 3 + i);
    }
    let mut mixer = Mixer::new(48_000);
    let generic = setting::root().join("settings/generic");
    let mut zero = SoundBoard::load(&generic, &generic, 0, 1, &mut mixer);
    let mut one = SoundBoard::load(&generic, &generic, 1, 1, &mut mixer);
    let (mut heard0, mut heard1) = (Vec::new(), Vec::new());
    for _ in 0..150 {
        game.step(1);
        heard0.extend(zero.after_step(&game, &everything()).into_iter().map(|c| c.id));
        heard1.extend(one.after_step(&game, &everything()).into_iter().map(|c| c.id));
    }
    println!("player 0 heard {heard0:?}; player 1 heard {} cues", heard1.len());
    assert!(heard1.iter().any(|id| id == "sfx_cannon"), "the side that sees it hears the guns");
    assert!(!heard0.iter().any(|id| id.starts_with("sfx_")), "nothing from the shroud");
}

/// A pack in memory whose advisor voices four lines, each a generic sound of a known length.
fn advisor_pack(mixer: &mut Mixer) -> SoundBoard {
    let generic = setting::root().join("settings/generic");
    let index = r#"{ "voices": { "faction_a": { "advisor": {
        "unit_ready": ["audio/ui/select_1.wav"],
        "low_power": ["audio/sfx/cannon_1.wav"],
        "base_attacked": ["audio/sfx/explode_large_1.wav"],
        "radar_online": ["audio/ui/order_1.wav"] } } } }"#;
    let mut files = BTreeMap::new();
    files.insert(classic_render::sound::VOICE_INDEX.to_string(), index.as_bytes().to_vec());
    for f in classic_render::sound::voice_files_named(index) {
        files.insert(f.clone(), std::fs::read(generic.join(&f)).unwrap());
    }
    let pack = classic_render::platform::Files::Memory { label: "advisor".into(), files };
    SoundBoard::from_files(&[classic_render::platform::Files::Dir(generic), pack], 0, 1, mixer)
}

#[test]
fn the_advisor_queues_lines_most_urgent_first_and_drops_stale_ones() {
    use classic_render::lines::Speech;
    let mut mixer = Mixer::new(48_000);
    let mut b = advisor_pack(&mut mixer);
    let say = |key: &str| Speech { who: "advisor".into(), key: key.into(), variant: 0, engine: false };
    let first = b.speak("faction_a", &say("radar_online"), &mixer, 0.0).expect("nothing speaking: said at once");
    mixer.play(first.sound);
    // Three more while it speaks: only two may wait, so the least important goes.
    for key in ["unit_ready", "low_power", "base_attacked"] {
        assert!(b.speak("faction_a", &say(key), &mixer, 0.2).is_none(), "{key} waits");
    }
    println!("waiting: {:?}", b.waiting());
    assert_eq!(b.waiting(), ["voice faction_a advisor.base_attacked", "voice faction_a advisor.low_power"]);
    // Nothing comes out while the first line plays.
    assert!(b.next_line(&mixer, 0.3).is_none());
    mixer.stop_all();
    let next = b.next_line(&mixer, 0.4).unwrap();
    assert_eq!(next.id, "voice faction_a advisor.base_attacked", "the attack warning jumps ahead");
    mixer.play(next.sound);
    // The other waits past its time and is dropped unsaid.
    mixer.stop_all();
    assert!(b.next_line(&mixer, 3.5).is_none());
    assert!(b.waiting().is_empty());
    let t = Tables::builtin();
    assert!(t.voices.priority("game_lost") > t.voices.priority("base_attacked"));
    assert_eq!(t.voices.priority("repaired"), t.voices.default_priority);
}

#[test]
fn a_unit_leaving_a_factory_is_heard_at_the_door_and_the_owner_hears_it_is_ready() {
    use classic_sim::world::Event;
    let mut game = game();
    let mut mixer = Mixer::new(48_000);
    let mut b = board(&mut mixer);
    let heard = |game: &mut Game, b: &mut SoundBoard, factory: &str, unit: &str, owner: u32| {
        let f = game.kind(factory).unwrap();
        let u = game.kind(unit).unwrap();
        let fid = game.spawn(f, owner, 20, 20);
        let uid = game.spawn(u, owner, 21, 22);
        game.events.push(Event::UnitBuilt { tick: game.state.tick, factory: fid, entity: uid, kind: u });
        let mut ids: Vec<String> = b.after_step(game, &everything()).into_iter().map(|c| c.id).collect();
        ids.sort();
        ids
    };
    assert_eq!(
        heard(&mut game, &mut b, "heavy_factory", "battle_tank", 0),
        ["sfx_exit_heavy_factory", "ui_unit_ready"]
    );
    assert_eq!(heard(&mut game, &mut b, "barracks", "infantry", 0), ["sfx_exit_barracks", "ui_unit_ready"]);
    // Someone else's factory: the door, but not the ready blip.
    assert_eq!(heard(&mut game, &mut b, "light_factory", "quad", 1), ["sfx_exit_light_factory"]);
    assert_eq!(heard(&mut game, &mut b, "air_factory", "gunship", 1), ["sfx_exit_air_factory"]);
}

#[test]
fn moving_vehicles_and_working_harvesters_keep_their_loops_going_and_let_them_go() {
    use classic_sim::CommandOrder;
    let mut game = game();
    let mut mixer = Mixer::new(48_000);
    let mut b = board(&mut mixer);
    let tank = game.kind("battle_tank").unwrap();
    let quad = game.kind("quad").unwrap();
    let tanks: Vec<u32> = (0..3).map(|i| game.spawn(tank, 0, 2 + i, 0)).collect();
    let q = game.spawn(quad, 0, 2, 19);
    game.order(0, &tanks, CommandOrder::Move { x: 28, y: 0 });
    game.order(0, &[q], CommandOrder::Move { x: 28, y: 19 });
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut most = 0;
    let mut buf = vec![0.0f32; 2 * 48_000 / 15];
    for _ in 0..1200 {
        game.step(1);
        b.update_loops(&game, &everything(), &mut mixer);
        mixer.render(&mut buf, 2);
        for id in b.loops_playing() {
            *seen.entry(id.to_string()).or_default() += 1;
        }
        most = most.max(mixer.loops());
    }
    println!("ticks each loop played: {seen:?}, most at once {most}");
    for id in ["loop_engine_heavy", "loop_engine_light", "loop_harvest"] {
        assert!(seen.contains_key(id), "no {id}");
    }
    // One loop per sound however many units need it.
    assert!(most <= 5);
    // Everyone has long arrived: the engines are off, and pausing lets the rest go.
    assert!(!b.loops_playing().contains(&"loop_engine_light"));
    b.stop_loops(&mut mixer);
    for _ in 0..10 {
        mixer.render(&mut buf, 2);
    }
    assert_eq!(mixer.loops(), 0);
}

#[test]
fn a_loop_rule_must_name_a_loop() {
    let sounds = include_str!("../../../data/audio/sounds.json");
    let events = include_str!("../../../data/audio/events.json");
    let bad = events.replace(r#""sound": "loop_harvest""#, r#""sound": "sfx_cannon""#);
    assert!(Tables::parse(sounds, &bad).unwrap_err().contains("not a loop"));
    let bad = events.replace(r#""sound": "ui_unit_ready""#, r#""sound": "loop_harvest""#);
    assert!(Tables::parse(sounds, &bad).unwrap_err().contains("is a loop"));
}

#[test]
fn the_music_follows_the_fighting() {
    use classic_render::music::{MusicBoard, Pool};
    use classic_render::platform::Files;
    let generic = setting::root().join("settings/generic");
    let mut mixer = Mixer::new(48_000);
    let mut m = MusicBoard::from_files(&[Files::Dir(generic)], 0, 1, &mut mixer);
    assert!(m.warnings.is_empty(), "{:?}", m.warnings);
    for pool in Pool::ALL {
        assert!(m.pieces.iter().any(|p| p.pool == pool), "no {} music", pool.id());
    }
    let pool_of = |m: &MusicBoard| m.pieces.iter().find(|p| Some(p.id.as_str()) == m.playing()).map(|p| p.pool);
    let mut buf = vec![0.0f32; 2 * 48_000 / 15];
    m.title(&mut mixer);
    assert_eq!(pool_of(&m), Some(Pool::Menu));
    mixer.render(&mut buf, 2);
    assert!(buf.iter().any(|&s| s != 0.0), "the title music plays");

    let mut game = game();
    m.new_game(&game);
    battle(&mut game, 6);
    let mut moods = vec![];
    for _ in 0..1500 {
        game.step(1);
        m.after_step(&game, &mut mixer);
        mixer.render(&mut buf, 2);
        if moods.last() != Some(&m.mood) {
            moods.push(m.mood);
        }
    }
    println!(
        "moods {:?}, tension {:.1}, playing {:?}",
        moods.iter().map(|p| p.id()).collect::<Vec<_>>(),
        m.tension,
        m.playing()
    );
    assert_eq!(moods, [Pool::Calm, Pool::Battle, Pool::Calm], "into battle with the fight, and calm again after");
    assert_eq!(pool_of(&m), Some(Pool::Calm));

    m.over(true, &mut mixer);
    assert_eq!(pool_of(&m), Some(Pool::Won));
    let stinger = m.playing().map(String::from);
    for _ in 0..300 {
        mixer.render(&mut buf, 2);
        m.over(true, &mut mixer);
    }
    assert_eq!(m.playing().map(String::from), stinger, "the stinger plays once");
    assert_eq!(mixer.music(), None, "and then it is quiet");
}

#[test]
fn the_private_packs_music_loads_when_they_are_cloned_in() {
    use classic_render::music::MusicBoard;
    use classic_render::platform::Files;
    let generic = setting::root().join("settings/generic");
    for dir in setting::private_packs() {
        let mut mixer = Mixer::new(48_000);
        let m = MusicBoard::from_files(&[Files::Dir(generic.clone()), Files::Dir(dir.clone())], 0, 1, &mut mixer);
        assert!(m.warnings.is_empty(), "{}: {:?}", dir.display(), m.warnings);
        let ids: Vec<&str> = m.pieces.iter().map(|p| p.id.as_str()).collect();
        println!("{}: {ids:?}", dir.display());
    }
}
