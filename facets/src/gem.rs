//! Cut gems, shaded pixel by pixel: every facet of the crown has its own
//! slant, so it catches the light (and the room around it) on its own, and
//! the table shows the pavilion facets underneath, lit from the far side as
//! light bounces back out of the stone. A window reflection and a star glint
//! sit on top. Each gem is drawn once per scale into a premultiplied image,
//! then copied (or scaled, while it pops) every frame.

use core::f32::consts::TAU;

/// colour as three floats 0..1
#[derive(Clone, Copy)]
struct C3(f32, f32, f32);

impl C3 {
    fn hex(c: u32) -> C3 {
        C3(((c >> 16) & 0xFF) as f32 / 255.0, ((c >> 8) & 0xFF) as f32 / 255.0, (c & 0xFF) as f32 / 255.0)
    }
    fn mix(self, o: C3, t: f32) -> C3 {
        C3(self.0 + (o.0 - self.0) * t, self.1 + (o.1 - self.1) * t, self.2 + (o.2 - self.2) * t)
    }
    fn add(self, o: C3, k: f32) -> C3 {
        C3(self.0 + o.0 * k, self.1 + o.1 * k, self.2 + o.2 * k)
    }
    fn scale(self, k: f32) -> C3 {
        C3(self.0 * k, self.1 * k, self.2 * k)
    }
}

const WHITE: C3 = C3(1.0, 1.0, 1.0);

fn norm3(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let l = (x * x + y * y + z * z).sqrt().max(1e-6);
    (x / l, y / l, z / l)
}
fn dot3(a: (f32, f32, f32), b: (f32, f32, f32)) -> f32 {
    a.0 * b.0 + a.1 * b.1 + a.2 * b.2
}
fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// the light: from the top left, in front (screen y points down)
fn light() -> (f32, f32, f32) {
    norm3(-0.45, -0.65, 0.62)
}

/// one gem's outline and colours, in unit space round its middle
pub struct Cut<'a> {
    pub pts: &'a [(f32, f32)],
    /// a round outline (a brilliant) instead of the polygon
    pub round: bool,
    /// a step cut (bands round a flat table) instead of a brilliant's star
    pub step: bool,
    pub base: u32,
    pub light: u32,
    pub dark: u32,
}

/// table centre and size, as a share of the outline
const TABLE: f32 = 0.5;
const TABLE_DY: f32 = -0.06;

struct Geo {
    n: usize,
    /// outline points, outward edge normals and their offsets
    p: [(f32, f32); 8],
    en: [(f32, f32); 8],
    eo: [f32; 8],
    /// direction of each point from the table centre, as an angle
    ang: [f32; 8],
}

fn geo(cut: &Cut) -> Geo {
    let n = cut.pts.len().min(8);
    let mut g = Geo { n, p: [(0.0, 0.0); 8], en: [(0.0, 0.0); 8], eo: [0.0; 8], ang: [0.0; 8] };
    let mut area = 0.0;
    for i in 0..n {
        let (a, b) = (cut.pts[i], cut.pts[(i + 1) % n]);
        area += a.0 * b.1 - b.0 * a.1;
    }
    let sgn = if area > 0.0 { 1.0 } else { -1.0 };
    for i in 0..n {
        g.p[i] = cut.pts[i];
        let (a, b) = (cut.pts[i], cut.pts[(i + 1) % n]);
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let l = (ex * ex + ey * ey).sqrt().max(1e-6);
        let nn = (ey / l * sgn, -ex / l * sgn);
        g.en[i] = nn;
        g.eo[i] = nn.0 * a.0 + nn.1 * a.1;
        g.ang[i] = (a.1 - TABLE_DY).atan2(a.0);
    }
    g
}

/// signed distance to the outline in units (negative inside)
fn outline(cut: &Cut, g: &Geo, x: f32, y: f32) -> f32 {
    if cut.round {
        return (x * x + y * y).sqrt() - 0.96;
    }
    let mut d = -1e9f32;
    for i in 0..g.n {
        d = d.max(g.en[i].0 * x + g.en[i].1 * y - g.eo[i]);
    }
    d
}

