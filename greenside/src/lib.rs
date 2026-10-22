//! Greenside - an original wasmga.me game. MIT licensed.
//! Mini golf over 18 holes: pull back and let go to putt, round rails,
//! bumpers, sand, water, slopes, windmills and sliding gates. Runs on the wg
//! engine: the page shows the setup, pause and game over screens.

pub mod bot;
pub mod draw;
mod glyphs;
pub mod golf;
pub mod holes;

use golf::*;
use wg::{Game, Input, Pointer, Screen};

pub struct Greenside {
    s: State,
    /// not saved: sparks, splashes, the trail
    a: draw::Anim,
    /// not saved: a pull under way, and how long an arrow has been held
    drag: Drag,
    hold_ms: i32,
}

impl Game for Greenside {
    type Save = State;
    const PLAN: (f32, f32) = (PW, PH);
    const CHOICES: &'static [i32] = &CHOICES;

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let mut s = State::zero();
        s.start();
        Greenside { s, a: draw::Anim::new(), drag: Drag::default(), hold_ms: 0 }
    }
    fn save(&mut self) -> &mut State {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, _seed: u32) {
        self.s.start();
        self.a.reset();
        self.drag = Drag::default();
    }
    fn update(&mut self, input: &Input) -> u32 {
        self.a.tick(&self.s, input.dt);
        let theme = self.s.opt[O_THEME].clamp(0, N_THEMES - 1) as usize;
        let fx = &mut draw::AnimFx(&mut self.a, theme);
        self.s.frame(input.held, input.pressed, input.dt, self.drag.on, &mut self.hold_ms, fx)
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        self.s.pointer(&mut self.drag, p.x, p.y, p.inside, p.pressed & wg::M_LEFT != 0, p.released & wg::M_LEFT != 0)
    }
    fn draw(&mut self, g: &mut Screen) {
        draw::draw(g, &self.s, &self.a, &self.drag);
    }
    /// what the picture shows, leaving out the clocks
    fn picture(&mut self) -> u32 {
        let (s, d) = (&self.s, &self.drag);
        let shown = [s.opt[0], s.opt[1], s.phase, s.hole, s.strokes, s.score, d.on as i32, s.played];
        let at = [s.x, s.y, s.aim, s.power, d.x0, d.y0, d.x, d.y];
        wg::hash(wg::hash(0, wg::bytes_of(&shown)), wg::bytes_of(&at))
    }
    fn moving(&self) -> bool {
        matches!(self.s.phase, ROLL | SUNK | SPLASH | PICKED) || self.s.busy() || self.a.busy()
    }
    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.over()
    }
    /// the catalog's stats: Holes, Strokes, To par, Holes in one, Splashes, Time
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        [s.played, s.total, s.to_par(), s.aces, s.splashes, s.time_ms].get(i).copied().unwrap_or(-1)
    }
    fn reached(&self) -> i32 {
        self.s.hole
    }
    fn cleared(&self) -> Option<(i32, u32)> {
        (self.s.cleared >= 0).then(|| (self.s.cleared, self.s.cleared_points() as u32))
    }
    fn loaded(&mut self) -> bool {
        self.a.reset();
        self.drag = Drag::default();
        self.hold_ms = 0;
        self.s.valid()
    }
}

wg::game!(Greenside);
