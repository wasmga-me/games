//! Updraft - an original wasmga.me game. MIT licensed.
//! An endless climb: a puff bounces off every ledge it lands on, steered
//! left and right, up past springs, gems, gusts and birds, until it falls
//! off the bottom of the screen. Runs on the wg engine: the page shows the
//! setup, pause and game over screens.

pub mod bot;
pub mod draw;
mod glyphs;
pub mod play;

use play::*;
use wg::{Game, Input, Pointer, Screen};

pub struct Updraft {
    s: State,
    /// not saved: sparks and the clock clouds and gems move by
    a: draw::Anim,
    /// not saved: the x a finger or the mouse steers to
    steer: Option<f32>,
}

impl Game for Updraft {
    type Save = State;
    const PLAN: (f32, f32) = (PW, PH);
    const CHOICES: &'static [i32] = &CHOICES;

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let mut s = State::zero();
        s.start(1);
        Updraft { s, a: draw::Anim::new(), steer: None }
    }
    fn save(&mut self) -> &mut State {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        self.s.start(seed);
        self.a = draw::Anim::new();
    }
    fn update(&mut self, input: &Input) -> u32 {
        self.a.tick(input.dt);
        let theme = self.s.opt[O_THEME].clamp(0, N_THEMES - 1) as usize;
        if input.held & (wg::LEFT | wg::RIGHT) != 0 {
            self.steer = None;
        }
        self.s.frame(input.held, self.steer, input.dt, &mut draw::AnimFx(&mut self.a, theme))
    }
    /// a finger held, or the mouse moved, steers to where it is
    fn pointer(&mut self, p: &Pointer) -> u32 {
        if p.inside && (p.held != 0 || !p.touch) {
            self.steer = Some(p.x.clamp(0.0, PW - 0.01));
        } else if p.touch || !p.inside {
            self.steer = None;
        }
        0
    }
    fn draw(&mut self, g: &mut Screen) {
        draw::draw(g, &self.s, &self.a);
    }
    /// the sky and the puff are always moving
    fn moving(&self) -> bool {
        true
    }
    fn score(&self) -> u32 {
        self.s.score().max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.over()
    }
    /// the catalog's stats: Height, Gems, Springs, Birds, Time
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        [s.height(), s.gem_count, s.springs, s.stomps, s.time_ms].get(i).copied().unwrap_or(-1)
    }
    fn loaded(&mut self) -> bool {
        self.a = draw::Anim::new();
        self.steer = None;
        self.s.valid()
    }
}

wg::game!(Updraft);
