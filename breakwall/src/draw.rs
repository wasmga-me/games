//! The picture: a still layer with the backdrop, walls and bricks (patched a
//! brick at a time as they break), and everything that moves drawn over it
//! each frame, at the screen's own resolution.

use crate::game::*;
use crate::levels::{BOMB, COLS, EMPTY, GOLD, ROWS, SILVER};
use crate::math::{absf, clampf, sinf};
use wg::{mix, rgb, Screen};

fn minf(a: f32, b: f32) -> f32 {
    a.min(b)
}

pub struct Theme {
    pub top: u32,
    pub bottom: u32,
    pub wall: u32,
    pub wall_hi: u32,
    pub text: u32,
    pub dim: u32,
    pub paddle: u32,
    pub paddle_hi: u32,
    pub ball: u32,
    pub glow: u32,
    pub bricks: [u32; 8],
    pub silver: u32,
    pub gold: u32,
    pub bomb: u32,
}

pub static THEMES: [Theme; 4] = [
    // Night
    Theme {
        top: rgb(28, 24, 64), bottom: rgb(12, 10, 30), wall: rgb(58, 50, 120), wall_hi: rgb(96, 86, 180),
        text: rgb(255, 247, 232), dim: rgb(150, 140, 200), paddle: rgb(101, 79, 240), paddle_hi: rgb(170, 156, 255),
        ball: rgb(255, 255, 255), glow: rgb(140, 170, 255),
        bricks: [rgb(255, 90, 84), rgb(255, 150, 60), rgb(255, 210, 70), rgb(80, 210, 120), rgb(46, 206, 210),
            rgb(70, 140, 255), rgb(200, 100, 240), rgb(236, 232, 250)],
        silver: rgb(170, 176, 196), gold: rgb(232, 180, 50), bomb: rgb(60, 40, 52),
    },
    // Neon
    Theme {
        top: rgb(6, 6, 10), bottom: rgb(0, 0, 0), wall: rgb(40, 40, 52), wall_hi: rgb(0, 230, 255),
        text: rgb(240, 250, 255), dim: rgb(120, 140, 160), paddle: rgb(0, 200, 255), paddle_hi: rgb(180, 245, 255),
        ball: rgb(255, 255, 255), glow: rgb(0, 230, 255),
        bricks: [rgb(255, 40, 120), rgb(255, 120, 0), rgb(255, 240, 0), rgb(60, 255, 100), rgb(0, 255, 230),
            rgb(40, 120, 255), rgb(190, 60, 255), rgb(255, 255, 255)],
        silver: rgb(150, 160, 175), gold: rgb(255, 196, 0), bomb: rgb(70, 10, 30),
    },
    // Paper
    Theme {
        top: rgb(251, 246, 234), bottom: rgb(236, 226, 204), wall: rgb(196, 184, 160), wall_hi: rgb(222, 212, 190),
        text: rgb(42, 33, 24), dim: rgb(120, 104, 84), paddle: rgb(42, 33, 24), paddle_hi: rgb(110, 96, 80),
        ball: rgb(42, 33, 24), glow: rgb(160, 140, 110),
        bricks: [rgb(214, 64, 43), rgb(226, 120, 40), rgb(222, 176, 40), rgb(70, 150, 80), rgb(40, 150, 150),
            rgb(50, 100, 200), rgb(130, 70, 170), rgb(150, 140, 120)],
        silver: rgb(160, 160, 166), gold: rgb(200, 150, 40), bomb: rgb(60, 50, 50),
    },
    // Sunset
    Theme {
        top: rgb(255, 140, 90), bottom: rgb(90, 40, 110), wall: rgb(70, 30, 80), wall_hi: rgb(130, 70, 130),
        text: rgb(255, 250, 240), dim: rgb(255, 210, 200), paddle: rgb(255, 250, 240), paddle_hi: rgb(255, 255, 255),
        ball: rgb(255, 255, 255), glow: rgb(255, 220, 160),
        bricks: [rgb(232, 50, 80), rgb(255, 120, 60), rgb(255, 196, 70), rgb(120, 200, 110), rgb(60, 190, 200),
            rgb(70, 110, 220), rgb(160, 80, 200), rgb(255, 236, 220)],
        silver: rgb(200, 190, 210), gold: rgb(255, 190, 40), bomb: rgb(50, 20, 50),
    },
];

