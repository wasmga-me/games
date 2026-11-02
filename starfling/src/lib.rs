//! Starfling - an original wasmga.me toy. MIT licensed.
//! An orbit sandbox: tap to set a planet going round the sun, drag to fling
//! one (a dotted line shows where it will go), and watch planets swing,
//! slingshot, escape or crash together. Runs on the wg engine; the page
//! shows the setup and pause screens.

pub mod sim;

use sim::*;
use std::collections::VecDeque;
use wg::{hex, mix, Game, Input, Pointer, Screen};

/// Plan units across the screen's shorter side.
const VIEW: f32 = 520.0;
/// Planets further than this from the suns are gone for good.
const FAR: f32 = 1600.0;
/// A press that moves further than this (plan units) flings.
const DRAG: f32 = 9.0;
/// Fling speed per unit of drag.
const FLING: f32 = 1.5;
/// Seconds of path the fling preview shows.
const PREVIEW_SECS: f32 = 5.0;
/// Trail points: one every TRAIL_EVERY seconds, short or long.
const TRAIL_EVERY: f32 = 1.0 / 30.0;
const TRAIL_LEN: [usize; 2] = [45, 300];
/// Merge flashes last this long, in seconds.
const FLASH_SECS: f32 = 0.6;

const SPACE: u32 = 0x0b0a1c;
const PLANETS: [u32; N_COLOURS as usize] = [0x6fd3ff, 0xff8a5c, 0x9be36d, 0xd59bff, 0xffd166, 0x5ce1c2, 0xff6f9f];
const SUNS: [u32; 2] = [0xffd27a, 0x9fd0ff];

pub struct Fling {
    pub s: State,
    /// not saved: trails per body slot, oldest first
    trails: Vec<VecDeque<(f32, f32)>>,
    trail_clock: f32,
    flashes: Vec<(f32, f32, u32, f32)>,
    press: Option<Press>,
    /// the predicted path of the fling in progress, in world units
    path: Vec<(f32, f32)>,
    /// the plan's size, and the world point at its centre
    plan: (f32, f32),
    camera: (f32, f32),
}

#[derive(Clone, Copy)]
struct Press {
    /// where it went down and where it is, in world units
    x: f32,
    y: f32,
    at: (f32, f32),
    dragging: bool,
}

impl Fling {
    fn to_world(&self, x: f32, y: f32) -> (f32, f32) {
        (x - self.plan.0 / 2.0 + self.camera.0, y - self.plan.1 / 2.0 + self.camera.1)
    }
    fn to_plan(&self, x: f32, y: f32) -> (f32, f32) {
        (x - self.camera.0 + self.plan.0 / 2.0, y - self.camera.1 + self.plan.1 / 2.0)
    }
    fn planet_gm(&mut self) -> f32 {
        300.0 + 2200.0 * self.s.rng.float().powi(2)
    }
    /// A planet on a circle at a random spot in view.
    fn random_planet(&mut self) {
        let a = self.s.rng.float() * core::f32::consts::TAU;
        let d = 55.0 + (VIEW * 0.45 - 55.0) * self.s.rng.float();
        let (cx, cy) = self.s.centre();
        let gm = self.planet_gm();
        let slot = self.s.planet_at(cx + d * a.cos(), cy + d * a.sin(), gm);
        self.added(slot);
    }
    /// A new planet in this slot: the trail there was another body's.
    fn added(&mut self, slot: Option<usize>) {
        if let Some(i) = slot {
            self.trails[i].clear();
        }
    }
    fn reset_view(&mut self) {
        self.camera = self.s.centre();
        self.trails.iter_mut().for_each(VecDeque::clear);
        self.flashes.clear();
        self.press = None;
        self.path.clear();
    }
    fn colour(&self, b: &Body) -> u32 {
        if b.kind == KIND_STAR {
            hex(SUNS[(b.colour & 1) as usize])
        } else {
            hex(PLANETS[b.colour.clamp(0, N_COLOURS - 1) as usize])
        }
    }
    /// The fling velocity of a press: along the drag.
    fn fling_velocity(p: &Press) -> (f32, f32) {
        ((p.at.0 - p.x) * FLING, (p.at.1 - p.y) * FLING)
    }
}

