//! The tiles: a block with an ivory face on a coloured back, with its
//! picture from art.rs on the face.

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
    // the picture, 3:4, centred on the face
    let pw = (w * 0.88).min(h * 0.9 * 0.75);
    let ph = pw / 0.75;
    crate::art::draw(s, face, x + (w - pw) / 2.0, y + (h - ph) / 2.0, pw, ph);
}
