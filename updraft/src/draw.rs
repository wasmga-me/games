//! The picture, drawn whole each frame (the sky scrolls): the sky darkening
//! with height, clouds drifting by slower than the ledges, the ledges,
//! springs, gems, gusts, birds and the puff, then the height and gems.

use crate::play::*;
use wg::{mix, rgb, Screen};

pub struct Theme {
    pub sky_top: u32,
    pub sky_bottom: u32,
    /// the sky high up
    pub high_top: u32,
    pub high_bottom: u32,
    pub cloud: u32,
    pub grass: u32,
    pub soil: u32,
    pub slide: u32,
    pub crumble: u32,
    pub puff: u32,
    pub puff_hi: u32,
    pub leaf: u32,
    pub ink: u32,
    pub stars: f32,
}

pub static THEMES: [Theme; N_THEMES as usize] = [
    // Day
    Theme {
        sky_top: rgb(120, 192, 250), sky_bottom: rgb(206, 236, 255), high_top: rgb(40, 60, 150), high_bottom: rgb(110, 150, 230),
        cloud: rgb(255, 255, 255), grass: rgb(110, 196, 92), soil: rgb(150, 104, 68), slide: rgb(120, 200, 230),
        crumble: rgb(214, 176, 120), puff: rgb(255, 196, 64), puff_hi: rgb(255, 232, 150), leaf: rgb(70, 170, 80),
        ink: rgb(24, 40, 70), stars: 0.0,
    },
    // Sunset
    Theme {
        sky_top: rgb(250, 130, 110), sky_bottom: rgb(255, 214, 150), high_top: rgb(70, 40, 110), high_bottom: rgb(190, 90, 130),
        cloud: rgb(255, 232, 220), grass: rgb(150, 190, 90), soil: rgb(120, 70, 60), slide: rgb(150, 170, 240),
        crumble: rgb(230, 170, 110), puff: rgb(255, 250, 240), puff_hi: rgb(255, 255, 255), leaf: rgb(90, 160, 80),
        ink: rgb(70, 30, 50), stars: 0.3,
    },
    // Night
    Theme {
        sky_top: rgb(16, 22, 60), sky_bottom: rgb(44, 60, 120), high_top: rgb(4, 4, 16), high_bottom: rgb(22, 24, 64),
        cloud: rgb(120, 136, 190), grass: rgb(60, 150, 110), soil: rgb(70, 60, 90), slide: rgb(110, 150, 255),
        crumble: rgb(150, 130, 170), puff: rgb(140, 230, 255), puff_hi: rgb(220, 250, 255), leaf: rgb(120, 230, 150),
        ink: rgb(240, 244, 255), stars: 1.0,
    },
    // Candy
    Theme {
        sky_top: rgb(255, 170, 214), sky_bottom: rgb(255, 230, 242), high_top: rgb(140, 90, 220), high_bottom: rgb(230, 150, 230),
        cloud: rgb(255, 255, 255), grass: rgb(120, 220, 190), soil: rgb(240, 120, 160), slide: rgb(150, 210, 255),
        crumble: rgb(250, 210, 150), puff: rgb(255, 120, 150), puff_hi: rgb(255, 200, 215), leaf: rgb(90, 200, 150),
        ink: rgb(90, 30, 80), stars: 0.2,
    },
];

const GEM: u32 = rgb(80, 220, 200);
const GEM_HI: u32 = rgb(210, 255, 248);

#[derive(Clone, Copy, Default)]
struct Part {
    x: f32,
    alt: f32,
    vx: f32,
    vy: f32,
    life: f32,
    c: u32,
}

/// Animation, not saved.
pub struct Anim {
    parts: [Part; 48],
    next: usize,
    pub now: f32,
}

impl Anim {
    pub fn new() -> Anim {
        Anim { parts: [Part::default(); 48], next: 0, now: 0.0 }
    }
    fn add(&mut self, p: Part) {
        self.parts[self.next] = p;
        self.next = (self.next + 1) % self.parts.len();
    }
    pub fn tick(&mut self, dt: i32) {
        let t = dt as f32 / 1000.0;
        self.now += t;
        for p in self.parts.iter_mut().filter(|p| p.life > 0.0) {
            p.life -= t;
            p.x += p.vx * t;
            p.alt += p.vy * t;
            p.vy -= 500.0 * t;
        }
    }
    fn burst(&mut self, x: f32, alt: f32, n: usize, v: f32, c: u32) {
        for k in 0..n {
            let a = k as f32 / n as f32 * core::f32::consts::TAU + x * 0.07;
            self.add(Part { x, alt, vx: a.cos() * v, vy: a.sin() * v + 80.0, life: 0.5, c });
        }
    }
}

