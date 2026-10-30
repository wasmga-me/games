//! Unpark - an original wasmga.me game. MIT licensed; the cars are Kenney's
//! Car Kit (CC0), rendered by games-src/tools/render3d.
//! A car park is packed with cars nose to tail. Tap a car to drive it the
//! way it faces, or swipe it to back it out; with a clear road it drives
//! out onto the ring road and away, otherwise it bumps into what is in its
//! way. Empty the park to finish the level. 30 levels.
mod art;
mod glyphs;
mod puzzle;

use puzzle::{Lot, Way, CONE, DX, DY, EMPTY, LEVELS};
use wg::{hex, mix, tr, Game, Image, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 1;

const PLAY: i32 = 0;
const CLEARED: i32 = 1;
const DONE: i32 = 2;

/// the board's tilt: the ground's depth shows at this fraction of its width
const TILT: f32 = 0.848;
/// model units in a square
const CELL: f32 = 1.6;
/// the ring road's middle, squares out from the park's edge, and its width
const RING: f32 = 0.95;
const ROAD: f32 = 1.3;
/// how fast a car drives, squares a second, and how fast it starts
const SPEED: f32 = 11.0;
const PULL: f32 = 0.18;
/// a bump: there and back, ms
const BUMP_MS: f32 = 420.0;
/// how far past its way a car must be dragged to drive, squares
const SWIPE: f32 = 0.3;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 1],
    level: i32,
    phase: i32,
    lot: Lot,
    bumps: i32,
    level_ms: i32,
    time_ms: i32,
    all_bumps: i32,
    all_out: i32,
    done: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
    /// keyboard cursor, a square
    cx: i32,
    cy: i32,
    rep: KeyRepeat,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}
unsafe impl wg::Plain for Lot {}
unsafe impl wg::Plain for puzzle::Car {}

/// a car driving out: its model, its path and how far along it is
struct Drive {
    model: i8,
    path: Vec<Leg>,
    s: f32,
    /// ms since it set off
    t: f32,
}

/// One straight stretch of a drive: where it starts, its way and length,
/// and the way the car faces along it (backing up, the other way).
#[derive(Clone, Copy)]
struct Leg {
    x: f32,
    y: f32,
    d: usize,
    len: f32,
    face: f32,
}

/// a car bumping into something and back
#[derive(Clone, Copy)]
struct Bump {
    car: usize,
    d: usize,
    reach: f32,
    by: i32,
    t: f32,
}

/// where the park is drawn: its top left and a square's width in plan units
#[derive(Clone, Copy)]
struct View {
    x: f32,
    y: f32,
    s: f32,
}

impl View {
    /// a point on the ground, in squares, to plan units
    fn at(&self, x: f32, y: f32) -> (f32, f32) {
        (self.x + x * self.s, self.y + y * self.s * TILT)
    }
    /// plan units to a point on the ground, in squares
    fn ground(&self, px: f32, py: f32) -> (f32, f32) {
        ((px - self.x) / self.s, (py - self.y) / (self.s * TILT))
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    None,
    Car(usize),
    Next,
}

struct Unpark {
    s: Save,
    tall: bool,
    hover: Hit,
    keys: bool,
    sprites: Vec<Image>,
    drives: Vec<Drive>,
    bump: Option<Bump>,
    /// a press on a car, waiting to see if it becomes a swipe: the car and
    /// where the press began
    press: Option<(usize, f32, f32)>,
}

fn angle_lerp(a: f32, b: f32, t: f32) -> f32 {
    let mut d = (b - a) % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d < -180.0 {
        d += 360.0;
    }
    a + d * t
}

/// the heading of way d, degrees clockwise from up
fn heading(d: usize) -> f32 {
    d as f32 * 90.0
}

impl Unpark {
    fn plan_wh(&self) -> (f32, f32) {
        if self.tall { (300.0, 400.0) } else { (400.0, 300.0) }
    }

    /// Fits the park, its ring road and the road out between the top bar
    /// and the bottom of the screen.
    fn view(&self) -> View {
        let (w, h) = self.plan_wh();
        let lot = &self.s.lot;
        let (top, bottom) = (44.0, h - 10.0);
        let edge = RING + ROAD / 2.0 + 0.2;
        let (gw, gh) = (lot.w as f32 + 2.0 * edge, lot.h as f32 + 2.0 * edge);
        // the cars stand up out of their squares by about half a square
        let s = ((w - 16.0) / gw).min((bottom - top) / (gh * TILT + 0.5));
        let x = (w - lot.w as f32 * s) / 2.0;
        let y = top + 0.5 * s + edge * s * TILT + ((bottom - top) - (gh * TILT + 0.5) * s) / 2.0;
        View { x, y, s }
    }

    fn card(&self) -> (f32, f32, f32, f32) {
        let (w, h) = self.plan_wh();
        let (pw, ph) = (248.0, if self.s.phase == CLEARED { 158.0 } else { 96.0 });
        (w / 2.0 - pw / 2.0, h / 2.0 - ph / 2.0, pw, ph)
    }
    fn next_box(&self) -> (f32, f32, f32, f32) {
        let (px, py, pw, ph) = self.card();
        let (bw, bh) = (pw - 48.0, 42.0);
        (px + (pw - bw) / 2.0, py + ph - bh - 20.0, bw, bh)
    }

    /// The car under a point: its squares, reaching up by its height,
    /// the nearest one wins.
    fn car_at(&self, px: f32, py: f32) -> Option<usize> {
        let v = self.view();
        let (gx, gy) = v.ground(px, py);
        let lot = &self.s.lot;
        let mut best: Option<(usize, f32)> = None;
        for i in 0..lot.n as usize {
            let c = &lot.cars[i];
            if c.out != 0 {
                continue;
            }
            let (x0, y0, x1, y1) = car_box(c);
            // the roof shows above the squares
            if gx >= x0 && gx < x1 && gy >= y0 - 0.55 && gy < y1 {
                let front = y1;
                if best.is_none_or(|b| front > b.1) {
                    best = Some((i, front));
                }
            }
        }
        best.map(|b| b.0)
    }

