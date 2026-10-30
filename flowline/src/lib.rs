//! Flowline - an original wasmga.me game. MIT licensed.
//! A board of pipe pieces lies between a tap on the left and a drain on the
//! right. Tap a piece to turn it a quarter; the water runs through every
//! pipe joined to the tap as you go. Join the tap to the drain to finish
//! the level. 30 levels, from 4 x 4 to 8 x 8. The page shows the setup,
//! pause and game over screens.
mod glyphs;
mod puzzle;

use puzzle::{Grid, DX, DY, LEVELS, MAXN};
use wg::{hex, mix, tr, Game, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 1;

/// options, in the catalog `setup` order
const O_LEVEL: usize = 0;
const O_THEME: usize = 1;
const THEMES: usize = 3;

const PLAY: i32 = 0;
const CLEARED: i32 = 1;
const DONE: i32 = 2;

/// a quarter turn, and water filling one pipe, ms
const TURN_MS: f32 = 130.0;
const FILL_MS: f32 = 110.0;
/// the drain's basin filling once the water is through
const BASIN_MS: f32 = 650.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 2],
    level: i32,
    phase: i32,
    grid: Grid,
    par: i32,
    /// quarter turns, this level and the whole run
    turns: i32,
    all_turns: i32,
    level_ms: i32,
    time_ms: i32,
    done: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
    /// the water is through and the basin is filling
    through: i32,
    /// keyboard cursor, a square
    cx: i32,
    cy: i32,
    rep: KeyRepeat,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}

struct Theme {
    bg: u32,
    bg2: u32,
    well: u32,
    tile: u32,
    pipe: u32,
    pipe_lo: u32,
    channel: u32,
    water: u32,
    water_hi: u32,
    text: u32,
    dim: u32,
    overlay: u32,
    button: u32,
}

const THEME: [Theme; THEMES] = [
    // Night
    Theme { bg: 0x1a1f33, bg2: 0x10131f, well: 0x262c45, tile: 0x2f3655, pipe: 0xb8c0d8, pipe_lo: 0x7d86a3, channel: 0x1d2236, water: 0x2f8bff, water_hi: 0x8cc8ff, text: 0xf2f4fb, dim: 0x8f97b8, overlay: 0x0b0d16, button: 0x3b63d6 },
    // Brass
    Theme { bg: 0x2a2320, bg2: 0x1a1513, well: 0x3a302b, tile: 0x463a33, pipe: 0xe0b45e, pipe_lo: 0xa67c32, channel: 0x2a211c, water: 0x37a6e8, water_hi: 0xa7dcff, text: 0xf6ead8, dim: 0xb5a289, overlay: 0x120e0c, button: 0x2f8fd0 },
    // Paper
    Theme { bg: 0xf6f1e6, bg2: 0xe7dfcc, well: 0xe2d8c4, tile: 0xece4d3, pipe: 0x8a96a8, pipe_lo: 0x5f6b7d, channel: 0xd9d2c3, water: 0x2f7fe0, water_hi: 0x9ccbff, text: 0x3a2f24, dim: 0x8a7a66, overlay: 0x5a4a38, button: 0x2f7fe0 },
];

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    None,
    Cell(i32, i32),
    Next,
}

struct Flowline {
    s: Save,
    tall: bool,
    hover: Hit,
    keys: bool,
    /// not saved: each pipe's turn still to show (quarters, eases to 0) and
    /// how full of water it is (0..1)
    spin: [f32; MAXN * MAXN],
    wet: [f32; MAXN * MAXN],
    basin: f32,
}

