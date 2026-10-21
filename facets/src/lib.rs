//! Facets - an original wasmga.me game. MIT licensed.
//! Swap two neighbouring gems to line up three or more of a colour. Four in
//! a line leave a bomb, lines crossing leave a cross, five in a line leave a
//! prism. Rules in logic.rs; this file draws them and takes input.
pub mod gem;
pub mod logic;
mod glyphs;

use logic::*;
use wg::{hex, mix, Game, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 2;
/// options, in catalog `setup` order: mode, difficulty, theme, hints
const O_MODE: usize = 0;
const O_DIFF: usize = 1;
const O_THEME: usize = 2;
const O_HINTS: usize = 3;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 4],
    b: Board,
    /// the picked gem (-1 for none) and the keyboard cursor
    sel: [i32; 2],
    cur: [i32; 2],
    rep: KeyRepeat,
}
unsafe impl wg::Plain for Save {}

struct Theme {
    top: u32,
    bottom: u32,
    well: u32,
    cell_a: u32,
    cell_b: u32,
    ink: u32,
    dim: u32,
    accent: u32,
    bar: u32,
}

const THEMES: [Theme; 3] = [
    // Dusk
    Theme { top: 0x2a1f4a, bottom: 0x15102a, well: 0x0f0b1f, cell_a: 0x1c1636, cell_b: 0x221b40, ink: 0xf6f0ff, dim: 0xa89cc8, accent: 0xffd166, bar: 0x2f2654 },
    // Dawn
    Theme { top: 0xf7efe0, bottom: 0xe9dcc3, well: 0xcdbd9f, cell_a: 0xe2d6bd, cell_b: 0xdacdb2, ink: 0x2b2233, dim: 0x7a6a5a, accent: 0xc2410c, bar: 0xd6c8ac },
    // Lagoon
    Theme { top: 0x123c48, bottom: 0x0a2129, well: 0x071a20, cell_a: 0x0f2c35, cell_b: 0x12333d, ink: 0xe9fbff, dim: 0x8fbac4, accent: 0x7cf2c8, bar: 0x1a4652 },
];

/// base, light and dark of each gem colour (1..=7)
const GEM: [(u32, u32, u32); 8] = [
    (0, 0, 0),
    (0xe8364f, 0xff8fa0, 0x8c1027), // ruby
    (0xff8a2a, 0xffc98a, 0xa84a08), // amber
    (0xffd23f, 0xfff2a6, 0xb08a00), // citrine
    (0x2fc36b, 0x9cf2bd, 0x11703a), // emerald
    (0x2f8cff, 0x9fd0ff, 0x1146a8), // sapphire
    (0xa056f0, 0xd9b5ff, 0x5a1fa0), // amethyst
    (0xe9f4f7, 0xffffff, 0x8aa2ad), // pearl
];

/// the outline of each gem colour, as points round (0, 0), radius about 1
fn shape(c: u8) -> &'static [(f32, f32)] {
    match c {
        1 => &[(-0.78, -0.62), (-0.62, -0.78), (0.62, -0.78), (0.78, -0.62), (0.78, 0.62), (0.62, 0.78), (-0.62, 0.78), (-0.78, 0.62)],
        2 => &[(0.0, -0.95), (0.84, -0.48), (0.84, 0.48), (0.0, 0.95), (-0.84, 0.48), (-0.84, -0.48)],
        3 => &[(0.0, -1.0), (0.78, 0.0), (0.0, 1.0), (-0.78, 0.0)],
        4 => &[(-0.42, -0.9), (0.42, -0.9), (0.7, -0.6), (0.7, 0.6), (0.42, 0.9), (-0.42, 0.9), (-0.7, 0.6), (-0.7, -0.6)],
        5 => &[(0.0, -0.92), (0.65, -0.65), (0.92, 0.0), (0.65, 0.65), (0.0, 0.92), (-0.65, 0.65), (-0.92, 0.0), (-0.65, -0.65)],
        6 => &[(0.0, -0.95), (0.92, 0.72), (-0.92, 0.72)],
        _ => &[(0.0, -0.95), (0.9, -0.3), (0.56, 0.82), (-0.56, 0.82), (-0.9, -0.3)],
    }
}

