//! Unbolt - an original wasmga.me game. MIT licensed.
//! Plates lie in layers on a board, held down by coloured screws. Tap a
//! screw that nothing covers to take it out: it goes into the open box of
//! its colour, or else into one of five spare holes. Full boxes leave and
//! the next comes in; free plates fall away. When the spare holes are full
//! and no screw has anywhere to go, the try is lost. 30 levels, three tries
//! each. The page shows the setup, pause and game over screens.
mod glyphs;
mod puzzle;

use puzzle::{Ev, Level, DONE, IN_BOX, IN_SPARE, LEVELS, MAXS, ON_BOARD, PLATE_R, SCREW_R, SPARE};
use wg::controls::{self, Icon, Row};
use wg::{hex, mix, tr, Game, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 1;

/// options, in the catalog `setup` order
const O_LEVEL: usize = 0;
const O_THEME: usize = 1;
const THEMES: usize = 3;

const PLAY: i32 = 0;
const CLEARED: i32 = 1;
const FAILED: i32 = 2;
const DONE_ALL: i32 = 3;
const LOST: i32 = 4;

const TRIES: i32 = 3;
const FLY_MS: f32 = 330.0;
const BOX_MS: f32 = 300.0;
const FALL_MS: f32 = 750.0;
const SHAKE_MS: f32 = 260.0;
const END_MS: i32 = 900;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 2],
    level: i32,
    phase: i32,
    tries: i32,
    level_ms: i32,
    time_ms: i32,
    /// screws taken out this level and the whole run, levels cleared, tries lost
    taken: i32,
    all_taken: i32,
    done: i32,
    lost: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
    end_ms: i32,
    /// keyboard cursor: a screw
    cur: i32,
    rep: KeyRepeat,
    lv: Level,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}

struct Theme {
    bg: u32,
    bg2: u32,
    board: u32,
    hole: u32,
    text: u32,
    dim: u32,
    overlay: u32,
    button: u32,
    /// plates by layer, from the bottom
    plates: [u32; 6],
    screws: [u32; 7],
}

const THEME: [Theme; THEMES] = [
    // Workshop
    Theme { bg: 0x2b2f3a, bg2: 0x1b1e26, board: 0x3a4050, hole: 0x1c2029, text: 0xf1f3f8, dim: 0x9aa1b3, overlay: 0x0d0f14, button: 0x3c7be0,
        plates: [0xc9a27a, 0xb7c7d8, 0xd9b86a, 0x9fc3b0, 0xd3a9c3, 0xc2c9a0], screws: [0xe5484d, 0xf39a2b, 0xf2d03b, 0x3fbf6a, 0x2fb8d6, 0x4a6ff0, 0xa65fe6] },
    // Paper
    Theme { bg: 0xf4eedf, bg2: 0xe6dcc6, board: 0xe0d4bb, hole: 0xb9aa8c, text: 0x3a2f24, dim: 0x8a7a66, overlay: 0x5a4a38, button: 0x3768d0,
        plates: [0xffffff, 0xd8e6f2, 0xf7e2b0, 0xd6eedd, 0xf2d8e6, 0xe9ecd0], screws: [0xd83b45, 0xe8861a, 0xe0b51c, 0x2f9e57, 0x1f9cbc, 0x3b5fd8, 0x914fd0] },
    // Night
    Theme { bg: 0x161a2c, bg2: 0x0c0e18, board: 0x22283f, hole: 0x0e111c, text: 0xf2f4fb, dim: 0x8f97b8, overlay: 0x05060c, button: 0x4f6ff0,
        plates: [0x5c6896, 0x4f7f8f, 0x7a6597, 0x5f8a6a, 0x8b6a7c, 0x6f7a8c], screws: [0xff5a63, 0xffa53d, 0xffd84d, 0x4fd47c, 0x3fd0ee, 0x6b8cff, 0xbd7cff] },
];

#[derive(Clone, Copy)]
enum Anim {
    /// a screw on its way: drawn at `from` until its delay is up
    Fly { screw: usize, from: (f32, f32), to: (f32, f32), delay: f32, t: f32 },
    /// a full box leaving with its three screws
    BoxOut { place: usize, colour: u8, held: [u8; 3], delay: f32, t: f32 },
    BoxIn { place: usize, delay: f32, t: f32 },
    Fall { plate: usize, delay: f32, t: f32 },
}

