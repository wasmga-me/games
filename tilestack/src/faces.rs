//! The tiles: a block with an ivory face on a coloured back, and the
//! pictures on the faces, all drawn here from shapes and text.

use crate::board::{DRAGONS, FLOWERS, SEASONS, WINDS};
use wg::{hex, mix, Screen};

pub struct Theme {
    pub table_top: u32,
    pub table_bottom: u32,
    pub panel: u32,
    pub text: u32,
    pub dim: u32,
    pub face_hi: u32,
    pub face_lo: u32,
    pub edge: u32,
    /// the ivory layer under the face, and the coloured back under that
    pub side: u32,
    pub back: u32,
    /// the picked tile, and a hinted pair
    pub accent: u32,
    pub hint: u32,
    pub btn: u32,
    pub btn_ink: u32,
}

pub const THEMES: [Theme; 4] = [
    // Jade
    Theme {
        table_top: hex(0x25604a),
        table_bottom: hex(0x153a2c),
        panel: hex(0x10291f),
        text: hex(0xf3efe2),
        dim: hex(0x8fb5a5),
        face_hi: hex(0xfffdf5),
        face_lo: hex(0xf0e7d0),
        edge: hex(0xcfc2a2),
        side: hex(0xe8dcbc),
        back: hex(0x2a8a66),
        accent: hex(0xf2b632),
        hint: hex(0x35c4e6),
        btn: hex(0x1f4c3c),
        btn_ink: hex(0xf3efe2),
    },
    // Night
    Theme {
        table_top: hex(0x212a48),
        table_bottom: hex(0x0f1426),
        panel: hex(0x0a0e1b),
        text: hex(0xe8ecf8),
        dim: hex(0x7c88ad),
        face_hi: hex(0xfdfcf8),
        face_lo: hex(0xe9e5d9),
        edge: hex(0xc9c3b2),
        side: hex(0xdcd5c1),
        back: hex(0x4555b0),
        accent: hex(0xffc642),
        hint: hex(0x4ee0a0),
        btn: hex(0x252e50),
        btn_ink: hex(0xe8ecf8),
    },
    // Sand
    Theme {
        table_top: hex(0xf1e7d3),
        table_bottom: hex(0xe0cfae),
        panel: hex(0xf9f3e6),
        text: hex(0x3a2c1e),
        dim: hex(0x8d7a62),
        face_hi: hex(0xffffff),
        face_lo: hex(0xf4ede0),
        edge: hex(0xd3c6ae),
        side: hex(0xeee3cc),
        back: hex(0xc4633a),
        accent: hex(0xe39a12),
        hint: hex(0x2b97cf),
        btn: hex(0xffffff),
        btn_ink: hex(0x3a2c1e),
    },
    // Plum
    Theme {
        table_top: hex(0x4d2445),
        table_bottom: hex(0x2a1226),
        panel: hex(0x1f0d1c),
        text: hex(0xf7ecf2),
        dim: hex(0xb28aa6),
        face_hi: hex(0xfffbf4),
        face_lo: hex(0xf0e5d6),
        edge: hex(0xd0c1ad),
        side: hex(0xe4d5bd),
        back: hex(0x9b2f5c),
        accent: hex(0xffc24a),
        hint: hex(0x52d1c4),
        btn: hex(0x3e1d39),
        btn_ink: hex(0xf7ecf2),
    },
];

const RED: u32 = hex(0xc8322a);
const GREEN: u32 = hex(0x17824a);
const DARK_GREEN: u32 = hex(0x0e5a32);
const BLUE: u32 = hex(0x1d5aa6);
const INK: u32 = hex(0x1d2a3a);

/// Tile sizes in plan units: the face, and how far the block below it shows.
#[derive(Clone, Copy, Default, PartialEq)]
pub struct Size {
    pub w: f32,
    pub h: f32,
    pub depth: f32,
}

impl Size {
    pub fn radius(&self) -> f32 {
        self.w * 0.14
    }
    /// room left round a tile in its sprite, for the shadow
    pub fn pad(&self) -> f32 {
        (self.depth * 2.2).max(2.0)
    }
}

