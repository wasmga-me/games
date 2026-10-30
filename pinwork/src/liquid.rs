//! Water, lava and stone drawn as one body each: every drop adds a soft
//! bump to a field, and the field's outline is the liquid's surface, drawn
//! a pixel at a time with a dark edge and a lit top.

use crate::levels::{FLOOR, FRAME_R, FRAME_TOP, LAVA, STONE, WATER};
use crate::sim::Grain;
use crate::PLAN;
use wg::{blend, hex, mix, Screen};

/// the field's cells, plan units, where it starts (x 0) and its size: the
/// whole width of the plan, from above the frame to the bottom
const FC: f32 = 1.5;
const FY0: f32 = FRAME_TOP - 20.0;
const FW: usize = (PLAN.0 / FC) as usize + 2;
const FH: usize = ((PLAN.1 - FY0) / FC) as usize + 2;
const _: () = assert!(FRAME_R < PLAN.0 && FLOOR < PLAN.1);
/// how far a drop's bump reaches, plan units, and the field's value at the
/// surface
const REACH: f32 = 6.5;
const SURFACE: f32 = 0.45;
/// the dark edge and the lit band under the top, plan units
const EDGE: f32 = 0.9;
const BAND: f32 = 2.2;

struct Look {
    edge: u32,
    top: u32,
    body: u32,
    /// how see-through the body is
    alpha: f32,
}

const LOOKS: [(u8, Look); 3] = [
    (WATER, Look { edge: hex(0x1452a8), top: hex(0x9fe3ff), body: hex(0x3fa9f5), alpha: 0.86 }),
    (LAVA, Look { edge: hex(0x8c1a04), top: hex(0xffe14a), body: hex(0xff8a1c), alpha: 1.0 }),
    (STONE, Look { edge: hex(0x23242b), top: hex(0xa7acb6), body: hex(0x6e737d), alpha: 1.0 }),
];

/// One field: its values, and each row's first and last cell touched.
struct Field {
    v: Vec<f32>,
    span: Vec<(usize, usize)>,
}

impl Field {
    fn new() -> Field {
        Field { v: vec![0.0; FW * FH], span: vec![(FW, 0); FH] }
    }
    fn clear(&mut self) {
        for (y, s) in self.span.iter_mut().enumerate() {
            if s.0 <= s.1 {
                self.v[y * FW + s.0..=y * FW + s.1].fill(0.0);
            }
            *s = (FW, 0);
        }
    }
    /// Adds a drop's bump at plan p.
    fn add(&mut self, p: (f32, f32)) {
        let (fx, fy) = (p.0 / FC, (p.1 - FY0) / FC);
        let r = REACH / FC;
        let (x0, x1) = ((fx - r).ceil().max(0.0) as i32, (fx + r).floor().min((FW - 1) as f32) as i32);
        let (y0, y1) = ((fy - r).ceil().max(0.0) as i32, (fy + r).floor().min((FH - 1) as f32) as i32);
        if x1 < x0 || y1 < y0 {
            return;
        }
        let (x0, x1, y0, y1) = (x0 as usize, x1 as usize, y0 as usize, y1 as usize);
        let inv = 1.0 / (r * r);
        for y in y0..=y1 {
            let dy = y as f32 - fy;
            let row = &mut self.v[y * FW..(y + 1) * FW];
            let mut touched = false;
            for (x, v) in row.iter_mut().enumerate().take(x1 + 1).skip(x0) {
                let dx = x as f32 - fx;
                let w = 1.0 - (dx * dx + dy * dy) * inv;
                if w > 0.0 {
                    *v += w * w;
                    touched = true;
                }
            }
            if touched {
                let s = &mut self.span[y];
                *s = (s.0.min(x0), s.1.max(x1));
            }
        }
    }
}

pub struct Liquids {
    fields: [Field; 3],
}

impl Liquids {
    pub fn new() -> Liquids {
        Liquids { fields: [Field::new(), Field::new(), Field::new()] }
    }

    pub fn draw(&mut self, s: &mut Screen, grains: &[Grain]) {
        for f in &mut self.fields {
            f.clear();
        }
        for g in grains {
            if let Some(i) = LOOKS.iter().position(|(kind, _)| *kind == g.kind) {
                self.fields[i].add(g.p);
            }
        }
        for (f, (_, look)) in self.fields.iter().zip(LOOKS.iter()) {
            draw_field(s, f, look);
        }
    }
}