impl Anim {
    fn delay(&mut self) -> &mut f32 {
        match self {
            Anim::Fly { delay, .. } | Anim::BoxOut { delay, .. } | Anim::BoxIn { delay, .. } | Anim::Fall { delay, .. } => delay,
        }
    }
    fn t(&mut self) -> &mut f32 {
        match self {
            Anim::Fly { t, .. } | Anim::BoxOut { t, .. } | Anim::BoxIn { t, .. } | Anim::Fall { t, .. } => t,
        }
    }
    fn ms(&self) -> f32 {
        match self {
            Anim::Fly { .. } => FLY_MS,
            Anim::BoxOut { .. } | Anim::BoxIn { .. } => BOX_MS,
            Anim::Fall { .. } => FALL_MS,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    None,
    Screw(usize),
    Button,
    Ctl(Icon),
}

struct Unbolt {
    s: Save,
    tall: bool,
    keys: bool,
    hover: Hit,
    anims: Vec<Anim>,
    /// a covered screw that was tapped wiggles, ms left
    shake: (usize, f32),
}

impl Unbolt {
    fn theme(&self) -> &'static Theme {
        &THEME[self.s.opt[O_THEME] as usize]
    }
    fn plan_wh(&self) -> (f32, f32) {
        if self.tall { (300.0, 400.0) } else { (400.0, 300.0) }
    }
    /// the board: its left, top and side
    fn board(&self) -> (f32, f32, f32) {
        if self.tall { (24.0, 142.0, 252.0) } else { (14.0, 42.0, 250.0) }
    }
    fn to_px(&self, x: i16, y: i16) -> (f32, f32) {
        let (bx, by, side) = self.board();
        (bx + x as f32 / 1000.0 * side, by + y as f32 / 1000.0 * side)
    }
    fn unit(&self) -> f32 {
        self.board().2 / 1000.0
    }
    /// box place a: its box
    fn box_rect(&self, a: usize) -> (f32, f32, f32, f32) {
        if self.tall { (150.0 - 118.0 + a as f32 * 124.0, 44.0, 112.0, 42.0) } else { (288.0, 46.0 + a as f32 * 56.0, 100.0, 42.0) }
    }
    fn hole_r(&self) -> f32 {
        SCREW_R as f32 * self.unit() * 1.08
    }
    fn box_hole(&self, a: usize, k: usize) -> (f32, f32) {
        let (x, y, w, h) = self.box_rect(a);
        (x + w / 2.0 + (k as f32 - 1.0) * w * 0.29, y + h / 2.0)
    }
    fn spare_hole(&self, k: usize) -> (f32, f32) {
        let gap = self.hole_r() * 2.6;
        if self.tall {
            (150.0 + (k as f32 - 2.0) * gap, 112.0)
        } else {
            (338.0 + (k as f32 - 2.0) * gap * 0.92, 182.0)
        }
    }
    /// the card over the board, in the middle of the screen
    fn card(&self) -> (f32, f32, f32, f32) {
        let (w, h) = self.plan_wh();
        let (pw, ph) = (248.0, 158.0);
        (w / 2.0 - pw / 2.0, h / 2.0 - ph / 2.0, pw, ph)
    }
    /// its button, inside the card along the bottom
    fn button_box(&self) -> (f32, f32, f32, f32) {
        let (px, py, pw, ph) = self.card();
        let (bw, bh) = (pw - 48.0, 42.0);
        (px + (pw - bw) / 2.0, py + ph - bh - 20.0, bw, bh)
    }
    /// where a screw sits now, by its state (a full box's screws stay in
    /// their holes while it leaves)
    fn screw_xy(&self, i: usize) -> (f32, f32) {
        let s = &self.s.lv.screw[i];
        match s.state {
            IN_BOX | DONE => self.box_hole(s.slot as usize / 3, s.slot as usize % 3),
            IN_SPARE => self.spare_hole(s.slot as usize),
            _ => self.to_px(s.x, s.y),
        }
    }
    fn flying(&self, i: usize) -> bool {
        self.anims.iter().any(|a| matches!(a, Anim::Fly { screw, .. } if *screw == i))
    }
    /// Restart, in the top bar while a level is on
    fn controls(&self) -> Row {
        let row = controls::row(&[Icon::Restart], self.plan_wh().0 - 8.0, 22.0);
        if self.s.phase == PLAY { row } else { row.hidden() }
    }
    /// Restart costs a try, as getting stuck does, so not on the last one.
    fn can_restart(&self) -> bool {
        self.s.phase == PLAY && self.s.taken > 0 && self.s.tries > 1 && self.anims.is_empty()
    }
    fn lose_try(&mut self) {
        self.s.tries -= 1;
        self.s.lost += 1;
    }
    fn restart(&mut self) -> u32 {
        if !self.can_restart() {
            return 0;
        }
        self.lose_try();
        self.begin(self.s.level);
        wg::EV_BLIP
    }

    fn hit(&self, x: f32, y: f32) -> Hit {
        if let Some(i) = self.controls().hit(x, y) {
            return Hit::Ctl(i);
        }
        if self.s.phase == CLEARED || self.s.phase == FAILED {
            let b = self.button_box();
            let on = x >= b.0 && x < b.0 + b.2 && y >= b.1 && y < b.1 + b.3;
            return if on && self.anims.is_empty() { Hit::Button } else { Hit::None };
        }
        if self.s.phase != PLAY {
            return Hit::None;
        }
        // the topmost screw under the point: screws on higher plates first
        let r = SCREW_R as f32 * self.unit() * 1.3;
        let lv = &self.s.lv;
        (0..lv.screws as usize)
            .filter(|&i| lv.screw[i].state == ON_BOARD)
            .filter(|&i| {
                let (sx, sy) = self.screw_xy(i);
                (sx - x).hypot(sy - y) < r
            })
            .max_by_key(|&i| (!lv.blocked(i) as i32, lv.plate[lv.screw[i].plate as usize].layer))
            .map_or(Hit::None, Hit::Screw)
    }

    fn begin(&mut self, level: i32) {
        let s = &mut self.s;
        s.level = level;
        s.phase = PLAY;
        s.lv = puzzle::deal(level);
        s.level_ms = 0;
        s.taken = 0;
        s.end_ms = 0;
        s.cur = 0;
        self.anims.clear();
        self.shake = (0, 0.0);
    }

    /// how long until box place a has its new box in place, if one is coming
    fn box_wait(&self, a: usize) -> f32 {
        self.anims
            .iter()
            .filter_map(|an| match *an {
                Anim::BoxIn { place, delay, t } if place == a => Some(delay + (1.0 - t) * BOX_MS),
                _ => None,
            })
            .fold(0.0, f32::max)
    }

    fn tap(&mut self, i: usize) -> u32 {
        if self.s.phase != PLAY || i >= self.s.lv.screws as usize || self.s.lv.screw[i].state != ON_BOARD {
            return 0;
        }
        // where each screw was, for the ones that move
        let before: Vec<(f32, f32)> = (0..self.s.lv.screws as usize).map(|j| self.screw_xy(j)).collect();
        let Some(evs) = self.s.lv.tap(i) else {
            self.shake = (i, SHAKE_MS);
            return 0;
        };
        self.s.taken += 1;
        self.s.all_taken += 1;
        let mut t0 = 0.0f32;
        let mut last_land = 0.0f32;
        let mut ev = wg::EV_BLIP;
        for e in evs {
            match e {
                Ev::Moved(j) => {
                    let s = self.s.lv.screw[j];
                    let mut delay = t0;
                    if s.state != IN_SPARE {
                        delay = delay.max(self.box_wait(s.slot as usize / 3));
                    }
                    self.anims.retain(|a| !matches!(a, Anim::Fly { screw, .. } if *screw == j));
                    self.anims.push(Anim::Fly { screw: j, from: before[j], to: self.screw_xy(j), delay, t: 0.0 });
                    last_land = last_land.max(delay + FLY_MS);
                }
                Ev::BoxDone(a, colour, held) => {
                    let start = last_land.max(self.box_wait(a)) + 60.0;
                    self.anims.push(Anim::BoxOut { place: a, colour, held, delay: start, t: 0.0 });
                    t0 = start + BOX_MS * 0.6;
                    ev |= wg::EV_HIT;
                }
                Ev::BoxIn(a) => {
                    self.anims.push(Anim::BoxIn { place: a, delay: t0, t: 0.0 });
                    t0 += BOX_MS;
                }
                Ev::PlateGone(p) => {
                    self.anims.push(Anim::Fall { plate: p, delay: FLY_MS * 0.35, t: 0.0 });
                }
            }
        }
        if self.s.lv.cleared() {
            return ev | self.clear_level();
        }
        if self.s.lv.stuck() {
            self.lose_try();
            if self.s.tries <= 0 {
                self.s.phase = LOST;
                self.s.end_ms = END_MS;
                return ev | wg::EV_LOSE;
            }
            self.s.phase = FAILED;
            return ev | wg::EV_LOSE;
        }
        ev
    }

    fn clear_level(&mut self) -> u32 {
        let s = &mut self.s;
        let score = 10 * s.lv.screws + 50 * s.tries;
        s.score += score;
        s.cleared = s.level;
        s.cleared_score = score;
        s.done += 1;
        s.phase = if s.level + 1 >= LEVELS { DONE_ALL } else { CLEARED };
        if s.phase == DONE_ALL {
            s.end_ms = END_MS;
        }
        wg::EV_LEVEL | wg::EV_WIN
    }

    fn button(&mut self) -> u32 {
        match self.s.phase {
            CLEARED => {
                self.begin(self.s.level + 1);
                self.s.tries = TRIES;
                wg::EV_BLIP
            }
            FAILED => {
                self.begin(self.s.level);
                wg::EV_BLIP
            }
            _ => 0,
        }
    }

    /// arrow keys: the nearest screw on the board that way
    fn step_cursor(&mut self, dx: f32, dy: f32) {
        let lv = &self.s.lv;
        let on: Vec<usize> = (0..lv.screws as usize).filter(|&i| lv.screw[i].state == ON_BOARD).collect();
        if on.is_empty() {
            return;
        }
        let cur = self.s.cur as usize;
        if !on.contains(&cur) {
            self.s.cur = on[0] as i32;
            return;
        }
        let (x0, y0) = self.screw_xy(cur);
        let best = on
            .iter()
            .filter_map(|&i| {
                let (x, y) = self.screw_xy(i);
                let (ex, ey) = (x - x0, y - y0);
                let along = ex * dx + ey * dy;
                let side = (ex * dy - ey * dx).abs();
                (along > 1.0 && side < along * 1.5).then_some((i, along + side * 1.5))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = best {
            self.s.cur = i as i32;
        }
    }
}

fn draw_screw(s: &mut Screen, x: f32, y: f32, r: f32, c: u32, turn: f32, alpha: f32) {
    s.disc(x, y + r * 0.12, r, 0, 0.3 * alpha);
    s.disc(x, y, r, mix(hex(0x000000), c, 0.62), alpha);
    s.disc(x, y - r * 0.04, r * 0.84, c, alpha);
    s.disc(x - r * 0.28, y - r * 0.32, r * 0.3, hex(0xffffff), 0.3 * alpha);
    let (ca, sa) = (turn.cos() * r * 0.5, turn.sin() * r * 0.5);
    let ink = mix(hex(0x000000), c, 0.45);
    s.line(x - ca, y - sa, x + ca, y + sa, r * 0.13, ink, alpha);
    s.line(x + sa, y - ca, x - sa, y + ca, r * 0.13, ink, alpha);
}

impl Game for Unbolt {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS, THEMES as i32];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        Unbolt {
            s: Save {
                version: VERSION,
                opt: [0, 0],
                level: 0,
                phase: PLAY,
                tries: TRIES,
                level_ms: 0,
                time_ms: 0,
                taken: 0,
                all_taken: 0,
                done: 0,
                lost: 0,
                score: 0,
                cleared: -1,
                cleared_score: 0,
                end_ms: 0,
                cur: 0,
                rep: KeyRepeat(0),
                lv: Level::empty(),
            },
            tall: false,
            keys: false,
            hover: Hit::None,
            anims: Vec::new(),
            shake: (0, 0.0),
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
        self.s = Save { opt, ..Unbolt::new().s };
        self.begin(opt[O_LEVEL]);
        self.keys = false;
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        if h as f32 > w as f32 * 1.1 { (300.0, 400.0) } else { (400.0, 300.0) }
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        for a in self.anims.iter_mut() {
            // the delay first, then the rest of the frame plays
            let d = a.delay();
            let wait = d.min(dt);
            *d -= wait;
            let ms = a.ms();
            *a.t() += (dt - wait) / ms;
        }
        self.anims.retain_mut(|a| *a.t() < 1.0);
        if self.shake.1 > 0.0 {
            self.shake.1 = (self.shake.1 - dt).max(0.0);
        }
        if self.s.phase == PLAY && self.s.taken > 0 {
            self.s.level_ms = self.s.level_ms.saturating_add(input.dt);
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        if self.s.end_ms > 0 && self.anims.is_empty() {
            self.s.end_ms -= input.dt;
            if self.s.end_ms <= 0 {
                self.s.end_ms = 0;
                ev |= wg::EV_GAMEOVER;
            }
        }
        let dirs = self.s.rep.dirs(input);
        if dirs != 0 {
            self.keys = true;
            self.hover = Hit::None;
        }
        if self.s.phase == PLAY {
            if dirs & wg::LEFT != 0 { self.step_cursor(-1.0, 0.0); }
            if dirs & wg::RIGHT != 0 { self.step_cursor(1.0, 0.0); }
            if dirs & wg::UP != 0 { self.step_cursor(0.0, -1.0); }
            if dirs & wg::DOWN != 0 { self.step_cursor(0.0, 1.0); }
            if input.pressed & wg::A != 0 {
                self.keys = true;
                let lv = &self.s.lv;
                if lv.screw[self.s.cur as usize].state != ON_BOARD {
                    self.step_cursor(1.0, 0.0);
                }
                ev |= self.tap(self.s.cur as usize);
                // on to a screw still on the board
                let lv = &self.s.lv;
                if self.s.phase == PLAY && lv.screw[self.s.cur as usize].state != ON_BOARD {
                    let (x0, y0) = self.to_px(lv.screw[self.s.cur as usize].x, lv.screw[self.s.cur as usize].y);
                    let near = (0..lv.screws as usize)
                        .filter(|&i| lv.screw[i].state == ON_BOARD)
                        .min_by(|&a, &b| {
                            let d = |i: usize| {
                                let (x, y) = self.to_px(lv.screw[i].x, lv.screw[i].y);
                                (x - x0).hypot(y - y0) - if lv.blocked(i) { 0.0 } else { 1000.0 }
                            };
                            d(a).total_cmp(&d(b))
                        });
                    if let Some(i) = near {
                        self.s.cur = i as i32;
                    }
                }
            }
        } else if input.pressed & wg::A != 0 && self.anims.is_empty() {
            ev |= self.button();
        }
        ev
    }

    fn key(&mut self, code: i32) -> bool {
        controls::is_restart_key(code) && self.restart() != 0
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.hover = Hit::None;
            return 0;
        }
        let h = self.hit(p.x, p.y);
        self.hover = if p.touch { Hit::None } else { h };
        if p.pressed & wg::M_LEFT != 0 {
            self.keys = false;
            return match h {
                Hit::Screw(i) => {
                    self.s.cur = i as i32;
                    self.tap(i)
                }
                Hit::Button => self.button(),
                Hit::Ctl(_) => self.restart(),
                Hit::None => 0,
            };
        }
        0
    }

    fn picture(&mut self) -> u32 {
        let hv = match self.hover {
            Hit::None => -1,
            Hit::Screw(i) => i as i32,
            Hit::Button => 1000,
            Hit::Ctl(i) => 1001 + i as i32,
        };
        let shown = [hv, self.keys as i32, self.tall as i32, self.anims.is_empty() as i32];
        let mut c = self.s;
        c.level_ms = 0;
        c.time_ms = 0;
        c.end_ms = 0;
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        !self.anims.is_empty() || self.shake.1 > 0.0
    }

    fn draw(&mut self, s: &mut Screen) {
        self.tall = s.ph > s.pw;
        let th = self.theme();
        let w = self.plan_wh().0;
        let (bx, by, side) = self.board();
        let u = self.unit();
        let (pr, sr) = (PLATE_R as f32 * u, SCREW_R as f32 * u);
        let hr = self.hole_r();
        let lv = self.s.lv;
        let key = wg::hash(0, wg::bytes_of(&[self.s.opt[O_THEME], self.tall as i32]));
        let spares: Vec<(f32, f32)> = (0..SPARE).map(|k| self.spare_hole(k)).collect();
        s.background(key, |s| {
            let (x0, y0, x1, y1) = s.visible();
            s.rect_grad(x0, y0, x1 - x0, y1 - y0, 0.0, hex(th.bg), hex(th.bg2), 1.0);
            let pad = 8.0;
            s.shadow(bx - pad, by - pad + 3.0, side + 2.0 * pad, side + 2.0 * pad, 16.0, 10.0, 0, 0.3);
            s.rect(bx - pad, by - pad, side + 2.0 * pad, side + 2.0 * pad, 16.0, hex(th.board), 1.0);
            for &(x, y) in &spares {
                s.disc(x, y, hr + 2.0, mix(hex(th.board), hex(th.hole), 0.4), 1.0);
                s.disc(x, y, hr, hex(th.hole), 1.0);
            }
        });

        // the boxes: leaving, arriving, open
        let wait = |a: usize| {
            self.anims.iter().find_map(|an| match *an {
                Anim::BoxIn { place, delay, t } if place == a => Some((delay, t)),
                _ => None,
            })
        };
        let draw_box = |s: &mut Screen, a: usize, colour: u8, dx: f32, dy: f32, alpha: f32| {
            let (x, y, bw, bh) = self.box_rect(a);
            let c = hex(th.screws[colour as usize - 1]);
            s.shadow(x + dx, y + dy + 3.0, bw, bh, 10.0, 6.0, 0, 0.3 * alpha);
            s.rect(x + dx, y + dy, bw, bh, 10.0, mix(hex(0x000000), c, 0.7), alpha);
            s.rect(x + dx, y + dy, bw, bh - 3.0, 10.0, c, alpha);
            for k in 0..3 {
                let (hx, hy) = self.box_hole(a, k);
                s.disc(hx + dx, hy + dy, hr, mix(hex(0x000000), c, 0.5), alpha);
            }
        };
        for a in 0..2 {
            let open = lv.open[a];
            if open < 0 {
                continue;
            }
            let (dx, alpha) = match wait(a) {
                Some((d, _)) if d > 0.0 => continue,
                Some((_, t)) => ((1.0 - wg::ease_out(t)) * 140.0, t.min(1.0)),
                None => (0.0, 1.0),
            };
            draw_box(s, a, lv.colour[open as usize], dx, 0.0, alpha);
            for j in 0..lv.screws as usize {
                let sc = &lv.screw[j];
                if sc.state == IN_BOX && sc.slot as usize / 3 == a && !self.flying(j) {
                    let (x, y) = self.box_hole(a, sc.slot as usize % 3);
                    draw_screw(s, x + dx, y, sr, hex(th.screws[sc.colour as usize - 1]), 0.0, alpha);
                }
            }
        }
        for an in &self.anims {
            if let Anim::BoxOut { place, colour, held, t, .. } = *an {
                let e = t * t;
                let dy = -e * 60.0;
                let alpha = 1.0 - e;
                draw_box(s, place, colour, 0.0, dy, alpha);
                for (k, &j) in held.iter().enumerate() {
                    if !self.flying(j as usize) {
                        let (x, y) = self.box_hole(place, k);
                        draw_screw(s, x, y + dy, sr, hex(th.screws[colour as usize - 1]), 0.0, alpha);
                    }
                }
            }
        }
        // screws waiting in the spare holes
        for k in 0..SPARE {
            let j = lv.spare[k] as usize;
            if j > 0 && !self.flying(j - 1) {
                let (x, y) = spares[k];
                draw_screw(s, x, y, sr, hex(th.screws[lv.screw[j - 1].colour as usize - 1]), 0.0, 1.0);
            }
        }

        // the plates, bottom layer first, each with its screws
        let mark = match self.hover {
            Hit::Screw(i) => Some(i),
            _ if self.keys && self.s.phase == PLAY => Some(self.s.cur as usize),
            _ => None,
        };
        let mut order: Vec<usize> = (0..lv.plates as usize).collect();
        order.sort_by_key(|&p| (lv.plate[p].layer, p));
        for &p in &order {
            let pl = &lv.plate[p];
            if pl.gone != 0 {
                continue;
            }
            let (ax, ay) = self.to_px(pl.ax, pl.ay);
            let (qx, qy) = self.to_px(pl.bx, pl.by);
            let c = hex(th.plates[pl.layer as usize % 6]);
            s.line(ax, ay + 3.0, qx, qy + 3.0, pr, 0, 0.22);
            s.line(ax, ay, qx, qy, pr, mix(hex(0x000000), c, 0.72), 0.94);
            s.line(ax, ay - 1.0, qx, qy - 1.0, pr - 1.6, c, 0.84);
            for j in 0..lv.screws as usize {
                let sc = &lv.screw[j];
                if sc.plate as usize != p {
                    continue;
                }
                let (mut x, y) = self.to_px(sc.x, sc.y);
                s.disc(x, y, hr, mix(hex(0x000000), c, 0.55), 0.9);
                if sc.state != ON_BOARD || self.flying(j) {
                    continue;
                }
                if self.shake.1 > 0.0 && self.shake.0 == j {
                    x += (self.shake.1 * 0.09).sin() * 2.5 * self.shake.1 / SHAKE_MS;
                }
                if mark == Some(j) {
                    s.ring(x, y, sr + 3.5, 2.0, hex(th.text), if self.keys { 0.9 } else { 0.6 });
                }
                draw_screw(s, x, y, sr, hex(th.screws[sc.colour as usize - 1]), 0.6, 1.0);
            }
        }
        // free plates dropping away
        for an in &self.anims {
            if let Anim::Fall { plate, t, .. } = *an {
                let pl = &lv.plate[plate];
                let (ax, ay) = self.to_px(pl.ax, pl.ay);
                let (qx, qy) = self.to_px(pl.bx, pl.by);
                let (mx, my) = ((ax + qx) / 2.0, (ay + qy) / 2.0);
                let spin = t * 0.9 * if plate % 2 == 0 { 1.0 } else { -1.0 };
                let (cs, sn) = (spin.cos(), spin.sin());
                let rot = |x: f32, y: f32| (mx + (x - mx) * cs - (y - my) * sn, my + (x - mx) * sn + (y - my) * cs + t * t * 260.0);
                let (a2, b2) = (rot(ax, ay), rot(qx, qy));
                let c = hex(th.plates[pl.layer as usize % 6]);
                let alpha = (1.0 - t * t) * 0.95;
                s.line(a2.0, a2.1, b2.0, b2.1, pr * (1.0 + t * 0.15), mix(hex(0x000000), c, 0.72), alpha);
                s.line(a2.0, a2.1 - 1.0, b2.0, b2.1 - 1.0, pr * (1.0 + t * 0.15) - 1.6, c, alpha);
            }
        }
        // screws in the air: they turn out of their hole, then fly
        for an in &self.anims {
            if let Anim::Fly { screw, from, to, delay, t } = *an {
                let c = hex(th.screws[lv.screw[screw].colour as usize - 1]);
                if delay > 0.0 {
                    draw_screw(s, from.0, from.1, sr, c, 0.6, 1.0);
                    continue;
                }
                let lift = (t / 0.3).min(1.0);
                let f = ((t - 0.3) / 0.7).clamp(0.0, 1.0);
                let e = f * f * (3.0 - 2.0 * f);
                let x = from.0 + (to.0 - from.0) * e;
                let y = from.1 + (to.1 - from.1) * e - (std::f32::consts::PI * f).sin() * 30.0 - lift * (1.0 - f) * 6.0;
                let grow = 1.0 + 0.25 * lift * (1.0 - f);
                draw_screw(s, x, y, sr * grow, c, 0.6 + lift * 6.0 + f * 3.0, 1.0);
            }
        }

        // the top bar: level and tries
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let ty = 22.0;
        s.text(&format!("{} {}", level_l, self.s.level + 1), 14.0, ty, 17.0, hex(th.text), 1.0);
        let hover = if let Hit::Ctl(i) = self.hover { Some(i) } else { None };
        let right = self.controls().draw(s, hover, |_| self.can_restart(), w);
        let right = if self.tall { right - 20.0 } else { 268.0 };
        for i in 0..TRIES {
            let x = right - (TRIES - 1 - i) as f32 * 16.0;
            if i < self.s.tries {
                s.disc(x, ty, 5.5, hex(0xe5484d), 1.0);
            } else {
                s.ring(x, ty, 5.0, 1.4, hex(th.dim), 0.8);
            }
        }

        if self.anims.is_empty() && (self.s.phase == CLEARED || self.s.phase == FAILED) {
            self.draw_card(s, th, w);
        }
    }

    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.phase == DONE_ALL || self.s.phase == LOST
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
            1 => s.all_taken,
            2 => s.lost,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION
            && s.opt.iter().zip(Self::CHOICES).all(|(&v, &c)| (0..c).contains(&v))
            && (0..LEVELS).contains(&s.level)
            && (PLAY..=LOST).contains(&s.phase)
            && s.lv.valid()
            && (0..=TRIES).contains(&s.tries)
            && (0..s.lv.screws).contains(&s.cur)
            && s.end_ms >= 0;
        self.anims.clear();
        self.shake = (0, 0.0);
        self.hover = Hit::None;
        ok
    }
}

impl Unbolt {
    fn draw_card(&self, s: &mut Screen, th: &Theme, w: f32) {
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(th.overlay), 0.45);
        let (px, py, pw, ph) = self.card();
        s.shadow(px, py + 4.0, pw, ph, 16.0, 14.0, 0, 0.45);
        s.rect(px - 1.0, py - 1.0, pw + 2.0, ph + 2.0, 17.0, hex(th.text), 0.1);
        s.rect(px, py, pw, ph, 16.0, hex(th.bg), 1.0);
        let won = self.s.phase == CLEARED;
        let title = if won {
            tr(&["Level cleared", "过关", "クリア", "레벨 클리어", "Nivel superado", "Nível concluído", "Niveau réussi", "Level geschafft", "Уровень пройден", "過關"])
        } else {
            tr(&["No room left", "没有空位了", "置く場所がありません", "빈칸이 없습니다", "No queda sitio", "Não há mais espaço", "Plus de place", "Kein Platz mehr", "Места больше нет", "沒有空位了"])
        };
        s.text_c(title, w / 2.0, py + 34.0, 21.0, hex(th.text), 1.0);
        let line = if won {
            let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
            format!("{} +{}", score_l, self.s.cleared_score)
        } else {
            let left_l = tr(&["Tries left", "剩余次数", "残りの挑戦", "남은 기회", "Intentos", "Tentativas", "Essais restants", "Versuche übrig", "Осталось попыток", "剩餘次數"]);
            format!("{} {}", left_l, self.s.tries)
        };
        let size = s.fit_size(&line, 14.0, pw - 32.0);
        s.text_c(&line, w / 2.0, py + 64.0, size, hex(th.dim), 1.0);
        let b = self.button_box();
        let label = if won {
            tr(&["Next level", "下一关", "次のレベル", "다음 레벨", "Siguiente nivel", "Próximo nível", "Niveau suivant", "Nächstes Level", "Следующий уровень", "下一關"])
        } else {
            tr(&["Try again", "再试一次", "もう一度", "다시 하기", "Reintentar", "Tentar de novo", "Réessayer", "Nochmal", "Ещё раз", "再試一次"])
        };
        let hv = self.hover == Hit::Button;
        let fill = hex(th.button);
        s.rect(b.0, b.1, b.2, b.3, b.3 / 2.0, if hv { mix(fill, hex(0xffffff), 0.15) } else { fill }, 1.0);
        let size = s.fit_size(label, 16.0, b.2 - 16.0);
        s.text_c(label, b.0 + b.2 / 2.0, b.1 + b.3 / 2.0, size, hex(0xffffff), 1.0);
    }
}