pub struct AnimFx<'a>(pub &'a mut Anim, pub usize);

impl Fx for AnimFx<'_> {
    fn gem(&mut self, x: f32, alt: f32) {
        self.0.burst(x, alt, 8, 110.0, GEM_HI);
    }
    fn crumble(&mut self, l: &Ledge) {
        let c = THEMES[self.1].crumble;
        for k in 0..6 {
            let x = l.x + (k as f32 + 0.5) * l.w / 6.0;
            self.0.add(Part { x, alt: l.alt - 4.0, vx: (k as f32 - 2.5) * 12.0, vy: -40.0, life: 0.6, c });
        }
    }
    fn stomp(&mut self, x: f32, alt: f32) {
        self.0.burst(x, alt, 10, 140.0, rgb(255, 240, 160));
    }
    fn hit(&mut self, x: f32, alt: f32) {
        self.0.burst(x, alt, 8, 120.0, rgb(255, 255, 255));
    }
}

/// a height to the screen's y
fn sy(cam: f32, alt: f32) -> f32 {
    PH - (alt - cam)
}

/// a number from an index, 0..1, for things placed the same every time
fn noise(k: i32, salt: u32) -> f32 {
    let mut h = (k as u32).wrapping_mul(2_654_435_761) ^ salt.wrapping_mul(40_503);
    h ^= h >> 15;
    h = h.wrapping_mul(2_246_822_519);
    h ^= h >> 13;
    (h & 0xffff) as f32 / 65_535.0
}

/// The sky's colours at the top and bottom of the climb: it darkens over
/// the first 2,000 m.
fn sky_colours(st: &State, t: &Theme) -> (u32, u32, f32) {
    let up = (st.cam / 20_000.0).clamp(0.0, 1.0);
    (mix(t.high_top, t.sky_top, up), mix(t.high_bottom, t.sky_bottom, up), up)
}

/// The sky's gradient, filled a row at a time (the whole screen, so it has
/// to be cheap); on a screen wider than the climb, the sky beyond its edges
/// is shaded, so the edges the puff wraps round show.
fn sky_fill(s: &mut Screen, st: &State, t: &Theme) {
    let (top, bottom, _) = sky_colours(st, t);
    let (_, y0, _, y1) = s.visible();
    let (w, h) = s.size_px();
    let (py0, py1) = (s.px(0.0, y0).1, s.px(0.0, y1).1);
    let (a, b) = (s.px(0.0, 0.0).0.round().clamp(0.0, w as f32) as usize, s.px(PW, 0.0).0.round().clamp(0.0, w as f32) as usize);
    for j in 0..h {
        let c = mix(bottom, top, (j as f32 + 0.5 - py0) / (py1 - py0));
        let row = s.row_px(j);
        row[a..b].fill(c);
        if a > 0 || b < w as usize {
            let dark = mix(0, c, 0.42);
            row[..a].fill(dark);
            row[b..].fill(dark);
        }
    }
}