/// which crown sector a direction falls in: i, with t from 0 at point i to
/// 1 at point i + 1
fn sector(g: &Geo, a: f32) -> (usize, f32) {
    for i in 0..g.n {
        let a0 = g.ang[i];
        // the step to the next point, the way the outline runs (either winding)
        let wrap = |mut d: f32| {
            while d <= -core::f32::consts::PI { d += TAU; }
            while d > core::f32::consts::PI { d -= TAU; }
            d
        };
        let t = wrap(a - a0) / wrap(g.ang[(i + 1) % g.n] - a0);
        if (0.0..=1.0).contains(&t) {
            return (i, t);
        }
    }
    (0, 0.5)
}

/// a number 0..1 that differs from facet to facet
fn facet_noise(i: usize) -> f32 {
    let v = (i as f32 * 12.9898 + 4.1).sin() * 43_758.547;
    v - v.floor()
}

/// colour of one sample at unit position x, y
fn shade(cut: &Cut, g: &Geo, x: f32, y: f32) -> C3 {
    let (base, lite, dark) = (C3::hex(cut.base), C3::hex(cut.light), C3::hex(cut.dark));
    let l = light();
    let h = norm3(l.0, l.1, l.2 + 1.0);
    // how far in from the outline
    let edge = -outline(cut, g, x, y);
    // the facet under this point: its 2D outward direction, how steep it is,
    // whether it is the table, an id, and how far to its nearest seam
    let (dir, slope, table, id, seam);
    if cut.step {
        // a step cut: bands running round the stone, each less steep
        let mut best = (0usize, -1e9f32);
        for i in 0..g.n {
            let d = g.en[i].0 * x + g.en[i].1 * y - g.eo[i];
            if d > best.1 {
                best = (i, d);
            }
        }
        // the seam between two sides runs along the bisector: how close
        let mut second = -1e9f32;
        for i in 0..g.n {
            if i != best.0 {
                second = second.max(g.en[i].0 * x + g.en[i].1 * y - g.eo[i]);
            }
        }
        const STEP: f32 = 0.13;
        let band = (edge / STEP).floor().clamp(0.0, 3.0) as usize;
        let into = edge - band as f32 * STEP;
        table = band >= 3;
        dir = g.en[best.0];
        slope = [0.95, 0.6, 0.32, 0.0][band];
        id = best.0 * 4 + band;
        seam = if table { edge - 3.0 * STEP } else { into.min(STEP - into).min((best.1 - second).abs() * 0.7) };
    } else {
        let (tx, ty) = (x / TABLE, (y - TABLE_DY) / TABLE);
        let tab = -outline(cut, g, tx, ty) * TABLE;
        let ang = (y - TABLE_DY).atan2(x);
        let (i, t) = sector(g, ang);
        let reach = (x * x + (y - TABLE_DY) * (y - TABLE_DY)).sqrt() * TAU / g.n as f32;
        table = tab > 0.0;
        if table {
            // the pavilion seen through the table: twice as many facets
            let k = i * 2 + (t >= 0.5) as usize;
            let u = if t >= 0.5 { t * 2.0 - 1.0 } else { t * 2.0 };
            let a = ang + (0.5 - u) * 0.5;
            dir = (a.cos(), a.sin());
            slope = 0.0;
            id = k;
            seam = tab.min(u.min(1.0 - u) * reach * 0.5);
        } else {
            // a crown facet, split at its middle so each half tilts
            // towards its own corner
            let pc = g.p[if t < 0.5 { i } else { (i + 1) % g.n }];
            let pl = (pc.0 * pc.0 + pc.1 * pc.1).sqrt().max(1e-4);
            let (nx, ny) = (g.en[i].0 + pc.0 / pl * 0.5, g.en[i].1 + pc.1 / pl * 0.5);
            let k = (nx * nx + ny * ny).sqrt().max(1e-4);
            dir = (nx / k, ny / k);
            slope = 0.85;
            id = 100 + i * 2 + (t >= 0.5) as usize;
            // the seams between facets matter more than the split in each
            seam = (-tab).min(t.min(1.0 - t) * reach).min((t - 0.5).abs() * reach * 2.5);
        }
    }
    let var = facet_noise(id + cut.base as usize % 7);
    let mut c;
    if table && !cut.step {
        // pavilion facets slope down and in; light comes back up through them
        let np = norm3(-dir.0 * 0.75, -dir.1 * 0.75, 0.66);
        let back = (-l.0, -l.1, l.2);
        let lit = (dot3(np, back) * 0.75 + var * 0.45 - 0.1).clamp(0.0, 1.0);
        c = dark.mix(base, 0.4 + 0.6 * lit).add(lite, 0.7 * lit.powi(5));
    } else {
        let n = norm3(dir.0 * slope, dir.1 * slope, 1.0);
        let diff = (dot3(n, l) * 0.9 + var * 0.35 - 0.1).clamp(0.0, 1.2);
        let spec = dot3(n, h).max(0.0).powi(60);
        // the room reflected in the facet: bright above, dark below
        let ry = 2.0 * n.2 * n.1;
        let sky = smooth(0.0, -0.6, ry);
        c = dark.mix(base, 0.2 + 0.8 * smooth(0.1, 0.9, diff)).add(lite, 0.22 * sky + 0.25 * (diff - 1.0).max(0.0) * 4.0).add(WHITE, 0.85 * spec);
        if table {
            // a step cut's table: deep, and darker towards the light
            c = c.mix(dark, 0.25 + 0.2 * smooth(-0.5, 0.5, -(x * 0.6 + y * 0.8)));
        }
    }
    // facet seams catch a thin line of light
    let line = 1.0 - smooth(0.0, 0.02, seam.abs());
    c = c.mix(lite, if table && !cut.step { 0.1 } else { 0.3 } * line);
    // the girdle: dark at the very edge, with light coming through the far side
    let rim = 1.0 - smooth(0.0, 0.06, edge);
    let far = ((x * 0.55 + y * 0.8) / (x * x + y * y).sqrt().max(1e-4)).max(0.0);
    c = c.mix(dark.scale(0.7), 0.5 * rim).add(lite, 0.5 * rim * far * far);
    // a window reflected on the upper left: a soft pane of light
    let w = x * 0.62 + y * 0.78;
    let band = smooth(-0.66, -0.56, w) * (1.0 - smooth(-0.44, -0.34, w));
    let cross = 1.0 - smooth(0.0, 0.03, ((x * 0.78 - y * 0.62) + 0.02).abs() - 0.02);
    c = c.add(WHITE, 0.16 * band * (1.0 - cross * 0.8));
    c
}