impl Flowline {
    fn theme(&self) -> &'static Theme {
        &THEME[self.s.opt[O_THEME] as usize]
    }
    fn plan_wh(&self) -> (f32, f32) {
        if self.tall { (300.0, 400.0) } else { (400.0, 300.0) }
    }
    /// the board's box: x, y, side, and a square's side
    fn board_box(&self) -> (f32, f32, f32, f32) {
        let w = self.plan_wh().0;
        let side = if self.tall { 220.0 } else { 236.0 };
        let (x, y) = ((w - side) / 2.0, if self.tall { 92.0 } else { 46.0 });
        (x, y, side, side / self.s.grid.n as f32)
    }
    /// the card over the board, in the middle of the screen
    fn card(&self) -> (f32, f32, f32, f32) {
        let (w, h) = self.plan_wh();
        let (pw, ph) = (248.0, 158.0);
        (w / 2.0 - pw / 2.0, h / 2.0 - ph / 2.0, pw, ph)
    }
    /// its button, inside the card along the bottom
    fn next_box(&self) -> (f32, f32, f32, f32) {
        let (px, py, pw, ph) = self.card();
        let (bw, bh) = (pw - 48.0, 42.0);
        (px + (pw - bw) / 2.0, py + ph - bh - 20.0, bw, bh)
    }
    fn hit(&self, x: f32, y: f32) -> Hit {
        if self.s.phase != PLAY {
            let b = self.next_box();
            let on = x >= b.0 && x < b.0 + b.2 && y >= b.1 && y < b.1 + b.3;
            return if self.s.phase == CLEARED && on { Hit::Next } else { Hit::None };
        }
        let (bx, by, _, c) = self.board_box();
        let (gx, gy) = (((x - bx) / c).floor() as i32, ((y - by) / c).floor() as i32);
        if self.s.grid.inside(gx, gy) { Hit::Cell(gx, gy) } else { Hit::None }
    }

    fn begin(&mut self, level: i32) {
        let g = puzzle::deal(level);
        let par = puzzle::solve(&g).map_or(0, |sol| puzzle::par(&g, &sol));
        let s = &mut self.s;
        s.level = level;
        s.phase = PLAY;
        s.grid = g;
        s.par = par;
        s.turns = 0;
        s.level_ms = 0;
        s.through = 0;
        let n = g.n;
        s.cx = s.cx.clamp(0, n - 1);
        s.cy = s.cy.clamp(0, n - 1);
        self.spin = [0.0; MAXN * MAXN];
        self.wet = [0.0; MAXN * MAXN];
        self.basin = 0.0;
    }

    /// turns the pipe on (x, y) a quarter, clockwise or back
    fn turn(&mut self, x: i32, y: i32, back: bool) -> u32 {
        if self.s.phase != PLAY || self.s.through != 0 || !self.s.grid.inside(x, y) {
            return 0;
        }
        let i = (y * self.s.grid.n + x) as usize;
        let r = &mut self.s.grid.rot[i];
        *r = (*r + if back { 3 } else { 1 }) % 4;
        self.spin[i] += if back { -1.0 } else { 1.0 };
        self.s.turns += 1;
        self.s.all_turns += 1;
        if self.s.grid.through() {
            self.s.through = 1;
        }
        wg::EV_BLIP
    }

    fn clear_level(&mut self) -> u32 {
        let s = &mut self.s;
        let n = s.grid.n;
        let score = (200 + 10 * n * n - 10 * (s.turns - s.par).max(0)).max(50);
        s.score += score;
        s.cleared = s.level;
        s.cleared_score = score;
        s.done += 1;
        if s.level + 1 >= LEVELS {
            s.phase = DONE;
            return wg::EV_LEVEL | wg::EV_WIN | wg::EV_GAMEOVER;
        }
        s.phase = CLEARED;
        wg::EV_LEVEL | wg::EV_WIN
    }

    fn next(&mut self) -> u32 {
        if self.s.phase != CLEARED {
            return 0;
        }
        self.begin(self.s.level + 1);
        wg::EV_BLIP
    }

    /// Water runs on into a pipe once the one before it is full, and is gone
    /// from pipes no longer joined to the tap.
    fn flow(&mut self, dt: f32) {
        let g = &self.s.grid;
        let dist = g.flow();
        let n = g.n;
        let step = dt / FILL_MS;
        let mut wet = self.wet;
        for y in 0..n {
            for x in 0..n {
                let i = (y * n + x) as usize;
                if dist[i] < 0 {
                    wet[i] = 0.0;
                    continue;
                }
                let fed = dist[i] == 0 || self.feeder(&dist, x, y).map_or(false, |j| self.wet[j] >= 1.0);
                if fed {
                    wet[i] = (wet[i] + step).min(1.0);
                }
            }
        }
        self.wet = wet;
    }

    /// the square the water comes into (x, y) from: a joined neighbour one
    /// pipe nearer the tap
    fn feeder(&self, dist: &[i32; MAXN * MAXN], x: i32, y: i32) -> Option<usize> {
        let g = &self.s.grid;
        let n = g.n;
        let d = dist[(y * n + x) as usize];
        let here = g.open(x, y);
        (0..4).find_map(|k| {
            let (nx, ny) = (x + DX[k], y + DY[k]);
            if here & (1 << k) == 0 || !g.inside(nx, ny) {
                return None;
            }
            let j = (ny * n + nx) as usize;
            (dist[j] == d - 1 && g.open(nx, ny) & (1 << ((k + 2) % 4)) != 0).then_some(j)
        })
    }

    /// the side the water comes in by on (x, y), 0..3, if any
    fn entry(&self, dist: &[i32; MAXN * MAXN], x: i32, y: i32) -> Option<usize> {
        let n = self.s.grid.n;
        if x == 0 && y == self.s.grid.tap && dist[(y * n) as usize] == 0 {
            return Some(3);
        }
        let j = self.feeder(dist, x, y)?;
        let (fx, fy) = ((j as i32) % n, (j as i32) / n);
        (0..4).find(|&k| x + DX[k] == fx && y + DY[k] == fy)
    }
}