pub const BOOST_LETTER: [&str; NBOOST] = ["W", "N", "M", "S", "L", "C", "F", "+", "B", "R"];
pub const BOOST_NAME: [&str; NBOOST] = ["Wide", "Narrow", "Multi", "Slow", "Laser", "Catch", "Fire", "Life", "Barrier", "Rush"];
const BOOST_COLOUR: [u32; NBOOST] = [
    rgb(60, 190, 110), rgb(120, 110, 130), rgb(70, 140, 255), rgb(240, 180, 40), rgb(240, 70, 70),
    rgb(160, 90, 230), rgb(255, 120, 30), rgb(240, 80, 150), rgb(40, 190, 200), rgb(110, 90, 100),
];

const NPART: usize = 96;

#[derive(Clone, Copy)]
struct Part {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    c: u32,
    s: f32,
}

/// animation state, not saved
pub struct Anim {
    pub now: f32,
    flash: [[f32; COLS]; ROWS],
    dirty: [[bool; COLS]; ROWS],
    parts: [Part; NPART],
    np: usize,
    trail: [[(f32, f32); 8]; MAX_BALLS],
    pub banner: f32,
    paddle_w: f32,
    caught: f32,
    caught_kind: i32,
    caught_x: f32,
    /// bumped when every brick is laid out anew, so the still layer redraws
    pub gen: u32,
}


pub fn brick_colour(t: &Theme, k: u8) -> u32 {
    match k {
        SILVER => t.silver,
        GOLD => t.gold,
        BOMB => t.bomb,
        1..=8 => t.bricks[k as usize - 1],
        _ => t.top,
    }
}

pub struct AnimFx<'a> {
    pub a: &'a mut Anim,
    pub theme: usize,
}

impl Anim {
    fn spark(&mut self, x: f32, y: f32, vx: f32, vy: f32, life: f32, c: u32, s: f32) {
        let p = Part { x, y, vx, vy, life, c, s };
        if self.np < NPART {
            self.parts[self.np] = p;
            self.np += 1;
        } else {
            // the oldest goes
            let i = (self.now as usize) % NPART;
            self.parts[i] = p;
        }
    }

    pub fn new() -> Anim {
        Anim {
            now: 0.0,
            flash: [[0.0; COLS]; ROWS],
            dirty: [[false; COLS]; ROWS],
            parts: [Part { x: 0.0, y: 0.0, vx: 0.0, vy: 0.0, life: 0.0, c: 0, s: 0.0 }; NPART],
            np: 0,
            trail: [[(-1.0, -1.0); 8]; MAX_BALLS],
            banner: 0.0,
            paddle_w: 0.0,
            caught: 1e9,
            caught_kind: 0,
            caught_x: 0.0,
            gen: 0,
        }
    }

    /// something is still playing out
    pub fn busy(&self) -> bool {
        self.np > 0 || self.caught < 900.0 || self.flash.iter().flatten().any(|&f| f > 0.0)
    }

    pub fn reset(&mut self) {
        self.flash = [[0.0; COLS]; ROWS];
        self.np = 0;
        self.banner = 0.0;
        self.caught = 1e9;
        self.gen = self.gen.wrapping_add(1);
        self.paddle_w = 0.0;
        self.dirty = [[false; COLS]; ROWS];
        for t in self.trail.iter_mut() {
            *t = [(-1.0, -1.0); 8];
        }
    }
}