    fn hit(&self, x: f32, y: f32) -> Hit {
        let inside = |(bx, by, bw, bh): (f32, f32, f32, f32)| x >= bx && x < bx + bw && y >= by && y < by + bh;
        match self.s.phase {
            PLAY => self.car_at(x, y).map_or(Hit::None, Hit::Car),
            CLEARED if inside(self.next_box()) => Hit::Next,
            _ => Hit::None,
        }
    }

    fn begin(&mut self, level: i32) {
        let s = &mut self.s;
        s.level = level;
        s.phase = PLAY;
        s.lot = puzzle::deal(level);
        s.bumps = 0;
        s.level_ms = 0;
        s.cx = s.cx.clamp(0, s.lot.w - 1);
        s.cy = s.cy.clamp(0, s.lot.h - 1);
        self.drives.clear();
        self.bump = None;
        self.press = None;
    }

    fn busy(&self, i: usize) -> bool {
        self.bump.is_some_and(|b| b.car == i)
    }

    /// Sends car i its front's way, or backing up.
    fn send(&mut self, i: usize, back: bool) -> u32 {
        if self.s.phase != PLAY || i >= self.s.lot.n as usize || self.s.lot.cars[i].out != 0 || self.busy(i) {
            return 0;
        }
        let c = self.s.lot.cars[i];
        let d = if back { (c.dir as usize + 2) % 4 } else { c.dir as usize };
        match self.s.lot.way(i, back) {
            Way::Out => {
                self.s.lot.cars[i].out = 1;
                self.s.all_out += 1;
                let path = self.route(&c, d);
                self.drives.push(Drive { model: c.model, path, s: 0.0, t: 0.0 });
                wg::EV_BLIP
            }
            Way::Blocked { gap, by } => {
                // a bump finishes before the next one starts
                if self.bump.is_some() {
                    return 0;
                }
                self.s.bumps += 1;
                self.s.all_bumps += 1;
                self.bump = Some(Bump { car: i, d, reach: gap as f32 + 0.16, by, t: 0.0 });
                wg::EV_HIT
            }
        }
    }

    /// The way out for car c going way d: straight off the park onto the
    /// ring road, round it the shorter way to the top right corner, and up
    /// the road out.
    fn route(&self, c: &puzzle::Car, d: usize) -> Vec<Leg> {
        let lot = &self.s.lot;
        let (w, h) = (lot.w as f32, lot.h as f32);
        let (x0, y0, x1, y1) = car_box(c);
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let face = heading(c.dir as usize);
        // to the ring's middle line
        let (ex, ey) = match d {
            0 => (cx, -RING),
            1 => (w + RING, cy),
            2 => (cx, h + RING),
            _ => (-RING, cy),
        };
        let mut legs = vec![Leg { x: cx, y: cy, d, len: (ex - cx).abs() + (ey - cy).abs(), face }];
        // the ring's corners clockwise from the top left, and the exit at
        // the top right
        let corners = [(-RING, -RING), (w + RING, -RING), (w + RING, h + RING), (-RING, h + RING)];
        let perim = 2.0 * (w + h + 4.0 * RING);
        // distance clockwise from the top left corner
        let along = |x: f32, y: f32| -> f32 {
            let (sw, sh) = (w + 2.0 * RING, h + 2.0 * RING);
            if y <= -RING + 0.01 {
                x + RING
            } else if x >= w + RING - 0.01 {
                sw + y + RING
            } else if y >= h + RING - 0.01 {
                sw + sh + (w + RING - x)
            } else {
                2.0 * sw + sh + (h + RING - y)
            }
        };
        let exit = along(corners[1].0, corners[1].1);
        let here = along(ex, ey);
        let cw = (exit - here).rem_euclid(perim);
        let clockwise = cw <= perim - cw;
        let mut at = (ex, ey);
        // the corners to pass, starting from the side entered
        let mut k = if clockwise { (d + 1) % 4 } else { d };
        for _ in 0..4 {
            if at == corners[1] {
                break;
            }
            let next = corners[k];
            let (dx, dy) = (next.0 - at.0, next.1 - at.1);
            let len = dx.abs() + dy.abs();
            if len > 0.001 {
                let way = if dx.abs() > dy.abs() { if dx > 0.0 { 1 } else { 3 } } else if dy > 0.0 { 2 } else { 0 };
                legs.push(Leg { x: at.0, y: at.1, d: way, len, face: heading(way) });
            }
            at = next;
            k = if clockwise { (k + 1) % 4 } else { (k + 3) % 4 };
        }
        // up and off the screen
        legs.push(Leg { x: at.0, y: at.1, d: 0, len: h + 2.0 * RING + 12.0, face: 0.0 });
        legs
    }

