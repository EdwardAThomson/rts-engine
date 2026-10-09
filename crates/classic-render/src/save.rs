//! Saved games. A save is what a replay is (the setting, the map, the seed, the rules' hash and the commands, each
//! with its tick) plus who the computer played and how hard, the tick it was saved on and the state hash there.
//! Loading starts the same game fresh and plays it forward to that tick: the player's own commands come from the
//! save, and the computer opponents think again exactly as they did, since they decide from the game alone. The
//! state hash at the end has to match the saved one, so a save that doesn't reproduce its game never loads.
//!
//! The opponents' orders are left out of the file because they come back by themselves, and their memory (the
//! wave out, the scout) with them; a state snapshot would have to store that memory too. Loading a long game
//! replays every tick of it, which takes the simulation's speed (thousands of ticks a second) rather than a moment.
//!
//! The file is text, one `name value` per line and a `c` line per command:
//!
//! ```text
//! classic-save 1
//! setting generic
//! map Skirmish map 1
//! map_hash 1a2b3c4d
//! seed 1
//! fog pack
//! rules <the rules table's hash>
//! player 0
//! faction 0
//! ai 1 normal
//! tick 4500
//! hash <the state hash at that tick>
//! c 120 0 7,8 move 30 41
//! c 300 0 - produce power_plant
//! ```

use classic_ai::{Ai, Difficulty};
use classic_sim::{Command, CommandOrder, Game, Rules};

/// The first line of every save, with the format's version.
const MAGIC: &str = "classic-save 1";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Save {
    /// The setting pack's id.
    pub setting: String,
    /// The map's name as the menu shows it, and a hash of its text.
    pub map: String,
    pub map_hash: String,
    pub seed: i32,
    /// The player's fog override (`on`, `shroud` or `off`), or `pack` for the pack's own fog.
    pub fog: String,
    /// The rules table's hash, fog override and all.
    pub rules: String,
    /// The local player and the faction they picked.
    pub player: u32,
    pub faction: usize,
    /// Each computer player and its difficulty.
    pub ais: Vec<(u32, Difficulty)>,
    pub tick: u32,
    pub hash: String,
    /// Every command not from a computer player, with its tick, in the order given.
    pub commands: Vec<(u32, Command)>,
}

/// A hash of a map's text, to tell a save's map from another of the same name.
pub fn map_hash(text: &str) -> String {
    // FNV-1a, 32 bits.
    let mut h: u32 = 0x811c_9dc5;
    for b in text.bytes() {
        h = (h ^ b as u32).wrapping_mul(0x0100_0193);
    }
    format!("{h:08x}")
}

impl Save {
    /// A save of `game` as it is now. `ais` are the computer players, whose commands are left out.
    pub fn of(game: &Game, ais: &[(u32, Difficulty)], game_info: SaveInfo) -> Save {
        let computer = |p: u32| ais.iter().any(|&(a, _)| a == p);
        Save {
            setting: game_info.setting,
            map: game_info.map,
            map_hash: game_info.map_hash,
            seed: game_info.seed,
            fog: game_info.fog,
            rules: game.rules.hash.clone(),
            player: game_info.player,
            faction: game_info.faction,
            ais: ais.to_vec(),
            tick: game.state.tick,
            hash: game.hash(),
            commands: game
                .command_log()
                .iter()
                .filter(|c| !computer(c.command.player))
                .map(|c| (c.tick, c.command.clone()))
                .collect(),
        }
    }

    pub fn to_text(&self, rules: &Rules) -> String {
        let mut out = format!("{MAGIC}\n");
        out += &format!("setting {}\nmap {}\nmap_hash {}\n", self.setting, self.map, self.map_hash);
        out += &format!("seed {}\nfog {}\nrules {}\n", self.seed, self.fog, self.rules);
        out += &format!("player {}\nfaction {}\n", self.player, self.faction);
        for (p, d) in &self.ais {
            out += &format!("ai {p} {}\n", d.id());
        }
        out += &format!("tick {}\nhash {}\n", self.tick, self.hash);
        for (tick, c) in &self.commands {
            out += &format!("c {tick} {}\n", command_text(rules, c));
        }
        out
    }

