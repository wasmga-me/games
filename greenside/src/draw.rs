//! The picture: a still layer with the ground, the green, sand, water,
//! slopes, the rails and the cup, and over it each frame the bumpers,
//! windmills, gates, the ball, the aim and the score strip.

use crate::golf::*;
use crate::holes::{Area, Hole, Slope};
use wg::{mix, rgb, tr, Screen};

pub struct Theme {
    pub ground: u32,
    pub ground_line: u32,
    pub grass: u32,
    pub stripe: u32,
    pub rail: u32,
    pub rail_hi: u32,
    pub sand: u32,
    pub sand_dot: u32,
    pub water: u32,
    pub water_hi: u32,
    pub bumper: u32,
    pub bumper_hi: u32,
    pub flag: u32,
    pub ink: u32,
    pub dim: u32,
    /// the ground is boards (true) or paving
    pub boards: bool,
}

pub static THEMES: [Theme; N_THEMES as usize] = [
    // Meadow
    Theme {
        ground: rgb(222, 212, 190), ground_line: rgb(206, 194, 168), grass: rgb(92, 178, 86), stripe: rgb(84, 166, 80),
        rail: rgb(140, 92, 54), rail_hi: rgb(186, 134, 86), sand: rgb(236, 214, 158), sand_dot: rgb(212, 186, 124),
        water: rgb(60, 142, 214), water_hi: rgb(150, 206, 246), bumper: rgb(226, 84, 62), bumper_hi: rgb(255, 170, 150),
        flag: rgb(226, 70, 52), ink: rgb(42, 33, 24), dim: rgb(110, 96, 78), boards: false,
    },
    // Desert
    Theme {
        ground: rgb(228, 180, 120), ground_line: rgb(214, 164, 104), grass: rgb(136, 186, 78), stripe: rgb(126, 176, 72),
        rail: rgb(178, 84, 58), rail_hi: rgb(220, 124, 86), sand: rgb(246, 226, 172), sand_dot: rgb(222, 196, 132),
        water: rgb(40, 160, 184), water_hi: rgb(160, 226, 236), bumper: rgb(46, 124, 186), bumper_hi: rgb(150, 200, 240),
        flag: rgb(46, 124, 186), ink: rgb(60, 34, 20), dim: rgb(130, 84, 56), boards: false,
    },
    // Autumn
    Theme {
        ground: rgb(92, 62, 44), ground_line: rgb(76, 50, 35), grass: rgb(108, 152, 60), stripe: rgb(99, 142, 56),
        rail: rgb(214, 132, 46), rail_hi: rgb(244, 174, 92), sand: rgb(228, 196, 140), sand_dot: rgb(200, 164, 104),
        water: rgb(52, 118, 158), water_hi: rgb(130, 186, 220), bumper: rgb(200, 64, 46), bumper_hi: rgb(250, 150, 120),
        flag: rgb(244, 196, 64), ink: rgb(255, 243, 224), dim: rgb(214, 186, 156), boards: true,
    },
    // Night
    Theme {
        ground: rgb(20, 22, 43), ground_line: rgb(30, 33, 62), grass: rgb(30, 112, 80), stripe: rgb(27, 102, 73),
        rail: rgb(122, 102, 255), rail_hi: rgb(192, 180, 255), sand: rgb(186, 168, 122), sand_dot: rgb(156, 138, 96),
        water: rgb(28, 78, 144), water_hi: rgb(90, 160, 255), bumper: rgb(255, 80, 160), bumper_hi: rgb(255, 180, 220),
        flag: rgb(255, 209, 102), ink: rgb(244, 239, 230), dim: rgb(160, 160, 200), boards: true,
    },
];

const BALL: u32 = rgb(250, 250, 246);
const NPART: usize = 64;

#[derive(Clone, Copy, Default)]
struct Part {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max: f32,
    /// 0 a spark, 1 a ring spreading on water
    kind: u8,
    c: u32,
}

/// Animation, not saved.
pub struct Anim {
    parts: [Part; NPART],
    next: usize,
    flash: [f32; 8],
    trail: [(f32, f32); 10],
    /// a new hole: the trail and sparks start again
    pub hole: i32,
}