impl<'a> Fx for AnimFx<'a> {
    fn broke(&mut self, r: usize, c: usize, kind: u8) {
        let col = brick_colour(&THEMES[self.theme], kind);
        let (x0, y0) = (BRICK_X0 + c as f32 * CELL_W, BRICK_Y0 + r as f32 * CELL_H);
        let seed = (r * 31 + c * 17) as f32 + self.a.now;
        for i in 0..7 {
            let f = i as f32;
            let x = x0 + 4.0 + (f * 4.3) % 24.0;
            let y = y0 + 3.0 + (f * 5.7) % 10.0;
            let vx = sinf(seed * 0.37 + f * 1.9) * 90.0;
            let vy = -40.0 - absf(sinf(seed * 0.21 + f * 2.7)) * 90.0;
            self.a.spark(x, y, vx, vy, 0.55 + 0.05 * f, col, 3.0 + (f * 0.7) % 2.5);
        }
        if kind == BOMB {
            for i in 0..10 {
                let a = i as f32 * 0.628;
                self.a.spark(x0 + 16.0, y0 + 8.0, sinf(a) * 160.0, sinf(a + 1.57) * 160.0, 0.4, rgb(255, 200, 80), 4.0);
            }
        }
    }
    fn hit(&mut self, r: usize, c: usize) {
        self.a.flash[r][c] = 1.0;
    }
    fn changed(&mut self, r: usize, c: usize) {
        self.a.dirty[r][c] = true;
    }
    fn caught(&mut self, kind: i32, x: f32) {
        self.a.caught = 0.0;
        self.a.caught_kind = kind;
        self.a.caught_x = x;
    }
    fn bounce(&mut self, x: f32, y: f32) {
        let t = &THEMES[self.theme];
        for i in 0..3 {
            let a = self.a.now * 0.1 + i as f32 * 2.1;
            self.a.spark(x, y, sinf(a) * 50.0, sinf(a + 1.57) * 50.0, 0.25, t.glow, 2.0);
        }
    }
    fn level(&mut self) {
        self.a.flash = [[0.0; COLS]; ROWS];
        self.a.banner = 0.0;
        self.a.gen = self.a.gen.wrapping_add(1);
    }
}

/// one frame of animation time
pub fn tick(a: &mut Anim, dt_ms: i32) {
    let dt = dt_ms as f32 / 1000.0;
    a.now += dt_ms as f32;
    a.banner += dt_ms as f32;
    a.caught += dt_ms as f32;
    for row in a.flash.iter_mut() {
        for f in row.iter_mut() {
            *f = (*f - dt * 6.0).max(0.0);
        }
    }
    let mut i = 0;
    while i < a.np {
        let p = &mut a.parts[i];
        p.life -= dt;
        if p.life <= 0.0 {
            a.np -= 1;
            a.parts[i] = a.parts[a.np];
            continue;
        }
        p.x += p.vx * dt;
        p.y += p.vy * dt;
        p.vy += 420.0 * dt;
        i += 1;
    }
}

// ---------- the still layer ----------

fn white() -> u32 {
    rgb(255, 255, 255)
}
fn black() -> u32 {
    rgb(0, 0, 0)
}

/// the backdrop over the whole screen, letterbox included (or the clip)
fn backdrop(g: &mut Screen, t: &Theme) {
    let (x0, y0, x1, y1) = g.visible();
    g.rect_grad(x0, y0, x1 - x0, y1 - y0, 0.0, t.top, t.bottom, 1.0);
}

fn draw_brick(g: &mut Screen, t: &Theme, s: &State, r: usize, c: usize) {
    let k = s.bricks[r][c];
    if k == EMPTY {
        return;
    }
    let x = BRICK_X0 + c as f32 * CELL_W + 1.0;
    let y = BRICK_Y0 + r as f32 * CELL_H + 1.0;
    let (w, h) = (CELL_W - 2.0, CELL_H - 2.0);
    let col = brick_colour(t, k);
    g.rect(x, y, w, h, 3.0, mix(black(), col, 0.3), 1.0);
    g.rect_grad(x, y, w, h - 2.5, 3.0, mix(white(), col, 0.3), col, 1.0);
    g.rect(x + 3.0, y + 1.5, w - 6.0, 2.0, 1.0, white(), 0.35);
    match k {
        GOLD => {
            // a slanted shine
            g.line(x + 8.0, y + h - 4.0, x + 13.0, y + 3.0, 1.6, white(), 0.5);
            g.line(x + 13.0, y + h - 4.0, x + 16.0, y + 5.0, 0.9, white(), 0.35);
        }
        SILVER => {
            let full = if s.level >= 15 { 3 } else { 2 };
            let cracks = full - s.hits[r][c] as i32;
            let ink = mix(black(), col, 0.55);
            if cracks >= 1 {
                g.line(x + 9.0, y + 2.0, x + 13.0, y + 7.0, 0.7, ink, 1.0);
                g.line(x + 13.0, y + 7.0, x + 11.0, y + 11.0, 0.7, ink, 1.0);
                g.line(x + 13.0, y + 7.0, x + 18.0, y + 9.0, 0.6, ink, 1.0);
            }
            if cracks >= 2 {
                g.line(x + 22.0, y + 2.0, x + 20.0, y + 6.0, 0.7, ink, 1.0);
                g.line(x + 20.0, y + 6.0, x + 24.0, y + 11.0, 0.7, ink, 1.0);
            }
        }
        BOMB => {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0 - 1.0);
            g.disc(cx, cy + 0.5, 4.2, rgb(20, 16, 20), 1.0);
            g.line(cx + 2.5, cy - 2.5, cx + 4.5, cy - 4.5, 0.8, rgb(200, 180, 150), 1.0);
            g.disc(cx + 5.0, cy - 5.0, 1.3, rgb(255, 200, 60), 1.0);
            g.disc(cx - 1.4, cy - 1.0, 1.1, white(), 0.5);
        }
        _ => {}
    }
}

