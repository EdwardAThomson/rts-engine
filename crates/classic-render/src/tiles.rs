//! Terrain tiles from `art/studio/tileset.py` (`art/tiles/tileset.json` and its page in a pack; the format is in
//! `art/README.md`, "Terrain tiles"). The ground is drawn in layers, bottom first, on a grid offset by half a tile:
//! drawn tile (x, y) has map tiles (x-1, y-1), (x, y-1), (x, y) and (x-1, y) at its north-west, north-east,
//! south-east and south-west corners, and shows each layer's tile for which of those four the layer covers (its
//! "corner case", bits 1, 2, 4 and 8 in that order). A pack without a tileset keeps the art index's plain tiles.

use classic_data::json::{self, Value};
use classic_sim::map::RESOURCE_PER_TILE;
use classic_sim::{Game, Terrain};

use crate::platform::Rect;

/// The tile set's index, from the pack's folder.
pub const TILESET: &str = "art/tiles/tileset.json";
/// Its one page of tiles.
pub const TILESET_PAGE: &str = "art/tiles/tileset.png";

/// Which map tiles a layer covers.
#[derive(Clone, Debug, PartialEq)]
pub enum Cover {
    /// Tiles of these kinds of ground.
    Ground(Vec<Terrain>),
    /// Tiles whose resource is at least this level (`resource_level`).
    Resource(u8),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: String,
    pub cover: Cover,
    /// The layer's average colour, for the minimap.
    pub colour: [u8; 3],
    /// By corner case (0 to 15), then by place: each tile's top-left corner in the page, if the layer has one.
    tiles: Vec<Vec<(f32, f32)>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tileset {
    /// Page pixels per tile.
    pub tile_px: f32,
    /// The textures repeat every `period` tiles both ways.
    pub period: i32,
    pub layers: Vec<Layer>,
}

fn terrain(name: &str) -> Option<Terrain> {
    match name {
        "open" => Some(Terrain::Open),
        "rock" => Some(Terrain::Rock),
        "cliff" => Some(Terrain::Cliff),
        _ => None,
    }
}

impl Tileset {
    pub fn parse(text: &str) -> Result<Tileset, String> {
        let doc = json::parse(text).map_err(|e| format!("{TILESET}: {e:?}"))?;
        let int = |key: &str| doc.get(key).and_then(Value::as_int).ok_or(format!("{TILESET}: no {key}"));
        let tile_px = int("tile_px")? as f32;
        let period = int("period")?.max(1) as i32;
        let mut layers = Vec::new();
        for l in doc.get("layers").and_then(Value::as_array).unwrap_or(&[]) {
            let id = l.get("id").and_then(Value::as_str).ok_or(format!("{TILESET}: a layer has no id"))?.to_string();
            let bad = |what: &str| format!("{TILESET}: layer {id}: {what}");
            let on = l.get("on").ok_or_else(|| bad("no 'on'"))?;
            let cover = if let Some(kinds) = on.get("ground").and_then(Value::as_array) {
                let kinds = kinds.iter().map(|k| k.as_str().and_then(terrain).ok_or_else(|| bad("unknown ground")));
                Cover::Ground(kinds.collect::<Result<_, _>>()?)
            } else if let Some(level) = on.get("resource").and_then(Value::as_int) {
                Cover::Resource(level.clamp(1, 2) as u8)
            } else {
                return Err(bad("'on' names neither ground nor resource"));
            };
            let colour = l.get("colour").and_then(Value::as_str).and_then(hex).ok_or_else(|| bad("bad colour"))?;
            let cases = l.get("tiles").and_then(Value::as_array).filter(|c| c.len() == 16);
            let mut tiles = Vec::new();
            for case in cases.ok_or_else(|| bad("'tiles' is not 16 cases"))? {
                let mut places = Vec::new();
                for p in case.as_array().unwrap_or(&[]) {
                    let xy = p.as_array().filter(|a| a.len() == 2).and_then(|a| Some((a[0].as_int()?, a[1].as_int()?)));
                    let (x, y) = xy.ok_or_else(|| bad("a tile is not [x, y]"))?;
                    places.push((x as f32, y as f32));
                }
                if !places.is_empty() && places.len() != (period * period) as usize {
                    return Err(bad("a case without one tile per place"));
                }
                tiles.push(places);
            }
            layers.push(Layer { id, cover, colour, tiles });
        }
        if layers.is_empty() {
            return Err(format!("{TILESET}: no layers"));
        }
        Ok(Tileset { tile_px, period, layers })
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// The page rectangle to draw for drawn tile (x, y) in `layer`, which covers the corners `case` says; none
    /// where it covers none, or has no tile for that case.
    pub fn source(&self, layer: &Layer, case: u8, x: i32, y: i32) -> Option<Rect> {
        let places = layer.tiles.get(usize::from(case))?;
        let p = (y.rem_euclid(self.period) * self.period + x.rem_euclid(self.period)) as usize;
        let &(sx, sy) = places.get(p)?;
        Some(Rect::new(sx, sy, self.tile_px, self.tile_px))
    }
}

/// How much resource a map tile shows: 0 none, 1 a light field, 2 thick, which is a tile at least three quarters
/// full whose four neighbours all hold some, so a field is thick in the middle and thins out at its edges and as
/// it is harvested.
pub fn resource_level(game: &Game, x: i32, y: i32) -> u8 {
    let (w, h) = (game.map.width, game.map.height);
    let at =
        |x: i32, y: i32| (0..w).contains(&x) && (0..h).contains(&y) && game.state.resource[(y * w + x) as usize] > 0;
    let amount = game.state.resource[(y * w + x) as usize];
    if amount <= 0 {
        return 0;
    }
    let thick = amount * 4 >= RESOURCE_PER_TILE * 3
        && [(0, -1), (1, 0), (0, 1), (-1, 0)].iter().all(|&(dx, dy)| at(x + dx, y + dy));
    if thick { 2 } else { 1 }
}

/// Whether `layer` covers map tile (x, y); tiles off the map take the nearest tile's ground.
pub fn covers(layer: &Layer, game: &Game, x: i32, y: i32) -> bool {
    let x = x.clamp(0, game.map.width - 1);
    let y = y.clamp(0, game.map.height - 1);
    match &layer.cover {
        Cover::Ground(kinds) => kinds.contains(&game.map.terrain[(y * game.map.width + x) as usize]),
        Cover::Resource(level) => resource_level(game, x, y) >= *level,
    }
}

/// The corner case of drawn tile (x, y) in `layer`.
pub fn corner_case(layer: &Layer, game: &Game, x: i32, y: i32) -> u8 {
    [(x - 1, y - 1), (x, y - 1), (x, y), (x - 1, y)]
        .iter()
        .enumerate()
        .filter(|(_, (cx, cy))| covers(layer, game, *cx, *cy))
        .fold(0, |case, (i, _)| case | 1 << i)
}

fn hex(s: &str) -> Option<[u8; 3]> {
    let s = s.strip_prefix('#').filter(|s| s.len() == 6)?;
    let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

#[cfg(test)]
mod tests {
    use super::*;
    use classic_sim::{GameOptions, Rules};

    fn tiles(n: usize) -> String {
        let place = "[0,0]";
        let case = format!("[{}]", vec![place; n].join(","));
        let mut cases = vec!["null".to_string(); 16];
        for c in cases.iter_mut().skip(1) {
            *c = case.clone();
        }
        cases.join(",")
    }

    fn set(period: usize) -> String {
        format!(
            r##"{{"tile_px": 64, "period": {period}, "layers": [
              {{"id": "open", "on": {{"ground": ["open", "rock", "cliff"]}}, "colour": "#d0b080", "tiles": [{t}]}},
              {{"id": "resource_light", "on": {{"resource": 1}}, "colour": "#d09030", "tiles": [{t}]}},
              {{"id": "rock", "on": {{"ground": ["rock", "cliff"]}}, "colour": "#706050", "tiles": [{t}]}}]}}"##,
            t = tiles(period * period)
        )
    }