impl Anim {
    pub fn new() -> Anim {
        Anim { parts: [Part::default(); NPART], next: 0, flash: [0.0; 8], trail: [(-1.0, -1.0); 10], hole: -1 }
    }
    pub fn reset(&mut self) {
        *self = Anim::new();
    }
    fn add(&mut self, p: Part) {
        self.parts[self.next] = p;
        self.next = (self.next + 1) % NPART;
    }
    pub fn busy(&self) -> bool {
        self.parts.iter().any(|p| p.life > 0.0) || self.flash.iter().any(|&f| f > 0.0)
    }
    pub fn tick(&mut self, s: &State, dt: i32) {
        let t = dt as f32 / 1000.0;
        for p in self.parts.iter_mut().filter(|p| p.life > 0.0) {
            p.life -= t;
            p.x += p.vx * t;
            p.y += p.vy * t;
            p.vx *= 1.0 - 3.0 * t;
            p.vy *= 1.0 - 3.0 * t;
        }
        for f in self.flash.iter_mut() {
            *f = (*f - t * 4.0).max(0.0);
        }
        if self.hole != s.hole {
            self.hole = s.hole;
            self.trail = [(-1.0, -1.0); 10];
        }
        // the trail follows a rolling ball and fades once it stops
        let at = if s.phase == ROLL { (s.x, s.y) } else { (-1.0, -1.0) };
        if dt > 0 {
            self.trail.rotate_right(1);
            self.trail[0] = at;
        }
    }
}

pub struct AnimFx<'a>(pub &'a mut Anim, pub usize);

impl Fx for AnimFx<'_> {
    fn bump(&mut self, i: usize, x: f32, y: f32) {
        if let Some(f) = self.0.flash.get_mut(i) {
            *f = 1.0;
        }
        let c = THEMES[self.1].bumper_hi;
        for k in 0..6 {
            let a = k as f32 * core::f32::consts::FRAC_PI_3 + x * 0.1;
            self.0.add(Part { x, y, vx: a.cos() * 90.0, vy: a.sin() * 90.0, life: 0.35, max: 0.35, kind: 0, c });
        }
    }
    fn rail(&mut self, x: f32, y: f32, speed: f32) {
        if speed < 180.0 {
            return;
        }
        let c = THEMES[self.1].rail_hi;
        for k in 0..3 {
            let a = k as f32 * 2.1 + y * 0.13;
            self.0.add(Part { x, y, vx: a.cos() * 60.0, vy: a.sin() * 60.0, life: 0.25, max: 0.25, kind: 0, c });
        }
    }
    fn splash(&mut self, x: f32, y: f32) {
        let c = THEMES[self.1].water_hi;
        for k in 0..3 {
            let life = 0.6 + 0.2 * k as f32;
            self.0.add(Part { x, y, vx: 0.0, vy: 0.0, life, max: life, kind: 1, c });
        }
        for k in 0..8 {
            let a = k as f32 * core::f32::consts::FRAC_PI_4;
            self.0.add(Part { x, y, vx: a.cos() * 70.0, vy: a.sin() * 70.0, life: 0.4, max: 0.4, kind: 0, c });
        }
    }
    fn sunk(&mut self, x: f32, y: f32) {
        let c = THEMES[self.1].flag;
        for k in 0..12 {
            let a = k as f32 * core::f32::consts::FRAC_PI_6;
            let v = if k % 2 == 0 { 120.0 } else { 70.0 };
            self.0.add(Part { x, y, vx: a.cos() * v, vy: a.sin() * v, life: 0.6, max: 0.6, kind: 0, c });
        }
    }
}

/// Fills the green's outline (any shape, even-odd) a row of pixels at a
/// time, in mowed stripes; or in one colour when `stripe` is None.
fn fill_shape(s: &mut Screen, pts: &[(f32, f32)], dx: f32, dy: f32, c: u32, stripe: Option<u32>, alpha: f32) {
    let n = pts.len();
    let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
    let (_, py0) = s.px(0.0, y0 + dy);
    let (_, py1) = s.px(0.0, y1 + dy);
    let (_, h) = s.size_px();
    for j in (py0.floor() as i32).max(0)..(py1.ceil() as i32).min(h) {
        let y = (j as f32 + 0.5 - s.oy) / s.k - dy;
        let mut xs = [0.0f32; 16];
        let mut m = 0;
        for i in 0..n {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            if (a.1 <= y) != (b.1 <= y) && m < xs.len() {
                xs[m] = a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) + dx;
                m += 1;
            }
        }
        let xs = &mut xs[..m];
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(core::cmp::Ordering::Equal));
        for pair in xs.chunks_exact(2) {
            let (xa, xb) = (pair[0], pair[1]);
            let Some(sc) = stripe else {
                s.span_px(j, s.ox + xa * s.k, s.ox + xb * s.k, c, alpha);
                continue;
            };
            // diagonal stripes 26 units wide
            let band = |x: f32| ((x + y * 0.55) / 26.0).floor();
            let mut x = xa;
            while x < xb {
                let b = band(x);
                let end = ((b + 1.0) * 26.0 - y * 0.55).min(xb);
                let col = if b as i32 % 2 == 0 { c } else { sc };
                s.span_px(j, s.ox + x * s.k, s.ox + end * s.k, col, alpha);
                x = end.max(x + 0.01);
            }
        }
    }
}

