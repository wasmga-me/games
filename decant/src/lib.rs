//! Decant - an original wasmga.me game. MIT licensed.
//! Coloured liquids are layered in glass tubes. Pour the top layer of one
//! tube into an empty tube or onto the same colour, until every tube holds
//! one colour. 30 levels, from 3 colours to 10. The page shows the setup,
//! pause and game over screens.
mod glyphs;
mod puzzle;

use puzzle::{Stand, CAP, LEVELS, MAXP};
use wg::{hex, mix, tr, Game, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 1;

/// options, in the catalog `setup` order
const O_LEVEL: usize = 0;
const O_THEME: usize = 1;
const THEMES: usize = 3;

const PLAY: i32 = 0;
const CLEARED: i32 = 1;
const DONE: i32 = 2;

/// moves kept for undo
const HIST: usize = 400;
/// a pour, from lifting the tube to setting it down, ms
const POUR_MS: f32 = 760.0;
/// parts of a pour: tipping over, pouring, going back
const TIP: f32 = 0.28;
const BACK: f32 = 0.78;
/// how high a lifted tube rises
const LIFT: f32 = 14.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 2],
    level: i32,
    phase: i32,
    stand: Stand,
    /// the level as dealt, for Restart
    start: Stand,
    par: i32,
    /// the tube lifted, -1 for none
    sel: i32,
    /// moves as from | to << 4 | layers << 8
    hist: [u16; HIST],
    hist_len: i32,
    moves: i32,
    undos: i32,
    level_ms: i32,
    /// the whole run
    time_ms: i32,
    all_moves: i32,
    all_undos: i32,
    done: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
    /// keyboard cursor, a tube
    cursor: i32,
    rep: KeyRepeat,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}

struct Theme {
    bg: u32,
    bg2: u32,
    /// the glass, and the inside of an empty tube
    glass: u32,
    inside: u32,
    shelf: u32,
    shelf_lo: u32,
    text: u32,
    dim: u32,
    button: u32,
    overlay: u32,
}

const THEME: [Theme; THEMES] = [
    // Night
    Theme { bg: 0x1c2238, bg2: 0x0f1220, glass: 0xc9d3f2, inside: 0x252c47, shelf: 0x3a4263, shelf_lo: 0x262c45, text: 0xf2f4fb, dim: 0x8f97b8, button: 0x2c3352, overlay: 0x0b0d16 },
    // Paper
    Theme { bg: 0xf6f1e6, bg2: 0xe7dfcc, glass: 0x8a96a8, inside: 0xe8e2d4, shelf: 0xb89a74, shelf_lo: 0x8f7354, text: 0x3a2f24, dim: 0x8a7a66, button: 0xe4d9c4, overlay: 0x5a4a38 },
    // Lab
    Theme { bg: 0x2b3440, bg2: 0x1d232b, glass: 0xd8e4ec, inside: 0x323d4a, shelf: 0x56606e, shelf_lo: 0x3d4652, text: 0xeef2f6, dim: 0x9aa6b4, button: 0x3c4654, overlay: 0x14181e },
];

/// liquid colours, told apart by lightness as well as hue
const LIQUID: [u32; 10] = [
    0xe5384a, 0xffc22e, 0x3b5fe0, 0x2f9e44, 0xf7731a, 0x8e4ec6, 0x14b0aa, 0xe05ba6, 0xb5d334, 0x7a4a2a,
];

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    None,
    Tube(i32),
    Undo,
    Restart,
    Next,
}

/// a pour in progress; the save already has its result
#[derive(Clone, Copy)]
struct Pour {
    from: usize,
    to: usize,
    k: usize,
    colour: u8,
    t: f32,
}

/// A tube's place: the middle of its mouth, its inner half width and its
/// inner length, mouth to the bottom of the curve.
#[derive(Clone, Copy)]
struct Slot {
    x: f32,
    y: f32,
    hw: f32,
    len: f32,
}

type Pt = (f32, f32);

/// the tube's outline, mouth at (mx, my), tipped by phi (radians, positive
/// tips the mouth to the right)
fn tube_poly(mx: f32, my: f32, hw: f32, len: f32, phi: f32) -> Vec<Pt> {
    let (c, s) = (phi.cos(), phi.sin());
    let rot = |x: f32, y: f32| (mx + x * c - y * s, my + x * s + y * c);
    let mut pts = vec![rot(-hw, 0.0), rot(-hw, len - hw)];
    let segs = 14;
    for i in 1..segs {
        let a = std::f32::consts::PI * (1.0 - i as f32 / segs as f32);
        pts.push(rot(hw * a.cos(), len - hw + hw * a.sin()));
    }
    pts.push(rot(hw, len - hw));
    pts.push(rot(hw, 0.0));
    pts
}

/// A filled convex polygon of any number of points, smoothed at its edges
/// (Screen::poly takes up to 8).
fn fill(s: &mut Screen, pts: &[Pt], c: u32, alpha: f32) {
    let n = pts.len();
    if n < 3 || alpha <= 0.0 {
        return;
    }
    let px: Vec<Pt> = pts.iter().map(|p| s.px(p.0, p.1)).collect();
    let mut area = 0.0;
    for k in 0..n {
        let (a, b) = (px[k], px[(k + 1) % n]);
        area += a.0 * b.1 - b.0 * a.1;
    }
    let sign = if area > 0.0 { 1.0 } else { -1.0 };
    // each edge as its outward normal and offset
    let mut edges = Vec::with_capacity(n);
    let (mut minx, mut miny, mut maxx, mut maxy) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for k in 0..n {
        let (a, b) = (px[k], px[(k + 1) % n]);
        let (ex, ey) = (b.0 - a.0, b.1 - a.1);
        let l = (ex * ex + ey * ey).sqrt();
        if l < 1e-4 {
            continue;
        }
        let (nx, ny) = (ey / l * sign, -ex / l * sign);
        edges.push((nx, ny, nx * a.0 + ny * a.1));
        minx = minx.min(a.0);
        maxx = maxx.max(a.0);
        miny = miny.min(a.1);
        maxy = maxy.max(a.1);
    }
    let (w, h) = s.size_px();
    let (x0, x1) = (((minx - 1.0).floor() as i32).max(0), ((maxx + 1.0).ceil() as i32).min(w));
    let (y0, y1) = (((miny - 1.0).floor() as i32).max(0), ((maxy + 1.0).ceil() as i32).min(h));
    for j in y0..y1 {
        let py = j as f32 + 0.5;
        let row = s.row_px(j);
        for i in x0..x1 {
            let pxx = i as f32 + 0.5;
            // outside a convex shape is outside its furthest edge
            let d = edges.iter().fold(f32::MIN, |d, e| d.max(e.0 * pxx + e.1 * py - e.2));
            let cov = (0.5 - d).clamp(0.0, 1.0);
            if cov > 0.0 {
                wg::blend(&mut row[i as usize], c, (cov * alpha * 256.0) as u32);
            }
        }
    }
}