/// A whole tile, its face's top left at x, y, with its block and shadow.
pub fn tile(s: &mut Screen, x: f32, y: f32, z: Size, face: u8, t: &Theme) {
    let (w, h, d, r) = (z.w, z.h, z.depth, z.radius());
    s.shadow(x + d * 0.9, y + d * 1.3, w + d, h + d, r, d * 0.9, 0, 0.38);
    // the block: the coloured back, then ivory up to the face
    let steps = 6;
    for i in (1..=steps).rev() {
        let o = d * i as f32 / steps as f32;
        let c = if i * 2 > steps {
            mix(hex(0x000000), t.back, 0.15 * (i as f32 / steps as f32))
        } else {
            t.side
        };
        s.rect(x + o, y + o, w, h, r, c, 1.0);
    }
    s.rect(x, y, w, h, r, t.edge, 1.0);
    let e = (w * 0.035).max(0.6);
    s.rect_grad(
        x + e,
        y + e,
        w - 2.0 * e,
        h - 2.0 * e,
        r - e * 0.5,
        t.face_hi,
        t.face_lo,
        1.0,
    );
    picture(s, x + w * 0.1, y + h * 0.075, w * 0.8, h * 0.85, face);
}

/// The face's picture inside the box x, y, w, h.
pub fn picture(s: &mut Screen, x: f32, y: f32, w: f32, h: f32, face: u8) {
    let b = Area { x, y, w, h };
    match face {
        0..=8 => dots(s, &b, face + 1),
        9..=17 => bamboo(s, &b, face - 8),
        18..=26 => characters(s, &b, face - 17),
        _ if face < DRAGONS => wind(s, &b, face - WINDS),
        _ if face < FLOWERS => dragon(s, &b, face - DRAGONS),
        _ if face < SEASONS => flower(s, &b, face - FLOWERS),
        _ => season(s, &b, face - SEASONS),
    }
}