fn ground(s: &mut Screen, t: &Theme) {
    let (x0, y0, x1, y1) = s.visible();
    s.clear(t.ground);
    if t.boards {
        let mut y = (y0 / 22.0).floor() * 22.0;
        let mut row = 0;
        while y < y1 {
            s.rect(x0, y, x1 - x0, 1.6, 0.0, t.ground_line, 1.0);
            // the boards' ends, staggered from row to row
            let mut x = (x0 / 120.0).floor() * 120.0 + if row % 2 == 0 { 0.0 } else { 60.0 };
            while x < x1 {
                s.rect(x, y, 1.6, 22.0, 0.0, t.ground_line, 1.0);
                x += 120.0;
            }
            y += 22.0;
            row += 1;
        }
    } else {
        let mut y = (y0 / 40.0).floor() * 40.0;
        while y < y1 {
            let mut x = (x0 / 40.0).floor() * 40.0;
            while x < x1 {
                s.rect(x + 1.5, y + 1.5, 37.0, 37.0, 4.0, mix(t.ground, t.ground_line, 0.55), 1.0);
                x += 40.0;
            }
            y += 40.0;
        }
    }
}

fn sand(s: &mut Screen, a: &Area, t: &Theme) {
    s.rect(a.x - 1.5, a.y - 1.5, a.w + 3.0, a.h + 3.0, a.r + 1.5, t.sand_dot, 1.0);
    s.rect(a.x, a.y, a.w, a.h, a.r, t.sand, 1.0);
    // a scatter of grains
    let mut rng = wg::Rng::new(1);
    let mut y = a.y + 5.0;
    while y < a.y + a.h - 4.0 {
        let mut x = a.x + 5.0 + rng.below(3) as f32 * 2.0;
        while x < a.x + a.w - 4.0 {
            let jx = rng.below(5);
            if crate::golf::inside(a, x + jx as f32, y) {
                s.disc(x + jx as f32, y, 0.9, t.sand_dot, 0.8);
            }
            x += 7.0;
        }
        y += 6.0;
    }
}

fn water(s: &mut Screen, a: &Area, t: &Theme) {
    s.rect(a.x - 2.0, a.y - 2.0, a.w + 4.0, a.h + 4.0, a.r + 2.0, mix(t.water, 0, 0.35), 1.0);
    s.rect(a.x, a.y, a.w, a.h, a.r, t.water, 1.0);
    let mut y = a.y + 12.0;
    let mut row = 0;
    while y < a.y + a.h - 6.0 {
        let mut x = a.x + 10.0 + if row % 2 == 0 { 0.0 } else { 14.0 };
        while x < a.x + a.w - 14.0 {
            if crate::golf::inside(a, x, y) && crate::golf::inside(a, x + 10.0, y) {
                s.line(x, y, x + 5.0, y - 2.0, 0.9, t.water_hi, 0.6);
                s.line(x + 5.0, y - 2.0, x + 10.0, y, 0.9, t.water_hi, 0.6);
            }
            x += 28.0;
        }
        y += 14.0;
        row += 1;
    }
}

fn slope(s: &mut Screen, sl: &Slope, t: &Theme) {
    let a = &sl.at;
    s.rect(a.x, a.y, a.w, a.h, a.r, 0, 0.08);
    let l = (sl.ax * sl.ax + sl.ay * sl.ay).sqrt().max(1.0);
    let (ux, uy) = (sl.ax / l, sl.ay / l);
    let (px, py) = (-uy, ux);
    let mut y = a.y + 14.0;
    while y < a.y + a.h - 8.0 {
        let mut x = a.x + 14.0;
        while x < a.x + a.w - 8.0 {
            // a chevron pointing downhill
            let (tx, ty) = (x + ux * 5.0, y + uy * 5.0);
            s.line(x - ux * 3.0 + px * 6.0, y - uy * 3.0 + py * 6.0, tx, ty, 1.4, t.stripe, 1.0);
            s.line(x - ux * 3.0 - px * 6.0, y - uy * 3.0 - py * 6.0, tx, ty, 1.4, t.stripe, 1.0);
            s.line(x - ux * 3.0 + px * 6.0, y - uy * 3.0 + py * 6.0, tx, ty, 0.7, rgb(255, 255, 255), 0.18);
            s.line(x - ux * 3.0 - px * 6.0, y - uy * 3.0 - py * 6.0, tx, ty, 0.7, rgb(255, 255, 255), 0.18);
            x += 26.0;
        }
        y += 26.0;
    }
}