/// a pipe: arms from the middle out to each opening, turned by `ang`
/// radians; water fills `wet` of it, coming in by `entry`
#[allow(clippy::too_many_arguments)]
fn draw_pipe(s: &mut Screen, th: &Theme, cx: f32, cy: f32, c: f32, open: u8, ang: f32, wet: f32, entry: Option<usize>) {
    let arm = |k: usize| {
        let a = k as f32 * std::f32::consts::FRAC_PI_2 + ang;
        // up is k = 0
        (a.sin(), -a.cos())
    };
    let half = c * 0.5;
    let (outer, inner) = (c * 0.19, c * 0.085);
    // round ends stop at the square's edge, with a collar at each opening
    let reach = half - outer;
    let arms: Vec<usize> = (0..4).filter(|k| open & (1 << k) != 0).collect();
    for &k in &arms {
        let (dx, dy) = arm(k);
        s.line(cx, cy + c * 0.03, cx + dx * reach, cy + dy * reach + c * 0.03, outer, hex(th.pipe_lo), 1.0);
    }
    for &k in &arms {
        let (dx, dy) = arm(k);
        s.line(cx, cy, cx + dx * reach, cy + dy * reach, outer, hex(th.pipe), 1.0);
    }
    s.disc(cx, cy, outer * 1.08, hex(th.pipe), 1.0);
    for &k in &arms {
        let (dx, dy) = arm(k);
        let (ex, ey) = (cx + dx * (half - c * 0.07), cy + dy * (half - c * 0.07));
        let (px, py) = (-dy * outer * 1.15, dx * outer * 1.15);
        s.line(ex - px, ey - py, ex + px, ey + py, c * 0.05, hex(th.pipe), 1.0);
    }
    let half = half - inner;
    for &k in &arms {
        let (dx, dy) = arm(k);
        s.line(cx, cy, cx + dx * half, cy + dy * half, inner, hex(th.channel), 1.0);
    }
    if wet <= 0.0 {
        return;
    }
    // in by the entry to the middle, then out along the other arms
    let col = hex(th.water);
    let first = (wet * 2.0).min(1.0);
    let rest = ((wet - 0.5) * 2.0).max(0.0);
    if let Some(e) = entry {
        let (dx, dy) = arm(e);
        let (ex, ey) = (cx + dx * half, cy + dy * half);
        s.line(ex, ey, ex - dx * half * first, ey - dy * half * first, inner * 0.92, col, 1.0);
    }
    if rest > 0.0 {
        s.disc(cx, cy, inner * 0.92, col, 1.0);
        for &k in &arms {
            if Some(k) == entry {
                continue;
            }
            let (dx, dy) = arm(k);
            s.line(cx, cy, cx + dx * half * rest, cy + dy * half * rest, inner * 0.92, col, 1.0);
        }
    }
    // a glint on the water
    if wet >= 1.0 {
        for &k in &arms {
            let (dx, dy) = arm(k);
            s.line(cx + dx * half * 0.2, cy + dy * half * 0.2 - inner * 0.35, cx + dx * half * 0.8, cy + dy * half * 0.8 - inner * 0.35, inner * 0.22, hex(th.water_hi), 0.6);
        }
    }
}