/// The picture's box; points in it are given from 0 to 1 across and down.
struct Area {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl Area {
    fn at(&self, u: f32, v: f32) -> (f32, f32) {
        (self.x + u * self.w, self.y + v * self.h)
    }
}

/// The digits of the small index in a corner.
const INDEX: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

fn index(s: &mut Screen, b: &Area, text: &str, c: u32) {
    let size = b.w * 0.3;
    s.text(text, b.x - b.w * 0.06, b.y + size * 0.42, size, c, 1.0);
}

/// One dot: a coloured ring round a light ring round a coloured centre.
fn dot(s: &mut Screen, cx: f32, cy: f32, r: f32, c: u32) {
    s.disc(cx, cy, r, c, 1.0);
    s.disc(cx, cy, r * 0.68, hex(0xfffaf0), 1.0);
    s.disc(cx, cy, r * 0.46, c, 1.0);
    s.disc(cx - r * 0.12, cy - r * 0.14, r * 0.14, hex(0xffffff), 0.7);
}

fn dots(s: &mut Screen, b: &Area, n: u8) {
    let (g, r_, bl) = (GREEN, RED, BLUE);
    // (u, v, radius as a share of the width, colour)
    let list: &[(f32, f32, f32, u32)] = match n {
        1 => &[],
        2 => &[(0.5, 0.25, 0.22, g), (0.5, 0.75, 0.22, bl)],
        3 => &[
            (0.22, 0.17, 0.19, bl),
            (0.5, 0.5, 0.19, r_),
            (0.78, 0.83, 0.19, g),
        ],
        4 => &[
            (0.27, 0.26, 0.2, bl),
            (0.73, 0.26, 0.2, g),
            (0.27, 0.74, 0.2, g),
            (0.73, 0.74, 0.2, bl),
        ],
        5 => &[
            (0.24, 0.2, 0.18, bl),
            (0.76, 0.2, 0.18, g),
            (0.5, 0.5, 0.18, r_),
            (0.24, 0.8, 0.18, g),
            (0.76, 0.8, 0.18, bl),
        ],
        6 => &[
            (0.29, 0.16, 0.17, g),
            (0.71, 0.16, 0.17, g),
            (0.29, 0.5, 0.17, r_),
            (0.71, 0.5, 0.17, r_),
            (0.29, 0.84, 0.17, r_),
            (0.71, 0.84, 0.17, r_),
        ],
        7 => &[
            (0.2, 0.11, 0.14, g),
            (0.5, 0.22, 0.14, g),
            (0.8, 0.33, 0.14, g),
            (0.3, 0.6, 0.15, r_),
            (0.7, 0.6, 0.15, r_),
            (0.3, 0.87, 0.15, r_),
            (0.7, 0.87, 0.15, r_),
        ],
        8 => &[
            (0.29, 0.12, 0.15, bl),
            (0.71, 0.12, 0.15, bl),
            (0.29, 0.37, 0.15, bl),
            (0.71, 0.37, 0.15, bl),
            (0.29, 0.63, 0.15, bl),
            (0.71, 0.63, 0.15, bl),
            (0.29, 0.88, 0.15, bl),
            (0.71, 0.88, 0.15, bl),
        ],
        _ => &[
            (0.19, 0.16, 0.15, bl),
            (0.5, 0.16, 0.15, bl),
            (0.81, 0.16, 0.15, bl),
            (0.19, 0.5, 0.15, r_),
            (0.5, 0.5, 0.15, r_),
            (0.81, 0.5, 0.15, r_),
            (0.19, 0.84, 0.15, g),
            (0.5, 0.84, 0.15, g),
            (0.81, 0.84, 0.15, g),
        ],
    };
    if n == 1 {
        // one big rosette
        let (cx, cy) = b.at(0.5, 0.5);
        let r = b.w * 0.48;
        s.disc(cx, cy, r, GREEN, 1.0);
        s.disc(cx, cy, r * 0.84, hex(0xfffaf0), 1.0);
        for k in 0..12 {
            let a = k as f32 * core::f32::consts::PI / 6.0;
            s.disc(
                cx + a.cos() * r * 0.66,
                cy + a.sin() * r * 0.66,
                r * 0.11,
                BLUE,
                1.0,
            );
        }
        s.disc(cx, cy, r * 0.48, RED, 1.0);
        s.disc(cx, cy, r * 0.3, hex(0xfffaf0), 1.0);
        s.disc(cx, cy, r * 0.17, RED, 1.0);
        return;
    }
    for &(u, v, rr, c) in list {
        let (cx, cy) = b.at(u, v);
        dot(s, cx, cy, rr * b.w, c);
    }
}

/// A bamboo stick from v0 to v1 down the picture at u.
fn stick(s: &mut Screen, b: &Area, u: f32, v0: f32, v1: f32, c: u32) {
    let (x, y0) = b.at(u, v0);
    let (_, y1) = b.at(u, v1);
    let r = b.w * 0.085;
    let dark = mix(c, hex(0x000000), 0.35);
    s.line(x, y0 + r, x, y1 - r, r, dark, 1.0);
    s.line(x, y0 + r, x, y1 - r, r * 0.62, c, 1.0);
    s.line(
        x - r * 0.25,
        y0 + r * 1.2,
        x - r * 0.25,
        y1 - r * 1.2,
        r * 0.16,
        hex(0xffffff),
        0.45,
    );
    // joints at the ends and in the middle
    let ym = (y0 + y1) / 2.0;
    for y in [y0 + r * 0.6, ym, y1 - r * 0.6] {
        s.rect(
            x - r * 1.25,
            y - r * 0.32,
            r * 2.5,
            r * 0.64,
            r * 0.3,
            dark,
            1.0,
        );
    }
}

fn bamboo(s: &mut Screen, b: &Area, n: u8) {
    let (g, r) = (GREEN, RED);
    // (u, v0, v1, colour)
    let list: &[(f32, f32, f32, u32)] = match n {
        1 => &[],
        2 => &[(0.5, 0.04, 0.47, g), (0.5, 0.53, 0.96, g)],
        3 => &[
            (0.5, 0.04, 0.47, g),
            (0.27, 0.53, 0.96, g),
            (0.73, 0.53, 0.96, g),
        ],
        4 => &[
            (0.27, 0.04, 0.47, g),
            (0.73, 0.04, 0.47, g),
            (0.27, 0.53, 0.96, g),
            (0.73, 0.53, 0.96, g),
        ],
        5 => &[
            (0.18, 0.04, 0.47, g),
            (0.82, 0.04, 0.47, g),
            (0.5, 0.28, 0.72, r),
            (0.18, 0.53, 0.96, g),
            (0.82, 0.53, 0.96, g),
        ],
        6 => &[
            (0.18, 0.04, 0.47, g),
            (0.5, 0.04, 0.47, g),
            (0.82, 0.04, 0.47, g),
            (0.18, 0.53, 0.96, g),
            (0.5, 0.53, 0.96, g),
            (0.82, 0.53, 0.96, g),
        ],
        7 => &[
            (0.5, 0.02, 0.31, r),
            (0.18, 0.35, 0.64, g),
            (0.5, 0.35, 0.64, g),
            (0.82, 0.35, 0.64, g),
            (0.18, 0.68, 0.98, g),
            (0.5, 0.68, 0.98, g),
            (0.82, 0.68, 0.98, g),
        ],
        8 => &[
            (0.13, 0.04, 0.47, g),
            (0.38, 0.04, 0.47, r),
            (0.62, 0.04, 0.47, r),
            (0.87, 0.04, 0.47, g),
            (0.13, 0.53, 0.96, g),
            (0.38, 0.53, 0.96, r),
            (0.62, 0.53, 0.96, r),
            (0.87, 0.53, 0.96, g),
        ],
        _ => &[
            (0.18, 0.02, 0.31, g),
            (0.5, 0.02, 0.31, r),
            (0.82, 0.02, 0.31, g),
            (0.18, 0.35, 0.64, g),
            (0.5, 0.35, 0.64, r),
            (0.82, 0.35, 0.64, g),
            (0.18, 0.68, 0.98, g),
            (0.5, 0.68, 0.98, r),
            (0.82, 0.68, 0.98, g),
        ],
    };
    if n == 1 {
        bird(s, b);
        return;
    }
    for &(u, v0, v1, c) in list {
        stick(s, b, u, v0, v1, c);
    }
}

/// The one of bamboo: a bird with a fanned tail.
fn bird(s: &mut Screen, b: &Area) {
    // the tail: long feathers fanning down to the left
    let root = b.at(0.5, 0.6);
    for (tip, c) in [
        ((0.04, 0.8), BLUE),
        ((0.16, 0.97), GREEN),
        ((0.4, 1.0), RED),
        ((0.06, 0.58), GREEN),
    ] {
        leaf(s, root, b.at(tip.0, tip.1), b.w * 0.11, c);
    }
    // body, wing, head and crest
    let (bx, by) = b.at(0.56, 0.5);
    s.ellipse(bx, by, b.w * 0.26, b.h * 0.17, GREEN, 1.0);
    leaf(s, b.at(0.42, 0.44), b.at(0.8, 0.6), b.w * 0.1, DARK_GREEN);
    let (hx, hy) = b.at(0.72, 0.24);
    s.line(
        bx + b.w * 0.08,
        by - b.h * 0.08,
        hx,
        hy,
        b.w * 0.08,
        GREEN,
        1.0,
    );
    s.disc(hx, hy, b.w * 0.13, GREEN, 1.0);
    leaf(
        s,
        (hx - b.w * 0.02, hy - b.w * 0.1),
        b.at(0.62, 0.02),
        b.w * 0.06,
        RED,
    );
    s.tri(
        (hx + b.w * 0.1, hy - b.w * 0.05),
        (hx + b.w * 0.1, hy + b.w * 0.05),
        (hx + b.w * 0.3, hy + b.w * 0.02),
        hex(0xe59a1a),
        1.0,
    );
    s.disc(hx + b.w * 0.03, hy - b.w * 0.02, b.w * 0.035, INK, 1.0);
    // feet on a branch
    let (lx, ly) = b.at(0.56, 0.72);
    s.line(
        lx - b.w * 0.05,
        ly - b.h * 0.05,
        lx - b.w * 0.07,
        ly + b.h * 0.04,
        b.w * 0.025,
        RED,
        1.0,
    );
    s.line(
        lx + b.w * 0.08,
        ly - b.h * 0.05,
        lx + b.w * 0.08,
        ly + b.h * 0.04,
        b.w * 0.025,
        RED,
        1.0,
    );
}

/// A leaf from its base to its tip, `w` wide at the middle.
fn leaf(s: &mut Screen, base: (f32, f32), tip: (f32, f32), w: f32, c: u32) {
    let (dx, dy) = (tip.0 - base.0, tip.1 - base.1);
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (nx, ny) = (-dy / len * w, dx / len * w);
    let at = |t: f32, k: f32| (base.0 + dx * t + nx * k, base.1 + dy * t + ny * k);
    s.poly(
        &[
            base,
            at(0.25, 0.75),
            at(0.5, 1.0),
            at(0.75, 0.75),
            tip,
            at(0.75, -0.75),
            at(0.5, -1.0),
            at(0.25, -0.75),
        ],
        c,
        1.0,
    );
}

/// The numerals one to nine and the sign for ten thousand, from the font.
const NUMERALS: [&str; 9] = ["一", "二", "三", "四", "五", "六", "七", "八", "九"];
const WAN: &str = "萬";
const WIND_SIGNS: [&str; 4] = ["東", "南", "西", "北"];
const WIND_LETTERS: [&str; 4] = ["E", "S", "W", "N"];

fn characters(s: &mut Screen, b: &Area, n: u8) {
    let (x, y) = b.at(0.56, 0.24);
    s.text_c(NUMERALS[n as usize - 1], x, y, b.w * 0.72, INK, 1.0);
    let (x, y) = b.at(0.5, 0.75);
    s.text_c(WAN, x, y, b.w * 0.88, RED, 1.0);
    index(s, b, INDEX[n as usize], BLUE);
}

fn wind(s: &mut Screen, b: &Area, n: u8) {
    let (x, y) = b.at(0.53, 0.55);
    s.text_c(WIND_SIGNS[n as usize], x, y, b.w * 1.08, INK, 1.0);
    index(s, b, WIND_LETTERS[n as usize], RED);
}

fn dragon(s: &mut Screen, b: &Area, n: u8) {
    let (x, y) = b.at(0.5, 0.52);
    match n {
        0 => s.text_c("中", x, y, b.w * 1.12, RED, 1.0),
        1 => s.text_c("發", x, y, b.w * 1.1, GREEN, 1.0),
        _ => {
            // the white dragon: a blue frame
            let (fw, fh) = (b.w * 0.9, b.h * 0.86);
            let (fx, fy) = (b.x + (b.w - fw) / 2.0, b.y + (b.h - fh) / 2.0);
            let t = b.w * 0.09;
            s.rect(fx, fy, fw, fh, t, BLUE, 1.0);
            s.rect(
                fx + t,
                fy + t,
                fw - 2.0 * t,
                fh - 2.0 * t,
                t * 0.5,
                hex(0xfffaf0),
                1.0,
            );
            let i = t * 1.9;
            s.rect(
                fx + i,
                fy + i,
                fw - 2.0 * i,
                fh - 2.0 * i,
                t * 0.5,
                BLUE,
                1.0,
            );
            let j = i + t * 0.7;
            s.rect(
                fx + j,
                fy + j,
                fw - 2.0 * j,
                fh - 2.0 * j,
                t * 0.3,
                hex(0xfffaf0),
                1.0,
            );
        }
    }
}

/// The four flowers: a stem, a leaf and a bloom of a colour and shape each.
fn flower(s: &mut Screen, b: &Area, n: u8) {
    let (petals, colour, centre, size) = match n {
        0 => (5, hex(0xe0457b), hex(0xf6d04d), 0.2),   // plum
        1 => (3, hex(0x8a3fb0), hex(0xf6d04d), 0.25),  // orchid
        2 => (12, hex(0xe5a017), hex(0x9a5b0a), 0.12), // chrysanthemum
        _ => (8, hex(0xf07a6a), hex(0xf6d04d), 0.16),  // lotus
    };
    s.line(
        b.at(0.5, 0.45).0,
        b.at(0.5, 0.45).1,
        b.at(0.46, 0.98).0,
        b.at(0.46, 0.98).1,
        b.w * 0.05,
        DARK_GREEN,
        1.0,
    );
    leaf(s, b.at(0.48, 0.8), b.at(0.08, 0.6), b.w * 0.13, GREEN);
    leaf(s, b.at(0.47, 0.88), b.at(0.9, 0.7), b.w * 0.11, GREEN);
    let (cx, cy) = b.at(0.5, 0.34);
    let ring = b.w * if petals > 8 { 0.3 } else { 0.24 };
    for k in 0..petals {
        let a =
            k as f32 * 2.0 * core::f32::consts::PI / petals as f32 - core::f32::consts::PI / 2.0;
        let (px, py) = (cx + a.cos() * ring, cy + a.sin() * ring);
        s.disc(px, py, b.w * size, colour, 1.0);
        s.disc(
            px - a.cos() * b.w * 0.03,
            py - a.sin() * b.w * 0.03,
            b.w * size * 0.45,
            mix(colour, hex(0xffffff), 0.35),
            1.0,
        );
    }
    s.disc(cx, cy, b.w * 0.11, centre, 1.0);
    index(s, b, INDEX[n as usize + 1], RED);
}

/// The four seasons, each on a pale round: a sprout, the sun, a falling
/// leaf, a snowflake.
fn season(s: &mut Screen, b: &Area, n: u8) {
    let (cx, cy) = b.at(0.5, 0.56);
    let r = b.w * 0.5;
    let pale = [hex(0xdff2d8), hex(0xfde8c4), hex(0xf9dcc8), hex(0xdcebfa)][n as usize];
    s.disc(cx, cy, r, pale, 1.0);
    match n {
        0 => {
            s.line(
                cx,
                cy + r * 0.75,
                cx,
                cy - r * 0.1,
                r * 0.08,
                DARK_GREEN,
                1.0,
            );
            leaf(s, (cx, cy), (cx - r * 0.75, cy - r * 0.55), r * 0.24, GREEN);
            leaf(
                s,
                (cx, cy - r * 0.08),
                (cx + r * 0.7, cy - r * 0.7),
                r * 0.24,
                GREEN,
            );
        }
        1 => {
            let c = hex(0xf08a12);
            for k in 0..8 {
                let a = k as f32 * core::f32::consts::PI / 4.0;
                s.line(
                    cx + a.cos() * r * 0.55,
                    cy + a.sin() * r * 0.55,
                    cx + a.cos() * r * 0.82,
                    cy + a.sin() * r * 0.82,
                    r * 0.08,
                    c,
                    1.0,
                );
            }
            s.disc(cx, cy, r * 0.42, c, 1.0);
            s.disc(cx - r * 0.1, cy - r * 0.1, r * 0.2, hex(0xffc94a), 1.0);
        }
        2 => {
            // a maple leaf: five pointed lobes and a stem
            let c = hex(0xd2451e);
            s.line(
                cx,
                cy + r * 0.05,
                cx + r * 0.08,
                cy + r * 0.8,
                r * 0.07,
                mix(c, hex(0x000000), 0.3),
                1.0,
            );
            for (k, len) in [(0, 0.78f32), (1, 0.66), (4, 0.66), (2, 0.5), (3, 0.5)] {
                let a = -core::f32::consts::PI / 2.0
                    + (k as f32 - if k > 2 { 5.0 } else { 0.0 }) * 1.15;
                leaf(
                    s,
                    (cx, cy + r * 0.05),
                    (cx + a.cos() * r * len, cy + r * 0.05 + a.sin() * r * len),
                    r * 0.2,
                    c,
                );
            }
            s.disc(cx, cy + r * 0.05, r * 0.16, c, 1.0);
        }
        _ => {
            let c = hex(0x2a78c8);
            for k in 0..3 {
                let a = k as f32 * core::f32::consts::PI / 3.0 + core::f32::consts::PI / 2.0;
                let (dx, dy) = (a.cos() * r * 0.8, a.sin() * r * 0.8);
                s.line(cx - dx, cy - dy, cx + dx, cy + dy, r * 0.07, c, 1.0);
                for sgn in [-1.0f32, 1.0] {
                    // a little V near each end
                    let (ex, ey) = (cx + dx * 0.62 * sgn, cy + dy * 0.62 * sgn);
                    for side in [-0.6f32, 0.6] {
                        let b2 = a
                            + side
                            + if sgn > 0.0 {
                                0.0
                            } else {
                                core::f32::consts::PI
                            };
                        s.line(
                            ex,
                            ey,
                            ex + b2.cos() * r * 0.25,
                            ey + b2.sin() * r * 0.25,
                            r * 0.055,
                            c,
                            1.0,
                        );
                    }
                }
            }
            s.disc(cx, cy, r * 0.12, c, 1.0);
        }
    }
    index(s, b, INDEX[n as usize + 1], BLUE);
}