fn rail(s: &mut Screen, ax: f32, ay: f32, bx: f32, by: f32, r: f32, t: &Theme) {
    s.line(ax + 2.5, ay + 3.5, bx + 2.5, by + 3.5, r + 0.5, 0, 0.28);
    s.line(ax, ay, bx, by, r, t.rail, 1.0);
}
fn rail_hi(s: &mut Screen, ax: f32, ay: f32, bx: f32, by: f32, t: &Theme) {
    s.line(ax - 0.8, ay - 1.0, bx - 0.8, by - 1.0, 1.1, t.rail_hi, 0.75);
}

fn still(s: &mut Screen, h: &Hole, t: &Theme) {
    ground(s, t);
    fill_shape(s, h.shape, 4.0, 5.0, 0, None, 0.25);
    fill_shape(s, h.shape, 0.0, 0.0, t.grass, Some(t.stripe), 1.0);
    for sl in h.slopes {
        slope(s, sl, t);
    }
    for a in h.sand {
        sand(s, a, t);
    }
    for a in h.water {
        water(s, a, t);
    }
    // the tee mat
    let (tx, ty) = h.tee;
    s.rect(tx - 12.0, ty - 9.0, 24.0, 18.0, 4.0, mix(t.grass, 0, 0.3), 1.0);
    s.disc(tx - 7.0, ty, 1.6, rgb(255, 255, 255), 0.85);
    s.disc(tx + 7.0, ty, 1.6, rgb(255, 255, 255), 0.85);
    // the cup
    let (cx, cy) = h.cup;
    s.disc(cx, cy, CUP_R + 2.0, mix(t.grass, rgb(255, 255, 255), 0.25), 1.0);
    s.disc(cx, cy, CUP_R, rgb(22, 24, 20), 1.0);
    s.disc(cx + 1.0, cy + 1.5, CUP_R - 2.0, rgb(8, 9, 8), 1.0);
    // rails: shadows first, so every joint shows clean
    for (ax, ay, bx, by) in h.rails() {
        s.line(ax + 2.5, ay + 3.5, bx + 2.5, by + 3.5, RAIL + 0.5, 0, 0.28);
    }
    for (ax, ay, bx, by) in h.rails() {
        s.line(ax, ay, bx, by, RAIL, t.rail, 1.0);
    }
    for (ax, ay, bx, by) in h.rails() {
        rail_hi(s, ax, ay, bx, by, t);
    }
}

fn flag(s: &mut Screen, h: &Hole, t: &Theme, lift: f32) {
    let (cx, cy) = h.cup;
    let top = cy - 34.0 - lift;
    s.line(cx, cy, cx + 13.0, cy - 6.0, 1.3, 0, 0.18);
    s.line(cx + 1.0, cy - lift, cx + 3.0, top, 1.2, rgb(240, 240, 236), 1.0);
    s.tri((cx + 3.0, top), (cx + 21.0, top + 6.0), (cx + 3.0, top + 12.0), t.flag, 1.0);
}

const PAR: [&str; 10] = ["Par", "标准杆", "パー", "파", "Par", "Par", "Par", "Par", "Пар", "標準桿"];

