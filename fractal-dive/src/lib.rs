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
use wg::{hex, mix, Game, Input, Pointer, Rng, Screen};

const VERSION: u32 = 1;
pub const O_SET: usize = 0;
pub const O_COLOURS: usize = 1;
pub const N_COLOURS: i32 = 8;

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
    /// the screen in pixels, and device pixels per CSS pixel
    sw: u32,
    sh: u32,
    dpr: f32,
    /// screen pixels per worked-out value (more on dense screens)
    cell: u32,
    /// iterations a frame may spend, tuned to keep frames coming on time
    budget: u64,
    /// a pan's leftover part of a cell, kept so pans move whole cells
    rest: (f32, f32),
    /// the two-finger pinch in progress: its middle and spread
    pinch: Option<(f32, f32, f32)>,
    /// Julia mode: a finger or the mouse on the map, choosing the constant
    picking: bool,
    /// the map's picture, and the size and colours it was drawn for
    map: Vec<u32>,
    map_key: (u32, u32, usize),
}

/// The part of the Mandelbrot set the map shows: left, top, width, height.
const MAP: (f64, f64, f64, f64) = (-2.1, -1.2, 2.8, 2.4);

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
        let (w, h) = (self.sw as f32, self.sh as f32);
        let r = (w.min(h) * 0.055).clamp(20.0 * self.dpr, 30.0 * self.dpr);
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
        self.s.view.zoom_about(x as f64, y as f64, self.sw, self.sh, f);
    }
    /// Pans by whole cells, so the values already worked out stay sharp.
    fn pan_by(&mut self, dx: f32, dy: f32) {
        let c = self.cell as f32;
        let (x, y) = (self.rest.0 + dx, self.rest.1 + dy);
        let (cx, cy) = ((x / c).round() * c, (y / c).round() * c);
        self.rest = (x - cx, y - cy);
        if cx != 0.0 || cy != 0.0 {
            // in whole cells of the picture's own grid
            self.s.view.pan((cx / c) as f64, (cy / c) as f64, self.r.h.max(1));
        }
    }
    /// The mouse wheel, in hundredths of a line (down zooms out).
    pub fn wheel(&mut self, d: i32) {
        let (x, y) = if self.at.0 >= 0.0 { self.at } else { (self.sw as f32 / 2.0, self.sh as f32 / 2.0) };
        self.zoom_at(x, y, 2f64.powf(-d as f64 / 100.0 * 0.2));
    }
    /// Two fingers at x, y (their middle) and d apart, in screen pixels;
    /// d 0 when they lift. They zoom and move the view together.
    pub fn pinch(&mut self, x: f32, y: f32, d: f32) {
        if d <= 0.0 {
            self.pinch = None;
            return;
        }
        self.press = None;
        self.button = 0;
        if let Some((px, py, pd)) = self.pinch {
            self.pan_by(px - x, py - y);
            self.zoom_at(x, y, (d / pd.max(1.0)) as f64);
        }
        self.pinch = Some((x, y, d));
    }
    /// Device pixels per CSS pixel, in hundredths: the picture is worked out
    /// about once per CSS pixel, and the buttons keep their size.
    pub fn set_dpr(&mut self, d: i32) {
        self.dpr = (d as f32 / 100.0).clamp(1.0, 4.0);
    }
    /// A new run or a load: no press, and the picture worked out afresh.
    fn forget(&mut self) {
        self.r.reset();
        self.press = None;
        self.button = 0;
        self.pinch = None;
        self.picking = false;
    }
    /// Julia mode's map of the Mandelbrot set, top left: x, y, w, h in
    /// screen pixels. Each point on it is a Julia set's constant.
    fn map_rect(&self) -> Option<(f32, f32, f32, f32)> {
        self.julia()?;
        let w = (self.sw.min(self.sh) as f32 * 0.3).clamp(110.0 * self.dpr, 180.0 * self.dpr).min(self.sw as f32 * 0.45).round();
        let h = (w * (MAP.3 / MAP.2) as f32).round();
        let m = (10.0 * self.dpr).round();
        Some((m, m, w, h))
    }
    fn on_map(&self, x: f32, y: f32) -> bool {
        self.map_rect().is_some_and(|(mx, my, mw, mh)| x >= mx && y >= my && x < mx + mw && y < my + mh)
    }
    /// The Julia constant at screen point x, y of the map.
    fn pick(&mut self, x: f32, y: f32) {
        let Some((mx, my, mw, mh)) = self.map_rect() else {
            return;
        };
        let fx = ((x - mx) / mw).clamp(0.0, 1.0) as f64;
        let fy = ((y - my) / mh).clamp(0.0, 1.0) as f64;
        self.s.jr = MAP.0 + fx * MAP.2;
        self.s.ji = MAP.1 + fy * MAP.3;
    }
    /// Draws the map (its picture worked out once per size and colours) and
    /// where the constant is on it.
    fn draw_map(&mut self, s: &mut Screen, pal: &Palette) {
        let Some((mx, my, mw, mh)) = self.map_rect() else {
            return;
        };
        let (w, h) = (mw as u32, mh as u32);
        let colours = self.s.opt[O_COLOURS].clamp(0, N_COLOURS - 1) as usize;
        if self.map_key != (w, h, colours) {
            self.map_key = (w, h, colours);
            self.map.clear();
            let mut spent = 0;
            for j in 0..h {
                for i in 0..w {
                    let cr = MAP.0 + (i as f64 + 0.5) / w as f64 * MAP.2;
                    let ci = MAP.1 + (j as f64 + 0.5) / h as f64 * MAP.3;
                    let v = escape(cr, ci, None, 160, &mut spent);
                    self.map.push(if v < 0.0 { pal.inside_colour() } else { pal.at(v * 4.0) });
                }
            }
        }
        let (sw, sh) = s.size_px();
        let (x0, y0) = (mx as i32, my as i32);
        for j in 0..h as i32 {
            if y0 + j >= sh {
                break;
            }
            let row = s.row_px(y0 + j);
            for i in 0..(w as i32).min(sw - x0) {
                // dimmed, so the marker stands out
                row[(x0 + i) as usize] = mix(self.map[(j as u32 * w + i as u32) as usize], hex(0x120f26), 0.75);
            }
        }
        let ink = hex(0xf4efe6);
        s.rect(mx - 1.0, my - 1.0, mw + 2.0, 1.5, 0.0, ink, 0.5);
        s.rect(mx - 1.0, my + mh - 0.5, mw + 2.0, 1.5, 0.0, ink, 0.5);
        s.rect(mx - 1.0, my - 1.0, 1.5, mh + 2.0, 0.0, ink, 0.5);
        s.rect(mx + mw - 0.5, my - 1.0, 1.5, mh + 2.0, 0.0, ink, 0.5);
        let px = mx + ((self.s.jr - MAP.0) / MAP.2) as f32 * mw;
        let py = my + ((self.s.ji - MAP.1) / MAP.3) as f32 * mh;
        let r = 5.0 * self.dpr;
        s.disc(px, py, r * 1.6, hex(0x120f26), 0.5);
        s.ring(px, py, r, 2.0 * self.dpr, ink, 1.0);
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
            sw: 1,
            sh: 1,
            dpr: 1.0,
            cell: 1,
            budget: BUDGET,
            rest: (0.0, 0.0),
            pinch: None,
            picking: false,
            map: Vec::new(),
            map_key: (0, 0, 0),
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
        // spend more while frames keep coming on time, less when they're late
        if !self.r.finished(&self.s.view) {
            self.budget = if dt <= 18 { self.budget + self.budget / 8 } else if dt > 24 { self.budget * 3 / 4 } else { self.budget };
            self.budget = self.budget.clamp(BUDGET / 4, BUDGET * 8);
        }
        let (w, h) = (self.sw as f32, self.sh as f32);
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
        if self.picking {
            if p.held == 0 {
                self.picking = false;
            } else {
                self.pick(x, y);
            }
            self.at = (x, y);
            return 0;
        }
        if p.pressed & wg::M_LEFT != 0 && self.on_map(x, y) {
            self.picking = true;
            self.pick(x, y);
            self.at = (x, y);
            return 0;
        }
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
                    self.pan_by(x - self.sw as f32 / 2.0, y - self.sh as f32 / 2.0);
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
        !self.r.finished(&self.s.view) || self.press.is_some() || self.button != 0 || self.keys != 0 || self.pinch.is_some() || self.picking
    }
    fn draw(&mut self, s: &mut Screen) {
        let (w, h) = s.size_px();
        (self.sw, self.sh) = (w.max(1) as u32, h.max(1) as u32);
        // about one value per CSS pixel, and at most about a million
        let mut cell = self.dpr.round().max(1.0) as u32;
        while (self.sw / cell) * (self.sh / cell) > 1_000_000 {
            cell += 1;
        }
        self.cell = cell;
        self.r.size(self.sw.div_ceil(cell) as i32, self.sh.div_ceil(cell) as i32);
        let julia = self.julia();
        self.r.follow(&self.s.view);
        self.r.refine(&self.s.view, julia, self.budget);
        let colours = self.s.opt[O_COLOURS].clamp(0, N_COLOURS - 1) as usize;
        if self.pal.as_ref().map(|p| p.0) != Some(colours) {
            self.pal = Some((colours, palette(colours)));
        }
        if let Some((c, pal)) = self.pal.take() {
            self.r.paint(s, &pal, self.cell);
            self.draw_map(s, &pal);
            self.pal = Some((c, pal));
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
        let size = (w.min(h) as f32 * 0.035).clamp(13.0 * self.dpr, 18.0 * self.dpr);
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

/// The mouse wheel, in hundredths of a line (down is positive).
#[unsafe(no_mangle)]
pub extern "C" fn wg_wheel(d: i32) {
    WG_GAME.get().game.wheel(d)
}
/// Two fingers: their middle and how far apart, in screen pixels; 0 apart
/// when they lift.
#[unsafe(no_mangle)]
pub extern "C" fn wg_pinch(x: i32, y: i32, d: i32) {
    WG_GAME.get().game.pinch(x as f32, y as f32, d as f32)
}
/// Device pixels per CSS pixel, in hundredths.
#[unsafe(no_mangle)]
pub extern "C" fn wg_dpr(d: i32) {
    WG_GAME.get().game.set_dpr(d)
}