/// the part of a convex polygon below the line y (below = larger y) or above it
fn clip(poly: &[Pt], y: f32, below: bool) -> Vec<Pt> {
    let inside = |p: &Pt| if below { p.1 >= y } else { p.1 <= y };
    let mut out = Vec::with_capacity(poly.len() + 2);
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        let (ia, ib) = (inside(&a), inside(&b));
        if ia {
            out.push(a);
        }
        if ia != ib {
            let t = (y - a.1) / (b.1 - a.1);
            out.push((a.0 + (b.0 - a.0) * t, y));
        }
    }
    out
}

fn area(poly: &[Pt]) -> f32 {
    let mut a = 0.0;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        a += p.0 * q.1 - q.0 * p.1;
    }
    (a / 2.0).abs()
}

/// the height a volume of liquid fills a tube to: its surface, a y
fn level(poly: &[Pt], vol: f32) -> f32 {
    let lo = poly.iter().map(|p| p.1).fold(f32::MAX, f32::min);
    let hi = poly.iter().map(|p| p.1).fold(f32::MIN, f32::max);
    let (mut a, mut b) = (lo, hi);
    for _ in 0..22 {
        let m = (a + b) / 2.0;
        if area(&clip(poly, m, true)) > vol { a = m } else { b = m }
    }
    (a + b) / 2.0
}

/// one layer's volume, for a tube of this size
fn unit(hw: f32, len: f32) -> f32 {
    // the tube holds CAP layers with a little room left at the top
    let full = area(&tube_poly(0.0, 0.0, hw, len, 0.0));
    full / (CAP as f32 + 0.75)
}

struct Decant {
    s: Save,
    tall: bool,
    hover: Hit,
    keys: bool,
    pour: Option<Pour>,
    /// a refused pour shakes this tube, ms left
    shake: (i32, f32),
    /// tubes just finished shine, ms left
    glow: [f32; MAXP],
    /// the accessible board's focus is on Undo (0) or Restart (1), -1 on a tube
    on_button: i32,
}