/// the star glint: brightness at x, y (units), for adding over the gem
fn glint(x: f32, y: f32) -> f32 {
    let (gx, gy) = (x + 0.36, y + 0.46);
    let (ax, ay) = (gx.abs(), gy.abs());
    let arm = |a: f32, b: f32| (1.0 - b * 30.0 - a * 3.4).max(0.0);
    let core = (1.0 - (gx * gx + gy * gy).sqrt() * 9.0).max(0.0);
    // the diagonal arms are shorter and fainter
    let (dx, dy) = ((gx + gy) * 0.7071, (gx - gy) * 0.7071);
    let diag = (1.0 - dy.abs() * 34.0 - dx.abs() * 7.0).max(0.0) + (1.0 - dx.abs() * 34.0 - dy.abs() * 7.0).max(0.0);
    (arm(ax, ay) + arm(ay, ax) + 0.45 * diag + core * core * 1.6).min(1.0)
}

/// a premultiplied picture of a gem, `size` pixels square, radius r pixels
pub struct Image {
    pub size: i32,
    pub px: Vec<u32>,
}

pub fn render(cut: &Cut, size: i32, r: f32) -> Image {
    let g = geo(cut);
    let mut px = vec![0u32; (size * size) as usize];
    let mid = size as f32 / 2.0;
    // four samples a pixel, on a rotated grid
    const SS: [(f32, f32); 4] = [(-0.375, -0.125), (0.125, -0.375), (0.375, 0.125), (-0.125, 0.375)];
    for j in 0..size {
        for i in 0..size {
            let (x, y) = ((i as f32 + 0.5 - mid) / r, (j as f32 + 0.5 - mid) / r);
            let star = glint(x, y);
            let d = outline(cut, &g, x, y) * r;
            let cov = (0.5 - d).clamp(0.0, 1.0);
            if cov <= 0.0 && star <= 0.0 {
                continue;
            }
            let mut c = C3(0.0, 0.0, 0.0);
            if cov > 0.0 {
                for (sx, sy) in SS {
                    c = c.add(shade(cut, &g, x + sx / r, y + sy / r), 0.25);
                }
            }
            // premultiplied: the gem at its coverage, then the glint added
            let a = (cov + star * (1.0 - cov)).clamp(0.0, 1.0);
            let col = c.scale(cov).add(WHITE, star);
            let ch = |v: f32| (v.clamp(0.0, a) * 255.0 + 0.5) as u32;
            px[(j * size + i) as usize] = ((a * 255.0 + 0.5) as u32) << 24 | ch(col.2) << 16 | ch(col.1) << 8 | ch(col.0);
        }
    }
    Image { size, px }
}

