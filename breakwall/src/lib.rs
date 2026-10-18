//! Breakwall - an original wasmga.me game. MIT licensed.
//! Bounce the ball off the paddle and break every brick, over 30 levels,
//! with boosts falling from broken bricks. Runs on the wg engine: the page
//! shows the setup, pause and game over screens, so this only plays.

pub mod draw;
pub mod game;
pub mod levels;
pub mod math;
mod glyphs;

use game::*;
use wg::{Game, Input, Pointer, Screen};

pub struct Breakwall {
    s: State,
    /// not saved: sparks, flashes, trails, the still layer's dirty cells
    a: draw::Anim,
}

impl Game for Breakwall {
    type Save = State;
    const PLAN: (f32, f32) = (PW, PH);
    const CHOICES: &'static [i32] = &CHOICES;

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let mut s = State::zero();
        s.start(1);
        let mut a = draw::Anim::new();
        a.reset();
        Breakwall { s, a }
    }
    fn save(&mut self) -> &mut State {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        self.s.start(seed);
        self.a.reset();
    }
    fn update(&mut self, input: &Input) -> u32 {
        draw::tick(&mut self.a, input.dt);
        let mut fx = draw::AnimFx { a: &mut self.a, theme: self.s.opt[O_THEME] as usize };
        self.s.frame(input.held as i32, input.dt, &mut fx) as u32
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        let b = p.held as i32 | if p.touch { 8 } else { 0 };
        self.s.mouse(p.x, p.y, b, !p.inside);
        0
    }
    fn draw(&mut self, g: &mut Screen) {
        draw::draw(g, &self.s, &mut self.a);
    }
    /// the ball is nearly always moving; only a finished run stands still
    fn moving(&self) -> bool {
        self.s.phase != OVER || self.a.busy()
    }
    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    /// over the moment the last ball is lost or the last level cleared
    fn over(&self) -> bool {
        self.s.over()
    }
    /// the catalog's stats: Level, Bricks, Boosts, Best streak, Balls lost, Time
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        [s.level + 1, s.broken, s.boosts, s.best_streak, s.lost, s.time_ms].get(i).copied().unwrap_or(-1)
    }
    /// the level the run is on, for the page's level picker
    fn reached(&self) -> i32 {
        self.s.level
    }
    /// the level just cleared and its own score, for its scoreboard
    fn cleared(&self) -> Option<(i32, u32)> {
        (self.s.cleared >= 0).then(|| (self.s.cleared, self.s.cleared_score as u32))
    }
    fn loaded(&mut self) -> bool {
        self.a.reset();
        self.a.banner = 1e9;
        let s = &mut self.s;
        s.prev_buttons = 0xFF;
        s.touch_on = 0;
        s.mbtn = 0;
        s.launch = 0;
        s.valid()
    }
}

wg::game!(Breakwall);