    fn game(map: &str) -> Game {
        let pack = classic_tools::setting::load("generic").unwrap();
        let rules = Rules::from_table(&pack.rules).unwrap();
        Game::new(GameOptions { map, seed: 1, players: None, rules: Some(&rules) }).unwrap()
    }

    #[test]
    fn a_tile_set_parses_and_rejects_a_case_short_of_places() {
        let t = Tileset::parse(&set(2)).unwrap();
        assert_eq!((t.tile_px, t.period, t.layers.len()), (64.0, 2, 3));
        assert_eq!(t.layer("rock").unwrap().cover, Cover::Ground(vec![Terrain::Rock, Terrain::Cliff]));
        assert_eq!(t.layer("resource_light").unwrap().cover, Cover::Resource(1));
        assert_eq!(t.layer("open").unwrap().colour, [0xd0, 0xb0, 0x80]);
        assert!(t.source(t.layer("rock").unwrap(), 0, 0, 0).is_none(), "case 0 draws nothing");
        assert!(Tileset::parse(&set(2).replacen("[[0,0],[0,0],[0,0],[0,0]]", "[[0,0]]", 1)).is_err());
    }

    #[test]
    fn corner_cases_follow_the_map_and_resource_thickens_inside_a_field() {
        let map = "; t
; t
#####....1
#####.....\n#####..~~~\n.......~~~\n.......~~~\n.2........\n";
        let g = game(map);
        let t = Tileset::parse(&set(2)).unwrap();
        let rock = t.layer("rock").unwrap();
        assert_eq!(corner_case(rock, &g, 2, 2), 15, "inside the plateau");
        assert_eq!(corner_case(rock, &g, 5, 1), 1 | 8, "its east edge: only the west corners are rock");
        assert_eq!(corner_case(rock, &g, 8, 6), 0);
        assert_eq!(corner_case(rock, &g, 0, 0), 15, "off the map takes the nearest tile");
        assert_eq!(resource_level(&g, 8, 3), 2, "the middle of a full field is thick");
        assert_eq!(resource_level(&g, 7, 3), 1, "its edge is light");
        assert_eq!(resource_level(&g, 0, 3), 0);
        let res = t.layer("resource_light").unwrap();
        assert_eq!(corner_case(res, &g, 7, 2), 4, "a field's north-west corner");
    }

    #[test]
    fn the_place_wraps_with_the_period() {
        let t = Tileset::parse(&set(2).replacen("[[0,0],[0,0],[0,0],[0,0]]", "[[0,0],[1,0],[2,0],[3,0]]", 1)).unwrap();
        let open = t.layer("open").unwrap();
        let x = |x, y| t.source(open, 1, x, y).unwrap().x;
        assert_eq!([x(0, 0), x(1, 0), x(0, 1), x(1, 1)], [0.0, 1.0, 2.0, 3.0]);
        assert_eq!(x(2, 2), x(0, 0));
        assert_eq!(x(-1, 0), x(1, 0), "the half-tile row off the map's west edge wraps too");
    }

    #[test]
    fn the_generic_packs_tile_set_loads() {
        let path = classic_tools::setting::root().join("settings/generic").join(TILESET);
        let t = Tileset::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        for id in ["open", "rock", "cliff", "resource_light", "resource_thick"] {
            assert!(t.layer(id).is_some(), "{id}");
        }
        for l in &t.layers {
            assert!(t.source(l, 15, 3, 5).is_some(), "{}: no full tile", l.id);
        }
    }
}