    /// A save from its text. Kind ids in commands are looked up in `rules`, the rules the game will run under.
    pub fn parse(text: &str, rules: &Rules) -> Result<Save, String> {
        let mut lines = text.lines();
        if lines.next().map(str::trim) != Some(MAGIC) {
            return Err("not a saved game, or one from another version".into());
        }
        let mut s = Save {
            setting: String::new(),
            map: String::new(),
            map_hash: String::new(),
            seed: 0,
            fog: "pack".into(),
            rules: String::new(),
            player: 0,
            faction: 0,
            ais: Vec::new(),
            tick: 0,
            hash: String::new(),
            commands: Vec::new(),
        };
        for (n, line) in lines.enumerate() {
            let line = line.trim_end();
            if line.is_empty() {
                continue;
            }
            let bad = |what: &str| format!("save line {}: {what}: {line:?}", n + 2);
            let (name, value) = line.split_once(' ').ok_or_else(|| bad("no value"))?;
            let num = |v: &str| v.parse::<i64>().map_err(|_| bad("not a number"));
            match name {
                "setting" => s.setting = value.into(),
                "map" => s.map = value.into(),
                "map_hash" => s.map_hash = value.into(),
                "seed" => s.seed = num(value)? as i32,
                "fog" => s.fog = value.into(),
                "rules" => s.rules = value.into(),
                "player" => s.player = num(value)? as u32,
                "faction" => s.faction = num(value)? as usize,
                "ai" => {
                    let (p, d) =
                        value.split_once(' ').ok_or_else(|| bad("an ai line needs a player and a difficulty"))?;
                    s.ais.push((num(p)? as u32, Difficulty::from_id(d).ok_or_else(|| bad("unknown difficulty"))?));
                }
                "tick" => s.tick = num(value)? as u32,
                "hash" => s.hash = value.into(),
                "c" => {
                    let (tick, rest) = value.split_once(' ').ok_or_else(|| bad("a command needs a tick"))?;
                    let command = parse_command(rest, rules).map_err(|e| bad(&e))?;
                    s.commands.push((num(tick)? as u32, command));
                }
                _ => return Err(bad("unknown line")),
            }
        }
        if s.commands.windows(2).any(|w| w[1].0 < w[0].0) || s.commands.last().is_some_and(|c| c.0 > s.tick) {
            return Err("the save's commands are out of order".into());
        }
        Ok(s)
    }

    /// Play `game`, a fresh start of this save's game, forward to the saved tick, with the save's computer
    /// opponents thinking again and its own commands given on their ticks; `after_step` sees each tick, as the
    /// player's effects and score would. Returns the game and its opponents, ready to go on, once the state hash
    /// matches the saved one.
    pub fn replay(&self, mut game: Game, mut after_step: impl FnMut(&Game)) -> Result<(Game, Vec<Ai>), String> {
        if game.state.tick != 0 {
            return Err("a save loads into a fresh game".into());
        }
        if game.rules.hash != self.rules {
            return Err("this save was made under other rules or another tuning".into());
        }
        let mut ais: Vec<Ai> = self.ais.iter().map(|&(p, d)| Ai::new(p, d.settings())).collect();
        let mut next = 0;
        let give = |game: &mut Game, next: &mut usize| {
            while let Some((_, c)) = self.commands.get(*next).filter(|(t, _)| *t == game.state.tick) {
                game.order(c.player, &c.ids, c.order);
                *next += 1;
            }
        };
        while game.state.tick < self.tick {
            // A player's clicks between frames come before the opponents' thinking on the same tick.
            give(&mut game, &mut next);
            for ai in &mut ais {
                ai.tick(&mut game);
            }
            game.step(1);
            after_step(&game);
            if game.events.len() > 10_000 {
                game.events.clear();
            }
        }
        // Clicks on the saved tick itself are still queued for its step.
        give(&mut game, &mut next);
        if next < self.commands.len() {
            return Err("the save has commands it never reached".into());
        }
        if game.hash() != self.hash {
            return Err(format!(
                "the save didn't reproduce its game (hash {} at tick {}, saved {})",
                game.hash(),
                self.tick,
                self.hash
            ));
        }
        Ok((game, ais))
    }
}

