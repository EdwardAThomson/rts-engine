//! The three strengths of the computer opponent: on the skirmish map, a stronger one beats a weaker one from either
//! start. Each match prints its result (`cargo test -p classic-ai --test difficulty -- --nocapture`).

use classic_ai::{Ai, Difficulty, winner};
use classic_sim::{Game, GameOptions};

const SKIRMISH: &str = include_str!("../../../maps/skirmish-01.txt");

/// Play `a` as player 0 against `b` as player 1 for up to `ticks`; the winner and the tick, if anyone won.
fn duel(seed: i32, a: Difficulty, b: Difficulty, ticks: u32) -> Option<(u32, u32)> {
    let mut g = Game::new(GameOptions { map: SKIRMISH, seed, players: None, rules: None }).expect("valid map");
    let mut ais = [Ai::new(0, a.settings()), Ai::new(1, b.settings())];
    for _ in 0..ticks {
        for ai in &mut ais {
            ai.tick(&mut g);
        }
        g.step(1);
        if let Some(w) = winner(&g) {
            return Some((w, g.state.tick));
        }
    }
    None
}

/// Over six seeds, from both starts, the stronger of each pair wins at least half of the twelve games and more than
/// twice as many as it loses; the rest run out of resources in a stalemate. Our own runs, when the presets were
/// last set (the 600-credit gun turret, issue #79): hard against normal won 10, lost 2; normal against easy won 9,
/// lost 1.
#[test]
fn a_stronger_opponent_beats_a_weaker_one_from_either_start() {
    use Difficulty::*;
    for (strong, weak) in [(Hard, Normal), (Normal, Easy)] {
        let (mut won, mut lost) = (0, 0);
        let mut games = String::new();
        for seed in 1..=6 {
            for strong_player in [0, 1] {
                let end = if strong_player == 0 {
                    duel(seed, strong, weak, 40_000)
                } else {
                    duel(seed, weak, strong, 40_000)
                };
                match end {
                    Some((w, _)) if w == strong_player => (won, games) = (won + 1, games + "W"),
                    Some(_) => (lost, games) = (lost + 1, games + "L"),
                    None => games += "-",
                }
            }
        }
        println!("{strong:?} against {weak:?}, seeds 1 to 6, each from both starts: {games} ({won} won, {lost} lost)");
        assert!(won >= 6 && won > 2 * lost, "{strong:?} should beat {weak:?}");
    }
}

#[test]
fn ids_name_each_difficulty_once() {
    for d in Difficulty::ALL {
        assert_eq!(Difficulty::from_id(d.id()), Some(d));
    }
    assert_eq!(Difficulty::from_id("impossible"), None);
    assert_eq!(Difficulty::default(), Difficulty::Normal);
    assert_eq!(Difficulty::Hard.next(), Difficulty::Easy);
    assert_eq!(Difficulty::Normal.settings(), classic_ai::Settings::normal());
}