impl Game for Flowline {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS, THEMES as i32];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        Flowline {
            s: Save {
                version: VERSION,
                opt: [0, 0],
                level: 0,
                phase: PLAY,
                grid: Grid { n: 4, tap: 0, drain: 0, pipe: [0; MAXN * MAXN], rot: [0; MAXN * MAXN] },
                par: 0,
                turns: 0,
                all_turns: 0,
                level_ms: 0,
                time_ms: 0,
                done: 0,
                score: 0,
                cleared: -1,
                cleared_score: 0,
                through: 0,
                cx: 0,
                cy: 0,
                rep: KeyRepeat(0),
            },
            tall: false,
            hover: Hit::None,
            keys: false,
            spin: [0.0; MAXN * MAXN],
            wet: [0.0; MAXN * MAXN],
            basin: 0.0,
        }
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, _seed: u32) {
        let opt = self.s.opt;
        self.s = Save { opt, ..Flowline::new().s };
        self.begin(opt[O_LEVEL]);
        self.keys = false;
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        if h as f32 > w as f32 * 1.1 { (300.0, 400.0) } else { (400.0, 300.0) }
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        let k = 0.5f32.powf(dt / (TURN_MS * 0.35));
        for sp in self.spin.iter_mut() {
            *sp *= k;
            if sp.abs() < 0.003 {
                *sp = 0.0;
            }
        }
        self.flow(dt);
        if self.s.through != 0 && self.s.phase == PLAY {
            let g = &self.s.grid;
            let last = ((g.drain * g.n) + g.n - 1) as usize;
            if self.wet[last] >= 1.0 {
                self.basin = (self.basin + dt / BASIN_MS).min(1.0);
                if self.basin >= 1.0 {
                    ev |= self.clear_level();
                }
            }
        }
        if self.s.phase == PLAY && self.s.turns > 0 {
            self.s.level_ms = self.s.level_ms.saturating_add(input.dt);
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        let dirs = self.s.rep.dirs(input);
        if dirs != 0 {
            self.keys = true;
            self.hover = Hit::None;
        }
        let n = self.s.grid.n;
        if self.s.phase == PLAY {
            if dirs & wg::LEFT != 0 { self.s.cx = (self.s.cx - 1).max(0); }
            if dirs & wg::RIGHT != 0 { self.s.cx = (self.s.cx + 1).min(n - 1); }
            if dirs & wg::UP != 0 { self.s.cy = (self.s.cy - 1).max(0); }
            if dirs & wg::DOWN != 0 { self.s.cy = (self.s.cy + 1).min(n - 1); }
            if input.pressed & wg::A != 0 {
                self.keys = true;
                ev |= self.turn(self.s.cx, self.s.cy, false);
            }
            if input.pressed & wg::B != 0 {
                self.keys = true;
                ev |= self.turn(self.s.cx, self.s.cy, true);
            }
        } else if self.s.phase == CLEARED && input.pressed & wg::A != 0 {
            ev |= self.next();
        }
        ev
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.hover = Hit::None;
            return 0;
        }
        let h = self.hit(p.x, p.y);
        self.hover = if p.touch { Hit::None } else { h };
        let pressed = p.pressed & (wg::M_LEFT | wg::M_RIGHT);
        if pressed != 0 {
            self.keys = false;
            return match h {
                Hit::Cell(x, y) => {
                    self.s.cx = x;
                    self.s.cy = y;
                    self.turn(x, y, pressed & wg::M_RIGHT != 0)
                }
                Hit::Next if pressed & wg::M_LEFT != 0 => self.next(),
                _ => 0,
            };
        }
        0
    }

    fn picture(&mut self) -> u32 {
        let hv = match self.hover {
            Hit::None => -1,
            Hit::Cell(x, y) => y * 16 + x,
            Hit::Next => 1000,
        };
        let shown = [hv, self.keys as i32, self.tall as i32];
        let mut c = self.s;
        c.level_ms = 0;
        c.time_ms = 0;
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        let dist = self.s.grid.flow();
        let settling = self.wet.iter().zip(dist.iter()).any(|(&w, &d)| d >= 0 && w < 1.0);
        self.spin.iter().any(|&s| s != 0.0) || settling || (self.s.through != 0 && self.basin < 1.0)
    }

    fn draw(&mut self, s: &mut Screen) {
        self.tall = s.ph > s.pw;
        let th = self.theme();
        let w = self.plan_wh().0;
        let g = self.s.grid;
        let n = g.n;
        let (bx, by, side, c) = self.board_box();
        let key = wg::hash(0, wg::bytes_of(&[self.s.opt[O_THEME], n, g.tap, g.drain, self.tall as i32]));
        s.background(key, |s| {
            let (x0, y0, x1, y1) = s.visible();
            s.rect_grad(x0, y0, x1 - x0, y1 - y0, 0.0, hex(th.bg), hex(th.bg2), 1.0);
            let pad = 6.0;
            s.shadow(bx - pad, by - pad + 2.0, side + 2.0 * pad, side + 2.0 * pad, 12.0, 8.0, 0, 0.25);
            s.rect(bx - pad, by - pad, side + 2.0 * pad, side + 2.0 * pad, 12.0, hex(th.well), 1.0);
            for y in 0..n {
                for x in 0..n {
                    let g2 = (c * 0.04).max(0.8);
                    s.rect(bx + x as f32 * c + g2, by + y as f32 * c + g2, c - 2.0 * g2, c - 2.0 * g2, c * 0.16, hex(th.tile), 1.0);
                }
            }
            // the tap's pipe and wheel on the left, the drain's on the right
            let (ty, dy) = (by + (g.tap as f32 + 0.5) * c, by + (g.drain as f32 + 0.5) * c);
            let (outer, inner) = (c * 0.19, c * 0.085);
            s.line(bx - 22.0, ty, bx, ty, outer, hex(th.pipe), 1.0);
            s.line(bx - 22.0, ty, bx, ty, inner, hex(th.water), 1.0);
            s.ring(bx - 22.0, ty, 10.0, 3.0, hex(th.pipe), 1.0);
            s.line(bx - 29.0, ty, bx - 15.0, ty, 1.6, hex(th.pipe), 1.0);
            s.line(bx - 22.0, ty - 7.0, bx - 22.0, ty + 7.0, 1.6, hex(th.pipe), 1.0);
            s.disc(bx - 22.0, ty, 3.0, hex(th.pipe_lo), 1.0);
            s.line(bx + side, dy, bx + side + 14.0, dy, outer, hex(th.pipe), 1.0);
            s.line(bx + side, dy, bx + side + 14.0, dy, inner, hex(th.channel), 1.0);
        });

        let dist = g.flow();
        let mark = match self.hover {
            Hit::Cell(x, y) => Some((x, y)),
            _ if self.keys && self.s.phase == PLAY => Some((self.s.cx, self.s.cy)),
            _ => None,
        };
        for y in 0..n {
            for x in 0..n {
                let i = (y * n + x) as usize;
                let (tx, ty) = (bx + x as f32 * c, by + y as f32 * c);
                if mark == Some((x, y)) {
                    let g2 = (c * 0.04).max(0.8);
                    let a = if self.keys { 0.3 } else { 0.14 };
                    s.rect(tx + g2, ty + g2, c - 2.0 * g2, c - 2.0 * g2, c * 0.16, hex(th.text), a);
                }
                let open = g.open(x, y);
                // drawn as it was, turned on toward where it is now
                let ang = -self.spin[i] * std::f32::consts::FRAC_PI_2;
                let entry = self.entry(&dist, x, y);
                draw_pipe(s, th, tx + c / 2.0, ty + c / 2.0, c, open, ang, self.wet[i], if self.spin[i].abs() > 0.05 { None } else { entry });
            }
        }
        // the drain's basin, filling once the water is through
        let dy = by + (g.drain as f32 + 0.5) * c;
        let (qx, qw, qh) = (bx + side + 12.0, 22.0, 30.0);
        s.rect(qx - 2.0, dy - qh / 2.0 - 2.0, qw + 4.0, qh + 4.0, 7.0, hex(th.pipe), 1.0);
        s.rect(qx, dy - qh / 2.0, qw, qh, 6.0, hex(th.channel), 1.0);
        if self.basin > 0.0 {
            let fill = qh * self.basin;
            s.set_clip(qx, dy + qh / 2.0 - fill, qw, fill);
            s.rect(qx, dy - qh / 2.0, qw, qh, 6.0, hex(th.water), 1.0);
            s.no_clip();
            let last = ((g.drain * n) + n - 1) as usize;
            if self.wet[last] >= 1.0 && self.basin < 1.0 {
                s.line(bx + side, dy, qx + qw / 2.0, dy + qh / 2.0 - fill, c * 0.08, hex(th.water), 1.0);
            }
        }

        // the top bar: level and turns
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let turns_l = tr(&["Turns", "旋转", "回転", "회전", "Giros", "Giros", "Rotations", "Drehungen", "Повороты", "旋轉"]);
        let ty = if self.tall { 26.0 } else { 22.0 };
        s.text(&format!("{} {}", level_l, self.s.level + 1), 14.0, ty, 17.0, hex(th.text), 1.0);
        s.text_r(&format!("{} {}", turns_l, self.s.turns), w - 14.0, ty, 15.0, hex(th.dim), 1.0);

        if self.s.phase == CLEARED {
            self.draw_cleared(s, th, w);
        }
    }

    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.phase == DONE
    }
    fn reached(&self) -> i32 {
        self.s.level
    }
    fn cleared(&self) -> Option<(i32, u32)> {
        (self.s.cleared >= 0).then(|| (self.s.cleared, self.s.cleared_score as u32))
    }
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        match i {
            0 => s.done,
            1 => s.all_turns,
            2 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let n = s.grid.n;
        let ok = s.version == VERSION
            && s.opt.iter().zip(Self::CHOICES).all(|(&v, &c)| (0..c).contains(&v))
            && (0..LEVELS).contains(&s.level)
            && (PLAY..=DONE).contains(&s.phase)
            && s.grid.valid()
            && n == puzzle::side(s.level)
            && (0..n).contains(&s.cx)
            && (0..n).contains(&s.cy);
        self.s.through = self.s.grid.through() as i32;
        self.spin = [0.0; MAXN * MAXN];
        // the water stands where it reaches
        let dist = self.s.grid.flow();
        for (w, d) in self.wet.iter_mut().zip(dist.iter()) {
            *w = if *d >= 0 { 1.0 } else { 0.0 };
        }
        self.basin = if self.s.phase != PLAY { 1.0 } else { 0.0 };
        self.hover = Hit::None;
        ok
    }
}