fn draw_still(g: &mut Screen, t: &Theme, s: &State) {
    backdrop(g, t);
    // walls round the field, open at the bottom
    let wl = 8.0;
    let tall = PH - FIELD_T + wl + 20.0;
    g.rect_grad(FIELD_L - wl, FIELD_T - wl, wl, tall, 3.0, t.wall_hi, t.wall, 1.0);
    g.rect_grad(FIELD_R, FIELD_T - wl, wl, tall, 3.0, t.wall_hi, t.wall, 1.0);
    g.rect_grad(FIELD_L - wl, FIELD_T - wl, FIELD_R - FIELD_L + 2.0 * wl, wl, 3.0, t.wall_hi, t.wall, 1.0);
    for r in 0..ROWS {
        for c in 0..COLS {
            draw_brick(g, t, s, r, c);
        }
    }
}

/// The backdrop, walls and bricks, drawn once per level; a brick that breaks
/// or cracks is redrawn on its own, inside its cell.
fn still(g: &mut Screen, t: &Theme, s: &State, a: &mut Anim) {
    if a.dirty.iter().flatten().any(|&d| d) {
        g.background_edit(|g| {
            for r in 0..ROWS {
                for c in 0..COLS {
                    if a.dirty[r][c] {
                        g.set_clip(BRICK_X0 + c as f32 * CELL_W, BRICK_Y0 + r as f32 * CELL_H, CELL_W, CELL_H);
                        backdrop(g, t);
                        draw_brick(g, t, s, r, c);
                    }
                }
            }
        });
        a.dirty = [[false; COLS]; ROWS];
    }
    let key = wg::hash(a.gen, wg::bytes_of(&[s.opt[O_THEME], s.level]));
    g.background(key, |g| draw_still(g, t, s));
}

// ---------- what moves ----------

fn heart(g: &mut Screen, x: f32, y: f32, s: f32, c: u32) {
    g.disc(x - s * 0.5, y - s * 0.2, s * 0.55, c, 1.0);
    g.disc(x + s * 0.5, y - s * 0.2, s * 0.55, c, 1.0);
    g.tri((x - s * 1.02, y), (x + s * 1.02, y), (x, y + s * 1.05), c, 1.0);
}

fn pill(g: &mut Screen, x: f32, y: f32, kind: i32) {
    let c = BOOST_COLOUR[kind as usize];
    let (w, h) = (ITEM_W, ITEM_H);
    g.shadow(x - w / 2.0, y - h / 2.0 + 2.0, w, h, h / 2.0, 3.0, black(), 0.25);
    g.rect_grad(x - w / 2.0, y - h / 2.0, w, h, h / 2.0, mix(white(), c, 0.35), mix(black(), c, 0.2), 1.0);
    g.text_c(BOOST_LETTER[kind as usize], x, y + 0.3, 10.0, white(), 1.0);
}