/// copies img centred on pixel cx, cy, scaled by z and faded by alpha,
/// inside the pixel box clip (x0, y0, x1, y1)
pub fn blit(s: &mut wg::Screen, img: &Image, cx: f32, cy: f32, z: f32, alpha: f32, clip: (i32, i32, i32, i32)) {
    if z <= 0.01 || alpha <= 0.0 {
        return;
    }
    let half = img.size as f32 / 2.0 * z;
    let (w, h) = s.size_px();
    let x0 = ((cx - half).floor() as i32).max(clip.0).max(0);
    let x1 = ((cx + half).ceil() as i32).min(clip.2).min(w);
    let y0 = ((cy - half).floor() as i32).max(clip.1).max(0);
    let y1 = ((cy + half).ceil() as i32).min(clip.3).min(h);
    let n = img.size;
    let fa = (alpha.clamp(0.0, 1.0) * 256.0) as u32;
    // the plain case: whole pixels, no scale, no fade
    let exact = (z - 1.0).abs() < 1e-3 && fa >= 256;
    let (ox, oy) = ((cx - half + 0.5).floor() as i32, (cy - half + 0.5).floor() as i32);
    for j in y0..y1 {
        let row = s.row_px(j);
        for i in x0..x1 {
            let c = if exact {
                let (u, v) = (i - ox, j - oy);
                if u < 0 || v < 0 || u >= n || v >= n {
                    continue;
                }
                img.px[(v * n + u) as usize]
            } else {
                // bilinear, premultiplied
                let u = (i as f32 + 0.5 - cx) / z + n as f32 / 2.0 - 0.5;
                let v = (j as f32 + 0.5 - cy) / z + n as f32 / 2.0 - 0.5;
                let (iu, iv) = (u.floor() as i32, v.floor() as i32);
                let (fu, fv) = (u - iu as f32, v - iv as f32);
                let get = |a: i32, b: i32| if a < 0 || b < 0 || a >= n || b >= n { 0 } else { img.px[(b * n + a) as usize] };
                let (p00, p10, p01, p11) = (get(iu, iv), get(iu + 1, iv), get(iu, iv + 1), get(iu + 1, iv + 1));
                let w = [(1.0 - fu) * (1.0 - fv), fu * (1.0 - fv), (1.0 - fu) * fv, fu * fv];
                let mut out = 0u32;
                for sh in [0, 8, 16, 24] {
                    let ch = |p: u32| ((p >> sh) & 0xFF) as f32;
                    let v = ch(p00) * w[0] + ch(p10) * w[1] + ch(p01) * w[2] + ch(p11) * w[3];
                    let v = (v * fa as f32 / 256.0 + 0.5) as u32;
                    out |= v.min(255) << sh;
                }
                out
            };
            let a = c >> 24;
            if a == 0 {
                continue;
            }
            let p = &mut row[i as usize];
            if a >= 255 {
                *p = c | 0xFF00_0000;
                continue;
            }
            // out = src + dst * (1 - a), channel pairs at a time
            let ia = 256 - (a + (a >> 7));
            let d = *p;
            let rb = ((c & 0xFF00FF) + (((d & 0xFF00FF) * ia) >> 8)) & 0xFF00FF;
            let gg = ((c & 0x00FF00) + (((d & 0x00FF00) * ia) >> 8)) & 0x00FF00;
            *p = 0xFF00_0000 | rb | gg;
        }
    }
}

/// the star glint alone, for gems drawn another way (the prism)
pub fn render_glint(size: i32, r: f32) -> Image {
    let mut px = vec![0u32; (size * size) as usize];
    let mid = size as f32 / 2.0;
    for j in 0..size {
        for i in 0..size {
            let a = glint((i as f32 + 0.5 - mid) / r, (j as f32 + 0.5 - mid) / r);
            if a > 0.0 {
                let v = (a * 255.0 + 0.5) as u32;
                px[(j * size + i) as usize] = v << 24 | v << 16 | v << 8 | v;
            }
        }
    }
    Image { size, px }
}