/// what makes a gem special, drawn over it: a bomb glows from inside, a
/// cross sends light out along its row and column
fn draw_special(s: &mut Screen, cx: f32, cy: f32, r: f32, c: u8, k: u8, alpha: f32) {
    let glow = hex(GEM[c as usize].1);
    match k {
        BOMB => {
            // a halo round it and a hot core
            for i in 0..5 {
                let f = i as f32 / 4.0;
                s.ring(cx, cy, r * (1.02 + 0.16 * f), r * 0.05, glow, (0.5 - 0.42 * f) * alpha);
            }
            for i in 0..4 {
                let f = i as f32 / 3.0;
                s.disc(cx, cy, r * (0.36 - 0.24 * f), mix(hex(0xffffff), glow, f), (0.22 + 0.25 * f) * alpha);
            }
        }
        CROSS => {
            // light streaming out along the row and the column
            let w = r * 0.045;
            for (dx, dy) in [(1.0f32, 0.0f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                let (x1, y1) = (cx + dx * r * 1.12, cy + dy * r * 1.12);
                s.line_grad(cx, cy, x1, y1, w * 3.0, w * 0.8, glow, glow, 0.3 * alpha);
                s.line_grad(cx, cy, x1, y1, w, w * 0.25, hex(0xffffff), hex(0xffffff), 0.95 * alpha);
            }
            s.disc(cx, cy, r * 0.12, hex(0xffffff), 0.9 * alpha);
        }
        _ => {}
    }
}

fn draw_prism(s: &mut Screen, cx: f32, cy: f32, r: f32, alpha: f32) {
    s.disc(cx, cy + r * 0.07, r * 0.95, 0, 0.28 * alpha);
    s.disc(cx, cy, r * 0.95, hex(0x1a1030), alpha);
    // a ring of the seven colours, as wedges
    let n = 7;
    for i in 0..n {
        let a0 = i as f32 / n as f32 * core::f32::consts::TAU - 1.5708;
        let a1 = (i + 1) as f32 / n as f32 * core::f32::consts::TAU - 1.5708;
        let p = |a: f32, k: f32| (cx + a.cos() * r * k, cy + a.sin() * r * k);
        let am = (a0 + a1) / 2.0;
        s.poly(&[p(a0, 0.86), p(am, 0.88), p(a1, 0.86), p(a1, 0.4), p(a0, 0.4)], hex(GEM[i + 1].0), alpha);
    }
    s.disc(cx, cy, r * 0.42, hex(0xffffff), alpha);
    s.disc(cx, cy, r * 0.3, hex(0xfff6d6), alpha);
    s.ring(cx, cy, r * 0.97, r * 0.07, hex(0xffffff), 0.7 * alpha);
}

// ---------- pictures that aren't saved ----------

struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    c: u32,
}
struct Popup {
    x: f32,
    y: f32,
    pts: i32,
    life: f32,
}
struct Blast {
    x: usize,
    y: usize,
    kind: u8,
    life: f32,
}

#[derive(Default)]
struct Fx {
    sparks: Vec<Spark>,
    pops: Vec<Popup>,
    blasts: Vec<Blast>,
    seed: u32,
}

impl Fx {
    fn rnd(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 24) as f32
    }
    fn busy(&self) -> bool {
        !self.sparks.is_empty() || !self.pops.is_empty() || !self.blasts.is_empty()
    }
    fn step(&mut self, dt: f32) {
        for p in &mut self.sparks {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.vy += 9.0 * dt;
            p.life -= dt * 1.6;
        }
        self.sparks.retain(|p| p.life > 0.0);
        for p in &mut self.pops {
            p.y -= 0.9 * dt;
            p.life -= dt * 0.9;
        }
        self.pops.retain(|p| p.life > 0.0);
        for b in &mut self.blasts {
            b.life -= dt * 2.2;
        }
        self.blasts.retain(|b| b.life > 0.0);
    }
}