fn paddle(g: &mut Screen, t: &Theme, s: &State, a: &Anim, x: f32, w: f32) {
    let y = PADDLE_Y - PADDLE_H / 2.0;
    let h = PADDLE_H;
    g.shadow(x - w / 2.0, y + 3.0, w, h, h / 2.0, 4.0, black(), 0.3);
    let (mut body, mut hi) = (t.paddle, t.paddle_hi);
    if s.catch_ms > 0 {
        body = mix(rgb(160, 90, 230), body, 0.55);
        hi = mix(rgb(220, 180, 255), hi, 0.55);
    }
    g.rect_grad(x - w / 2.0, y, w, h, h / 2.0, hi, body, 1.0);
    g.rect(x - w / 2.0 + 5.0, y + 1.5, w - 10.0, 2.0, 1.0, white(), 0.4);
    // end caps
    let cap = mix(black(), body, 0.25);
    g.rect(x - w / 2.0, y, 8.0, h, h / 2.0, cap, 0.5);
    g.rect(x + w / 2.0 - 8.0, y, 8.0, h, h / 2.0, cap, 0.5);
    if s.laser_ms > 0 {
        let lit = s.laser_ms > 2000 || (a.now as i32 / 120) % 2 == 0;
        for side in [-1.0f32, 1.0] {
            let cx = x + side * (w / 2.0 - 7.0);
            g.rect_grad(cx - 2.5, y - 5.0, 5.0, 7.0, 1.5, rgb(255, 120, 110), rgb(160, 40, 40), if lit { 1.0 } else { 0.5 });
        }
    }
}

fn hud(g: &mut Screen, t: &Theme, s: &State) {
    let cy = 19.0;
    g.text(&s.score.to_string(), FIELD_L - 4.0, cy, 17.0, t.text, 1.0);
    let n = (s.level + 1).to_string();
    let lw = g.text_width("Level ", 13.0);
    let x = PW / 2.0 - (lw + g.text_width(&n, 13.0)) / 2.0;
    g.text("Level ", x, cy, 13.0, t.dim, 1.0);
    g.text(&n, x + lw, cy, 13.0, t.text, 1.0);
    // lives, as hearts, right-aligned
    for i in 0..s.lives.clamp(0, 9) {
        heart(g, FIELD_R - 2.0 - i as f32 * 15.0, cy - 1.0, 5.2, rgb(255, 84, 112));
    }
    // timed boosts: a pill each, its bar running down
    let mut x = FIELD_L + 92.0;
    let timers = [(s.laser_ms, LASER_MS, B_LASER), (s.catch_ms, CATCH_MS, B_CATCH), (s.fire_ms, FIRE_MS, B_FIRE), (s.slow_ms, SLOW_MS, B_SLOW)];
    for (ms, full, kind) in timers {
        if ms <= 0 {
            continue;
        }
        let f = ms as f32 / full as f32;
        let c = BOOST_COLOUR[kind as usize];
        g.rect(x, cy - 6.0, 34.0, 12.0, 6.0, mix(black(), c, 0.5), 0.9);
        g.rect(x, cy - 6.0, 12.0 + 22.0 * f, 12.0, 6.0, c, 1.0);
        g.text_c(BOOST_LETTER[kind as usize], x + 6.5, cy + 0.3, 9.0, white(), 1.0);
        x += 40.0;
    }
    if s.shield != 0 {
        let c = BOOST_COLOUR[B_SHIELD as usize];
        g.rect(x, cy - 6.0, 13.0, 12.0, 6.0, c, 1.0);
        g.text_c("B", x + 6.5, cy + 0.3, 9.0, white(), 1.0);
    }
}