impl Flowline {
    fn draw_cleared(&self, s: &mut Screen, th: &Theme, w: f32) {
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(th.overlay), 0.45);
        let (px, py, pw, ph) = self.card();
        s.shadow(px, py + 4.0, pw, ph, 16.0, 14.0, 0, 0.45);
        s.rect(px - 1.0, py - 1.0, pw + 2.0, ph + 2.0, 17.0, hex(th.text), 0.1);
        s.rect(px, py, pw, ph, 16.0, hex(th.bg), 1.0);
        let title = tr(&["Level cleared", "过关", "クリア", "레벨 클리어", "Nivel superado", "Nível concluído", "Niveau réussi", "Level geschafft", "Уровень пройден", "過關"]);
        s.text_c(title, w / 2.0, py + 34.0, 21.0, hex(th.text), 1.0);
        let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
        let turns_l = tr(&["Turns", "旋转", "回転", "회전", "Giros", "Giros", "Rotations", "Drehungen", "Повороты", "旋轉"]);
        let line = format!("{} +{}    {} {}", score_l, self.s.cleared_score, turns_l, self.s.turns);
        let size = s.fit_size(&line, 14.0, pw - 32.0);
        s.text_c(&line, w / 2.0, py + 64.0, size, hex(th.dim), 1.0);
        let b = self.next_box();
        let label = tr(&["Next level", "下一关", "次のレベル", "다음 레벨", "Siguiente nivel", "Próximo nível", "Niveau suivant", "Nächstes Level", "Следующий уровень", "下一關"]);
        let hv = self.hover == Hit::Next;
        let fill = hex(th.button);
        s.rect(b.0, b.1, b.2, b.3, b.3 / 2.0, if hv { mix(fill, hex(0xffffff), 0.15) } else { fill }, 1.0);
        let size = s.fit_size(label, 16.0, b.2 - 16.0);
        s.text_c(label, b.0 + b.2 / 2.0, b.1 + b.3 / 2.0, size, hex(0xffffff), 1.0);
    }
}