/// Stars and clouds, drawn inside the climb.
fn sky(s: &mut Screen, st: &State, t: &Theme, now: f32) {
    let (top, bottom, up) = sky_colours(st, t);
    let stars = (t.stars + up * 0.8).min(1.0);
    if stars > 0.05 {
        // stars hang still, far away
        let base = (st.cam * 0.05 / 60.0).floor() as i32;
        for k in base - 2..base + 14 {
            for j in 0..6 {
                let id = k * 6 + j;
                let x = noise(id, 1) * PW;
                let y = PH - (k as f32 * 60.0 + noise(id, 2) * 60.0 - st.cam * 0.05);
                let tw = 0.6 + 0.4 * (now * 2.0 + id as f32).sin();
                s.disc(x, y, 0.8 + noise(id, 3) * 0.9, rgb(255, 255, 255), stars * tw * 0.8);
            }
        }
    }
    // clouds drift past at a third of the speed
    let par = 0.35;
    let base = ((st.cam * par) / 150.0).floor() as i32;
    for k in base - 1..base + 6 {
        let x = noise(k, 4) * (PW + 120.0) - 60.0 + (now * (6.0 + noise(k, 6) * 8.0)) % (PW + 160.0) - 80.0;
        let x = (x + 80.0).rem_euclid(PW + 160.0) - 80.0;
        let y = PH - (k as f32 * 150.0 + noise(k, 5) * 80.0 - st.cam * par);
        let w = 30.0 + noise(k, 7) * 40.0;
        // solid, in the sky's own colour mixed in, so overlapping puffs don't show seams
        let a = 0.35 + 0.25 * (1.0 - up);
        let c = mix(mix(top, bottom, (y / PH).clamp(0.0, 1.0)), t.cloud, a);
        s.disc(x, y, w * 0.45, c, 1.0);
        s.disc(x + w * 0.45, y + 4.0, w * 0.35, c, 1.0);
        s.disc(x - w * 0.45, y + 5.0, w * 0.3, c, 1.0);
        s.rect(x - w * 0.75, y + 2.0, w * 1.5, w * 0.3, w * 0.15, c, 1.0);
    }
}

fn ledge(s: &mut Screen, l: &Ledge, cam: f32, t: &Theme) {
    let y = sy(cam, l.alt);
    if y < -20.0 || y > PH + 30.0 {
        return;
    }
    if l.kind == GROUND {
        let (_, _, _, y1) = s.visible();
        s.rect(0.0, y, PW, y1 - y + 1.0, 0.0, t.soil, 1.0);
        s.rect(0.0, y, PW, 10.0, 0.0, t.grass, 1.0);
        s.rect(0.0, y, PW, 3.0, 0.0, mix(t.grass, rgb(255, 255, 255), 0.3), 1.0);
        return;
    }
    if l.crumble_ms > 0 {
        // two halves falling apart
        let f = (l.crumble_ms as f32 / 700.0).min(1.0);
        if f >= 1.0 {
            return;
        }
        let drop = 140.0 * f * f;
        let a = 1.0 - f;
        let h = l.w / 2.0 - 2.0;
        s.rect(l.x - 6.0 * f, y + drop, h, LEDGE_H, 4.0, t.crumble, a);
        s.rect(l.x + l.w / 2.0 + 2.0 + 6.0 * f, y + drop * 1.2, h, LEDGE_H, 4.0, t.crumble, a);
        return;
    }
    s.rect(l.x + 2.0, y + 4.0, l.w, LEDGE_H, 5.0, 0, 0.18);
    match l.kind {
        SLIDING => {
            s.rect(l.x, y, l.w, LEDGE_H, 5.0, t.slide, 1.0);
            s.rect(l.x + 4.0, y + 2.0, l.w - 8.0, 2.5, 1.2, rgb(255, 255, 255), 0.6);
            // arrows at the ends
            let c = mix(t.slide, 0, 0.35);
            s.tri((l.x + 5.0, y + 5.0), (l.x + 10.0, y + 2.0), (l.x + 10.0, y + 8.0), c, 1.0);
            s.tri((l.x + l.w - 5.0, y + 5.0), (l.x + l.w - 10.0, y + 2.0), (l.x + l.w - 10.0, y + 8.0), c, 1.0);
        }
        CRUMBLE => {
            s.rect(l.x, y, l.w, LEDGE_H, 4.0, t.crumble, 1.0);
            // a crack across the middle
            let c = mix(t.crumble, 0, 0.4);
            let m = l.x + l.w * 0.5;
            let pts = [(m - 2.0, y), (m + 2.0, y + 3.5), (m - 1.5, y + 6.5), (m + 1.5, y + LEDGE_H)];
            for k in 0..3 {
                s.line(pts[k].0, pts[k].1, pts[k + 1].0, pts[k + 1].1, 0.9, c, 1.0);
            }
            s.disc(l.x + l.w * 0.22, y + 4.0, 1.3, c, 0.7);
            s.disc(l.x + l.w * 0.78, y + 6.0, 1.3, c, 0.7);
        }
        _ => {
            s.rect(l.x, y + 3.0, l.w, LEDGE_H - 1.0, 5.0, t.soil, 1.0);
            s.rect(l.x, y, l.w, 6.0, 3.0, t.grass, 1.0);
            s.rect(l.x + 3.0, y + 1.0, l.w - 6.0, 1.6, 0.8, mix(t.grass, rgb(255, 255, 255), 0.4), 1.0);
        }
    }
    if l.spring >= 0.0 {
        let x = l.x + l.spring;
        // squashed for a moment after a bounce
        let up = 1.0 - (1.0 - (l.sprung_ms as f32 / 160.0).min(1.0)) * 0.6;
        let hgt = 13.0 * up;
        let c = rgb(150, 156, 170);
        for k in 0..3 {
            let (ya, yb) = (y - hgt * k as f32 / 3.0, y - hgt * (k as f32 + 1.0) / 3.0);
            s.line(x - 5.0, ya, x + 5.0, (ya + yb) / 2.0, 1.2, c, 1.0);
            s.line(x + 5.0, (ya + yb) / 2.0, x - 5.0, yb, 1.2, c, 1.0);
        }
        s.rect(x - 8.0, y - hgt - 3.5, 16.0, 4.5, 2.0, rgb(232, 72, 72), 1.0);
    }
}