/// the screws' names, in theme `screws` order
const NAMES: [[&str; 10]; 7] = [
    ["red", "红", "赤", "빨강", "rojo", "vermelho", "rouge", "Rot", "красный", "紅"],
    ["orange", "橙", "オレンジ", "주황", "naranja", "laranja", "orange", "Orange", "оранжевый", "橙"],
    ["yellow", "黄", "黄", "노랑", "amarillo", "amarelo", "jaune", "Gelb", "жёлтый", "黃"],
    ["green", "绿", "緑", "초록", "verde", "verde", "vert", "Grün", "зелёный", "綠"],
    ["cyan", "青", "水色", "하늘색", "cian", "ciano", "cyan", "Cyan", "голубой", "青"],
    ["blue", "蓝", "青", "파랑", "azul", "azul", "bleu", "Blau", "синий", "藍"],
    ["purple", "紫", "紫", "보라", "morado", "roxo", "violet", "Lila", "фиолетовый", "紫"],
];

fn colour_name(c: u8) -> &'static str {
    tr(&NAMES[(c as usize).clamp(1, 7) - 1])
}

impl Unbolt {
    /// the accessible board's cursor goes on to the nearest screw still on
    /// the board, one that can come out first
    fn cursor_on(&mut self) {
        let lv = &self.s.lv;
        if lv.screw[self.s.cur as usize].state == ON_BOARD {
            return;
        }
        let (x0, y0) = self.to_px(lv.screw[self.s.cur as usize].x, lv.screw[self.s.cur as usize].y);
        let d = |i: usize| {
            let (x, y) = self.to_px(lv.screw[i].x, lv.screw[i].y);
            (x - x0).hypot(y - y0) - if lv.blocked(i) { 0.0 } else { 1000.0 }
        };
        if let Some(i) = (0..lv.screws as usize).filter(|&i| lv.screw[i].state == ON_BOARD).min_by(|&a, &b| d(a).total_cmp(&d(b))) {
            self.s.cur = i as i32;
        }
    }
}