impl Sink for Fx {
    fn cleared(&mut self, x: usize, y: usize, colour: u8, _kind: u8) {
        let c = if colour == PRISM { 0xffffff } else { GEM[colour as usize].1 };
        if self.sparks.len() > 400 {
            return;
        }
        for _ in 0..5 {
            let a = self.rnd() * core::f32::consts::TAU;
            let v = 1.5 + self.rnd() * 2.5;
            let life = 0.7 + self.rnd() * 0.3;
            self.sparks.push(Spark { x: x as f32 + 0.5, y: y as f32 + 0.5, vx: a.cos() * v, vy: a.sin() * v - 1.5, life, c: hex(c) });
        }
    }
    fn points(&mut self, x: f32, y: f32, pts: i32, _chain: i32) {
        self.pops.push(Popup { x: x + 0.5, y: y + 0.5, pts, life: 1.0 });
    }
    fn blast(&mut self, x: usize, y: usize, kind: u8) {
        self.blasts.push(Blast { x, y, kind, life: 1.0 });
    }
}

// ---------- layout ----------

#[derive(Clone, Copy)]
struct Layout {
    tall: bool,
    /// board's top left and square size
    bx: f32,
    by: f32,
    cell: f32,
}

fn layout(pw: f32) -> Layout {
    if pw < 450.0 {
        Layout { tall: true, bx: 12.0, by: 96.0, cell: 47.0 }
    } else {
        Layout { tall: false, bx: 16.0, by: 26.0, cell: 46.0 }
    }
}

struct Facets {
    s: Save,
    fx: Fx,
    /// the seven cut gems and a glint at the current scale, and that scale
    gems: Vec<gem::Image>,
    gems_r: i32,
    /// pointer: the square pressed, where, and whether it was picked already
    press: Option<(i32, i32, f32, f32, bool)>,
    keyboard: bool,
    banner: f32,
    banner_level: i32,
    clock: f32,
    lay: Layout,
}