/// The accessible board: the page lays a square over each pipe, for screen
/// readers, keyboards and switches.
impl wg::Board for Flowline {
    fn grid(&self) -> Option<(i32, i32)> {
        Some((self.s.grid.n, self.s.grid.n))
    }
    fn cursor(&self) -> (i32, i32) {
        (self.s.cx, self.s.cy)
    }
    fn focus(&mut self, x: i32, y: i32) {
        if self.s.grid.inside(x, y) {
            self.s.cx = x;
            self.s.cy = y;
            self.keys = true;
            self.hover = Hit::None;
        }
    }
    fn rect(&self) -> (f32, f32, f32) {
        let (bx, by, _, c) = self.board_box();
        (bx, by, c)
    }
    fn cell(&self, x: i32, y: i32, out: &mut String) {
        let g = &self.s.grid;
        if !g.inside(x, y) {
            return;
        }
        const SIDES: [[&str; 9]; 4] = [
            ["up", "上", "上", "위", "arriba", "cima", "haut", "oben", "вверх"],
            ["right", "右", "右", "오른쪽", "derecha", "direita", "droite", "rechts", "вправо"],
            ["down", "下", "下", "아래", "abajo", "baixo", "bas", "unten", "вниз"],
            ["left", "左", "左", "왼쪽", "izquierda", "esquerda", "gauche", "links", "влево"],
        ];
        out.push_str(tr(&["pipe:", "管道通向", "管：", "관:", "tubo hacia", "cano para", "tuyau vers", "Rohr nach", "труба:"]));
        let open = g.open(x, y);
        let mut first = true;
        for (k, side) in SIDES.iter().enumerate() {
            if open & (1 << k) != 0 {
                out.push_str(if first { " " } else { ", " });
                out.push_str(tr(side));
                first = false;
            }
        }
        if g.flow()[(y * g.n + x) as usize] >= 0 {
            out.push_str(", ");
            out.push_str(tr(&["water flows", "有水", "水が流れている", "물이 흐름", "con agua", "com água", "l'eau passe", "Wasser fließt", "с водой"]));
        }
        if x == 0 && y == g.tap {
            out.push_str(", ");
            out.push_str(tr(&["tap on the left", "左边是水龙头", "左に蛇口", "왼쪽에 수도꼭지", "grifo a la izquierda", "torneira à esquerda", "robinet à gauche", "Hahn links", "слева кран", "左邊是水龍頭"]));
        }
        if x == g.n - 1 && y == g.drain {
            out.push_str(", ");
            out.push_str(tr(&["drain on the right", "右边是出水口", "右に排水口", "오른쪽에 배수구", "desagüe a la derecha", "ralo à direita", "évacuation à droite", "Abfluss rechts", "справа сток", "右邊是出水口"]));
        }
    }
    fn status(&self, out: &mut String) {
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let turns_l = tr(&["turns", "旋转", "回転", "회전", "giros", "giros", "rotations", "Drehungen", "повороты", "旋轉"]);
        out.push_str(&format!("{} {}, {} {}", level_l, self.s.level + 1, turns_l, self.s.turns));
    }
    fn keys(&self) -> &'static str {
        tr(&[
            "Arrows move, Space turns a pipe clockwise, X turns it back, I reads the level and turns. Join the tap on the left to the drain on the right.",
            "方向键移动，空格顺时针旋转管道，X 逆时针旋转，I 读出关卡和旋转次数。把左边的水龙头接到右边的出水口。",
            "矢印で移動、スペースで管を右回り、X で左回り、I でレベルと回転数。左の蛇口を右の排水口につなぎます。",
            "화살표로 이동, 스페이스로 관을 시계 방향으로, X로 반대로 회전, I로 레벨과 회전 수. 왼쪽 수도꼭지를 오른쪽 배수구에 이으세요.",
            "Flechas para moverte, Espacio gira el tubo a la derecha, X lo gira de vuelta, I lee nivel y giros. Une el grifo de la izquierda con el desagüe de la derecha.",
            "Setas movem, Espaço gira o cano no sentido horário, X gira de volta, I lê nível e giros. Ligue a torneira à esquerda ao ralo à direita.",
            "Flèches pour bouger, Espace tourne le tuyau dans le sens horaire, X le tourne dans l'autre sens, I lit niveau et rotations. Reliez le robinet à gauche à l'évacuation à droite.",
            "Pfeiltasten bewegen, Leertaste dreht ein Rohr im Uhrzeigersinn, X zurück, I liest Level und Drehungen vor. Verbinde den Hahn links mit dem Abfluss rechts.",
            "Стрелки двигают, пробел поворачивает трубу по часовой, X обратно, I читает уровень и повороты. Соедините кран слева со стоком справа.",
            "方向鍵移動，空白鍵順時針旋轉管道，X 逆時針旋轉，I 讀出關卡和旋轉次數。把左邊的水龍頭接到右邊的出水口。",
        ])
    }
    fn act(&mut self, x: i32, y: i32, a: i32, said: &mut String) -> u32 {
        if self.s.phase == CLEARED {
            let ev = self.next();
            self.status(said);
            return ev;
        }
        self.focus(x, y);
        let ev = self.turn(x, y, a != 0);
        self.cell(x, y, said);
        if ev != 0 && self.s.through != 0 {
            said.push_str(". ");
            said.push_str(tr(&["The water is through! Space for the next level.", "水流通了！按空格进入下一关。", "水が通った！スペースで次のレベル。", "물이 통했습니다! 스페이스로 다음 레벨.", "¡El agua pasa! Espacio para el siguiente nivel.", "A água passou! Espaço para o próximo nível.", "L'eau passe ! Espace pour le niveau suivant.", "Das Wasser fließt durch! Leertaste für das nächste Level.", "Вода прошла! Пробел для следующего уровня.", "水流通了！按空白鍵進入下一關。"]));
        }
        ev
    }
    fn rev(&mut self) -> u32 {
        let v = [self.s.phase, self.s.level, self.s.through, wg::lang() as i32];
        wg::hash(wg::hash(0, wg::bytes_of(&self.s.grid)), wg::bytes_of(&v))
    }
}