impl Unbolt {
    /// screw i in words: its colour and where it is
    fn screw_words(&self, i: usize, out: &mut String) {
        let lv = &self.s.lv;
        let sc = &lv.screw[i];
        out.push_str(&tr(&["# screw", "# 螺丝", "# のねじ", "# 나사", "tornillo #", "parafuso #", "vis #", "# Schraube", "винт: #", "# 螺絲"]).replace('#', colour_name(sc.colour)));
        out.push_str(", ");
        out.push_str(match sc.state {
            IN_BOX => tr(&["in its box", "在盒子里", "箱の中", "상자 안", "en su caja", "na caixa", "dans sa boîte", "in seiner Box", "в коробке", "在盒子裡"]),
            IN_SPARE => tr(&["in a spare slot", "在备用孔里", "予備の穴", "예비 구멍 안", "en un hueco de reserva", "num furo reserva", "dans un trou de réserve", "in einem Ersatzloch", "в запасном гнезде", "在備用孔裡"]),
            DONE => tr(&["cleared", "已收走", "片付いた", "정리됨", "guardado", "guardado", "rangée", "weggeräumt", "убран", "已收走"]),
            _ if lv.blocked(i) => tr(&["covered by a plate", "被板挡住", "板の下", "판에 가려짐", "tapado por una placa", "coberto por uma placa", "sous une plaque", "unter einer Platte", "под пластиной", "被板擋住"]),
            _ => tr(&["free", "可以拧下", "外せる", "뺄 수 있음", "libre", "livre", "libre", "frei", "свободен", "可以擰下"]),
        });
    }
}