fn gem(s: &mut Screen, x: f32, y: f32, now: f32) {
    let y = y + (now * 3.0 + x).sin() * 2.0;
    s.poly(&[(x, y - 8.0), (x + 6.5, y - 1.0), (x, y + 8.0), (x - 6.5, y - 1.0)], GEM, 1.0);
    s.poly(&[(x, y - 8.0), (x + 6.5, y - 1.0), (x, y - 1.0)], GEM_HI, 0.7);
    s.disc(x - 2.0, y - 3.0, 1.2, rgb(255, 255, 255), 0.9);
}

fn gust(s: &mut Screen, x: f32, y: f32, now: f32) {
    for k in 0..3 {
        let a = now * 5.0 + k as f32 * 2.094;
        let r = 7.0 + 4.0 * k as f32;
        s.ring(x, y, r, 1.3, rgb(255, 255, 255), 0.35);
        s.disc(x + a.cos() * r, y + a.sin() * r * 0.8, 2.2, rgb(255, 255, 255), 0.9);
    }
}

fn bird(s: &mut Screen, b: &Thing, cam: f32, now: f32, t: &Theme) {
    let (x, y) = (b.x, sy(cam, b.alt));
    let dir = if b.vx >= 0.0 { 1.0 } else { -1.0 };
    let body = mix(t.ink, rgb(90, 90, 110), 0.5);
    if b.alive == 2 {
        s.ellipse(x, y, 11.0, 8.0, body, 0.8);
        s.text_c("x x", x, y - 1.0, 8.0, rgb(255, 255, 255), 0.9);
        return;
    }
    let flap = (now * 14.0 + b.x * 0.05).sin();
    s.ellipse(x, y, 11.0, 8.0, body, 1.0);
    s.tri((x - 4.0 * dir, y - 2.0), (x + 4.0 * dir, y - 2.0), (x - 2.0 * dir, y - 2.0 - 11.0 * flap), mix(body, rgb(255, 255, 255), 0.25), 1.0);
    s.tri((x + 10.0 * dir, y - 2.0), (x + 17.0 * dir, y + 1.0), (x + 10.0 * dir, y + 3.0), rgb(255, 170, 40), 1.0);
    s.disc(x + 5.0 * dir, y - 3.0, 2.4, rgb(255, 255, 255), 1.0);
    s.disc(x + 5.8 * dir, y - 3.0, 1.2, rgb(20, 20, 30), 1.0);
}

