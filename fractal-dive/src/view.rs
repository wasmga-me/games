//! Where the view looks, and the picture: escape values worked out a budget
//! at a time, coarse to fine, kept per screen pixel.

use wg::Screen;

/// Iterations a frame may spend refining (about 10 ms in a browser).
pub const BUDGET: u64 = 7_000_000;
/// Coarsest block the refining starts from, in screen pixels.
const COARSE: u32 = 16;
/// Deepest zoom: past this, doubles can't tell neighbouring pixels apart.
const MIN_SPAN: f64 = 2e-13;
const MAX_SPAN: f64 = 8.0;
/// Escape radius squared; large, for smooth colours.
const ESCAPE: f64 = 256.0 * 256.0;

/// The point at the middle of the screen and the height the screen shows,
/// in the complex plane.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct View {
    pub cx: f64,
    pub cy: f64,
    pub span: f64,
}

impl View {
    pub fn home(julia: bool) -> View {
        if julia {
            View { cx: 0.0, cy: 0.0, span: 3.0 }
        } else {
            View { cx: -0.6, cy: 0.0, span: 2.6 }
        }
    }
    /// How far in, against the first view.
    pub fn zoom(&self) -> f64 {
        2.6 / self.span
    }
    pub fn valid(&self) -> bool {
        self.cx.is_finite() && self.cy.is_finite() && self.cx.abs() < 4.0 && self.cy.abs() < 4.0 && (MIN_SPAN..=MAX_SPAN).contains(&self.span)
    }
    /// The complex point at screen pixel x, y of a w x h screen.
    pub fn at(&self, x: f64, y: f64, w: u32, h: u32) -> (f64, f64) {
        let k = self.span / h.max(1) as f64;
        (self.cx + (x - w as f64 / 2.0) * k, self.cy + (y - h as f64 / 2.0) * k)
    }
    /// Zooms in by f (out when below 1), keeping screen pixel x, y still.
    pub fn zoom_about(&mut self, x: f64, y: f64, w: u32, h: u32, f: f64) {
        let (px, py) = self.at(x, y, w, h);
        let span = (self.span / f).clamp(MIN_SPAN, MAX_SPAN);
        let g = span / self.span;
        self.cx = px + (self.cx - px) * g;
        self.cy = py + (self.cy - py) * g;
        self.span = span;
        self.keep_near();
    }
    /// Moves the view by dx, dy screen pixels of an h pixel high screen.
    pub fn pan(&mut self, dx: f64, dy: f64, h: u32) {
        let k = self.span / h.max(1) as f64;
        self.cx += dx * k;
        self.cy += dy * k;
        self.keep_near();
    }
    fn keep_near(&mut self) {
        self.cx = self.cx.clamp(-3.0, 3.0);
        self.cy = self.cy.clamp(-3.0, 3.0);
    }
}

/// Smooth escape value at c (Mandelbrot) or from z = c (Julia with constant
/// j), or -1 inside, at most max iterations; adds the iterations spent.
#[inline]
pub fn escape(cr: f64, ci: f64, julia: Option<(f64, f64)>, max: u32, spent: &mut u64) -> f32 {
    let (mut zr, mut zi, kr, ki) = match julia {
        Some((jr, ji)) => (cr, ci, jr, ji),
        None => {
            // inside the main cardioid or the period-2 bulb: no need to iterate
            let xq = cr - 0.25;
            let q = xq * xq + ci * ci;
            if q * (q + xq) <= 0.25 * ci * ci || (cr + 1.0) * (cr + 1.0) + ci * ci <= 0.0625 {
                *spent += 1;
                return -1.0;
            }
            (0.0, 0.0, cr, ci)
        }
    };
    // periodicity: a z seen again means a cycle, so the point is inside
    let (mut pr, mut pi) = (zr, zi);
    let mut period = 8u32;
    let mut i = 0u32;
    let (mut r2, mut i2) = (zr * zr, zi * zi);
    while i < max {
        zi = 2.0 * zr * zi + ki;
        zr = r2 - i2 + kr;
        r2 = zr * zr;
        i2 = zi * zi;
        i += 1;
        if r2 + i2 > ESCAPE {
            *spent += i as u64;
            let log_zn = (r2 + i2).ln() * 0.5;
            let nu = (log_zn / core::f64::consts::LN_2).ln() / core::f64::consts::LN_2;
            return (i as f64 + 1.0 - nu).max(0.0) as f32;
        }
        if (zr - pr).abs() < 1e-15 && (zi - pi).abs() < 1e-15 {
            break;
        }
        if i == period {
            pr = zr;
            pi = zi;
            period *= 2;
        }
    }
    *spent += i as u64;
    -1.0
}

