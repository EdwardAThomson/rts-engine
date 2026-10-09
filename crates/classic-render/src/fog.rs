//! Drawing fog of war over the map for the local player (playbooks `plans/rts/renderer.md`, section 8): black over
//! shroud, a dark veil over fog, nothing over what is in sight, with soft edges between them.
//!
//! The sprite batch samples nearest pixels and has no per-corner colours, so the soft edges are baked: a page of
//! `CASES` small squares, one for each way the four corners of a square can be shroud, fog or in sight, each shaded
//! by blending its corners' darkness across it. Like the terrain tile set (`tiles`), each drawn square sits half a
//! tile up and left of its map tile, so the four map tiles round it are its corners, and the page's square for those
//! four views is drawn over it.

use classic_sim::{Game, TileView};

use crate::platform::{Rect, SpriteBatch, TexId};
use crate::scene::Camera;

/// Pixels along one side of a square on the page.
pub const CASE: u32 = 32;
/// Squares on the page: three views at each of four corners.
pub const CASES: u32 = 81;
/// Squares in a row of the page.
const ROW: u32 = 9;
/// Each square sits in a cell one pixel bigger all round, its edge pixels repeated there, so a sample that lands just
/// past its edge (nearest sampling at fractional zooms does) takes its own shade, never a neighbour's.
const CELL: u32 = CASE + 2;

/// How dark each view draws, as the veil's opacity: the renderer doc's fog shows the ground at about 55%.
pub fn darkness(v: TileView) -> f32 {
    match v {
        TileView::Shroud => 1.0,
        TileView::Fog => 0.45,
        TileView::Visible => 0.0,
    }
}

fn level(v: TileView) -> u32 {
    match v {
        TileView::Shroud => 0,
        TileView::Fog => 1,
        TileView::Visible => 2,
    }
}

/// The page's square for corners top-left, top-right, bottom-left and bottom-right.
pub fn case(corners: [TileView; 4]) -> u32 {
    corners.iter().rev().fold(0, |n, &v| n * 3 + level(v))
}

/// The page as RGBA rows: black, its alpha the corners' darkness blended across each square with a smooth step.
/// Square `c` is at `source(c)`.
pub fn page() -> (u32, u32, Vec<u8>) {
    let views = [TileView::Shroud, TileView::Fog, TileView::Visible];
    let (w, h) = (ROW * CELL, CASES.div_ceil(ROW) * CELL);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let smooth = |t: f32| t * t * (3.0 - 2.0 * t);
    for c in 0..CASES {
        let corner = |i: u32| darkness(views[(c / 3u32.pow(i) % 3) as usize]);
        let [tl, tr, bl, br] = [corner(0), corner(1), corner(2), corner(3)];
        let (ox, oy) = (c % ROW * CELL, c / ROW * CELL);
        for py in 0..CELL {
            let v = smooth((py.clamp(1, CASE) as f32 - 0.5) / CASE as f32);
            for px in 0..CELL {
                let u = smooth((px.clamp(1, CASE) as f32 - 0.5) / CASE as f32);
                let a = (tl * (1.0 - u) + tr * u) * (1.0 - v) + (bl * (1.0 - u) + br * u) * v;
                let i = (((oy + py) * w + ox + px) * 4 + 3) as usize;
                rgba[i] = (a * 255.0).round() as u8;
            }
        }
    }
    (w, h, rgba)
}

/// Where square `c` lies on the page.
pub fn source(c: u32) -> Rect {
    Rect::new((c % ROW * CELL + 1) as f32, (c / ROW * CELL + 1) as f32, CASE as f32, CASE as f32)
}

/// Draw `player`'s fog over the map tiles from (x0, y0) up to (x1, y1), `tile` world pixels each. Squares that
/// hang over the map's edge are cut to it, and a corner off the map takes the view of the map tile nearest it.
#[allow(clippy::too_many_arguments)]
pub fn draw(
    batch: &mut SpriteBatch,
    page: TexId,
    game: &Game,
    player: u32,
    cam: &Camera,
    tile: f32,
    (x0, y0, x1, y1): (i32, i32, i32, i32),
) {
    let (mw, mh) = (game.map.width, game.map.height);
    let (map_w, map_h) = (mw as f32 * tile, mh as f32 * tile);
    let view = |x: i32, y: i32| game.tile_view(player, x.clamp(0, mw - 1), y.clamp(0, mh - 1));
    for y in y0..=y1 {
        for x in x0..=x1 {
            let corners = [view(x - 1, y - 1), view(x, y - 1), view(x - 1, y), view(x, y)];
            if corners.iter().all(|&v| v == TileView::Visible) {
                continue;
            }
            let (wx, wy) = ((x as f32 - 0.5) * tile, (y as f32 - 0.5) * tile);
            let (cx0, cy0) = (wx.max(0.0), wy.max(0.0));
            let (cx1, cy1) = ((wx + tile).min(map_w), (wy + tile).min(map_h));
            if cx1 <= cx0 || cy1 <= cy0 {
                continue;
            }
            let src = source(case(corners));
            let k = src.w / tile;
            let src = Rect::new(src.x + (cx0 - wx) * k, src.y + (cy0 - wy) * k, (cx1 - cx0) * k, (cy1 - cy0) * k);
            let (sx, sy) = cam.to_screen(cx0, cy0);
            batch.sprite(page, src, Rect::new(sx, sy, (cx1 - cx0) * cam.zoom, (cy1 - cy0) * cam.zoom), [255; 4]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_square_is_shaded_from_its_corners() {
        let (w, _, rgba) = page();
        let alpha = |c: u32, px: u32, py: u32| {
            let s = source(c);
            rgba[(((s.y as u32 + py) * w + s.x as u32 + px) * 4 + 3) as usize]
        };
        let all = |v| case([v; 4]);
        // Uniform squares are flat; a shroud corner is near black there, an in-sight one near clear.
        assert!((0..CASE).all(|p| alpha(all(TileView::Shroud), p, p) == 255));
        assert!((0..CASE).all(|p| alpha(all(TileView::Visible), p, p) == 0));
        assert_eq!(alpha(all(TileView::Fog), 5, 9), (0.45f32 * 255.0).round() as u8);
        let edge = case([TileView::Shroud, TileView::Visible, TileView::Shroud, TileView::Visible]);
        assert!(alpha(edge, 0, 4) > 250 && alpha(edge, CASE - 1, 4) < 5);
        assert!(alpha(edge, CASE / 2, 4).abs_diff(128) < 8, "half way across, half dark");
    }
}