wg::game!(Flowline);
wg::board!(Flowline);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Flowline, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    #[test]
    fn plays_every_level() {
        let mut g = Flowline::new();
        g.start(5);
        for level in 0..LEVELS {
            assert_eq!(g.s.level, level);
            let sol = puzzle::solve(&g.s.grid).unwrap();
            let n = g.s.grid.n;
            // a turn back and forth first
            g.turn(0, 0, false);
            g.turn(0, 0, true);
            for i in 0..(n * n) as usize {
                while g.s.grid.rot[i] != sol.rot[i] && g.s.through == 0 {
                    g.turn(i as i32 % n, i as i32 / n, false);
                    assert!(g.loaded_check());
                }
            }
            assert_eq!(g.s.through, 1, "level {} not through", level + 1);
            let ev = run(&mut g, 16000);
            assert!(ev & wg::EV_LEVEL != 0, "level {} not cleared", level + 1);
            if level + 1 < LEVELS {
                assert_eq!(g.s.phase, CLEARED);
                g.next();
            } else {
                assert!(ev & wg::EV_GAMEOVER != 0 && g.over());
            }
        }
        println!("score {}", g.score());
    }

    impl Flowline {
        fn loaded_check(&mut self) -> bool {
            let (w, b) = (self.wet, self.basin);
            let ok = self.loaded();
            self.wet = w;
            self.basin = b;
            ok
        }
    }
}