impl Facets {
    fn theme(&self) -> &'static Theme {
        &THEMES[self.s.opt[O_THEME] as usize % THEMES.len()]
    }
    fn hint(&self) -> Option<(usize, usize, usize, usize)> {
        if self.s.opt[O_HINTS] == 0 && self.s.b.can_play() && self.s.b.idle_ms >= HINT_MS {
            self.s.b.find_move()
        } else {
            None
        }
    }
    fn cell_at(&self, x: f32, y: f32) -> Option<(i32, i32)> {
        let l = self.lay;
        let (cx, cy) = (((x - l.bx) / l.cell).floor() as i32, ((y - l.by) / l.cell).floor() as i32);
        if inb(cx, cy) { Some((cx, cy)) } else { None }
    }
    fn swap(&mut self, a: (i32, i32), b: (i32, i32)) -> u32 {
        self.s.sel = [-1, -1];
        if self.s.b.try_swap(a.0, a.1, b.0, b.1) { wg::EV_BLIP } else { 0 }
    }
    fn sel(&self) -> Option<(i32, i32)> {
        if inb(self.s.sel[0], self.s.sel[1]) { Some((self.s.sel[0], self.s.sel[1])) } else { None }
    }

    fn draw_back(&self, s: &mut Screen) {
        let t = self.theme();
        let l = self.lay;
        let (x0, y0, x1, y1) = s.visible();
        s.rect_grad(x0, y0, x1 - x0, y1 - y0, 0.0, hex(t.top), hex(t.bottom), 1.0);
        let w = l.cell * N as f32;
        s.shadow(l.bx - 6.0, l.by - 3.0, w + 12.0, w + 12.0, 12.0, 10.0, 0, 0.4);
        s.rect(l.bx - 6.0, l.by - 6.0, w + 12.0, w + 12.0, 12.0, hex(t.well), 1.0);
        for y in 0..N {
            for x in 0..N {
                let c = if (x + y) % 2 == 0 { t.cell_a } else { t.cell_b };
                s.rect(l.bx + x as f32 * l.cell + 1.0, l.by + y as f32 * l.cell + 1.0, l.cell - 2.0, l.cell - 2.0, 7.0, hex(c), 1.0);
            }
        }
        if l.tall {
            s.rect(12.0, 12.0, 376.0, 66.0, 12.0, hex(t.bar), 0.55);
        } else {
            s.rect(396.0, 26.0, 74.0, 368.0, 12.0, hex(t.bar), 0.55);
        }
    }

    fn draw_hud(&self, s: &mut Screen) {
        let t = self.theme();
        let b = &self.s.b;
        let (ink, dim, accent) = (hex(t.ink), hex(t.dim), hex(t.accent));
        let score = group(b.score);
        // what the bar shows, and its label and number
        let (label, value, frac) = match b.mode {
            MODE_TIMED => (wg::tr(&["Time", "时间", "タイム", "시간", "Tiempo", "Tempo", "Temps", "Zeit", "Время", "時間"]), wg::clock(b.time_left), b.time_left as f32 / TIMED_MS as f32),
            MODE_MOVES => (wg::tr(&["Moves", "步数", "手数", "이동", "Movimientos", "Movimentos", "Coups", "Züge", "Ходы", "步數"]), b.moves_left.to_string(), b.moves_left as f32 / MOVES as f32),
            _ => (wg::tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]), b.level.to_string(), b.progress as f32 / level_need(b.level) as f32),
        };
        let low = (b.mode == MODE_TIMED && b.time_left < 10_000) || (b.mode == MODE_MOVES && b.moves_left <= 3);
        let vc = if low { hex(0xff5a5a) } else { ink };
        if self.lay.tall {
            s.text(wg::tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки"]), 26.0, 30.0, 12.0, dim, 1.0);
            let size = s.fit_size(&score, 28.0, 190.0);
            s.text(&score, 26.0, 55.0, size, ink, 1.0);
            s.text_r(label, 374.0, 30.0, 12.0, dim, 1.0);
            s.text_r(&value, 374.0, 55.0, 28.0, vc, 1.0);
            if b.mode == MODE_CLASSIC {
                self.dots(s, 374.0 - s.text_width(&value, 28.0) - 14.0, 55.0, dim, accent);
            }
            s.rect(26.0, 70.0, 348.0, 4.0, 2.0, hex(t.well), 0.8);
            s.rect(26.0, 70.0, 348.0 * frac.clamp(0.0, 1.0), 4.0, 2.0, accent, 1.0);
        } else {
            let cx = 433.0;
            s.text_c(wg::tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки"]), cx, 50.0, 11.0, dim, 1.0);
            let size = s.fit_size(&score, 20.0, 64.0);
            s.text_c(&score, cx, 72.0, size, ink, 1.0);
            s.text_c(label, cx, 116.0, 11.0, dim, 1.0);
            let vs = s.fit_size(&value, 26.0, 64.0);
            s.text_c(&value, cx, 141.0, vs, vc, 1.0);
            // the bar runs up the panel
            let (bx, by, bh) = (cx - 3.0, 170.0, 190.0);
            s.rect(bx, by, 6.0, bh, 3.0, hex(t.well), 0.8);
            let f = frac.clamp(0.0, 1.0);
            s.rect(bx, by + bh * (1.0 - f), 6.0, bh * f, 3.0, accent, 1.0);
            if b.mode == MODE_CLASSIC {
                self.dots(s, cx + 21.0, 376.0, dim, accent);
            }
        }
    }

    /// Classic's reshuffles left, as three dots ending at x
    fn dots(&self, s: &mut Screen, x: f32, y: f32, dim: u32, accent: u32) {
        for i in 0..SHUFFLES {
            let cx = x - i as f32 * 10.0;
            if i < self.s.b.shuffles {
                s.disc(cx, y, 3.2, accent, 1.0);
            } else {
                s.ring(cx, y, 3.2, 1.2, dim, 0.8);
            }
        }
    }

    fn draw_board(&mut self, s: &mut Screen) {
        let l = self.lay;
        let b = self.s.b;
        let r = l.cell * 0.4;
        let phase = b.phase();
        // the gems, drawn once at this scale
        // whole pixels, so a small resize keeps the pictures
        let rp = (r * s.k).round().max(4.0);
        if self.gems_r != rp as i32 || self.gems.is_empty() {
            let size = (rp * 2.1).ceil() as i32 + 2;
            self.gems = (1..=COLOURS).map(|c| {
                let (base, light, dark) = GEM[c as usize];
                gem::render(&gem::Cut { pts: shape(c), round: c == 5, step: c == 4 || c == 1, base, light, dark }, size, rp)
            }).collect();
            self.gems.push(gem::render_glint(size, rp));
            self.gems_r = rp as i32;
        }
        let clip = {
            let (x0, y0) = s.px(l.bx - 6.0, l.by - 6.0);
            let (x1, y1) = s.px(l.bx + l.cell * N as f32 + 6.0, l.by + l.cell * N as f32 + 6.0);
            (x0.floor() as i32, y0.floor() as i32, x1.ceil() as i32, y1.ceil() as i32)
        };
        let t = self.theme();
        let sel = self.sel();
        if let Some((x, y)) = sel {
            let (px, py) = (l.bx + x as f32 * l.cell, l.by + y as f32 * l.cell);
            s.rect(px + 1.0, py + 1.0, l.cell - 2.0, l.cell - 2.0, 7.0, hex(t.accent), 0.28);
        }
        let hint = self.hint();
        if let Some((x, y, x2, y2)) = hint {
            let a = 0.18 + 0.16 * (self.clock * 5.0).sin();
            for (hx, hy) in [(x, y), (x2, y2)] {
                s.rect(l.bx + hx as f32 * l.cell + 1.0, l.by + hy as f32 * l.cell + 1.0, l.cell - 2.0, l.cell - 2.0, 7.0, hex(0xffffff), a);
            }
        }
        // where the two swapped gems are drawn mid-swap
        let swap_t = match phase {
            Phase::Swap | Phase::Back => wg::ease_out(b.t as f32 / SWAP_MS as f32),
            _ => -1.0,
        };
        let shrink = match phase {
            Phase::Shuffle => {
                let u = b.t as f32 / SHUFFLE_MS as f32;
                (1.0 - 2.0 * u).abs().clamp(0.0, 1.0)
            }
            _ => 1.0,
        };
        let clear_u = if phase == Phase::Clear { (b.t as f32 / CLEAR_MS as f32).clamp(0.0, 1.0) } else { 0.0 };
        s.set_clip(l.bx - 6.0, l.by - 6.0, l.cell * N as f32 + 12.0, l.cell * N as f32 + 12.0);
        for y in 0..N {
            for x in 0..N {
                let (g, k) = (b.gem[y][x], b.kind[y][x]);
                if g == 0 {
                    continue;
                }
                let mut gx = x as f32;
                let mut gy = y as f32 + b.off[y][x] as f32 / 1000.0;
                if swap_t >= 0.0 {
                    // the gem on one swapped square slides to the other
                    let (ix, iy) = (x as i32, y as i32);
                    let (a, z) = ((b.ax, b.ay), (b.bx, b.by));
                    let to = if (ix, iy) == a { Some(z) } else if (ix, iy) == z { Some(a) } else { None };
                    if let Some(to) = to {
                        gx += (to.0 - ix) as f32 * swap_t;
                        gy += (to.1 - iy) as f32 * swap_t;
                    }
                }
                let (cx, cy) = (l.bx + (gx + 0.5) * l.cell, l.by + (gy + 0.5) * l.cell);
                let dying = b.dying[y][x] != 0 && phase == Phase::Clear;
                let mut z = shrink.min(1.0);
                if dying {
                    z = 1.0 + 0.18 * clear_u - 1.18 * clear_u * clear_u;
                    let glow = if g == PRISM { 0xffffff } else { GEM[g as usize].1 };
                    s.ring(cx, cy, r * (0.6 + 0.8 * clear_u), r * 0.22 * (1.0 - clear_u) + 0.5, hex(glow), 0.9 * (1.0 - clear_u));
                }
                if z <= 0.02 {
                    continue;
                }
                let (px, py) = s.px(cx, cy);
                if g == PRISM {
                    draw_prism(s, cx, cy, r * z, 1.0);
                    gem::blit(s, &self.gems[COLOURS as usize], px, py, z, 1.0, clip);
                    continue;
                }
                gem::blit(s, &self.gems[g as usize - 1], px, py, z, 1.0, clip);
                if k != NORMAL {
                    draw_special(s, cx, cy, r * z, g, k, 1.0);
                }
            }
        }
        s.no_clip();
        if let Some((x, y)) = sel {
            let (cx, cy) = (l.bx + (x as f32 + 0.5) * l.cell, l.by + (y as f32 + 0.5) * l.cell);
            s.ring(cx, cy, l.cell * 0.49, 2.2, hex(t.accent), 1.0);
        }
        if self.keyboard && b.can_play() {
            let (x, y) = (self.s.cur[0] as f32, self.s.cur[1] as f32);
            let (px, py, w) = (l.bx + x * l.cell + 2.0, l.by + y * l.cell + 2.0, l.cell - 4.0);
            let c = hex(t.ink);
            for (ax, ay, bx2, by2) in [(px, py, px + w, py), (px + w, py, px + w, py + w), (px + w, py + w, px, py + w), (px, py + w, px, py)] {
                s.line(ax, ay, bx2, by2, 1.2, c, 0.85);
            }
        }
    }

    fn draw_fx(&self, s: &mut Screen) {
        let l = self.lay;
        let w = l.cell * N as f32;
        let at = |x: f32, y: f32| (l.bx + x * l.cell, l.by + y * l.cell);
        for bl in &self.fx.blasts {
            let (cx, cy) = at(bl.x as f32 + 0.5, bl.y as f32 + 0.5);
            let u = 1.0 - bl.life;
            let a = bl.life;
            match bl.kind {
                CROSS => {
                    let h = l.cell * 0.5 * (1.0 - u * 0.7);
                    s.rect(l.bx, cy - h / 2.0, w, h, h / 2.0, hex(0xffffff), 0.7 * a);
                    s.rect(cx - h / 2.0, l.by, h, w, h / 2.0, hex(0xffffff), 0.7 * a);
                }
                BOMB => {
                    s.ring(cx, cy, l.cell * (0.6 + 1.2 * u), l.cell * 0.18 * a + 1.0, hex(0xfff3c4), 0.9 * a);
                }
                _ => {
                    for i in 0..3 {
                        let rr = l.cell * (0.5 + 4.0 * u) - i as f32 * l.cell * 0.4;
                        if rr > 0.0 {
                            s.ring(cx, cy, rr, 3.0, hex(GEM[1 + (i * 2) as usize].1), 0.8 * a);
                        }
                    }
                }
            }
        }
        for p in &self.fx.sparks {
            let (x, y) = at(p.x, p.y);
            s.disc(x, y, 2.4 * p.life + 0.6, p.c, p.life.min(1.0));
        }
        let ink = hex(self.theme().ink);
        for p in &self.fx.pops {
            let (x, y) = at(p.x, p.y);
            let a = (p.life * 2.0).min(1.0);
            let txt = p.pts.to_string();
            s.text_c(&txt, x + 1.0, y + 1.5, 17.0, 0, 0.45 * a);
            s.text_c(&txt, x, y, 17.0, ink, a);
        }
    }

    fn draw_banner(&self, s: &mut Screen) {
        let l = self.lay;
        let w = l.cell * N as f32;
        let (cx, cy) = (l.bx + w / 2.0, l.by + w / 2.0);
        let t = self.theme();
        let b = &self.s.b;
        let text: Option<(String, f32)> = if b.over() && b.end_ms > 0 {
            let msg = match b.end {
                END_STUCK => wg::tr(&["No moves left", "无路可走", "手詰まり", "이동할 수 없습니다", "Sin jugadas posibles", "Sem jogadas possíveis", "Aucun coup possible", "Kein Zug möglich", "Ходов нет", "無路可走"]),
                END_TIME => wg::tr(&["Time is up", "时间到", "タイムアップ", "시간 종료", "Tiempo agotado", "Tempo esgotado", "Temps écoulé", "Zeit abgelaufen", "Время вышло", "時間到"]),
                _ => wg::tr(&["Out of moves", "步数用完", "手数切れ", "남은 이동이 없습니다", "Sin movimientos", "Sem movimentos", "Plus de coups", "Keine Züge mehr", "Ходы закончились", "步數用完"]),
            };
            Some((msg.to_string(), (b.end_ms as f32 / 250.0).min(1.0)))
        } else if self.banner > 0.0 {
            Some((wg::tr(&["Level {}", "第 {} 关", "レベル {}", "레벨 {}", "Nivel {}", "Nível {}", "Niveau {}", "Level {}", "Уровень {}", "第 {} 關"]).replace("{}", &self.banner_level.to_string()), self.banner.min(1.0)))
        } else if b.phase() == Phase::Shuffle {
            Some((wg::tr(&["Shuffle", "洗牌", "シャッフル", "셔플", "Mezclando", "Embaralhando", "Mélange", "Mischen", "Перемешивание"]).to_string(), 1.0))
        } else {
            None
        };
        if let Some((msg, a)) = text {
            let tw = s.text_width(&msg, 30.0) + 44.0;
            s.shadow(cx - tw / 2.0, cy - 30.0, tw, 60.0, 30.0, 12.0, 0, 0.5 * a);
            s.rect(cx - tw / 2.0, cy - 30.0, tw, 60.0, 30.0, hex(t.well), 0.92 * a);
            s.text_c(&msg, cx, cy, 30.0, hex(t.accent), a);
        }
    }
}