/// The name of a hole's result: Birdie, Par, Bogey...
fn result_name(strokes: i32, par: i32) -> String {
    if strokes > MAX_STROKES {
        return tr(&["Out of strokes", "杆数用完", "打数切れ", "타수 초과", "Sin golpes", "Sem tacadas", "Plus de coups", "Keine Schläge mehr", "Удары кончились", "桿數用完"]).into();
    }
    if strokes == 1 {
        return tr(&["Hole in one!", "一杆进洞！", "ホールインワン！", "홀인원!", "¡Hoyo en uno!", "Hole in one!", "Trou en un !", "Hole-in-One!", "Хол-ин-ван!", "一桿進洞！"]).into();
    }
    match strokes - par {
        -3 => tr(&["Albatross", "信天翁", "アルバトロス", "알바트로스", "Albatros", "Albatroz", "Albatros", "Albatros", "Альбатрос", "信天翁"]).into(),
        -2 => tr(&["Eagle", "老鹰球", "イーグル", "이글", "Eagle", "Eagle", "Eagle", "Eagle", "Игл", "老鷹"]).into(),
        -1 => tr(&["Birdie", "小鸟球", "バーディー", "버디", "Birdie", "Birdie", "Birdie", "Birdie", "Бёрди", "博蒂"]).into(),
        0 => tr(&PAR).into(),
        1 => tr(&["Bogey", "柏忌", "ボギー", "보기", "Bogey", "Bogey", "Bogey", "Bogey", "Боги", "柏忌"]).into(),
        2 => tr(&["Double bogey", "双柏忌", "ダブルボギー", "더블 보기", "Doble bogey", "Bogey duplo", "Double bogey", "Doppel-Bogey", "Дабл-боги", "雙柏忌"]).into(),
        d => format!("+{d}"),
    }
}

fn hole_label(n: i32) -> String {
    match wg::lang() {
        wg::ZH | wg::ZT => format!("第 {n} 洞"),
        wg::JA => format!("{n}番ホール"),
        wg::KO => format!("{n}번 홀"),
        _ => format!("{} {n}", tr(&["Hole", "", "", "", "Hoyo", "Buraco", "Trou", "Bahn", "Лунка"])),
    }
}

fn to_par(d: i32) -> String {
    match d {
        0 => "±0".into(),
        d if d > 0 => format!("+{d}"),
        d => format!("{d}"),
    }
}

pub fn draw(s: &mut Screen, st: &State, a: &Anim, d: &Drag) {
    let ti = (st.opt[O_THEME].clamp(0, N_THEMES - 1)) as usize;
    let t = &THEMES[ti];
    let h = hole(st.hole);
    s.background((st.hole as u32 + 1) * 8 + ti as u32, |s| still(s, h, t));
    let tm = st.hole_ms as f32;

    for (i, &(x, y, r)) in h.bumpers.iter().enumerate() {
        let f = a.flash.get(i).copied().unwrap_or(0.0);
        let r = r + 2.5 * f;
        s.disc(x + 2.0, y + 3.0, r + 1.0, 0, 0.25);
        s.disc(x, y, r, t.bumper, 1.0);
        s.ring(x, y, r - 2.0, 1.6, mix(t.bumper_hi, rgb(255, 255, 255), f), 0.9);
        s.disc(x - r * 0.3, y - r * 0.35, r * 0.28, rgb(255, 255, 255), 0.35 + 0.5 * f);
    }
    for m in h.movers {
        let ((x0, y0, x1, y1), _) = gate(m, tm);
        rail(s, x0, y0, x1, y1, RAIL + 1.0, t);
        rail_hi(s, x0, y0, x1, y1, t);
    }

    // the ball's trail
    for i in 1..a.trail.len() {
        let (p, q) = (a.trail[i - 1], a.trail[i]);
        if p.0 < 0.0 || q.0 < 0.0 {
            break;
        }
        let f = 1.0 - i as f32 / a.trail.len() as f32;
        s.line(p.0, p.1, q.0, q.1, BALL_R * 0.8 * f, BALL, 0.25 * f);
    }

    // the ball (gone into the water while it splashes, dropping into the cup when sunk)
    let shown = match st.phase {
        SPLASH => 0.0,
        SUNK | OVER => (1.0 - st.phase_ms as f32 / 220.0).max(0.0),
        _ => 1.0,
    };
    if shown > 0.0 {
        let r = BALL_R * (0.5 + 0.5 * shown);
        s.disc(st.x + 1.8, st.y + 2.6, r, 0, 0.3 * shown);
        s.disc(st.x, st.y, r, BALL, 1.0);
        s.disc(st.x - 1.4, st.y - 1.6, r * 0.38, rgb(255, 255, 255), 0.9);
        s.ring(st.x, st.y, r, 0.6, rgb(150, 150, 140), 0.35);
    }

    // windmills over the ball, so a ball passing under shows it is under them
    for sp in h.spinners {
        for (x0, y0, x1, y1) in bars(sp, tm) {
            s.line(x0 + 3.0, y0 + 5.0, x1 + 3.0, y1 + 5.0, 3.5, 0, 0.25);
            s.line(x0, y0, x1, y1, 3.0, t.rail, 1.0);
            s.line(x0, y0, x1, y1, 1.0, t.rail_hi, 0.8);
        }
        s.disc(sp.x + 2.0, sp.y + 3.0, HUB_R, 0, 0.3);
        s.disc(sp.x, sp.y, HUB_R, t.rail_hi, 1.0);
        s.disc(sp.x, sp.y, HUB_R - 2.5, t.rail, 1.0);
    }

    // the flag lifts out as the ball comes near
    let (dx, dy) = (st.x - h.cup.0, st.y - h.cup.1);
    let near = ((60.0 - (dx * dx + dy * dy).sqrt()) / 40.0).clamp(0.0, 1.0);
    flag(s, h, t, near * 8.0);

    for p in a.parts.iter().filter(|p| p.life > 0.0) {
        let f = p.life / p.max;
        if p.kind == 1 {
            s.ring(p.x, p.y, 4.0 + (1.0 - f) * 18.0, 1.4, p.c, f);
        } else {
            s.disc(p.x, p.y, 1.2 + 1.3 * f, p.c, f);
        }
    }

    if st.phase == AIM {
        aim(s, st, d, t);
    }
    hud(s, st, t);
    banner(s, st);
}