    fn clear_level(&mut self) -> u32 {
        let s = &mut self.s;
        let n = s.lot.n;
        let score = (100 * n - 40 * s.bumps).max(10 * n);
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

    fn next(&mut self) -> u32 {
        if self.s.phase != CLEARED {
            return 0;
        }
        self.begin(self.s.level + 1);
        wg::EV_BLIP
    }

    fn cursor_move(&mut self, dx: i32, dy: i32) {
        self.s.cx = (self.s.cx + dx).clamp(0, self.s.lot.w - 1);
        self.s.cy = (self.s.cy + dy).clamp(0, self.s.lot.h - 1);
    }

    fn sprite(&self, model: i8, face: f32) -> usize {
        // 8 views, a turn of 45 degrees apart
        let k = ((face.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8;
        model as usize * 8 + k
    }

    /// Draws sprite `i` with the model's origin at ground point (gx, gy).
    fn put(&self, s: &mut Screen, v: &View, i: usize, gx: f32, gy: f32, alpha: f32) {
        let a = &art::ARTS[i];
        let (px, py) = v.at(gx, gy);
        let k = v.s / CELL / a.ppu;
        s.image(&self.sprites[i], px - a.ax * k, py - a.ay * k, a.w as f32 * k, a.h as f32 * k, alpha);
    }

    /// where a drive is now: ground point and heading
    fn drive_at(d: &Drive) -> (f32, f32, f32) {
        let mut s = d.s;
        let n = d.path.len();
        for (i, leg) in d.path.iter().enumerate() {
            if s <= leg.len || i + 1 == n {
                let (x, y) = (leg.x + DX[leg.d] as f32 * s, leg.y + DY[leg.d] as f32 * s);
                // turn smoothly through the last and first bit of a corner
                const R: f32 = 0.55;
                let mut face = leg.face;
                if i + 1 < n && leg.len - s < R {
                    let t = (1.0 - (leg.len - s) / R) * 0.5;
                    face = angle_lerp(leg.face, d.path[i + 1].face, t);
                } else if i > 0 && s < R {
                    let t = 0.5 + s / R * 0.5;
                    face = angle_lerp(d.path[i - 1].face, leg.face, t);
                }
                return (x, y, face);
            }
            s -= leg.len;
        }
        (0.0, 0.0, 0.0)
    }
}

/// An arrow on the ground ahead of a car, the way it drives on a tap.
fn arrow(s: &mut Screen, v: &View, c: &puzzle::Car) {
    let d = c.dir as usize;
    let (x, y) = c.end(false);
    let (gx, gy) = (x as f32 + 0.5 + DX[d] as f32 * 0.78, y as f32 + 0.5 + DY[d] as f32 * 0.78);
    let (px, py) = v.at(gx, gy);
    let k = v.s * 0.2;
    // ahead and aside, on the tilted ground
    let (fx, fy) = (DX[d] as f32 * k, DY[d] as f32 * k * TILT);
    let (sx, sy) = (-DY[d] as f32 * k, DX[d] as f32 * k * TILT);
    let tip = (px + fx, py + fy);
    let l = (px - fx * 0.6 + sx, py - fy * 0.6 + sy);
    let r = (px - fx * 0.6 - sx, py - fy * 0.6 - sy);
    s.tri((tip.0, tip.1 + 2.0), (l.0, l.1 + 2.0), (r.0, r.1 + 2.0), hex(0x000000), 0.25);
    s.tri(tip, l, r, hex(0xffffff), 0.95);
}

/// a car's squares as a box on the ground, in squares
fn car_box(c: &puzzle::Car) -> (f32, f32, f32, f32) {
    let (ax, ay) = c.end(true);
    let (bx, by) = c.end(false);
    (ax.min(bx) as f32, ay.min(by) as f32, ax.max(bx) as f32 + 1.0, ay.max(by) as f32 + 1.0)
}

impl Game for Unpark {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let sheet = Image::png(art::SHEET);
        let sprites = art::ARTS.iter().map(|a| sheet.crop(a.x, a.y, a.w, a.h)).collect();
        Unpark {
            s: blank(),
            tall: false,
            hover: Hit::None,
            keys: false,
            sprites,
            drives: Vec::new(),
            bump: None,
            press: None,

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
        self.s = Save { opt, ..blank() };
        self.begin(opt[0]);
        self.keys = false;
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        if h as f32 > w as f32 * 1.1 { (300.0, 400.0) } else { (400.0, 300.0) }
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        for d in self.drives.iter_mut() {
            d.t += dt;
            // pulls away, then drives on at full speed
            let v = SPEED * (d.t / 1000.0 / PULL).min(1.0);
            d.s += v * dt / 1000.0;
        }
        let total = |d: &Drive| d.path.iter().map(|l| l.len).sum::<f32>();
        self.drives.retain(|d| d.s < total(d));
        if let Some(b) = &mut self.bump {
            b.t += dt / BUMP_MS;
            if b.t >= 1.0 {
                self.bump = None;
            }
        }
        // the level clears once the last car has driven away, or on loading
        // a save made while it drove
        if self.s.phase == PLAY && self.s.lot.left() == 0 && self.drives.is_empty() {
            ev |= self.clear_level();
        }
        // the clock runs from the level's first move
        if self.s.phase == PLAY && (self.s.lot.left() < self.s.lot.n || self.s.bumps > 0) {
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
            if dirs & wg::UP != 0 { self.cursor_move(0, -1); }
            if dirs & wg::DOWN != 0 { self.cursor_move(0, 1); }
            let pick = input.pressed & (wg::A | wg::B);
            if pick != 0 {
                self.keys = true;
                if let Some(i) = self.s.lot.car_at(self.s.cx, self.s.cy) {
                    ev |= self.send(i, pick & wg::A == 0);
                }
            }
        } else if self.s.phase == CLEARED && input.pressed & wg::A != 0 {
            ev |= self.next();
        }
        ev
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        let mut ev = 0;
        if let Some((i, x0, y0)) = self.press {
            if p.held & wg::M_LEFT == 0 {
                // let go without a swipe: a tap drives it forward
                self.press = None;
                if self.car_at(p.x, p.y) == Some(i) {
                    ev |= self.send(i, false);
                }
            } else {
                let v = self.view();
                let (dx, dy) = ((p.x - x0) / v.s, (p.y - y0) / (v.s * TILT));
                let c = self.s.lot.cars[i];
                let d = c.dir as usize;
                let along = dx * DX[d] as f32 + dy * DY[d] as f32;
                if along.abs() > SWIPE && along.abs() > 0.6 * (dx * dx + dy * dy).sqrt() {
                    self.press = None;
                    ev |= self.send(i, along < 0.0);
                }
            }
        }
        if !p.inside {
            self.hover = Hit::None;
            return ev;
        }
        let h = self.hit(p.x, p.y);
        self.hover = if p.touch { Hit::None } else { h };
        if p.pressed & wg::M_LEFT != 0 {
            self.keys = false;
            match h {
                Hit::Car(i) => {
                    let c = &self.s.lot.cars[i];
                    let (x0, y0, ..) = car_box(c);
                    self.s.cx = x0 as i32;
                    self.s.cy = y0 as i32;
                    self.press = Some((i, p.x, p.y));
                }
                Hit::Next => ev |= self.next(),
                Hit::None => {}
            }
        }
        ev
    }

    fn picture(&mut self) -> u32 {
        let hv = match self.hover {
            Hit::None => -1,
            Hit::Car(i) => i as i32,
            Hit::Next => 1000,
        };
        let shown = [hv, self.keys as i32, self.tall as i32];
        let mut c = self.s;
        c.level_ms = 0;
        c.time_ms = 0;
        c.rep = KeyRepeat(0);
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        !self.drives.is_empty() || self.bump.is_some()
    }

    fn draw(&mut self, s: &mut Screen) {
        self.tall = s.ph > s.pw;
        let v = self.view();
        let (w, _) = self.plan_wh();
        let lot = self.s.lot;
        let key = wg::hash(0, wg::bytes_of(&[self.s.level, self.tall as i32, lot.w, lot.h]));
        s.background(key, |s| {
            draw_ground(s, &v, &lot);
            self.draw_trees(s, &v, &lot);
        });

        // everything standing on the ground, back to front
        let mut items: Vec<(f32, Item)> = Vec::new();
        for y in 0..lot.h {
            for x in 0..lot.w {
                let p = lot.prop(x, y);
                if p != EMPTY {
                    items.push((y as f32 + 0.5, Item::Prop(p, x, y)));
                }
            }
        }
        for i in 0..lot.n as usize {
            let c = &lot.cars[i];
            if c.out == 0 {
                let (_, y0, _, y1) = car_box(c);
                items.push(((y0 + y1) / 2.0, Item::Car(i)));
            }
        }
        for (k, d) in self.drives.iter().enumerate() {
            items.push((Self::drive_at(d).1, Item::Drive(k)));
        }
        items.sort_by(|a, b| a.0.total_cmp(&b.0));

        let marked = match (self.hover, self.keys && self.s.phase == PLAY) {
            (Hit::Car(i), _) => Some(i),
            (_, true) => lot.car_at(self.s.cx, self.s.cy),
            _ => None,
        };
        if self.keys && self.s.phase == PLAY {
            // the cursor's square
            let (x, y) = v.at(self.s.cx as f32, self.s.cy as f32);
            s.rect(x + 1.0, y + 1.0, v.s - 2.0, v.s * TILT - 2.0, 4.0, hex(0xffe066), 0.35);
        }
        for (_, item) in items {
            match item {
                Item::Prop(p, x, y) => {
                    let i = if p == CONE { art::CONE } else { art::BOX };
                    self.put(s, &v, i, x as f32 + 0.5, y as f32 + 0.5, 1.0);
                }
                Item::Car(i) => {
                    let c = &lot.cars[i];
                    let (x0, y0, x1, y1) = car_box(c);
                    let (mut gx, mut gy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                    if let Some(b) = self.bump {
                        // there and back, quick out and slower back
                        if b.car == i {
                            let out = if b.t < 0.45 { wg::ease_out(b.t / 0.45) } else { 1.0 - wg::ease_out((b.t - 0.45) / 0.55) };
                            gx += DX[b.d] as f32 * b.reach * out;
                            gy += DY[b.d] as f32 * b.reach * out;
                        } else if b.by == i as i32 && b.t > 0.4 {
                            // the car hit rocks a little
                            let k = (1.0 - (b.t - 0.4) / 0.6) * ((b.t - 0.4) * 60.0).sin() * 0.05;
                            gx += DX[b.d] as f32 * k;
                            gy += DY[b.d] as f32 * k;
                        }
                    }
                    if marked == Some(i) {
                        let (px, py) = v.at(x0, y0);
                        s.shadow(px, py, (x1 - x0) * v.s, (y1 - y0) * v.s * TILT, 6.0, 7.0, hex(0xffffff), if self.keys { 0.8 } else { 0.55 });
                    }
                    self.put(s, &v, self.sprite(c.model, heading(c.dir as usize)), gx, gy, 1.0);
                    if marked == Some(i) && self.bump.is_none() {
                        arrow(s, &v, c);
                    }
                }
                Item::Drive(k) => {
                    let d = &self.drives[k];
                    let (gx, gy, face) = Self::drive_at(d);
                    self.put(s, &v, self.sprite(d.model, face), gx, gy, 1.0);
                }
            }
        }
        if let Some(b) = self.bump {
            // a red mark where it hit
            if b.t > 0.35 && b.t < 0.9 {
                let c = &lot.cars[b.car];
                let (fx, fy) = {
                    let back = b.d != c.dir as usize;
                    let (x, y) = c.end(back);
                    (x as f32 + 0.5 + DX[b.d] as f32 * (0.5 + b.reach), y as f32 + 0.5 + DY[b.d] as f32 * (0.5 + b.reach))
                };
                let (px, py) = v.at(fx, fy);
                let a = 1.0 - (b.t - 0.35) / 0.55;
                let r = v.s * (0.18 + (b.t - 0.35) * 0.5);
                s.disc(px, py - v.s * 0.35, r, hex(0xff5a4e), 0.85 * a);
                s.text_c("!", px, py - v.s * 0.35, r * 1.5, hex(0xffffff), a);
            }
        }

        // the top bar
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let left_l = tr(&["Cars left", "剩余车辆", "残りの車", "남은 차", "Coches", "Carros", "Voitures", "Autos übrig", "Машин", "剩餘車輛"]);
        let th = THEME;
        // a soft shade under the words, so they read over grass and road
        let shaded = |s: &mut Screen, t: &str, x: f32, y: f32, size: f32, c: u32, right: bool| {
            for (dy, col, a) in [(1.2, 0x1d2a14, 0.45), (0.0, c, 1.0)] {
                if right {
                    s.text_r(t, x, y + dy, size, hex(col), a);
                } else {
                    s.text(t, x, y + dy, size, hex(col), a);
                }
            }
        };
        shaded(s, &format!("{} {}", level_l, self.s.level + 1), 14.0, 20.0, 17.0, th.text, false);
        shaded(s, &format!("{} {}", left_l, lot.left()), w - 14.0, 20.0, 15.0, th.dim, true);
        if self.s.bumps > 0 {
            let bumps_l = tr(&["Bumps", "碰撞", "衝突", "충돌", "Choques", "Batidas", "Chocs", "Rempler", "Удары", "碰撞"]);
            shaded(s, &format!("{} {}", bumps_l, self.s.bumps), w - 14.0, 38.0, 12.0, th.dim, true);
        }
        if self.s.phase != PLAY {
            self.draw_cleared(s, w);
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
            1 => s.all_out,
            2 => s.all_bumps,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION
            && (0..LEVELS).contains(&s.opt[0])
            && (0..LEVELS).contains(&s.level)
            && (PLAY..=DONE).contains(&s.phase)
            && s.lot.valid()
            && (0..s.lot.w).contains(&s.cx)
            && (0..s.lot.h).contains(&s.cy);
        self.drives.clear();
        self.bump = None;
        self.press = None;
        self.hover = Hit::None;
        ok
    }
}

fn blank() -> Save {
    Save {
        version: VERSION,
        opt: [0],
        level: 0,
        phase: PLAY,
        lot: Lot::empty(4, 5),
        bumps: 0,
        level_ms: 0,
        time_ms: 0,
        all_bumps: 0,
        all_out: 0,
        done: 0,
        score: 0,
        cleared: -1,
        cleared_score: 0,
        cx: 0,
        cy: 0,
        rep: KeyRepeat(0),
    }
}

enum Item {
    Prop(u8, i32, i32),
    Car(usize),
    Drive(usize),
}

struct Theme {
    grass: u32,
    grass2: u32,
    road: u32,
    lot: u32,
    paint: u32,
    kerb: u32,
    text: u32,
    dim: u32,
    card: u32,
    accent: u32,
}

const THEME: Theme = Theme {
    grass: 0x7cb85a,
    grass2: 0x5f9e45,
    road: 0x4a4f58,
    lot: 0x5b616b,
    paint: 0xeef0f2,
    kerb: 0xc9ccd1,
    text: 0xffffff,
    dim: 0xe4f2dc,
    card: 0x2f3540,
    accent: 0x3d7be0,
};

/// The grass, the ring road and the road out, and the park with its bays
/// and barriers: drawn once a level.
fn draw_ground(s: &mut Screen, v: &View, lot: &Lot) {
    let th = THEME;
    let (x0, y0, x1, y1) = s.visible();
    s.rect_grad(x0, y0, x1 - x0, y1 - y0, 0.0, hex(th.grass), hex(th.grass2), 1.0);
    // tufts of grass, the same each time
    let mut rng = wg::Rng::new(77);
    for _ in 0..((x1 - x0) * (y1 - y0) / 90.0) as i32 {
        let (x, y) = (x0 + rng.float() * (x1 - x0), y0 + rng.float() * (y1 - y0));
        s.disc(x, y, 1.2 + rng.float() * 1.6, hex(0x4f8a39), 0.18);
    }
    let (w, h) = (lot.w as f32, lot.h as f32);
    let half = ROAD / 2.0;
    let ring = |s: &mut Screen, grow: f32, c: u32| {
        let (ax, ay) = v.at(-RING - half - grow, -RING - half - grow);
        let (bx, by) = v.at(w + RING + half + grow, h + RING + half + grow);
        s.rect(ax, ay, bx - ax, by - ay, v.s * (0.9 + grow), hex(c), 1.0);
    };
    // the kerb, then the road, then the park inside
    ring(s, 0.12, th.kerb);
    ring(s, 0.0, th.road);
    // the road out, up from the top right corner
    let (rx, _) = v.at(w + RING - half - 0.12, 0.0);
    let (_, ry) = v.at(0.0, -RING);
    s.rect(rx, y0 - 2.0, (ROAD + 0.24) * v.s, ry - y0 + 2.0, 0.0, hex(th.kerb), 1.0);
    s.rect(rx + 0.12 * v.s, y0 - 2.0, ROAD * v.s, ry - y0 + 2.0, 0.0, hex(th.road), 1.0);
    // the park, a step up
    let (ax, ay) = v.at(-0.18, -0.18);
    let (bx, by) = v.at(w + 0.18, h + 0.18);
    s.rect(ax, ay, bx - ax, by - ay + 3.0, v.s * 0.3, hex(0x3a3f47), 1.0);
    s.rect(ax, ay, bx - ax, by - ay, v.s * 0.3, hex(th.kerb), 1.0);
    let (ax, ay) = v.at(-0.06, -0.06);
    let (bx, by) = v.at(w + 0.06, h + 0.06);
    s.rect(ax, ay, bx - ax, by - ay, v.s * 0.22, hex(th.lot), 1.0);
    // dashes down the ring's middle
    let dash = |s: &mut Screen, a: (f32, f32), b: (f32, f32)| {
        let len = ((b.0 - a.0).abs() + (b.1 - a.1).abs()).max(0.01);
        let n = (len / 0.7) as i32;
        for k in 0..n {
            let t0 = (k as f32 + 0.25) / n as f32;
            let t1 = (k as f32 + 0.65) / n as f32;
            let p = v.at(a.0 + (b.0 - a.0) * t0, a.1 + (b.1 - a.1) * t0);
            let q = v.at(a.0 + (b.0 - a.0) * t1, a.1 + (b.1 - a.1) * t1);
            s.line(p.0, p.1, q.0, q.1, v.s * 0.035, hex(0xf3d36b), 0.85);
        }
    };
    let c = [(-RING, -RING), (w + RING, -RING), (w + RING, h + RING), (-RING, h + RING)];
    for i in 0..4 {
        dash(s, c[i], c[(i + 1) % 4]);
    }
    // bays: a line between each pair of squares, faint grid
    for y in 0..=lot.h {
        let (ax, ay) = v.at(0.0, y as f32);
        let (bx, _) = v.at(w, y as f32);
        s.line(ax + v.s * 0.08, ay, bx - v.s * 0.08, ay, v.s * 0.022, hex(th.paint), 0.35);
    }
    for x in 0..=lot.w {
        let (ax, ay) = v.at(x as f32, 0.0);
        let (_, by) = v.at(x as f32, h);
        s.line(ax, ay + v.s * 0.08, ax, by - v.s * 0.08, v.s * 0.022, hex(th.paint), 0.35);
    }
    // oil stains
    for _ in 0..lot.w * lot.h / 5 {
        let (x, y) = v.at(rng.float() * w, rng.float() * h);
        s.ellipse(x, y, v.s * (0.12 + rng.float() * 0.2), v.s * 0.1 * TILT, hex(0x3c4048), 0.35);
    }
    // barriers: red and white blocks past the edge squares that have one
    for d in 0..4 {
        let count = if d % 2 == 0 { lot.w } else { lot.h };
        for k in 0..count {
            let (x, y) = match d {
                0 => (k, 0),
                1 => (lot.w - 1, k),
                2 => (k, lot.h - 1),
                _ => (0, k),
            };
            if !lot.fenced(x, y, d) {
                continue;
            }
            let (gx, gy) = (x as f32 + 0.5 + DX[d] as f32 * 0.62, y as f32 + 0.5 + DY[d] as f32 * 0.62);
            let along = d % 2 == 0;
            let (hw, hd) = if along { (0.46, 0.1) } else { (0.1, 0.46) };
            let (ax, ay) = v.at(gx - hw, gy - hd);
            let (bx, by) = v.at(gx + hw, gy + hd);
            let lift = v.s * 0.2;
            s.shadow(ax, ay + lift * 0.5, bx - ax, by - ay, 2.0, 3.0, 0, 0.35);
            // the front face, then the top with stripes
            s.rect(ax, ay - lift + (by - ay), bx - ax, lift, 1.5, hex(0xb02a2a), 1.0);
            s.rect(ax, ay - lift, bx - ax, by - ay, 1.5, hex(0xf2f2f2), 1.0);
            let stripes = 4;
            for i in 0..stripes {
                if i % 2 == 0 {
                    continue;
                }
                if along {
                    let sx = ax + (bx - ax) * i as f32 / stripes as f32;
                    s.rect(sx, ay - lift, (bx - ax) / stripes as f32, by - ay, 0.0, hex(0xe0413c), 1.0);
                    s.rect(sx, ay - lift + (by - ay), (bx - ax) / stripes as f32, lift, 0.0, hex(0xf2f2f2), 0.85);
                } else {
                    let sy = ay - lift + (by - ay) * i as f32 / stripes as f32;
                    s.rect(ax, sy, bx - ax, (by - ay) / stripes as f32, 0.0, hex(0xe0413c), 1.0);
                }
            }
        }
    }
}

impl Unpark {
    /// Trees and bushes on the grass, clear of the roads and the top bar.
    fn draw_trees(&self, s: &mut Screen, v: &View, lot: &Lot) {
        let (x0, y0, x1, y1) = s.visible();
        let (gx0, gy0) = v.ground(x0, y0);
        let (gx1, gy1) = v.ground(x1, y1);
        let (w, h) = (lot.w as f32, lot.h as f32);
        let out = RING + ROAD / 2.0 + 1.1;
        let road_x = w + RING;
        let (_, bar) = v.ground(0.0, 46.0);
        let mut rng = wg::Rng::new(4242);
        const STEP: f32 = 1.5;
        let kinds = [art::TREE_DEFAULT, art::TREE_OAK, art::TREE_DETAILED, art::TREE_FAT, art::BUSHLARGE, art::BUSHLARGE];
        let mut y = (gy0 / STEP).floor() * STEP;
        while y < gy1 + 2.0 {
            let mut x = (gx0 / STEP).floor() * STEP;
            while x < gx1 + 1.0 {
                let (tx, ty) = (x + rng.float() * STEP * 0.8, y + rng.float() * STEP * 0.8);
                let kind = kinds[rng.below(kinds.len() as u32) as usize];
                let near_ring = tx > -out && tx < w + out && ty > -out && ty < h + out + 1.2;
                let near_exit = (tx - road_x).abs() < ROAD / 2.0 + 0.9 && ty < 0.0;
                if rng.float() < 0.6 && !near_ring && !near_exit && ty - 1.4 > bar {
                    self.put(s, v, kind, tx, ty, 1.0);
                }
                x += STEP;
            }
            y += STEP;
        }
    }

    fn draw_cleared(&self, s: &mut Screen, w: f32) {
        let th = THEME;
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0x10141a), 0.45);
        let (px, py, pw, ph) = self.card();
        s.shadow(px, py + 4.0, pw, ph, 16.0, 14.0, 0, 0.45);
        s.rect(px, py, pw, ph, 16.0, hex(th.card), 1.0);
        let title = tr(&["Level cleared", "过关", "クリア", "레벨 클리어", "Nivel superado", "Nível concluído", "Niveau réussi", "Level geschafft", "Уровень пройден", "過關"]);
        let size = s.fit_size(title, 21.0, pw - 32.0);
        s.text_c(title, w / 2.0, py + 34.0, size, hex(th.text), 1.0);
        let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
        let bumps_l = tr(&["Bumps", "碰撞", "衝突", "충돌", "Choques", "Batidas", "Chocs", "Rempler", "Удары", "碰撞"]);
        let line = format!("{} +{}    {} {}", score_l, self.s.cleared_score, bumps_l, self.s.bumps);
        let size = s.fit_size(&line, 14.0, pw - 32.0);
        s.text_c(&line, w / 2.0, py + 64.0, size, hex(0xb9c2cf), 1.0);
        if self.s.phase == CLEARED {
            let b = self.next_box();
            let label = tr(&["Next level", "下一关", "次のレベル", "다음 레벨", "Siguiente nivel", "Próximo nível", "Niveau suivant", "Nächstes Level", "Следующий уровень", "下一關"]);
            let hv = self.hover == Hit::Next;
            let fill = hex(th.accent);
            s.rect(b.0, b.1, b.2, b.3, b.3 / 2.0, if hv { mix(hex(0xffffff), fill, 0.15) } else { fill }, 1.0);
            let size = s.fit_size(label, 16.0, b.2 - 16.0);
            s.text_c(label, b.0 + b.2 / 2.0, b.1 + b.3 / 2.0, size, hex(0xffffff), 1.0);
        }
    }
}

const WAYS: [[&str; 10]; 4] = [
    ["up", "上", "上", "위", "arriba", "para cima", "vers le haut", "nach oben", "вверх", "上"],
    ["right", "右", "右", "오른쪽", "a la derecha", "para a direita", "vers la droite", "nach rechts", "вправо", "右"],
    ["down", "下", "下", "아래", "abajo", "para baixo", "vers le bas", "nach unten", "вниз", "下"],
    ["left", "左", "左", "왼쪽", "a la izquierda", "para a esquerda", "vers la gauche", "nach links", "влево", "左"],
];

/// The accessible board: the park's squares, each saying what stands on it.
impl wg::Board for Unpark {
    fn grid(&self) -> Option<(i32, i32)> {
        Some((self.s.lot.w, self.s.lot.h))
    }
    fn cursor(&self) -> (i32, i32) {
        (self.s.cx, self.s.cy)
    }
    fn focus(&mut self, x: i32, y: i32) {
        self.s.cx = x.clamp(0, self.s.lot.w - 1);
        self.s.cy = y.clamp(0, self.s.lot.h - 1);
        self.keys = true;
        self.hover = Hit::None;
    }
    fn rect(&self) -> (f32, f32, f32) {
        let v = self.view();
        (v.x, v.y, v.s)
    }
    fn cell_box(&self, x: i32, y: i32) -> Option<(f32, f32, f32, f32)> {
        let v = self.view();
        let (px, py) = v.at(x as f32, y as f32);
        Some((px, py, v.s, v.s * TILT))
    }
    fn cell(&self, x: i32, y: i32, out: &mut String) {
        let lot = &self.s.lot;
        if let Some(i) = lot.car_at(x, y) {
            let c = &lot.cars[i];
            let what = if c.len == 3 {
                tr(&["Bus facing #", "朝#的公交车", "#向きのバス", "# 방향 버스", "Autobús mirando #", "Ônibus virado #", "Bus tourné #", "Bus, Front #", "Автобус носом #", "朝#的公車"])
            } else {
                tr(&["Car facing #", "朝#的汽车", "#向きの車", "# 방향 자동차", "Coche mirando #", "Carro virado #", "Voiture tournée #", "Auto, Front #", "Машина носом #", "朝#的汽車"])
            };
            out.push_str(&what.replace('#', tr(&WAYS[c.dir as usize])));
            let part = if c.end(false) == (x, y) {
                tr(&["front", "车头", "前", "앞", "delante", "frente", "avant", "vorn", "перед", "車頭"])
            } else if c.end(true) == (x, y) {
                tr(&["back", "车尾", "後ろ", "뒤", "detrás", "traseira", "arrière", "hinten", "зад", "車尾"])
            } else {
                tr(&["middle", "中间", "中央", "가운데", "centro", "meio", "milieu", "Mitte", "середина", "中間"])
            };
            out.push_str(", ");
            out.push_str(part);
            if lot.way(i, false) == Way::Out {
                out.push_str(", ");
                out.push_str(tr(&["clear road ahead", "前方畅通", "前が空いている", "앞길이 비어 있음", "vía libre delante", "caminho livre à frente", "voie libre devant", "vorne frei", "впереди свободно", "前方暢通"]));
            } else if lot.way(i, true) == Way::Out {
                out.push_str(", ");
                out.push_str(tr(&["clear road behind", "后方畅通", "後ろが空いている", "뒷길이 비어 있음", "vía libre detrás", "caminho livre atrás", "voie libre derrière", "hinten frei", "сзади свободно", "後方暢通"]));
            }
        } else {
            out.push_str(match lot.prop(x, y) {
                CONE => tr(&["Cone", "锥桶", "コーン", "고깔", "Cono", "Cone", "Cône", "Pylon", "Конус", "錐筒"]),
                EMPTY => tr(&["Empty", "空", "空き", "비어 있음", "Vacío", "Vazio", "Vide", "Leer", "Пусто", "空"]),
                _ => tr(&["Crate", "木箱", "木箱", "상자", "Caja", "Caixa", "Caisse", "Kiste", "Ящик", "木箱"]),
            });
        }
        for d in 0..4 {
            let edge = match d {
                0 => y == 0,
                1 => x == lot.w - 1,
                2 => y == lot.h - 1,
                _ => x == 0,
            };
            if edge && lot.fenced(x, y, d) {
                out.push_str(", ");
                let b = tr(&["barrier #", "#有护栏", "#に柵", "# 방향 차단벽", "barrera #", "barreira #", "barrière #", "Absperrung #", "барьер #", "#有護欄"]);
                out.push_str(&b.replace('#', tr(&WAYS[d])));
            }
        }
    }
    fn status(&self, out: &mut String) {
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let left_l = tr(&["cars left", "辆车待开出", "台残り", "대 남음", "coches por sacar", "carros restantes", "voitures restantes", "Autos übrig", "машин осталось", "輛車待開出"]);
        let bumps_l = tr(&["bumps", "次碰撞", "回衝突", "번 충돌", "choques", "batidas", "chocs", "Rempler", "ударов", "次碰撞"]);
        out.push_str(&format!("{} {}, {} {}, {} {}", level_l, self.s.level + 1, self.s.lot.left(), left_l, self.s.bumps, bumps_l));
    }
    fn keys(&self) -> &'static str {
        tr(&[
            "Arrows move between squares, Space drives the car on that square the way it faces, X backs it up, I reads the level and cars left.",
            "方向键在格子间移动，空格让该格的车向前开，X 让它倒车，I 读出关卡和剩余车辆。",
            "矢印でマスを移動、スペースでそのマスの車を前へ、X でバック、I でレベルと残りの車。",
            "화살표로 칸 이동, 스페이스로 그 칸의 차를 앞으로, X로 후진, I로 레벨과 남은 차.",
            "Flechas para moverse entre casillas, Espacio conduce el coche de esa casilla hacia delante, X da marcha atrás, I lee nivel y coches que quedan.",
            "Setas movem entre as casas, Espaço dirige o carro daquela casa para a frente, X dá ré, I lê nível e carros restantes.",
            "Flèches pour changer de case, Espace avance la voiture de cette case, X recule, I lit le niveau et les voitures restantes.",
            "Pfeiltasten wechseln das Feld, Leertaste fährt das Auto dort vorwärts, X setzt zurück, I liest Level und übrige Autos vor.",
            "Стрелки переходят по клеткам, пробел ведёт машину на клетке вперёд, X сдаёт назад, I читает уровень и сколько машин осталось.",
            "方向鍵在格子間移動，空白鍵讓該格的車向前開，X 讓它倒車，I 讀出關卡和剩餘車輛。",
        ])
    }
    fn act(&mut self, x: i32, y: i32, a: i32, said: &mut String) -> u32 {
        if self.s.phase == CLEARED {
            let ev = self.next();
            self.status(said);
            return ev;
        }
        self.focus(x, y);
        let Some(i) = self.s.lot.car_at(x, y) else {
            said.push_str(tr(&["No car here", "这里没有车", "ここに車はありません", "여기에 차가 없습니다", "Aquí no hay coche", "Não há carro aqui", "Pas de voiture ici", "Hier steht kein Auto", "Здесь нет машины", "這裡沒有車"]));
            return 0;
        };
        let way = self.s.lot.way(i, a != 0);
        let ev = self.send(i, a != 0);
        match way {
            _ if ev == 0 => said.push_str(tr(&["Wait a moment", "请稍等", "少し待って", "잠시 기다리세요", "Espera un momento", "Espere um pouco", "Un instant", "Einen Moment", "Подождите", "請稍等"])),
            Way::Out => {
                said.push_str(tr(&["Drove out", "开出去了", "出ていった", "빠져나감", "Salió", "Saiu", "Sortie", "Rausgefahren", "Выехала", "開出去了"]));
                said.push_str(". ");
                if self.s.lot.left() == 0 {
                    said.push_str(tr(&["Level cleared! Space for the next level.", "过关！按空格进入下一关。", "クリア！スペースで次のレベル。", "레벨 클리어! 스페이스로 다음 레벨.", "¡Nivel superado! Espacio para el siguiente.", "Nível concluído! Espaço para o próximo.", "Niveau réussi ! Espace pour le suivant.", "Level geschafft! Leertaste für das nächste.", "Уровень пройден! Пробел для следующего.", "過關！按空白鍵進入下一關。"]));
                } else {
                    self.status(said);
                }
            }
            Way::Blocked { by, .. } => {
                said.push_str(if by >= 0 {
                    tr(&["Bumped into a car", "撞到了车", "車にぶつかった", "차에 부딪힘", "Chocó con un coche", "Bateu num carro", "Choc contre une voiture", "Gegen ein Auto gestoßen", "Врезалась в машину", "撞到了車"])
                } else {
                    tr(&["Bumped into something", "撞到了障碍", "障害物にぶつかった", "장애물에 부딪힘", "Chocó con un obstáculo", "Bateu num obstáculo", "Choc contre un obstacle", "Gegen ein Hindernis gestoßen", "Врезалась в препятствие", "撞到了障礙"])
                });
            }
        }
        ev
    }
    fn rev(&mut self) -> u32 {
        let v = [self.s.phase, self.s.level, self.tall as i32, wg::lang() as i32];
        wg::hash(wg::hash(0, wg::bytes_of(&self.s.lot)), wg::bytes_of(&v))
    }
}

wg::game!(Unpark);
wg::board!(Unpark);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Unpark, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    #[test]
    fn plays_every_level() {
        let mut g = Unpark::new();
        g.start(1);
        for level in 0..LEVELS {
            assert_eq!(g.s.level, level);
            // a bump first, when some car is stuck
            let lot = g.s.lot;
            if let Some(i) = (0..lot.n as usize).find(|&i| lot.way(i, false) != Way::Out) {
                assert_eq!(g.send(i, false), wg::EV_HIT);
                run(&mut g, 600);
            }
            let mut ev = 0;
            loop {
                let lot = g.s.lot;
                let Some((i, back)) = (0..lot.n as usize)
                    .filter(|&i| lot.cars[i].out == 0)
                    .find_map(|i| [false, true].into_iter().find(|&b| lot.way(i, b) == Way::Out).map(|b| (i, b)))
                else {
                    break;
                };
                assert_eq!(g.send(i, back), wg::EV_BLIP);
                assert!(g.loaded_check());
                ev |= run(&mut g, 50);
            }
            ev |= run(&mut g, 8000);
            assert!(ev & wg::EV_LEVEL != 0, "level {} not cleared", level + 1);
            if level + 1 < LEVELS {
                assert_eq!(g.s.phase, CLEARED);
                g.next();
            } else {
                assert!(g.over());
            }
        }
        assert_eq!(g.s.done, LEVELS);
    }

    #[test]
    fn clears_after_loading_mid_drive() {
        let mut g = Unpark::new();
        g.start(1);
        loop {
            let lot = g.s.lot;
            let Some(i) = (0..lot.n as usize).find(|&i| lot.cars[i].out == 0 && lot.free(i)) else { break };
            let back = lot.way(i, false) != Way::Out;
            g.send(i, back);
        }
        assert!(!g.drives.is_empty() && g.loaded());
        assert!(run(&mut g, 16) & wg::EV_LEVEL != 0);
    }

    #[test]
    fn every_route_ends_off_the_top() {
        let g = {
            let mut g = Unpark::new();
            g.start(1);
            g
        };
        let lot = g.s.lot;
        for i in 0..lot.n as usize {
            for d in 0..4 {
                let path = g.route(&lot.cars[i], d);
                let last = path.last().unwrap();
                assert_eq!(last.d, 0);
                assert!(last.x > lot.w as f32 && last.y < 0.0);
                assert!(path.len() <= 7);
            }
        }
    }

    impl Unpark {
        fn loaded_check(&mut self) -> bool {
            let keep = std::mem::take(&mut self.drives);
            let ok = self.loaded();
            self.drives = keep;
            ok
        }
    }
}