/// Most iterations a pixel gets at this zoom.
pub fn max_iter(v: &View) -> u32 {
    (220.0 + 110.0 * v.zoom().max(1.0).log2()).min(12000.0) as u32
}

/// The picture: per screen pixel, the escape value and the size of the
/// block it was filled from (1 when worked out for this pixel).
pub struct Render {
    pub w: u32,
    pub h: u32,
    vals: Vec<f32>,
    blk: Vec<u8>,
    /// scratch for `follow`, kept to save allocating a screenful each move
    nv: Vec<f32>,
    nb: Vec<u8>,
    cols: Vec<isize>,
    /// the view and set the values belong to
    view: Option<View>,
    julia: Option<(f64, f64)>,
    /// refining: the block size of the pass and the row it reached
    pass: u32,
    row: u32,
    /// the colours need painting again
    dirty: bool,
}

impl Default for Render {
    fn default() -> Self {
        Self::new()
    }
}

impl Render {
    pub fn new() -> Render {
        Render { w: 0, h: 0, vals: Vec::new(), blk: Vec::new(), nv: Vec::new(), nb: Vec::new(), cols: Vec::new(), view: None, julia: None, pass: COARSE, row: 0, dirty: true }
    }
    /// Forget the picture (a new run or a load).
    pub fn reset(&mut self) {
        self.blk.fill(u8::MAX);
        self.restart(None);
    }
    /// Refining starts over from the coarsest pass, for `view`.
    fn restart(&mut self, view: Option<View>) {
        self.view = view;
        self.pass = COARSE;
        self.row = 0;
        self.dirty = true;
    }
    pub fn size(&mut self, w: i32, h: i32) {
        let (w, h) = (w.max(1) as u32, h.max(1) as u32);
        if (w, h) != (self.w, self.h) {
            self.w = w;
            self.h = h;
            self.vals = vec![0.0; (w * h) as usize];
            self.blk = vec![u8::MAX; (w * h) as usize];
            self.restart(None);
        }
    }
    /// Every pixel of `view` worked out and painted.
    pub fn finished(&self, view: &View) -> bool {
        self.view == Some(*view) && self.pass == 0 && !self.dirty
    }
    /// The view moved to `new` (once a frame, however many moves made it):
    /// carry the values over to where they now show, each block as big as
    /// it is now stretched to.
    pub fn follow(&mut self, new: &View) {
        let Some(old) = self.view else {
            return;
        };
        if old == *new || self.w == 0 {
            return;
        }
        let (w, h) = (self.w as usize, self.h as usize);
        // new pixel -> old pixel: scale s and offset
        let s = new.span / old.span;
        let k = old.span / self.h as f64;
        let ox = (new.cx - old.cx) / k + w as f64 / 2.0 - s * w as f64 / 2.0;
        let oy = (new.cy - old.cy) / k + h as f64 / 2.0 - s * h as f64 / 2.0;
        let grow = (1.0 / s).max(1.0);
        // a pan by whole pixels keeps every value as fine as it was; deep
        // in, the centre's rounding is worth part of a pixel, so allow for it
        let slack = (4.0 * f64::EPSILON * (old.cx.abs() + old.cy.abs() + 1.0) / k).clamp(1e-6, 0.5);
        let whole = s == 1.0 && (ox - ox.round()).abs() < slack && (oy - oy.round()).abs() < slack;
        let (ox, oy) = if whole { (ox.round(), oy.round()) } else { (ox, oy) };
        // the old column of each new one, -1 when off the old screen
        self.cols.clear();
        self.cols.extend((0..w).map(|x| {
            let sx = (ox + (x as f64 + 0.5) * s).floor();
            if sx < 0.0 || sx >= w as f64 { -1 } else { sx as isize }
        }));
        let (vals, blk) = (&self.vals, &self.blk);
        let (nv, nb) = (&mut self.nv, &mut self.nb);
        nv.clear();
        nv.resize(w * h, 0.0);
        nb.clear();
        nb.resize(w * h, u8::MAX);
        for y in 0..h {
            let sy = (oy + (y as f64 + 0.5) * s).floor();
            if sy < 0.0 || sy >= h as f64 {
                continue;
            }
            let row = sy as usize * w;
            for (x, &sx) in self.cols.iter().enumerate() {
                if sx < 0 {
                    continue;
                }
                let i = row + sx as usize;
                let b = blk[i];
                if b == u8::MAX {
                    continue;
                }
                nv[y * w + x] = vals[i];
                // a block stretched by zooming counts as coarser, and a
                // value that moved by part of a pixel is worked out again
                nb[y * w + x] = if whole { b } else { ((b as f64 * grow).ceil().min(254.0) as u8).max(2) };
            }
        }
        core::mem::swap(&mut self.vals, &mut self.nv);
        core::mem::swap(&mut self.blk, &mut self.nb);
        self.restart(Some(*new));
    }
    /// Works out values, coarse to fine, until `budget` iterations are spent.
    pub fn refine(&mut self, view: &View, julia: Option<(f64, f64)>, budget: u64) {
        if self.view != Some(*view) || self.julia != julia {
            if self.julia != julia {
                self.blk.fill(u8::MAX);
            }
            self.julia = julia;
            self.restart(Some(*view));
        }
        let (w, h) = (self.w, self.h);
        let max = max_iter(view);
        let k = view.span / h as f64;
        let x0 = view.cx - w as f64 / 2.0 * k;
        let y0 = view.cy - h as f64 / 2.0 * k;
        let mut spent = 0u64;
        while self.pass > 0 {
            let b = self.pass;
            while self.row < h {
                let y = self.row;
                let ci = y0 + (y as f64 + 0.5) * k;
                let mut x = 0;
                while x < w {
                    let i = (y * w + x) as usize;
                    if self.blk[i] > 1 && self.blk[i] as u32 >= b {
                        let v = escape(x0 + (x as f64 + 0.5) * k, ci, julia, max, &mut spent);
                        self.vals[i] = v;
                        self.blk[i] = 1;
                        // fill the block's other pixels that are coarser
                        for yy in y..(y + b).min(h) {
                            let row = (yy * w) as usize;
                            for xx in x..(x + b).min(w) {
                                let j = row + xx as usize;
                                if self.blk[j] as u32 > b {
                                    self.vals[j] = v;
                                    self.blk[j] = b as u8;
                                }
                            }
                        }
                        self.dirty = true;
                    }
                    x += b;
                }
                self.row += b;
                if spent >= budget {
                    return;
                }
            }
            self.pass /= 2;
            self.row = 0;
        }
    }
    /// Paints the values in the palette's colours: every time, as the zoom
    /// buttons and label are drawn over it afresh.
    pub fn paint(&mut self, s: &mut Screen, pal: &Palette) {
        self.dirty = false;
        let (w, h) = (self.w as usize, self.h as usize);
        let (sw, sh) = s.size_px();
        if sw as usize != w || sh as usize != h {
            return;
        }
        for y in 0..h {
            let row = s.row_px(y as i32);
            let vals = &self.vals[y * w..(y + 1) * w];
            let blk = &self.blk[y * w..(y + 1) * w];
            for x in 0..w {
                row[x] = if blk[x] == u8::MAX {
                    pal.inside
                } else {
                    pal.at(vals[x])
                };
            }
        }
    }
}