/// The screen pixels (along one axis) whose middles fall between field
/// points n and n + 1, the field starting at plan `base`.
fn pixels(o: f32, k: f32, base: f32, n: usize, max: i32) -> (i32, i32) {
    let at = |n: usize| ((o + (base + n as f32 * FC) * k - 0.5).ceil() as i32).clamp(0, max);
    (at(n), at(n + 1))
}

fn draw_field(s: &mut Screen, f: &Field, look: &Look) {
    let (k, ox, oy) = (s.k, s.ox, s.oy);
    let (sw, sh) = (s.sw, s.sh);
    let px = 1.0 / k;
    // a pixel in field cells, a field cell in pixels, and the bands in pixels
    let (dt, kfc) = (1.0 / (k * FC), k * FC);
    let (band_px, edge_px) = (BAND * k, EDGE * k);
    for cy in 0..FH - 1 {
        let (a, b) = (f.span[cy], f.span[cy + 1]);
        let (c0, c1) = (a.0.min(b.0), a.1.max(b.1).min(FW - 2));
        if c0 > c1 {
            continue;
        }
        let (j0, j1) = pixels(oy, k, FY0, cy, sh);
        if j0 >= j1 {
            continue;
        }
        let (r0, r1) = (&f.v[cy * FW..(cy + 1) * FW], &f.v[(cy + 1) * FW..(cy + 2) * FW]);
        for cx in c0..=c1 {
            let (v00, v10, v01, v11) = (r0[cx], r0[cx + 1], r1[cx], r1[cx + 1]);
            let lo = v00.min(v10).min(v01).min(v11);
            let hi = v00.max(v10).max(v01).max(v11);
            if hi < SURFACE * 0.5 {
                continue;
            }
            let (i0, i1) = pixels(ox, k, 0.0, cx, sw);
            if i0 >= i1 {
                continue;
            }
            // outside everywhere in the cell: nothing; deep inside
            // everywhere (past the lit band, where there is one): one colour
            let steep = (hi - lo) / FC * 1.5 + 1e-3;
            if (hi - SURFACE) / steep < -px {
                continue;
            }
            let lit = v01 > v00 || v11 > v10;
            if (lo - SURFACE) / steep > if lit { BAND } else { EDGE } + 2.0 * px {
                for j in j0..j1 {
                    s.span_px(j, i0 as f32, i1 as f32, look.body, look.alpha);
                }
                continue;
            }
            let tx0 = (i0 as f32 + 0.5 - ox) / k / FC - cx as f32;
            for j in j0..j1 {
                let ty = ((j as f32 + 0.5 - oy) / k - FY0) / FC - cy as f32;
                let left = v00 + (v01 - v00) * ty;
                let right = v10 + (v11 - v10) * ty;
                let gx = right - left;
                let row = s.row_px(j);
                let mut tx = tx0;
                for p in &mut row[i0 as usize..i1 as usize] {
                    let v = left + (right - left) * tx;
                    let t = tx;
                    tx += dt;
                    if v < SURFACE * 0.5 {
                        continue;
                    }
                    // how far inside the surface, in pixels, from the slope
                    let gy = (v01 - v00) + (v11 - v10 - (v01 - v00)) * t;
                    let slope = (gx * gx + gy * gy).sqrt().max(1e-3);
                    let depth = (v - SURFACE) * kfc / slope;
                    if depth <= -0.5 {
                        continue;
                    }
                    let cover = (depth + 0.5).min(1.0);
                    if depth > band_px + 0.5 {
                        blend(p, look.body, (cover * look.alpha * 256.0) as u32);
                        continue;
                    }
                    // a lit band under the top surface, where the field rises
                    // downward, and a dark edge
                    let up = (gy / slope * 1.6 - 0.4).clamp(0.0, 1.0);
                    let band = (band_px - depth + 0.5).clamp(0.0, 1.0) * up;
                    let edge = (edge_px - depth + 0.5).clamp(0.0, 1.0);
                    let mut c = look.body;
                    if band > 0.0 {
                        c = mix(look.top, c, band);
                    }
                    if edge > 0.0 {
                        c = mix(look.edge, c, edge);
                    }
                    let alpha = look.alpha + (1.0 - look.alpha) * edge.max(band);
                    blend(p, c, (cover * alpha * 256.0) as u32);
                }
            }
        }
    }
}