pub fn draw(g: &mut Screen, s: &State, a: &mut Anim) {
    let t = &THEMES[s.opt[O_THEME] as usize];
    still(g, t, s, a);

    for r in 0..ROWS {
        for c in 0..COLS {
            let f = a.flash[r][c];
            if f > 0.0 && s.bricks[r][c] != EMPTY {
                let x = BRICK_X0 + c as f32 * CELL_W + 1.0;
                let y = BRICK_Y0 + r as f32 * CELL_H + 1.0;
                g.rect(x, y, CELL_W - 2.0, CELL_H - 2.0, 3.0, white(), f * 0.7);
            }
        }
    }
    for p in a.parts[..a.np].iter() {
        let al = clampf(p.life * 3.0, 0.0, 1.0);
        g.rect(p.x - p.s / 2.0, p.y - p.s / 2.0, p.s, p.s, p.s * 0.3, p.c, al);
    }
    for it in s.items.iter().filter(|i| i.alive != 0) {
        pill(g, it.x, it.y, it.kind);
    }
    for sh in s.shots.iter().filter(|x| x.alive != 0) {
        g.line(sh.x, sh.y, sh.x, sh.y + 9.0, 1.4, rgb(255, 120, 100), 1.0);
        g.line(sh.x, sh.y, sh.x, sh.y + 5.0, 0.7, rgb(255, 255, 220), 1.0);
    }
    if s.shield != 0 {
        let p = 0.6 + 0.25 * sinf(a.now * 0.008);
        let c = BOOST_COLOUR[B_SHIELD as usize];
        g.shadow(FIELD_L, SHIELD_Y - 1.0, FIELD_R - FIELD_L, 2.0, 1.0, 5.0, c, 0.4 * p);
        g.line(FIELD_L, SHIELD_Y, FIELD_R, SHIELD_Y, 1.4, mix(white(), c, 0.5), p);
    }
    // the paddle eases between widths
    let target = s.paddle_w();
    if a.paddle_w <= 0.0 {
        a.paddle_w = target;
    }
    a.paddle_w += (target - a.paddle_w) * 0.25;
    if absf(target - a.paddle_w) < 0.3 {
        a.paddle_w = target;
    }
    let lost = s.phase == DYING || s.phase == MISS;
    if !(lost && (a.now as i32 / 140) % 2 == 0) {
        let pw = minf(a.paddle_w, FIELD_R - FIELD_L);
        let px = clampf(s.paddle_x, FIELD_L + pw / 2.0, FIELD_R - pw / 2.0);
        paddle(g, t, s, a, px, pw);
    }
    let fire = s.fire_ms > 0;
    for (i, b) in s.balls.iter().enumerate() {
        let tr = &mut a.trail[i];
        if b.alive == 0 || b.stuck != 0 {
            *tr = [(-1.0, -1.0); 8];
            if b.alive == 0 {
                continue;
            }
        } else {
            tr.copy_within(0..7, 1);
            tr[0] = (b.x, b.y);
        }
        let (core, halo) = if fire { (rgb(255, 230, 150), rgb(255, 110, 30)) } else { (t.ball, t.glow) };
        for (k, &(x, y)) in tr.iter().enumerate().skip(1) {
            if x < 0.0 {
                break;
            }
            let f = 1.0 - k as f32 / 8.0;
            g.disc(x, y, BALL_R * (0.4 + 0.5 * f), halo, 0.25 * f);
        }
        g.shadow(b.x - 1.0, b.y - 1.0, 2.0, 2.0, 1.0, BALL_R * 3.0, halo, if fire { 0.6 } else { 0.35 });
        g.disc(b.x, b.y, BALL_R, core, 1.0);
        g.disc(b.x - 1.5, b.y - 1.5, BALL_R * 0.38, white(), 0.6);
    }

    hud(g, t, s);

    // what a caught boost was, rising from where it was caught
    if a.caught < 900.0 {
        let k = a.caught / 900.0;
        let x = clampf(a.caught_x, FIELD_L + 30.0, FIELD_R - 30.0);
        g.text_c(BOOST_NAME[a.caught_kind as usize], x, PADDLE_Y - 22.0 - 18.0 * k, 12.0, t.text, 1.0 - k * k);
    }
    // the level's number as it starts, and a word when it's cleared
    let mid = BRICK_Y0 + 12.0 * CELL_H + 28.0;
    if s.phase == PLAY && a.banner < 1600.0 {
        let al = clampf((1600.0 - a.banner) / 400.0, 0.0, 1.0) * clampf(a.banner / 150.0, 0.0, 1.0);
        let n = (s.level + 1).to_string();
        let w1 = g.text_width("Level ", 30.0);
        let x = PW / 2.0 - (w1 + g.text_width(&n, 30.0)) / 2.0;
        g.text("Level ", x, mid, 30.0, t.text, al);
        g.text(&n, x + w1, mid, 30.0, t.text, al);
    }
    if s.phase == CLEAR {
        let al = clampf(s.phase_ms as f32 / 200.0, 0.0, 1.0);
        let word = if s.won != 0 { "Every level cleared" } else { "Cleared" };
        g.text_c(word, PW / 2.0, mid, 30.0, t.text, al);
    }
}