impl Decant {
    fn theme(&self) -> &'static Theme {
        &THEME[self.s.opt[O_THEME] as usize]
    }
    fn plan_wh(&self) -> (f32, f32) {
        if self.tall { (300.0, 400.0) } else { (400.0, 300.0) }
    }
    fn tubes(&self) -> usize {
        self.s.stand.n as usize
    }
    /// tubes in the first row, and rows
    fn rows(&self) -> (usize, usize) {
        let n = self.tubes();
        let most = if self.tall { 6 } else { 7 };
        if n <= most { (n, 1) } else { ((n + 1) / 2, 2) }
    }
    fn colw(&self) -> f32 {
        (self.plan_wh().0 - 24.0) / self.rows().0 as f32
    }
    fn slot(&self, p: usize) -> Slot {
        let (w, h) = self.plan_wh();
        let (first, rows) = self.rows();
        let n = self.tubes();
        let (row, i, count) = if p < first { (0, p, first) } else { (1, p - first, n - first) };
        // the tubes stand between the top bar and the buttons
        let (top, bottom) = (40.0, h - 58.0);
        let band = (bottom - top) / rows as f32;
        let colw = self.colw();
        // slim enough to stand upright in a short row
        let room = band - 26.0 - LIFT;
        let hw = (colw * 0.26).min(if self.tall { 21.0 } else { 17.0 }).min(room / 7.4);
        let len = room.min(hw * 2.0 * 4.6);
        let x = w / 2.0 + (i as f32 - (count as f32 - 1.0) / 2.0) * colw;
        // short tubes stand in the middle of their band, not on its floor
        let spare = (room - len) / 2.0;
        let floor = top + band * (row as f32 + 1.0) - 8.0 - spare;
        Slot { x, y: floor - len - 4.0, hw, len }
    }
    /// the boxes of Undo and Restart
    fn buttons(&self) -> [(f32, f32, f32, f32); 2] {
        let (w, h) = self.plan_wh();
        let (bw, bh) = (110.0, 34.0);
        let y = h - bh - 12.0;
        [(w / 2.0 - bw - 6.0, y, bw, bh), (w / 2.0 + 6.0, y, bw, bh)]
    }
    /// the level cleared card, in the middle of the screen
    fn card(&self) -> (f32, f32, f32, f32) {
        let (w, h) = self.plan_wh();
        let (pw, ph) = (248.0, if self.s.phase == CLEARED { 158.0 } else { 96.0 });
        (w / 2.0 - pw / 2.0, h / 2.0 - ph / 2.0, pw, ph)
    }
    /// its button, inside the card along the bottom
    fn next_box(&self) -> (f32, f32, f32, f32) {
        let (px, py, pw, ph) = self.card();
        let (bw, bh) = (pw - 48.0, 42.0);
        (px + (pw - bw) / 2.0, py + ph - bh - 20.0, bw, bh)
    }
    fn hit(&self, x: f32, y: f32) -> Hit {
        let inside = |(bx, by, bw, bh): (f32, f32, f32, f32)| x >= bx && x < bx + bw && y >= by && y < by + bh;
        if self.s.phase != PLAY {
            return if self.s.phase == CLEARED && inside(self.next_box()) { Hit::Next } else { Hit::None };
        }
        let [u, r] = self.buttons();
        if inside(u) {
            return Hit::Undo;
        }
        if inside(r) {
            return Hit::Restart;
        }
        let half = self.colw() / 2.0;
        for p in 0..self.tubes() {
            let sl = self.slot(p);
            if (x - sl.x).abs() < half && y > sl.y - LIFT - 8.0 && y < sl.y + sl.len + 12.0 {
                return Hit::Tube(p as i32);
            }
        }
        Hit::None
    }

    fn begin(&mut self, level: i32) {
        let (st, par) = puzzle::deal(level);
        let s = &mut self.s;
        s.level = level;
        s.phase = PLAY;
        s.stand = st;
        s.start = st;
        s.par = par;
        s.sel = -1;
        s.hist_len = 0;
        s.moves = 0;
        s.undos = 0;
        s.level_ms = 0;
        s.cursor = s.cursor.clamp(0, st.n - 1);
        self.pour = None;
        self.glow = [0.0; MAXP];
    }

    fn tap(&mut self, p: i32) -> u32 {
        if self.s.phase != PLAY || p < 0 {
            return 0;
        }
        // one pour at a time
        if self.pour.is_some() {
            return 0;
        }
        let sel = self.s.sel;
        if sel < 0 {
            if self.s.stand.len(p as usize) > 0 {
                self.s.sel = p;
                return wg::EV_BLIP;
            }
            return 0;
        }
        if sel == p {
            self.s.sel = -1;
            return 0;
        }
        let (a, b) = (sel as usize, p as usize);
        let k = self.s.stand.can(a, b);
        if k == 0 {
            // pick that tube up instead when it can't take this colour
            if self.s.stand.len(b) > 0 && self.s.stand.top(b) != self.s.stand.top(a) {
                self.s.sel = p;
                return wg::EV_BLIP;
            }
            self.shake = (sel, 300.0);
            self.s.sel = -1;
            return wg::EV_HIT;
        }
        let colour = self.s.stand.top(a);
        self.s.stand.shift(a, b, k);
        self.pour = Some(Pour { from: a, to: b, k, colour, t: 0.0 });
        self.s.sel = -1;
        self.s.moves += 1;
        self.s.all_moves += 1;
        if self.s.hist_len as usize == HIST {
            self.s.hist.copy_within(1.., 0);
            self.s.hist_len -= 1;
        }
        self.s.hist[self.s.hist_len as usize] = pack(a, b, k);
        self.s.hist_len += 1;
        wg::EV_BLIP
    }

    fn clear_level(&mut self) -> u32 {
        let s = &mut self.s;
        let c = puzzle::colours(s.level) as i32;
        let score = (100 * c + 20 * (s.par - s.moves) - 40 * s.undos).max(50 * c);
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

    fn undo(&mut self) -> u32 {
        if self.s.phase != PLAY || self.s.hist_len == 0 || self.pour.is_some() {
            return 0;
        }
        self.s.hist_len -= 1;
        let (a, b, k) = unpack(self.s.hist[self.s.hist_len as usize]);
        self.s.stand.shift(b, a, k);
        self.s.sel = -1;
        self.s.undos += 1;
        self.s.all_undos += 1;
        wg::EV_BLIP
    }

    fn restart(&mut self) -> u32 {
        if self.s.phase != PLAY || self.s.hist_len == 0 || self.pour.is_some() {
            return 0;
        }
        self.s.stand = self.s.start;
        self.s.undos += self.s.hist_len;
        self.s.all_undos += self.s.hist_len;
        self.s.hist_len = 0;
        self.s.sel = -1;
        wg::EV_BLIP
    }

    fn next(&mut self) -> u32 {
        if self.s.phase != CLEARED {
            return 0;
        }
        self.begin(self.s.level + 1);
        wg::EV_BLIP
    }

    fn act(&mut self, h: Hit) -> u32 {
        match h {
            Hit::Tube(p) => self.tap(p),
            Hit::Undo => self.undo(),
            Hit::Restart => self.restart(),
            Hit::Next => self.next(),
            Hit::None => 0,
        }
    }

    /// moves the keyboard cursor; up and down go to the other row
    fn cursor_move(&mut self, dx: i32, dy: i32) {
        let n = self.tubes() as i32;
        let (first, rows) = self.rows();
        let first = first as i32;
        let mut c = self.s.cursor + dx;
        if dy != 0 && rows == 2 {
            c = if c < first { (c + first).min(n - 1) } else { c - first };
        }
        self.s.cursor = c.clamp(0, n - 1);
    }

    /// Where a tube is drawn and how far it is tipped. The tube that pours
    /// rises, moves over the one it pours into and tips toward it.
    fn pose(&self, p: usize) -> (f32, f32, f32) {
        let sl = self.slot(p);
        let mut rest = (sl.x, sl.y, 0.0);
        let (shaking, ms) = self.shake;
        if shaking == p as i32 && ms > 0.0 {
            rest.0 += (ms * 0.09).sin() * 4.0 * ms / 300.0;
        }
        if self.s.sel == p as i32 {
            rest.1 -= LIFT;
        }
        let Some(f) = self.pour.filter(|f| f.from == p) else {
            return rest;
        };
        let to = self.slot(f.to);
        // tipped further the less is left in it
        let left = self.s.stand.len(p) as f32 / CAP as f32;
        let tip = 1.95 - 0.4 * left;
        // from its own side, unless the tube would stick out of the screen there
        let mut dir = if to.x >= sl.x { 1.0 } else { -1.0 };
        let reach = sl.len * tip.sin() + sl.hw * 2.0;
        let mouth = |dir: f32| to.x - dir * (sl.hw * 0.9 + to.hw * 0.4);
        let w = self.plan_wh().0;
        if !(0.0..=w).contains(&(mouth(dir) - dir * reach)) {
            dir = -dir;
        }
        let over = (mouth(dir), to.y - 10.0, dir * tip);
        let m = if f.t < TIP {
            wg::ease_out(f.t / TIP)
        } else if f.t < BACK {
            1.0
        } else {
            1.0 - wg::ease_out((f.t - BACK) / (1.0 - BACK))
        };
        let from = (sl.x, sl.y - LIFT, 0.0);
        (from.0 + (over.0 - from.0) * m, from.1 + (over.1 - from.1) * m, from.2 + (over.2 - from.2) * m)
    }

    /// how much of the pour has run, 0..1
    fn poured(&self) -> f32 {
        match self.pour {
            Some(f) => ((f.t - TIP) / (BACK - TIP - 0.06)).clamp(0.0, 1.0),
            None => 1.0,
        }
    }

    /// the layers in tube p as drawn, bottom first, in layers (a pour moves
    /// them a little at a time); neighbours of one colour run together
    fn layers(&self, p: usize) -> Vec<(u8, f32)> {
        let st = &self.s.stand;
        let mut out: Vec<(u8, f32)> = Vec::new();
        let mut add = |c: u8, v: f32| {
            if v <= 0.0 {
                return;
            }
            match out.last_mut() {
                Some(l) if l.0 == c => l.1 += v,
                _ => out.push((c, v)),
            }
        };
        let len = st.len(p);
        let (mut keep, mut extra) = (len, 0.0);
        let mut colour = 0;
        if let Some(f) = self.pour {
            let g = self.poured();
            if f.to == p {
                keep = len - f.k;
                extra = f.k as f32 * g;
                colour = f.colour;
            } else if f.from == p {
                extra = f.k as f32 * (1.0 - g);
                colour = f.colour;
            }
        }
        for i in 0..keep {
            add(st.tube[p][i], 1.0);
        }
        add(colour, extra);
        out
    }

    fn draw_tube(&self, s: &mut Screen, p: usize, th: &Theme, (mx, my, phi): (f32, f32, f32)) {
        let sl = self.slot(p);
        let (hw, len) = (sl.hw, sl.len);
        let glass_w = (hw * 0.16).max(1.6);
        let outer = tube_poly(mx, my, hw + glass_w, len + glass_w, phi);
        let inner = tube_poly(mx, my, hw, len, phi);
        if phi == 0.0 {
            // a soft shadow on the shelf
            s.shadow(mx - hw, my + len - 4.0, hw * 2.0, 8.0, 4.0, 5.0, 0, 0.25);
        }
        if self.glow[p] > 0.0 {
            let a = self.glow[p] / 800.0;
            let c = hex(LIQUID[self.s.stand.tube[p][0].max(1) as usize - 1]);
            s.shadow(mx - hw - 2.0, my, hw * 2.0 + 4.0, len, hw, 12.0, c, 0.7 * a);
        }
        fill(s, &outer, hex(th.glass), 0.55);
        fill(s, &inner, hex(th.inside), 1.0);
        // the liquid, each layer up to its own level
        let u = unit(hw, len);
        let mut vol = 0.0;
        let mut below = f32::MAX;
        let mut layers = self.layers(p);
        // tipped past level, the layer by the mouth is the lowest
        if phi.abs() > std::f32::consts::FRAC_PI_2 {
            layers.reverse();
        }
        let count = layers.len();
        for (i, &(c, v)) in layers.iter().enumerate() {
            vol += v * u;
            let top = level(&inner, vol);
            let band = clip(&clip(&inner, top, true), below, false);
            let col = hex(LIQUID[c as usize - 1]);
            fill(s, &band, col, 1.0);
            if i + 1 == count {
                // the surface, seen a little from above
                let skin = clip(&clip(&inner, top, true), top + (hw * 0.22).max(1.5), false);
                if skin.len() >= 3 {
                    fill(s, &skin, mix(hex(0xffffff), col, 0.35), 1.0);
                }
            }
            below = top;
        }
        // the glass catches the light down one side
        let (c, sn) = (phi.cos(), phi.sin());
        let at = |x: f32, y: f32| (mx + x * c - y * sn, my + x * sn + y * c);
        let (a, b) = (at(-hw * 0.55, hw * 0.9), at(-hw * 0.55, len - hw * 1.3));
        s.line(a.0, a.1, b.0, b.1, (hw * 0.13).max(1.0), hex(0xffffff), 0.35);
        let (a, b) = (at(hw * 0.62, hw * 1.4), at(hw * 0.62, len * 0.45));
        s.line(a.0, a.1, b.0, b.1, (hw * 0.07).max(0.7), hex(0xffffff), 0.2);
        // and the far side is in shade
        let (a, b) = (at(hw * 0.78, hw * 0.6), at(hw * 0.78, len - hw * 1.1));
        s.line(a.0, a.1, b.0, b.1, (hw * 0.16).max(1.0), hex(0x000000), 0.14);
        // the lip
        let (l, r) = (at(-hw - glass_w * 1.6, 0.0), at(hw + glass_w * 1.6, 0.0));
        s.line(l.0, l.1, r.0, r.1, glass_w * 1.1, hex(th.glass), 0.9);
    }

    /// the stream from a tipped tube's lip down to the liquid below
    fn draw_stream(&self, s: &mut Screen, f: &Pour) {
        let g = self.poured();
        if f.t <= TIP || f.t >= BACK || g >= 1.0 {
            return;
        }
        let (mx, my, phi) = self.pose(f.from);
        let sl = self.slot(f.from);
        // the lip corner that hangs lowest
        let x = if phi > 0.0 { sl.hw } else { -sl.hw };
        let (c, sn) = (phi.cos(), phi.sin());
        let lip = (mx + x * c, my + x * sn);
        let to = self.slot(f.to);
        let inner = tube_poly(to.x, to.y, to.hw, to.len, 0.0);
        let vol: f32 = self.layers(f.to).iter().map(|l| l.1).sum::<f32>() * unit(to.hw, to.len);
        let surface = level(&inner, vol);
        let col = hex(LIQUID[f.colour as usize - 1]);
        let w = sl.hw * 0.26 * (1.0 - g * 0.4);
        // it falls in a slight curve toward the middle of the tube
        let steps = 8;
        let mut prev = lip;
        for i in 1..=steps {
            let t = i as f32 / steps as f32;
            let x = lip.0 + (to.x - lip.0) * (1.0 - (1.0 - t) * (1.0 - t));
            let y = lip.1 + (surface - lip.1) * t;
            s.line(prev.0, prev.1, x, y, w, col, 1.0);
            prev = (x, y);
        }
    }
}

