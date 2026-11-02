//! Fractal Dive - an original wasmga.me toy. MIT licensed.
//! Dive into the Mandelbrot set or a Julia set: hold a finger or the mouse
//! still to dive toward it, drag to look around, tap to zoom in twice as
//! far. Runs on the wg engine; the page shows the setup and pause screens.
//!
//! Drawing: every screen pixel keeps its escape value and how coarse it is
//! (the size of the block it was filled from). Each frame spends a budget of
//! iterations refining, from 16-pixel blocks down to single pixels, and a
//! move or zoom maps the old values onto the new view first, so the picture
//! follows the finger at once and sharpens as it catches up.

pub mod view;

use view::*;
use wg::{hex, Game, Input, Pointer, Rng, Screen};

const VERSION: u32 = 1;
pub const O_SET: usize = 0;
pub const O_COLOURS: usize = 1;
pub const N_COLOURS: i32 = 4;

/// Julia constants worth diving into, one picked per run.
const JULIAS: [(f64, f64); 8] = [
    (-0.8, 0.156),
    (-0.7269, 0.1889),
    (0.285, 0.01),
    (-0.4, 0.6),
    (-0.835, -0.2321),
    (-0.70176, -0.3842),
    (0.355, 0.355),
    (-0.162, 1.04),
];

/// Everything a save keeps.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Save {
    pub version: u32,
    /// options, in the catalog `setup` order: set, colours
    pub opt: [i32; 2],
    pub rng: Rng,
    pub view: View,
    /// the Julia constant
    pub jr: f64,
    pub ji: f64,
}
unsafe impl wg::Plain for Save {}

/// Hold this long without moving to dive.
const HOLD_MS: i32 = 260;
/// A press that moves further than this (screen pixels) looks around.
const DRAG_PX: f32 = 7.0;
/// Zoom per second while diving.
const DIVE_RATE: f64 = 2.2;

pub struct Dive {
    pub s: Save,
    pub r: Render,
    /// not saved: the press in progress
    press: Option<Press>,
    /// the pointer as last seen, in screen pixels
    at: (f32, f32),
    /// zoom buttons in the corner: which one is held
    button: i32,
    keys: u32,
    /// the colours option and its palette, built when it changes
    pal: Option<(usize, Palette)>,
}

#[derive(Clone, Copy)]
struct Press {
    x: f32,
    y: f32,
    ms: i32,
    dragging: bool,
    right: bool,
}

impl Dive {
    fn julia(&self) -> Option<(f64, f64)> {
        (self.s.opt[O_SET] == 1).then_some((self.s.jr, self.s.ji))
    }
    /// The zoom buttons: centres and radius, bottom right.
    fn buttons(&self) -> [(f32, f32, f32); 2] {
        let (w, h) = (self.r.w as f32, self.r.h as f32);
        let r = (w.min(h) * 0.055).clamp(20.0, 30.0);
        let m = r * 0.6;
        [(w - m - r, h - m - 3.0 * r - m * 0.5, r), (w - m - r, h - m - r, r)]
    }
    fn button_at(&self, x: f32, y: f32) -> i32 {
        for (i, (bx, by, r)) in self.buttons().iter().enumerate() {
            if (x - bx).powi(2) + (y - by).powi(2) <= (r * 1.15).powi(2) {
                return i as i32 + 1;
            }
        }
        0
    }
    fn zoom_at(&mut self, x: f32, y: f32, f: f64) {
        self.s.view.zoom_about(x as f64, y as f64, self.r.w, self.r.h, f);
    }
    fn pan_by(&mut self, dx: f32, dy: f32) {
        self.s.view.pan(dx as f64, dy as f64, self.r.h);
    }
    /// A new run or a load: no press, and the picture worked out afresh.
    fn forget(&mut self) {
        self.r.reset();
        self.press = None;
        self.button = 0;
    }
}

impl Game for Dive {
    type Save = Save;
    const CHOICES: &'static [i32] = &[2, N_COLOURS];