/// The accessible board: a square over each screw on the board; arrows go
/// to the nearest one.
impl wg::Board for Unbolt {
    fn grid(&self) -> Option<(i32, i32)> {
        Some((self.s.lv.screws.max(1), 1))
    }
    fn cursor(&self) -> (i32, i32) {
        (self.s.cur, 0)
    }
    fn focus(&mut self, x: i32, _y: i32) {
        if (0..self.s.lv.screws).contains(&x) {
            self.s.cur = x;
            self.keys = true;
            self.hover = Hit::None;
        }
    }
    fn rect(&self) -> (f32, f32, f32) {
        (0.0, 0.0, 1.0)
    }
    fn cell_box(&self, x: i32, _y: i32) -> Option<(f32, f32, f32, f32)> {
        let i = x.clamp(0, MAXS as i32 - 1) as usize;
        // under the level's card the focused square is its button
        if (self.s.phase == CLEARED || self.s.phase == FAILED) && x == self.s.cur {
            return Some(self.button_box());
        }
        // arrows go only to screws still on the board; I reads the rest
        if self.s.lv.screw[i].state != ON_BOARD || x >= self.s.lv.screws {
            return Some((0.0, 0.0, 0.0, 0.0));
        }
        let (sx, sy) = self.screw_xy(i);
        let r = SCREW_R as f32 * self.unit() * 1.3;
        Some((sx - r, sy - r, r * 2.0, r * 2.0))
    }
    fn cell(&self, x: i32, _y: i32, out: &mut String) {
        if !(0..self.s.lv.screws).contains(&x) {
            return;
        }
        if (self.s.phase == CLEARED || self.s.phase == FAILED) && x == self.s.cur {
            out.push_str(if self.s.phase == CLEARED {
                tr(&["Next level", "下一关", "次のレベル", "다음 레벨", "Siguiente nivel", "Próximo nível", "Niveau suivant", "Nächstes Level", "Следующий уровень", "下一關"])
            } else {
                tr(&["Try again", "再试一次", "もう一度", "다시 하기", "Reintentar", "Tentar de novo", "Réessayer", "Nochmal", "Ещё раз", "再試一次"])
            });
            return;
        }
        self.screw_words(x as usize, out);
    }
    fn status(&self, out: &mut String) {
        let lv = &self.s.lv;
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let tries_l = tr(&["tries left", "剩余次数", "残りの挑戦", "남은 기회", "intentos", "tentativas", "essais restants", "Versuche übrig", "осталось попыток", "剩餘次數"]);
        out.push_str(&format!("{} {}, {} {}. ", level_l, self.s.level + 1, tries_l, self.s.tries));
        out.push_str(tr(&["Boxes: ", "盒子：", "箱：", "상자: ", "Cajas: ", "Caixas: ", "Boîtes : ", "Boxen: ", "Коробки: ", "盒子："]));
        let mut first = true;
        for a in 0..2 {
            if lv.open[a] >= 0 {
                if !first {
                    out.push_str(", ");
                }
                first = false;
                out.push_str(&format!("{} {}/3", colour_name(lv.colour[lv.open[a] as usize]), lv.fill[a]));
            }
        }
        out.push_str(". ");
        out.push_str(&tr(&["Spare slots used: # of 5", "备用孔已用 # / 5", "予備の穴 # / 5", "예비 구멍 # / 5 사용", "Huecos de reserva: # de 5", "Furos reserva: # de 5", "Trous de réserve : # sur 5", "Ersatzlöcher: # von 5", "Запасные гнёзда: # из 5", "備用孔已用 # / 5"]).replace('#', &lv.spare_used().to_string()));
        for &j in lv.spare[..SPARE].iter().filter(|&&j| j != 0) {
            out.push_str(", ");
            out.push_str(colour_name(lv.screw[j as usize - 1].colour));
        }
    }
    fn keys(&self) -> &'static str {
        tr(&[
            "Arrow keys move between screws, Space removes an exposed screw, I announces the level, remaining tries, open toolboxes, and spare slots. R restarts the level.",
            "方向键在螺丝间移动，空格拧下可以拧下的螺丝，I 读出关卡、剩余次数、盒子和备用孔。R 重新开始本关。",
            "矢印でねじを移動、スペースで外せるねじを外す、I でレベル、挑戦回数、箱と予備の穴。R でレベルをやり直し。",
            "화살표로 나사 사이 이동, 스페이스로 뺄 수 있는 나사 빼기, I로 레벨, 기회, 상자와 예비 구멍. R로 레벨 다시 시작.",
            "Flechas para moverte entre tornillos, Espacio saca un tornillo libre, I lee nivel, intentos, cajas y huecos de reserva. R reinicia el nivel.",
            "Setas movem entre parafusos, Espaço tira um parafuso livre, I lê nível, tentativas, caixas e furos reserva. R reinicia o nível.",
            "Flèches pour passer d'une vis à l'autre, Espace retire une vis libre, I lit niveau, essais, boîtes et trous de réserve. R recommence le niveau.",
            "Pfeiltasten wechseln zwischen Schrauben, Leertaste dreht eine freie heraus, I liest Level, Versuche, Boxen und Ersatzlöcher vor. R startet das Level neu.",
            "Стрелки переходят между винтами, пробел выкручивает свободный винт, I читает уровень, попытки, коробки и запасные гнёзда. R начинает уровень заново.",
            "方向鍵在螺絲間移動，空白鍵擰下可以擰下的螺絲，I 讀出關卡、剩餘次數、盒子和備用孔。R 重新開始本關。",
        ])
    }
    fn act(&mut self, x: i32, y: i32, _a: i32, said: &mut String) -> u32 {
        if self.s.phase == CLEARED || self.s.phase == FAILED {
            if !self.anims.is_empty() {
                return 0;
            }
            let ev = self.button();
            self.status(said);
            return ev;
        }
        self.focus(x, y);
        let i = self.s.cur as usize;
        if self.s.phase != PLAY || self.s.lv.screw[i].state != ON_BOARD {
            self.cell(x, 0, said);
            return 0;
        }
        let gone = (0..self.s.lv.plates as usize).filter(|&p| self.s.lv.plate[p].gone != 0).count();
        let ev = self.tap(i);
        if ev == 0 {
            said.push_str(tr(&["Covered by a plate", "被板挡住", "板の下です", "판에 가려져 있습니다", "Tapado por una placa", "Coberto por uma placa", "Sous une plaque", "Unter einer Platte", "Под пластиной", "被板擋住"]));
            return 0;
        }
        self.screw_words(i, said);
        let fell = (0..self.s.lv.plates as usize).filter(|&p| self.s.lv.plate[p].gone != 0).count() - gone;
        if fell > 0 {
            said.push_str(". ");
            said.push_str(&tr(&["Plates fell: #", "掉落 # 块板", "板が # 枚落ちた", "판 # 개 떨어짐", "Placas caídas: #", "Placas caídas: #", "Plaques tombées : #", "Platten gefallen: #", "Упало пластин: #", "掉落 # 塊板"]).replace('#', &fell.to_string()));
        }
        let end = match self.s.phase {
            CLEARED | DONE_ALL => tr(&["Level cleared! Press Space for the next level.", "过关！按空格进入下一关。", "クリア！スペースで次のレベル。", "레벨 클리어! 스페이스로 다음 레벨.", "¡Nivel superado! Espacio para el siguiente.", "Nível concluído! Espaço para o próximo.", "Niveau réussi ! Espace pour le suivant.", "Level geschafft! Leertaste für das nächste.", "Уровень пройден! Пробел для следующего.", "過關！按空白鍵進入下一關。"]),
            FAILED => tr(&["No spare slots left. Press Space to retry.", "没有空位了。按空格再试一次。", "置く場所がありません。スペースでもう一度。", "빈칸이 없습니다. 스페이스로 다시 하기.", "No queda sitio. Espacio para reintentar.", "Não há mais espaço. Espaço para tentar de novo.", "Plus de place. Espace pour réessayer.", "Kein Platz mehr. Leertaste für nochmal.", "Места больше нет. Пробел, чтобы ещё раз.", "沒有空位了。按空白鍵再試一次。"]),
            LOST => tr(&["No spare slots or tries left. Game over.", "没有空位，也没有次数了。游戏结束。", "置く場所も挑戦も残っていません。ゲームオーバー。", "빈칸도 기회도 없습니다. 게임 오버.", "No queda sitio ni intentos. Fin de la partida.", "Sem espaço e sem tentativas. Fim de jogo.", "Plus de place ni d'essais. Partie terminée.", "Kein Platz und keine Versuche mehr. Spiel vorbei.", "Ни места, ни попыток. Игра окончена.", "沒有空位，也沒有次數了。遊戲結束。"]),
            _ => "",
        };
        if !end.is_empty() {
            said.push_str(". ");
            said.push_str(end);
        }
        if self.s.phase == PLAY {
            self.cursor_on();
        }
        ev
    }
    fn rev(&mut self) -> u32 {
        let v = [self.s.phase, self.s.level, self.s.tries, self.tall as i32, wg::lang() as i32];
        wg::hash(wg::hash(0, wg::bytes_of(&self.s.lv)), wg::bytes_of(&v))
    }
}