/// 1234567 as "1,234,567"
fn group(n: i32) -> String {
    let d = n.to_string();
    let mut out = String::new();
    for (i, ch) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

impl Game for Facets {
    type Save = Save;
    const PLAN: (f32, f32) = (480.0, 420.0);
    const CHOICES: &'static [i32] = &[3, 3, 3, 2];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let mut b = Board::zeroed();
        b.start(1, MODE_CLASSIC, DIFF_NORMAL);
        Facets {
            s: Save { version: VERSION, opt: [0, DIFF_NORMAL, 0, 0], b, sel: [-1, -1], cur: [3, 3], rep: KeyRepeat(0) },
            fx: Fx::default(),
            gems: Vec::new(),
            gems_r: 0,
            press: None,
            keyboard: false,
            banner: 0.0,
            banner_level: 1,
            clock: 0.0,
            lay: layout(480.0),
        }
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        if h as f32 > w as f32 * 1.12 { (400.0, 480.0) } else { Self::PLAN }
    }
    fn start(&mut self, seed: u32) {
        let opt = self.s.opt;
        self.s.b.start(seed, opt[O_MODE], opt[O_DIFF]);
        self.s.sel = [-1, -1];
        self.s.cur = [3, 3];
        self.fx = Fx { seed, ..Fx::default() };
        self.press = None;
        self.banner = 0.0;
    }
    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt;
        self.clock += dt as f32 / 1000.0;
        self.fx.step(dt as f32 / 1000.0);
        self.banner = (self.banner - dt as f32 / 1000.0).max(0.0);
        let mut ev = 0;
        // keyboard: a cursor; Space picks a gem, an arrow then swaps it
        let dirs = self.s.rep.dirs(input);
        if input.pressed != 0 {
            self.keyboard = true;
        }
        if !self.s.b.over() {
            let (x, y) = (self.s.cur[0], self.s.cur[1]);
            for (bit, dx, dy) in [(wg::LEFT, -1, 0), (wg::RIGHT, 1, 0), (wg::UP, 0, -1), (wg::DOWN, 0, 1)] {
                if dirs & bit == 0 || !inb(x + dx, y + dy) {
                    continue;
                }
                self.s.cur = [x + dx, y + dy];
                if let Some(sel) = self.sel() {
                    if input.pressed & bit != 0 && sel == (x, y) {
                        ev |= self.swap((x, y), (x + dx, y + dy));
                    }
                }
                break;
            }
            if input.pressed & wg::A != 0 {
                let c = (self.s.cur[0], self.s.cur[1]);
                match self.sel() {
                    Some(sel) if sel == c => self.s.sel = [-1, -1],
                    Some(sel) if (sel.0 - c.0).abs() + (sel.1 - c.1).abs() == 1 => ev |= self.swap(sel, c),
                    _ => self.s.sel = [c.0, c.1],
                }
            }
            if input.pressed & wg::B != 0 {
                self.s.sel = [-1, -1];
            }
        }
        if dt > 0 {
            let lv = self.s.b.level;
            let e = self.s.b.step(dt, &mut self.fx) as u32;
            if e & wg::EV_LEVEL != 0 {
                self.banner = 1.4;
                self.banner_level = self.s.b.level.max(lv);
            }
            if self.s.b.over() {
                self.s.sel = [-1, -1];
            }
            ev |= e;
        }
        ev
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.press = None;
            return 0;
        }
        if p.pressed != 0 || p.held != 0 {
            self.keyboard = false;
        }
        if self.s.b.over() {
            return 0;
        }
        let at = self.cell_at(p.x, p.y);
        if p.pressed & wg::M_LEFT != 0 {
            let Some(c) = at else {
                self.s.sel = [-1, -1];
                return 0;
            };
            self.s.cur = [c.0, c.1];
            if let Some(sel) = self.sel() {
                if (sel.0 - c.0).abs() + (sel.1 - c.1).abs() == 1 {
                    self.press = None;
                    return self.swap(sel, c);
                }
            }
            let was = self.sel() == Some(c);
            self.s.sel = [c.0, c.1];
            self.press = Some((c.0, c.1, p.x, p.y, was));
            return 0;
        }
        if let Some((x, y, px, py, was)) = self.press {
            if p.held & wg::M_LEFT != 0 {
                // a drag far enough one way swaps with that neighbour
                let (dx, dy) = (p.x - px, p.y - py);
                let reach = self.lay.cell * 0.38;
                if dx.abs().max(dy.abs()) > reach {
                    let (sx, sy) = if dx.abs() > dy.abs() { (dx.signum() as i32, 0) } else { (0, dy.signum() as i32) };
                    self.press = None;
                    if inb(x + sx, y + sy) {
                        return self.swap((x, y), (x + sx, y + sy));
                    }
                }
            } else if p.released & wg::M_LEFT != 0 {
                self.press = None;
                if was && at == Some((x, y)) {
                    self.s.sel = [-1, -1];
                }
            }
        }
        0
    }
    fn picture(&mut self) -> u32 {
        let b = &self.s.b;
        let mut h = wg::hash(0, wg::bytes_of(&b.gem));
        h = wg::hash(h, wg::bytes_of(&b.kind));
        let shown = [b.score, b.level, b.progress, b.time_left / 1000, b.moves_left, b.shuffles, b.phase, self.s.sel[0], self.s.sel[1], self.s.cur[0], self.s.cur[1], self.keyboard as i32, self.s.opt[O_THEME], self.hint().is_some() as i32, (b.end_ms > 0) as i32];
        wg::hash(h, wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        let b = &self.s.b;
        let settled = b.phase() == Phase::Idle || (b.phase() == Phase::Over && b.end_ms <= 0);
        !settled || self.fx.busy() || self.banner > 0.0 || self.hint().is_some()
    }
    fn draw(&mut self, s: &mut Screen) {
        self.lay = layout(s.pw);
        let key = 1 + self.s.opt[O_THEME] as u32 * 2 + self.lay.tall as u32;
        let me = &*self;
        s.background(key, |s| me.draw_back(s));
        self.draw_hud(s);
        self.draw_board(s);
        self.draw_fx(s);
        self.draw_banner(s);
    }
    fn score(&self) -> u32 {
        self.s.b.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.b.over()
    }
    fn stat(&self, i: usize) -> i32 {
        let b = &self.s.b;
        match i {
            0 => b.time_ms,
            1 => b.moves,
            2 => b.cleared,
            3 => b.best_chain,
            4 => b.specials,
            5 => b.biggest,
            6 if b.mode == MODE_CLASSIC => b.level,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        self.fx = Fx::default();
        self.press = None;
        self.banner = 0.0;
        let s = &self.s;
        s.version == VERSION
            && (0..3).contains(&s.opt[O_MODE])
            && s.b.mode == s.opt[O_MODE]
            && s.b.diff == s.opt[O_DIFF]
            && s.b.valid()
            && (s.sel == [-1, -1] || inb(s.sel[0], s.sel[1]))
            && inb(s.cur[0], s.cur[1])
    }
}

wg::game!(Facets);