fn puff(s: &mut Screen, st: &State, x: f32, t: &Theme) {
    let y = sy(st.cam, st.alt);
    // a squash on landing that springs back
    let q = (1.0 - st.bounce_ms as f32 / 160.0).max(0.0);
    let (rx, ry) = (R * (1.0 + 0.22 * q), R * (1.0 - 0.2 * q));
    let y = y + R * 0.2 * q;
    s.ellipse(x, y, rx, ry, t.puff, 1.0);
    s.ellipse(x - 3.0, y - 4.0, rx * 0.45, ry * 0.35, t.puff_hi, 0.8);
    // a sprout on top, leaning against the climb
    let lean = (-st.vx / 300.0) * 3.0;
    s.line(x, y - ry + 1.0, x + lean, y - ry - 6.0, 1.1, t.leaf, 1.0);
    s.ellipse(x + lean - 4.0, y - ry - 7.0, 4.0, 2.2, t.leaf, 1.0);
    s.ellipse(x + lean + 4.0, y - ry - 8.0, 4.0, 2.2, t.leaf, 1.0);
    let look = (st.vx / 300.0).clamp(-1.0, 1.0) * 2.5;
    let ey = y - 1.0 - (st.vy / 1100.0).clamp(-1.0, 1.0) * 1.5;
    for side in [-1.0, 1.0] {
        let ex = x + side * 4.6 + look;
        if st.phase == TUMBLE {
            s.line(ex - 2.0, ey - 2.0, ex + 2.0, ey + 2.0, 0.9, rgb(30, 30, 40), 1.0);
            s.line(ex - 2.0, ey + 2.0, ex + 2.0, ey - 2.0, 0.9, rgb(30, 30, 40), 1.0);
        } else {
            s.disc(ex, ey, 3.0, rgb(255, 255, 255), 1.0);
            s.disc(ex + look * 0.4, ey + 0.3, 1.6, rgb(30, 30, 40), 1.0);
        }
    }
    if st.gust_ms > 0 {
        for k in 0..4 {
            let lx = x - 14.0 + k as f32 * 9.0;
            let off = ((st.time_ms as f32 / 40.0) + k as f32 * 7.0) % 26.0;
            s.line(lx, y + 14.0 + off, lx, y + 24.0 + off, 1.0, rgb(255, 255, 255), 0.6);
        }
    }
}

fn hud(s: &mut Screen, st: &State, t: &Theme) {
    let unit = if wg::lang() == wg::RU { "м" } else { "m" };
    let h = format!("{} {unit}", st.height());
    s.text(&h, 16.0 + 1.0, 26.0 + 1.5, 24.0, 0, 0.18);
    s.text(&h, 16.0, 26.0, 24.0, t.ink, 1.0);
    let g = format!("{}", st.gem_count);
    let w = s.text_width(&g, 18.0);
    s.text_r(&g, PW - 16.0, 26.0, 18.0, t.ink, 1.0);
    let gx = PW - 16.0 - w - 12.0;
    s.poly(&[(gx, 18.0), (gx + 6.0, 24.0), (gx, 33.0), (gx - 6.0, 24.0)], GEM, 1.0);
    s.poly(&[(gx, 18.0), (gx + 6.0, 24.0), (gx, 24.0)], GEM_HI, 0.7);
}

pub fn draw(s: &mut Screen, st: &State, a: &Anim) {
    let t = &THEMES[st.opt[O_THEME].clamp(0, N_THEMES - 1) as usize];
    sky_fill(s, st, t);
    // everything else stays inside the climb: what goes past an edge comes in on the other side
    let (_, y0, _, y1) = s.visible();
    s.set_clip(0.0, y0, PW, y1 - y0);
    sky(s, st, t, a.now);
    for l in st.ledges.iter().filter(|l| l.w > 0.0) {
        ledge(s, l, st.cam, t);
    }
    for g in st.gems.iter().filter(|g| g.alive == 1) {
        gem(s, g.x, sy(st.cam, g.alt), a.now);
    }
    for g in st.gusts.iter().filter(|g| g.alive == 1) {
        gust(s, g.x, sy(st.cam, g.alt), a.now);
    }
    for b in st.birds.iter().filter(|b| b.alive != 0) {
        bird(s, b, st.cam, a.now, t);
    }
    // the puff, and its other half when it wraps past an edge
    puff(s, st, st.x, t);
    if st.x < R * 2.0 {
        puff(s, st, st.x + PW, t);
    } else if st.x > PW - R * 2.0 {
        puff(s, st, st.x - PW, t);
    }
    for p in a.parts.iter().filter(|p| p.life > 0.0) {
        s.disc(p.x, sy(st.cam, p.alt), 2.0, p.c, (p.life * 2.0).min(1.0));
    }
    hud(s, st, t);
    s.no_clip();
    edges(s);
}

/// A thin light line down each edge of the climb, on a screen wider than it.
fn edges(s: &mut Screen) {
    let (x0, y0, x1, y1) = s.visible();
    if x0 < -0.5 {
        s.rect(-1.5, y0, 1.5, y1 - y0, 0.0, rgb(255, 255, 255), 0.25);
    }
    if x1 > PW + 0.5 {
        s.rect(PW, y0, 1.5, y1 - y0, 0.0, rgb(255, 255, 255), 0.25);
    }
}