/// Colours by escape value: a looped gradient, and the inside's colour.
pub struct Palette {
    lut: Vec<u32>,
    inside: u32,
    /// gradient loops per unit of sqrt(escape value)
    density: f32,
}

impl Palette {
    #[inline]
    pub fn at(&self, v: f32) -> u32 {
        if v < 0.0 {
            return self.inside;
        }
        let n = self.lut.len();
        let t = v.sqrt() * self.density;
        let i = ((t - t.floor()) * n as f32) as usize;
        self.lut[i.min(n - 1)]
    }
}

/// Colour themes, in the catalog `setup` order: Ember, Ocean, Neon, Ink.
pub fn palette(i: usize) -> Palette {
    let stops: &[u32] = match i {
        0 => &[0x120f26, 0x5a1030, 0xc2381b, 0xff8a3c, 0xffd166, 0xfff7e8, 0xffc93c, 0x8a2130],
        1 => &[0x061a2e, 0x0b4f6c, 0x2ec4b6, 0xe8fff9, 0xffd166, 0x2a7ab0, 0x0b2d4c],
        2 => &[0x150a33, 0x654ff0, 0xff4fa3, 0xffe066, 0x4fe0d2, 0x2c1a7a],
        _ => &[0xfff7e8, 0x1a1730, 0x8c86a8, 0xfff7e8],
    };
    let inside = match i {
        3 => 0x1a1730,
        _ => 0x05030c,
    };
    let n = 1024;
    let mut lut = Vec::with_capacity(n);
    let k = stops.len();
    for j in 0..n {
        // a closed loop through the stops
        let t = j as f32 / n as f32 * k as f32;
        let a = t.floor() as usize % k;
        let b = (a + 1) % k;
        let f = t - t.floor();
        let f = f * f * (3.0 - 2.0 * f);
        lut.push(wg::mix(wg::hex(stops[b]), wg::hex(stops[a]), f));
    }
    Palette { lut, inside: wg::hex(inside), density: if i == 3 { 0.9 } else { 0.42 } }
}