impl Game for Decant {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS, THEMES as i32];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let st = Stand::empty(5);
        Decant {
            s: Save {
                version: VERSION,
                opt: [0, 0],
                level: 0,
                phase: PLAY,
                stand: st,
                start: st,
                par: 0,
                sel: -1,
                hist: [0; HIST],
                hist_len: 0,
                moves: 0,
                undos: 0,
                level_ms: 0,
                time_ms: 0,
                all_moves: 0,
                all_undos: 0,
                done: 0,
                score: 0,
                cleared: -1,
                cleared_score: 0,
                cursor: 0,
                rep: KeyRepeat(0),
            },
            tall: false,
            hover: Hit::None,
            keys: false,
            pour: None,
            shake: (-1, 0.0),
            glow: [0.0; MAXP],
            on_button: -1,
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
        self.s = Save { opt, ..Decant::new().s };
        self.begin(opt[O_LEVEL]);
        self.keys = false;
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        if h as f32 > w as f32 * 1.1 { (300.0, 400.0) } else { (400.0, 300.0) }
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        if let Some(f) = &mut self.pour {
            f.t += dt / POUR_MS;
            if f.t >= 1.0 {
                // a tube just finished shines
                let to = f.to;
                if self.s.stand.done(to) {
                    self.glow[to] = 800.0;
                }
                self.pour = None;
            }
        }
        // the level clears once its last pour lands, or on loading a save
        // made while it was pouring
        if self.s.phase == PLAY && self.pour.is_none() && self.s.stand.solved() {
            ev |= self.clear_level();
        }
        if self.shake.1 > 0.0 {
            self.shake.1 = (self.shake.1 - dt).max(0.0);
        }
        for g in self.glow.iter_mut() {
            *g = (*g - dt).max(0.0);
        }
        if self.s.phase == PLAY && (self.s.moves > 0 || self.s.undos > 0) {
            self.s.level_ms = self.s.level_ms.saturating_add(input.dt);
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        let dirs = self.s.rep.dirs(input);
        if dirs != 0 {
            self.keys = true;
            self.hover = Hit::None;
        }
        if self.s.phase == PLAY {
            if dirs & wg::LEFT != 0 { self.cursor_move(-1, 0); }
            if dirs & wg::RIGHT != 0 { self.cursor_move(1, 0); }
            if dirs & (wg::UP | wg::DOWN) != 0 { self.cursor_move(0, 1); }
            if input.pressed & wg::A != 0 {
                self.keys = true;
                ev |= self.tap(self.s.cursor);
            }
            if input.pressed & wg::B != 0 {
                ev |= self.undo();
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
        if p.pressed & wg::M_LEFT != 0 {
            self.keys = false;
            if let Hit::Tube(q) = h {
                self.s.cursor = q;
            }
            return self.act(h);
        }
        0
    }

    fn picture(&mut self) -> u32 {
        let s = &self.s;
        let hv = match self.hover {
            Hit::None => -1,
            Hit::Tube(p) => p,
            Hit::Undo => 100,
            Hit::Restart => 101,
            Hit::Next => 102,
        };
        let shown = [hv, self.keys as i32, self.tall as i32];
        let mut c = *s;
        c.level_ms = 0;
        c.time_ms = 0;
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        self.pour.is_some() || self.shake.1 > 0.0 || self.glow.iter().any(|&g| g > 0.0)
    }

    fn draw(&mut self, s: &mut Screen) {
        self.tall = s.ph > s.pw;
        let th = self.theme();
        let w = self.plan_wh().0;
        let n = self.tubes();
        let key = wg::hash(0, wg::bytes_of(&[self.s.opt[O_THEME], n as i32, self.tall as i32]));
        let slots: Vec<Slot> = (0..n).map(|p| self.slot(p)).collect();
        let (first, rows) = self.rows();
        // the room and a shelf per row, drawn once
        s.background(key, |s| {
            let (x0, y0, x1, y1) = s.visible();
            s.rect_grad(x0, y0, x1 - x0, y1 - y0, 0.0, hex(th.bg), hex(th.bg2), 1.0);
            for row in 0..rows {
                let (a, b) = if row == 0 { (0, first) } else { (first, n) };
                let (l, r) = (slots[a].x - slots[a].hw * 2.6, slots[b - 1].x + slots[b - 1].hw * 2.6);
                let y = slots[a].y + slots[a].len + 4.0;
                s.shadow(l, y + 3.0, r - l, 9.0, 4.0, 6.0, 0, 0.35);
                s.rect(l, y, r - l, 9.0, 4.0, hex(th.shelf_lo), 1.0);
                s.rect(l, y, r - l, 6.0, 3.0, hex(th.shelf), 1.0);
            }
        });

        let pouring = self.pour.map(|f| f.from);
        for p in 0..n {
            let sl = slots[p];
            let marked = self.hover == Hit::Tube(p as i32) || (self.keys && self.s.cursor == p as i32 && self.s.phase == PLAY);
            if marked {
                let y = sl.y + sl.len + 16.0;
                s.rect(sl.x - sl.hw * 1.4, y, sl.hw * 2.8, 3.0, 1.5, hex(th.text), if self.keys { 0.85 } else { 0.4 });
            }
            if pouring != Some(p) {
                self.draw_tube(s, p, th, self.pose(p));
            }
        }
        if let Some(f) = self.pour {
            // the stream lands behind the tipped tube's lip, in front of the other
            self.draw_stream(s, &f);
            self.draw_tube(s, f.from, th, self.pose(f.from));
        }

        // the top bar
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let moves_l = tr(&["Moves", "步数", "手数", "이동", "Movimientos", "Movimentos", "Coups", "Züge", "Ходы", "步數"]);
        s.text(&format!("{} {}", level_l, self.s.level + 1), 14.0, 20.0, 17.0, hex(th.text), 1.0);
        let right = format!("{} {}", moves_l, self.s.moves);
        s.text_r(&right, w - 14.0, 20.0, 15.0, hex(th.dim), 1.0);

        // the buttons
        let labels = [
            tr(&["Undo", "撤销", "元に戻す", "되돌리기", "Deshacer", "Desfazer", "Annuler", "Zurück", "Отменить", "復原"]),
            tr(&["Restart", "重来", "やり直す", "다시 시작", "Reiniciar", "Reiniciar", "Recommencer", "Neu starten", "Заново", "重來"]),
        ];
        let stuck = self.s.phase == PLAY && self.pour.is_none() && !self.s.stand.any_move();
        for (i, b) in self.buttons().iter().enumerate() {
            let on = self.s.hist_len > 0 && self.s.phase == PLAY;
            let hv = self.hover == if i == 0 { Hit::Undo } else { Hit::Restart };
            let fill = if hv && on { mix(hex(th.button), hex(th.text), 0.12) } else { hex(th.button) };
            if stuck && i == 0 {
                s.shadow(b.0, b.1, b.2, b.3, b.3 / 2.0, 7.0, hex(th.text), 0.45);
            }
            s.rect(b.0, b.1, b.2, b.3, b.3 / 2.0, fill, 1.0);
            let size = s.fit_size(labels[i], 15.0, b.2 - 16.0);
            s.text_c(labels[i], b.0 + b.2 / 2.0, b.1 + b.3 / 2.0, size, hex(th.text), if on { 1.0 } else { 0.4 });
        }
        if stuck {
            let msg = tr(&["No moves left", "无路可走", "注げる管がありません", "더 이상 부을 수 없습니다", "No quedan movimientos", "Sem jogadas", "Plus aucun coup", "Kein Zug mehr möglich", "Ходов нет", "無路可走"]);
            let y = self.buttons()[0].1 - 10.0;
            s.text_c(msg, w / 2.0, y, 13.0, hex(th.text), 0.9);
        }

        if self.s.phase != PLAY {
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
            1 => s.all_moves,
            2 => s.all_undos,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION
            && s.opt.iter().zip(Self::CHOICES).all(|(&v, &c)| (0..c).contains(&v))
            && (0..LEVELS).contains(&s.level)
            && (PLAY..=DONE).contains(&s.phase)
            && s.stand.valid(puzzle::colours(s.level))
            && s.start.valid(puzzle::colours(s.level))
            && s.stand.n == s.start.n
            && (-1..s.stand.n).contains(&s.sel)
            && (0..=HIST as i32).contains(&s.hist_len)
            && (0..s.stand.n.max(1)).contains(&s.cursor)
            && undoable(&s.stand, &s.hist[..s.hist_len as usize]);
        self.pour = None;
        self.shake = (-1, 0.0);
        self.glow = [0.0; MAXP];
        self.hover = Hit::None;
        ok
    }
}

impl Decant {
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
        let moves_l = tr(&["Moves", "步数", "手数", "이동", "Movimientos", "Movimentos", "Coups", "Züge", "Ходы", "步數"]);
        let line = format!("{} +{}    {} {}", score_l, self.s.cleared_score, moves_l, self.s.moves);
        let size = s.fit_size(&line, 14.0, pw - 32.0);
        s.text_c(&line, w / 2.0, py + 64.0, size, hex(th.dim), 1.0);
        if self.s.phase == CLEARED {
            let b = self.next_box();
            let label = tr(&["Next level", "下一关", "次のレベル", "다음 레벨", "Siguiente nivel", "Próximo nível", "Niveau suivant", "Nächstes Level", "Следующий уровень", "下一關"]);
            let hv = self.hover == Hit::Next;
            let fill = hex(LIQUID[2]);
            s.rect(b.0, b.1, b.2, b.3, b.3 / 2.0, if hv { mix(hex(0xffffff), fill, 0.15) } else { fill }, 1.0);
            let size = s.fit_size(label, 16.0, b.2 - 16.0);
            s.text_c(label, b.0 + b.2 / 2.0, b.1 + b.3 / 2.0, size, hex(0xffffff), 1.0);
        }
    }
}

/// a pour in the undo history
fn pack(from: usize, to: usize, k: usize) -> u16 {
    (from | to << 4 | k << 8) as u16
}
fn unpack(m: u16) -> (usize, usize, usize) {
    let m = m as usize;
    (m & 15, m >> 4 & 15, m >> 8 & 7)
}

/// whether every pour in the history can be taken back, last first
fn undoable(st: &Stand, hist: &[u16]) -> bool {
    let mut st = *st;
    let n = st.n as usize;
    hist.iter().rev().all(|&m| {
        let (a, b, k) = unpack(m);
        let ok = a < n && b < n && a != b && (1..=CAP).contains(&k) && st.len(b) >= k && st.len(a) + k <= CAP;
        if ok {
            st.shift(b, a, k);
        }
        ok
    })
}

/// the liquids' names, LIQUID order
const NAMES: [[&str; 10]; 10] = [
    ["red", "红", "赤", "빨강", "rojo", "vermelho", "rouge", "Rot", "красный", "紅"],
    ["yellow", "黄", "黄", "노랑", "amarillo", "amarelo", "jaune", "Gelb", "жёлтый", "黃"],
    ["blue", "蓝", "青", "파랑", "azul", "azul", "bleu", "Blau", "синий", "藍"],
    ["green", "绿", "緑", "초록", "verde", "verde", "vert", "Grün", "зелёный", "綠"],
    ["orange", "橙", "オレンジ", "주황", "naranja", "laranja", "orange", "Orange", "оранжевый", "橙"],
    ["purple", "紫", "紫", "보라", "morado", "roxo", "violet", "Lila", "фиолетовый", "紫"],
    ["teal", "青绿", "青緑", "청록", "turquesa", "turquesa", "turquoise", "Türkis", "бирюзовый", "青綠"],
    ["pink", "粉", "ピンク", "분홍", "rosa", "rosa", "rose", "Rosa", "розовый", "粉"],
    ["lime", "黄绿", "黄緑", "연두", "lima", "lima", "citron vert", "Hellgrün", "салатовый", "黃綠"],
    ["brown", "棕", "茶", "갈색", "marrón", "marrom", "marron", "Braun", "коричневый", "棕"],
];

/// The accessible board: one square per tube in a row, then Undo and
/// Restart, each laid over where it is drawn.
impl wg::Board for Decant {
    fn grid(&self) -> Option<(i32, i32)> {
        Some((self.s.stand.n + 2, 1))
    }
    fn cursor(&self) -> (i32, i32) {
        (if self.on_button >= 0 { self.s.stand.n + self.on_button } else { self.s.cursor }, 0)
    }
    fn focus(&mut self, x: i32, _y: i32) {
        let n = self.s.stand.n;
        self.on_button = if x >= n { (x - n).min(1) } else { -1 };
        if x < n {
            self.s.cursor = x.max(0);
        }
        self.keys = true;
        self.hover = Hit::None;
    }
    fn rect(&self) -> (f32, f32, f32) {
        (0.0, 0.0, 1.0)
    }
    fn cell_box(&self, x: i32, _y: i32) -> Option<(f32, f32, f32, f32)> {
        let n = self.s.stand.n;
        if x >= n {
            return Some(self.buttons()[(x - n).min(1) as usize]);
        }
        let sl = self.slot(x.max(0) as usize);
        let cw = self.colw();
        Some((sl.x - cw / 2.0, sl.y - LIFT - 8.0, cw, sl.len + LIFT + 20.0))
    }
    fn cell(&self, x: i32, _y: i32, out: &mut String) {
        let n = self.s.stand.n;
        if x >= n {
            out.push_str(if x == n {
                tr(&["Undo", "撤销", "元に戻す", "되돌리기", "Deshacer", "Desfazer", "Annuler", "Zurück", "Отменить", "復原"])
            } else {
                tr(&["Restart", "重来", "やり直す", "다시 시작", "Reiniciar", "Reiniciar", "Recommencer", "Neu starten", "Заново", "重來"])
            });
            return;
        }
        let p = x as usize;
        out.push_str(&tr(&["Tube #", "试管 #", "管 #", "관 #", "Tubo #", "Tubo #", "Tube #", "Röhre #", "Пробирка #", "試管 #"]).replace('#', &(x + 1).to_string()));
        out.push_str(": ");
        let len = self.s.stand.len(p);
        if len == 0 {
            out.push_str(tr(&["empty", "空", "空", "비어 있음", "vacío", "vazio", "vide", "leer", "пусто"]));
        }
        for (i, &c) in self.s.stand.tube[p][..len].iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(tr(&NAMES[c as usize - 1]));
        }
        if self.s.stand.done(p) {
            out.push_str(", ");
            out.push_str(tr(&["done", "完成", "完成", "완성", "completo", "completo", "terminé", "fertig", "готово"]));
        }
        if self.s.sel == x {
            out.push_str(", ");
            out.push_str(tr(&["lifted", "已拿起", "持ち上げ中", "들어 올림", "levantado", "levantado", "soulevé", "angehoben", "поднят", "已拿起"]));
        }
    }
    fn status(&self, out: &mut String) {
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let moves_l = tr(&["moves", "步数", "手数", "이동", "movimientos", "movimentos", "coups", "Züge", "ходы", "步數"]);
        out.push_str(&format!("{} {}, {} {}", level_l, self.s.level + 1, moves_l, self.s.moves));
    }
    fn keys(&self) -> &'static str {
        tr(&[
            "Arrows pick a tube, Space lifts it and then pours it into another, X undoes, I reads the level and moves. Tubes read from the bottom up.",
            "方向键选试管，空格拿起，再按空格倒进另一支，X 撤销，I 读出关卡和步数。试管从下往上读。",
            "矢印で管を選び、スペースで持ち上げて別の管に注ぐ、X で戻す、I でレベルと手数。管は下から読み上げます。",
            "화살표로 관 선택, 스페이스로 들어 올려 다른 관에 붓기, X로 되돌리기, I로 레벨과 이동 수. 관은 아래부터 읽습니다.",
            "Flechas para elegir tubo, Espacio lo levanta y luego lo vierte en otro, X deshace, I lee nivel y movimientos. Los tubos se leen de abajo arriba.",
            "Setas escolhem o tubo, Espaço levanta e depois despeja em outro, X desfaz, I lê nível e movimentos. Os tubos são lidos de baixo para cima.",
            "Flèches pour choisir un tube, Espace le soulève puis le verse dans un autre, X annule, I lit niveau et coups. Les tubes se lisent de bas en haut.",
            "Pfeiltasten wählen eine Röhre, Leertaste hebt sie an und gießt sie in eine andere, X nimmt zurück, I liest Level und Züge vor. Röhren werden von unten nach oben gelesen.",
            "Стрелки выбирают пробирку, пробел поднимает её и переливает в другую, X отменяет, I читает уровень и ходы. Пробирки читаются снизу вверх.",
            "方向鍵選試管，空白鍵拿起，再按空白鍵倒進另一支，X 復原，I 讀出關卡和步數。試管從下往上讀。",
        ])
    }
    fn act(&mut self, x: i32, y: i32, a: i32, said: &mut String) -> u32 {
        if self.s.phase == CLEARED {
            let ev = self.next();
            self.status(said);
            return ev;
        }
        self.focus(x, y);
        let n = self.s.stand.n;
        let undo_l = tr(&["Undone", "已撤销", "戻した", "되돌림", "Deshecho", "Desfeito", "Annulé", "Zurückgenommen", "Отменено", "已復原"]);
        let none_l = tr(&["Nothing to undo", "没有可撤销的", "戻せる手がありません", "되돌릴 수 없습니다", "Nada que deshacer", "Nada para desfazer", "Rien à annuler", "Nichts zurückzunehmen", "Нечего отменять", "沒有可復原的"]);
        if a != 0 || x == n {
            let ev = self.undo();
            said.push_str(if ev != 0 { undo_l } else { none_l });
            return ev;
        }
        if x > n {
            let ev = self.restart();
            said.push_str(if ev != 0 {
                tr(&["Restarted", "已重来", "やり直した", "다시 시작함", "Reiniciado", "Reiniciado", "Recommencé", "Neu gestartet", "Начато заново", "已重來"])
            } else {
                none_l
            });
            return ev;
        }
        let (sel, moves) = (self.s.sel, self.s.moves);
        let ev = self.tap(x);
        if self.s.moves > moves {
            let to = (self.s.hist[self.s.hist_len as usize - 1] >> 4 & 15) as i32;
            said.push_str(tr(&["Poured. ", "已倒入。", "注いだ。", "부었습니다. ", "Vertido. ", "Despejado. ", "Versé. ", "Gegossen. ", "Перелито. ", "已倒入。"]));
            self.cell(to, 0, said);
            if self.s.stand.solved() {
                said.push_str(". ");
                said.push_str(tr(&["Level cleared! Space for the next level.", "过关！按空格进入下一关。", "クリア！スペースで次のレベル。", "레벨 클리어! 스페이스로 다음 레벨.", "¡Nivel superado! Espacio para el siguiente.", "Nível concluído! Espaço para o próximo.", "Niveau réussi ! Espace pour le suivant.", "Level geschafft! Leertaste für das nächste.", "Уровень пройден! Пробел для следующего.", "過關！按空白鍵進入下一關。"]));
            } else if !self.s.stand.any_move() {
                said.push_str(". ");
                said.push_str(tr(&["No moves left", "无路可走", "注げる管がありません", "더 이상 부을 수 없습니다", "No quedan movimientos", "Sem jogadas", "Plus aucun coup", "Kein Zug mehr möglich", "Ходов нет", "無路可走"]));
            }
        } else if ev == wg::EV_HIT {
            said.push_str(tr(&["Can't pour there", "倒不进去", "そこには注げません", "거기에는 부을 수 없습니다", "No se puede verter ahí", "Não dá para despejar aí", "Impossible de verser ici", "Da passt es nicht hinein", "Туда нельзя перелить", "倒不進去"]));
        } else if sel >= 0 && self.s.sel < 0 {
            said.push_str(tr(&["Put down", "放下", "置いた", "내려놓음", "Dejado", "Colocado", "Reposé", "Abgestellt", "Поставлена", "放下"]));
        } else {
            self.cell(x, 0, said);
        }
        ev
    }
    fn rev(&mut self) -> u32 {
        let v = [self.s.phase, self.s.level, self.s.sel, self.s.hist_len, self.tall as i32, wg::lang() as i32];
        wg::hash(wg::hash(0, wg::bytes_of(&self.s.stand)), wg::bytes_of(&v))
    }
}