fn aim(s: &mut Screen, st: &State, drag: &Drag, t: &Theme) {
    let (ux, uy) = (st.aim.cos(), st.aim.sin());
    let len = 18.0 + 120.0 * st.power;
    let hot = mix(rgb(255, 255, 255), rgb(255, 110, 70), st.power);
    let mut d = 11.0;
    while d < len {
        let f = d / len;
        s.disc(st.x + ux * d, st.y + uy * d, 2.6 - 1.2 * f, hot, 0.95 - 0.4 * f);
        d += 8.0;
    }
    if drag.on {
        // where the pull began, and the pull
        s.ring(drag.x0, drag.y0, 6.0, 1.2, t.ink, 0.35);
        s.line(drag.x0, drag.y0, drag.x, drag.y, 1.0, t.ink, 0.3);
    }
}

fn hud(s: &mut Screen, st: &State, t: &Theme) {
    let h = hole(st.hole);
    let y = 20.0;
    let label = hole_label(st.hole + 1);
    let size = s.fit_size(&label, 18.0, 120.0);
    let mut x = 16.0;
    x = s.text(&label, x, y, size, t.ink, 1.0) + 16.0;
    let par = format!("{} {}", tr(&PAR), h.par);
    x = s.text(&par, x, y, 15.0, t.dim, 1.0) + 16.0;
    let strokes = format!("{} {}", tr(&["Strokes", "杆数", "打数", "타수", "Golpes", "Tacadas", "Coups", "Schläge", "Удары", "桿數"]), st.strokes);
    let size = s.fit_size(&strokes, 15.0, 130.0);
    s.text(&strokes, x, y, size, t.dim, 1.0);
    let r = PW - 16.0;
    let score = format!("{}", st.score);
    let w = s.text_width(&score, 18.0);
    s.text_r(&score, r, y, 18.0, t.ink, 1.0);
    if st.played > 0 {
        s.text_r(&to_par(st.to_par()), r - w - 12.0, y, 15.0, t.dim, 1.0);
    }
}

fn banner(s: &mut Screen, st: &State) {
    let (ms, total) = match st.phase {
        SUNK | PICKED => (st.phase_ms, SUNK_MS),
        _ => return,
    };
    let par = hole(st.cleared).par;
    let f = (ms as f32 / 180.0).min(1.0).min((total - ms) as f32 / 250.0).clamp(0.0, 1.0);
    let name = result_name(st.last_strokes, par);
    let size = s.fit_size(&name, 34.0, 300.0);
    let (cx, cy) = (PW / 2.0, PH / 2.0 + 4.0);
    let rise = 10.0 * (1.0 - wg::ease_out(f));
    s.shadow(cx - 170.0, cy - 46.0 + rise, 340.0, 92.0, 20.0, 12.0, 0, 0.35 * f);
    s.rect(cx - 170.0, cy - 46.0 + rise, 340.0, 92.0, 20.0, rgb(28, 30, 26), 0.82 * f);
    s.text_c(&name, cx, cy - 12.0 + rise, size, rgb(255, 255, 255), f);
    let pts = format!("+{}", st.cleared_points());
    s.text_c(&pts, cx, cy + 24.0 + rise, 20.0, rgb(255, 209, 102), f);
}