wg::game!(Unbolt);
wg::board!(Unbolt);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Unbolt, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    /// a screw a box wants, else any that can go
    fn pick(g: &Unbolt) -> Option<usize> {
        let lv = &g.s.lv;
        let n = lv.screws as usize;
        let wanted = |i: usize| (0..2).any(|a| lv.open[a] >= 0 && lv.colour[lv.open[a] as usize] == lv.screw[i].colour && lv.fill[a] < 3);
        (0..n).filter(|&i| lv.takes(i)).max_by_key(|&i| (wanted(i), std::cmp::Reverse(i)))
    }

    fn solve(lv: &puzzle::Level) -> Option<Vec<usize>> {
        if lv.cleared() {
            return Some(Vec::new());
        }
        let n = lv.screws as usize;
        let mut cand: Vec<usize> = (0..n).filter(|&i| lv.takes(i)).collect();
        let wanted = |i: &usize| (0..2).any(|a| lv.open[a] >= 0 && lv.colour[lv.open[a] as usize] == lv.screw[*i].colour && lv.fill[a] < 3);
        cand.sort_by_key(|i| !wanted(i));
        for i in cand {
            let mut next = *lv;
            next.tap(i);
            if let Some(mut rest) = solve(&next) {
                rest.insert(0, i);
                return Some(rest);
            }
        }
        None
    }

    #[test]
    fn plays_every_level() {
        let mut g = Unbolt::new();
        g.start(3);
        for level in 0..LEVELS {
            assert_eq!(g.s.level, level);
            // a covered screw only wiggles
            if let Some(i) = (0..g.s.lv.screws as usize).find(|&i| g.s.lv.blocked(i)) {
                assert_eq!(g.tap(i), 0);
            }
            let mut ev = 0;
            while g.s.phase == PLAY {
                let i = pick(&g).expect("stuck");
                ev |= g.tap(i);
                // every screw taken flies somewhere, the one that fills a box too
                assert!(g.anims.iter().all(|a| !matches!(a, Anim::Fly { from, to, .. } if from == to)));
                ev |= run(&mut g, 40);
                let keep = std::mem::take(&mut g.anims);
                assert!(g.loaded());
                g.anims = keep;
            }
            ev |= run(&mut g, 10_000);
            assert!(g.anims.is_empty());
            if g.s.phase == FAILED {
                println!("level {} lost a try", level + 1);
                g.button();
                for i in solve(&g.s.lv).expect("solvable") {
                    ev |= g.tap(i);
                }
                ev |= run(&mut g, 10_000);
            }
            assert!(ev & wg::EV_LEVEL != 0, "level {}", level + 1);
            if level + 1 < LEVELS {
                assert_eq!(g.s.phase, CLEARED);
                g.button();
            } else {
                assert!((ev | run(&mut g, 1000)) & wg::EV_GAMEOVER != 0);
                assert!(g.over());
            }
        }
        println!("score {} lost {}", g.score(), g.s.lost);
    }
}