impl Game for Fling {
    type Save = State;
    const CHOICES: &'static [i32] = &[3, 2];

    fn new() -> Self {
        let mut f = Fling {
            s: State::zero(),
            trails: vec![VecDeque::new(); MAX_BODIES],
            trail_clock: 0.0,
            flashes: Vec::new(),
            press: None,
            path: Vec::new(),
            plan: (VIEW, VIEW),
            camera: (0.0, 0.0),
        };
        f.start(1);
        f
    }
    fn save(&mut self) -> &mut State {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        self.s.start(seed);
        self.reset_view();
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        let k = VIEW / w.min(h).max(1) as f32;
        (w.max(1) as f32 * k, h.max(1) as f32 * k)
    }
    fn update(&mut self, input: &Input) -> u32 {
        if input.dt <= 0 {
            return 0;
        }
        let secs = input.dt as f32 / 1000.0;
        if input.pressed & wg::A != 0 {
            self.random_planet();
        }
        if input.pressed & wg::B != 0 {
            self.s.remove_last();
        }
        let mut merges = Merges::new();
        self.s.advance(secs, FAR, &mut merges);
        for (x, y, c) in merges {
            self.flashes.push((x, y, hex(PLANETS[c.clamp(0, N_COLOURS - 1) as usize]), 0.0));
        }
        self.flashes.iter_mut().for_each(|f| f.3 += secs);
        self.flashes.retain(|f| f.3 < FLASH_SECS);
        // the view drifts after the suns
        let (cx, cy) = self.s.centre();
        let t = 1.0 - (-secs * 3.0).exp();
        self.camera.0 += (cx - self.camera.0) * t;
        self.camera.1 += (cy - self.camera.1) * t;
        // trails
        self.trail_clock += secs;
        if self.trail_clock >= TRAIL_EVERY {
            self.trail_clock = (self.trail_clock - TRAIL_EVERY).min(TRAIL_EVERY);
            let len = TRAIL_LEN[self.s.opt[O_TRAILS].clamp(0, 1) as usize];
            for (b, tr) in self.s.bodies.iter().zip(self.trails.iter_mut()) {
                if b.kind == 0 {
                    tr.clear();
                    continue;
                }
                tr.push_back((b.x, b.y));
                while tr.len() > len {
                    tr.pop_front();
                }
            }
        }
        if let Some(p) = self.press.filter(|p| p.dragging) {
            let (vx, vy) = Fling::fling_velocity(&p);
            path(&self.s.bodies, p.x, p.y, vx, vy, PREVIEW_SECS, 0.05, &mut self.path);
        }
        0
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.press = None;
            self.path.clear();
            return 0;
        }
        let at = self.to_world(p.x, p.y);
        if p.pressed & wg::M_LEFT != 0 {
            self.press = Some(Press { x: at.0, y: at.1, at, dragging: false });
            return 0;
        }
        if p.released & wg::M_LEFT != 0 {
            if let Some(pr) = self.press.take() {
                let gm = self.planet_gm();
                if pr.dragging {
                    let (vx, vy) = Fling::fling_velocity(&pr);
                    let slot = self.s.fling(pr.x, pr.y, vx, vy, gm);
                    self.added(slot);
                } else {
                    let slot = self.s.planet_at(pr.x, pr.y, gm);
                    self.added(slot);
                }
            }
            self.path.clear();
            return 0;
        }
        if p.pressed & wg::M_RIGHT != 0 {
            self.s.remove_last();
        }
        if let Some(pr) = &mut self.press {
            pr.at = at;
            if !pr.dragging && ((at.0 - pr.x).powi(2) + (at.1 - pr.y).powi(2)).sqrt() > DRAG {
                pr.dragging = true;
            }
        }
        0
    }
    fn moving(&self) -> bool {
        true
    }
    fn draw(&mut self, s: &mut Screen) {
        self.plan = (s.pw, s.ph);
        let (x0, y0, x1, y1) = s.visible();
        s.background(1, |s| {
            s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(SPACE), 1.0);
            // a fixed starfield
            let mut rng = wg::Rng::new(7);
            let n = ((x1 - x0) * (y1 - y0) / 900.0) as i32;
            for _ in 0..n {
                let x = x0 + (x1 - x0) * rng.float();
                let y = y0 + (y1 - y0) * rng.float();
                let b = rng.float();
                s.disc(x, y, 0.5 + b * 0.8, hex(0xdfe6ff), 0.15 + b * 0.5);
            }
        });
        let hair = s.hair();
        let sky = hex(SPACE);
        // trails, fading toward their tails
        for (b, tr) in self.s.bodies.iter().zip(self.trails.iter()) {
            if b.kind == 0 || tr.len() < 2 {
                continue;
            }
            let c = self.colour(b);
            let n = tr.len() as f32;
            let w = if b.kind == KIND_STAR { 1.6 } else { 1.1 } * hair;
            for (i, (p, q)) in tr.iter().zip(tr.iter().skip(1)).enumerate() {
                let (a, z) = (self.to_plan(p.0, p.1), self.to_plan(q.0, q.1));
                // opaque, mixed into the sky, so joints don't show as beads
                s.line(a.0, a.1, z.0, z.1, w, mix(c, sky, 0.55 * (i as f32 + 1.0) / n), 1.0);
            }
            let (lx, ly) = tr.back().map_or((0.0, 0.0), |p| self.to_plan(p.0, p.1));
            let (bx, by) = self.to_plan(b.x, b.y);
            s.line(lx, ly, bx, by, w, mix(c, sky, 0.55), 1.0);
        }
        // suns, then planets
        for kind in [KIND_STAR, KIND_PLANET] {
            for b in self.s.bodies.iter().filter(|b| b.kind == kind) {
                let (x, y) = self.to_plan(b.x, b.y);
                let c = self.colour(b);
                if kind == KIND_STAR {
                    for (k, a) in [(3.2, 0.05), (2.3, 0.08), (1.6, 0.14)] {
                        s.disc(x, y, b.r * k, c, a);
                    }
                    s.disc(x, y, b.r, mix(hex(0xffffff), c, 0.45), 1.0);
                    s.disc(x - b.r * 0.15, y - b.r * 0.15, b.r * 0.6, hex(0xfffbf0), 0.6);
                } else {
                    s.disc(x, y, b.r * 1.9, c, 0.08);
                    s.disc(x, y, b.r, mix(hex(0x000000), c, 0.3), 1.0);
                    s.disc(x - b.r * 0.22, y - b.r * 0.22, b.r * 0.74, c, 1.0);
                    s.disc(x - b.r * 0.38, y - b.r * 0.38, b.r * 0.3, hex(0xffffff), 0.45);
                }
            }
        }
        // merge flashes
        for &(fx, fy, c, age) in &self.flashes {
            let (x, y) = self.to_plan(fx, fy);
            let t = age / FLASH_SECS;
            s.ring(x, y, 4.0 + 26.0 * wg::ease_out(t), 2.0 * hair + 2.0 * (1.0 - t), c, 0.8 * (1.0 - t));
        }
        // the fling in progress
        if let Some(p) = self.press {
            let (x, y) = self.to_plan(p.x, p.y);
            let c = hex(PLANETS[self.s.next_colour.clamp(0, N_COLOURS - 1) as usize]);
            s.disc(x, y, 4.0, c, 0.9);
            if p.dragging {
                let (ax, ay) = self.to_plan(p.at.0, p.at.1);
                s.line(x, y, ax, ay, 1.6 * hair, hex(0xffffff), 0.5);
                for (i, &(px, py)) in self.path.iter().enumerate().skip(1) {
                    let (px, py) = self.to_plan(px, py);
                    let fade = 1.0 - i as f32 / self.path.len() as f32;
                    s.disc(px, py, 1.6, c, 0.25 + 0.6 * fade);
                }
            } else {
                s.ring(x, y, 9.0, 1.5 * hair, c, 0.6);
            }
        }
    }
    fn score(&self) -> u32 {
        0
    }
    fn loaded(&mut self) -> bool {
        let ok = self.s.valid() && (0..3).contains(&self.s.opt[O_START]) && (0..2).contains(&self.s.opt[O_TRAILS]);
        self.reset_view();
        ok
    }
}

wg::game!(Fling);