    fn new() -> Self {
        let mut d = Dive {
            s: Save { version: VERSION, opt: [0, 0], rng: Rng::new(1), view: View::home(false), jr: JULIAS[0].0, ji: JULIAS[0].1 },
            r: Render::new(),
            press: None,
            at: (-1.0, -1.0),
            button: 0,
            keys: 0,
            pal: None,
        };
        d.start(1);
        d
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        let mut rng = Rng::new(seed);
        let (jr, ji) = JULIAS[rng.range(0, JULIAS.len() as i32 - 1) as usize];
        self.s = Save { version: VERSION, opt: self.s.opt, rng, view: View::home(self.s.opt[O_SET] == 1), jr, ji };
        self.forget();
    }
    /// Plan units are screen pixels: the picture is drawn pixel by pixel.
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        (w.max(1) as f32, h.max(1) as f32)
    }
    fn update(&mut self, input: &Input) -> u32 {
        self.keys = input.held;
        let dt = input.dt;
        if dt <= 0 {
            return 0;
        }
        let secs = dt as f64 / 1000.0;
        let (w, h) = (self.r.w as f32, self.r.h as f32);
        // arrows look around, A dives and B rises, at the centre
        let mut dx = 0.0;
        let mut dy = 0.0;
        let pan = (w.min(h) * 0.9 * secs as f32).round().max(1.0);
        if input.held & wg::LEFT != 0 { dx -= pan; }
        if input.held & wg::RIGHT != 0 { dx += pan; }
        if input.held & wg::UP != 0 { dy -= pan; }
        if input.held & wg::DOWN != 0 { dy += pan; }
        if dx != 0.0 || dy != 0.0 {
            self.pan_by(dx, dy);
        }
        let mut f = 1.0;
        if input.held & wg::A != 0 || self.button == 1 { f *= DIVE_RATE.powf(secs); }
        if input.held & wg::B != 0 || self.button == 2 { f /= DIVE_RATE.powf(secs); }
        if f != 1.0 {
            self.zoom_at(w / 2.0, h / 2.0, f);
        }
        if let Some(p) = &mut self.press {
            p.ms += dt;
            let p = *p;
            if !p.dragging && (p.ms >= HOLD_MS || p.right) {
                let f = DIVE_RATE.powf(secs);
                let (x, y) = self.at;
                self.zoom_at(x, y, if p.right { 1.0 / f } else { f });
            }
        }
        0
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.press = None;
            self.button = 0;
            return 0;
        }
        let (x, y) = (p.x, p.y);
        if p.pressed != 0 {
            self.at = (x, y);
            self.button = self.button_at(x, y);
            if self.button == 0 {
                self.press = Some(Press { x, y, ms: 0, dragging: false, right: p.pressed & wg::M_RIGHT != 0 });
            }
            return 0;
        }
        if p.held == 0 {
            if let Some(pr) = self.press.take() {
                // a quick tap zooms in twice as far, centred there
                if !pr.dragging && !pr.right && pr.ms < HOLD_MS {
                    self.zoom_at(x, y, 2.0);
                    self.pan_by(x - self.r.w as f32 / 2.0, y - self.r.h as f32 / 2.0);
                }
            }
            self.button = 0;
            self.at = (x, y);
            return 0;
        }
        if let Some(pr) = &mut self.press {
            let (dx, dy) = (x - self.at.0, y - self.at.1);
            if !pr.dragging && ((x - pr.x).powi(2) + (y - pr.y).powi(2)).sqrt() > DRAG_PX && pr.ms < HOLD_MS && !pr.right {
                pr.dragging = true;
            }
            if pr.dragging && (dx != 0.0 || dy != 0.0) {
                self.pan_by(-dx, -dy);
            }
        } else if self.button != 0 && self.button_at(x, y) != self.button {
            self.button = 0;
        }
        self.at = (x, y);
        0
    }
    /// The save, and which zoom button is lit.
    fn picture(&mut self) -> u32 {
        wg::hash(wg::hash(0, wg::bytes_of(&self.s)), &self.button.to_le_bytes())
    }
    fn moving(&self) -> bool {
        !self.r.finished(&self.s.view) || self.press.is_some() || self.button != 0 || self.keys != 0
    }
    fn draw(&mut self, s: &mut Screen) {
        let (w, h) = s.size_px();
        self.r.size(w, h);
        let julia = self.julia();
        self.r.follow(&self.s.view);
        self.r.refine(&self.s.view, julia, BUDGET);
        let colours = self.s.opt[O_COLOURS].clamp(0, N_COLOURS - 1) as usize;
        if self.pal.as_ref().map(|p| p.0) != Some(colours) {
            self.pal = Some((colours, palette(colours)));
        }
        if let Some((_, pal)) = &self.pal {
            self.r.paint(s, pal);
        }
        // the zoom buttons
        let ink = hex(0xf4efe6);
        for (i, (bx, by, r)) in self.buttons().iter().enumerate() {
            let on = self.button == i as i32 + 1;
            s.disc(*bx, *by, *r, hex(0x120f26), if on { 0.85 } else { 0.55 });
            s.ring(*bx, *by, *r, 2.0, ink, 0.6);
            s.rect(bx - r * 0.42, by - r * 0.07, r * 0.84, r * 0.14, r * 0.07, ink, 0.95);
            if i == 0 {
                s.rect(bx - r * 0.07, by - r * 0.42, r * 0.14, r * 0.84, r * 0.07, ink, 0.95);
            }
        }
        // how far in
        let z = self.s.view.zoom();
        let label = zoom_label(z);
        let size = (w.min(h) as f32 * 0.035).clamp(13.0, 18.0);
        let tw = s.text_width(&label, size);
        s.rect(10.0, h as f32 - size * 2.2 - 8.0, tw + size * 1.2, size * 2.0, size * 0.5, hex(0x120f26), 0.55);
        s.text(&label, 10.0 + size * 0.6, h as f32 - size * 1.2 - 8.0, size, ink, 0.9);
    }
    fn score(&self) -> u32 {
        0
    }
    fn loaded(&mut self) -> bool {
        self.forget();
        self.s.version == VERSION && self.s.view.valid() && self.s.jr.is_finite() && self.s.ji.is_finite() && self.s.jr.abs() < 4.0 && self.s.ji.abs() < 4.0
    }
}

/// "250x", "3,400,000x": digits only, so it reads the same in every language.
pub fn zoom_label(z: f64) -> String {
    format!("{}x", group(z.max(1.0).round() as u64))
}

fn group(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

wg::game!(Dive);