wg::game!(Decant);
wg::board!(Decant);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Decant, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    #[test]
    fn plays_every_level() {
        let mut g = Decant::new();
        g.start(7);
        let mut seen_level = 0;
        for level in 0..LEVELS {
            assert_eq!(g.s.level, level);
            let path = puzzle::solution(&g.s.stand, 400_000).unwrap();
            // a refused pour, an undo and a restart on the way
            g.tap(0);
            g.tap(0);
            let mut ev = 0;
            for (i, &(a, b)) in path.iter().enumerate() {
                if i == 1 {
                    assert!(g.s.hist_len == 1);
                    g.undo();
                    let (a0, b0) = path[0];
                    g.tap(a0 as i32);
                    g.tap(b0 as i32);
                    run(&mut g, 1000);
                }
                g.tap(a as i32);
                g.tap(b as i32);
                ev |= run(&mut g, 1000);
                assert!(g.loaded_check());
            }
            assert!(ev & wg::EV_LEVEL != 0, "level {} not cleared", level + 1);
            assert_eq!(g.cleared(), Some((level, g.s.cleared_score as u32)));
            seen_level += 1;
            if level + 1 < LEVELS {
                assert_eq!(g.s.phase, CLEARED);
                assert!(!g.over());
                g.next();
            } else {
                assert!(ev & wg::EV_GAMEOVER != 0);
                assert!(g.over());
            }
        }
        assert_eq!(seen_level, LEVELS);
        assert_eq!(g.s.done, LEVELS);
        println!("score {}", g.score());
    }

    #[test]
    fn clears_after_loading_mid_pour() {
        let mut g = Decant::new();
        g.start(1);
        let path = puzzle::solution(&g.s.stand, 400_000).unwrap();
        for &(a, b) in &path {
            g.tap(a as i32);
            g.tap(b as i32);
            if g.s.stand.solved() {
                break;
            }
            run(&mut g, 1000);
        }
        // the last pour is still running when the save is loaded
        assert!(g.pour.is_some() && g.loaded());
        assert!(run(&mut g, 16) & wg::EV_LEVEL != 0);
        assert_eq!(g.s.phase, CLEARED);
    }

    impl Decant {
        fn loaded_check(&mut self) -> bool {
            let keep = self.pour;
            let ok = self.loaded();
            self.pour = keep;
            ok
        }
    }
}
