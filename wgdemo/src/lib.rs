//! The smallest whole game on the wg crate, as a template: catch the dot.
//! Move with the arrows or tap where it is; five catches clear a level, and
//! the third level ends the run. It has setup options (a theme and a level
//! picker), stats, a still background and pointer input.
use wg::{hex, Game, Input, KeyRepeat, Pointer, Rng, Screen};

const VERSION: u32 = 3;
/// catches per level
const GOAL: i32 = 5;
const LEVELS: i32 = 3;
const GRID: i32 = 8;

/// Everything a save keeps; only Plain fields (see wg::Plain).
#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    /// options, in the catalog `setup` order: theme, level to start at
    opt: [i32; 2],
    level: i32,
    /// time on the level in play, and the last level cleared with its score
    level_ms: i32,
    cleared: i32,
    cleared_score: i32,
    rng: Rng,
    you: [i32; 2],
    dot: [i32; 2],
    caught: i32,
    moves: i32,
    time_ms: i32,
    rep: KeyRepeat,
}
unsafe impl wg::Plain for Save {}

struct Demo {
    s: Save,
    /// not saved: the catch flash
    flash: f32,
}

const THEMES: [(u32, u32, u32); 2] = [(0x1d2433, 0x2b3550, 0xffd166), (0xf4efe6, 0xe2d9c8, 0xd1495b)];

impl Demo {
    fn place_dot(&mut self) {
        loop {
            let d = [self.s.rng.range(0, GRID - 1), self.s.rng.range(0, GRID - 1)];
            if d != self.s.you {
                self.s.dot = d;
                return;
            }
        }
    }
    fn step(&mut self, to: [i32; 2]) -> u32 {
        if self.over() || to == self.s.you || to.iter().any(|&v| !(0..GRID).contains(&v)) {
            return 0;
        }
        self.s.you = to;
        self.s.moves += 1;
        if to != self.s.dot {
            return wg::EV_BLIP;
        }
        self.s.caught += 1;
        self.flash = 1.0;
        if self.s.caught < GOAL {
            self.place_dot();
            return wg::EV_HIT;
        }
        // a level's own score: quicker is better
        self.s.cleared = self.s.level;
        self.s.cleared_score = (1000 - self.s.level_ms / 50).max(100);
        self.s.level_ms = 0;
        if self.s.level + 1 >= LEVELS {
            return wg::EV_LEVEL | wg::EV_WIN | wg::EV_GAMEOVER;
        }
        self.s.level += 1;
        self.s.caught = 0;
        self.place_dot();
        wg::EV_LEVEL
    }
}

impl Game for Demo {
    type Save = Save;
    const PLAN: (f32, f32) = (300.0, 340.0);
    const CHOICES: &'static [i32] = &[2, LEVELS];

    fn new() -> Self {
        Demo {
            s: Save { version: VERSION, opt: [0, 0], level: 0, level_ms: 0, cleared: -1, cleared_score: 0, rng: Rng::new(1), you: [0, 0], dot: [3, 3], caught: 0, moves: 0, time_ms: 0, rep: KeyRepeat(0) },
            flash: 0.0,
        }
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        self.s = Save { rng: Rng::new(seed), opt: self.s.opt, ..Demo::new().s };
        self.s.level = self.s.opt[1];
        self.place_dot();
    }
    fn update(&mut self, input: &Input) -> u32 {
        self.flash = (self.flash - input.dt as f32 / 300.0).max(0.0);
        if self.over() {
            return 0;
        }
        self.s.time_ms += input.dt;
        self.s.level_ms += input.dt;
        let dirs = self.s.rep.dirs(input);
        let [x, y] = self.s.you;
        let mut ev = 0;
        if dirs & wg::LEFT != 0 { ev |= self.step([x - 1, y]); }
        if dirs & wg::RIGHT != 0 { ev |= self.step([x + 1, y]); }
        if dirs & wg::UP != 0 { ev |= self.step([x, y - 1]); }
        if dirs & wg::DOWN != 0 { ev |= self.step([x, y + 1]); }
        ev
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        if p.pressed & wg::M_LEFT == 0 {
            return 0;
        }
        let (cx, cy) = (((p.x - 20.0) / 32.5).floor() as i32, ((p.y - 60.0) / 32.5).floor() as i32);
        self.step([cx, cy])
    }
    /// Redraw only when what's shown changes: the clock shows whole seconds,
    /// so the time counts in seconds here (the default hashes the whole save).
    fn picture(&mut self) -> u32 {
        let s = &self.s;
        let shown = [s.opt[0], s.level, s.you[0], s.you[1], s.dot[0], s.dot[1], s.caught, s.time_ms / 1000];
        wg::hash(0, wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        self.flash > 0.0
    }
    fn draw(&mut self, s: &mut Screen) {
        let (bg, cell, accent) = THEMES[self.s.opt[0] as usize];
        s.background(self.s.opt[0] as u32 + 1, |s| {
            s.clear(hex(bg));
            s.shadow(20.0, 64.0, 260.0, 260.0, 12.0, 10.0, 0, 0.35);
            for y in 0..GRID {
                for x in 0..GRID {
                    s.rect(22.0 + x as f32 * 32.5, 62.0 + y as f32 * 32.5, 28.5, 28.5, 6.0, hex(cell), 1.0);
                }
            }
        });
        let ink = if self.s.opt[0] == 0 { hex(0xf4efe6) } else { hex(0x1d2433) };
        s.text(&format!("Level {}: {} / {}", self.s.level + 1, self.s.caught, GOAL), 20.0, 30.0, 18.0, ink, 1.0);
        s.text_r(&wg::clock(self.s.time_ms), 280.0, 30.0, 22.0, ink, 0.7);
        let at = |c: [i32; 2]| (36.25 + c[0] as f32 * 32.5, 76.25 + c[1] as f32 * 32.5);
        let (dx, dy) = at(self.s.dot);
        s.disc(dx, dy, 7.0, hex(accent), 1.0);
        let (yx, yy) = at(self.s.you);
        s.ring(yx, yy, 12.0 + 6.0 * self.flash, 3.5, ink, 1.0);
    }
    fn score(&self) -> u32 {
        ((self.s.level * GOAL + self.s.caught) * 100) as u32
    }
    fn over(&self) -> bool {
        self.s.level + 1 >= LEVELS && self.s.caught >= GOAL
    }
    fn reached(&self) -> i32 {
        self.s.level
    }
    fn cleared(&self) -> Option<(i32, u32)> {
        (self.s.cleared >= 0).then(|| (self.s.cleared, self.s.cleared_score as u32))
    }
    fn stat(&self, i: usize) -> i32 {
        match i {
            0 => self.s.time_ms,
            1 => self.s.moves,
            2 => self.s.level + 1,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let ok = |c: [i32; 2]| c.iter().all(|v| (0..GRID).contains(v));
        self.flash = 0.0;
        self.s.version == VERSION && ok(self.s.you) && ok(self.s.dot) && (0..=GOAL).contains(&self.s.caught) && (0..LEVELS).contains(&self.s.level) && (-1..LEVELS).contains(&self.s.cleared) && self.s.level_ms >= 0 && self.s.time_ms >= 0
    }
}

wg::game!(Demo);