/// What a save needs to know that the game itself doesn't.
#[derive(Clone, Debug, Default)]
pub struct SaveInfo {
    pub setting: String,
    pub map: String,
    pub map_hash: String,
    pub seed: i32,
    pub fog: String,
    pub player: u32,
    pub faction: usize,
}

/// A command as text: `<player> <ids> <order> <args>`, ids comma-separated or `-` for none, kinds by generic id.
pub fn command_text(rules: &Rules, c: &Command) -> String {
    let ids =
        if c.ids.is_empty() { "-".to_string() } else { c.ids.iter().map(u32::to_string).collect::<Vec<_>>().join(",") };
    let kind = |k| &rules.kind(k).id;
    let order = match c.order {
        CommandOrder::Move { x, y } => format!("move {x} {y}"),
        CommandOrder::Harvest => "harvest".to_string(),
        CommandOrder::Place { kind: k, x, y } => format!("place {} {x} {y}", kind(k)),
        CommandOrder::Produce { kind: k } => format!("produce {}", kind(k)),
        CommandOrder::Attack { target } => format!("attack {target}"),
        CommandOrder::Cancel { kind: k } => format!("cancel {}", kind(k)),
        CommandOrder::Repair { on } => format!("repair {}", on as u8),
        CommandOrder::Sell => "sell".to_string(),
        CommandOrder::Capture { target } => format!("capture {target}"),
        CommandOrder::RepairAt { pad } => format!("repair_at {pad}"),
    };
    format!("{} {ids} {order}", c.player)
}

/// A command from `command_text`'s text.
pub fn parse_command(text: &str, rules: &Rules) -> Result<Command, String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let num = |i: usize| -> Result<i64, String> {
        words.get(i).and_then(|w| w.parse().ok()).ok_or(format!("word {} should be a number", i + 1))
    };
    let kind = |i: usize| {
        let id = words.get(i).ok_or("a kind id is missing")?;
        rules.kind_id(id).ok_or(format!("the rules have no {id}"))
    };
    let player = num(0)? as u32;
    let ids = match words.get(1) {
        Some(&"-") => Vec::new(),
        Some(list) => {
            list.split(',').map(|i| i.parse().map_err(|_| format!("bad id in {list}"))).collect::<Result<_, _>>()?
        }
        None => return Err("no ids".into()),
    };
    let order = match words.get(2).copied() {
        Some("move") => CommandOrder::Move { x: num(3)? as i32, y: num(4)? as i32 },
        Some("harvest") => CommandOrder::Harvest,
        Some("place") => CommandOrder::Place { kind: kind(3)?, x: num(4)? as i32, y: num(5)? as i32 },
        Some("produce") => CommandOrder::Produce { kind: kind(3)? },
        Some("attack") => CommandOrder::Attack { target: num(3)? as u32 },
        Some("cancel") => CommandOrder::Cancel { kind: kind(3)? },
        Some("repair") => CommandOrder::Repair { on: num(3)? != 0 },
        Some("sell") => CommandOrder::Sell,
        Some("capture") => CommandOrder::Capture { target: num(3)? as u32 },
        Some("repair_at") => CommandOrder::RepairAt { pad: num(3)? as u32 },
        other => return Err(format!("unknown order {other:?}")),
    };
    Ok(Command { player, ids, order })
}
