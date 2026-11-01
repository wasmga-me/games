//! Boarding - an original wasmga.me game. MIT licensed; the buses' wheels,
//! the passengers and the trees are Kenney's Car Kit, Mini Characters and
//! Nature Kit (CC0), rendered by games-src/tools/render3d.
//! A depot is packed with buses of six colours and a queue waits at the
//! stop, each passenger for a bus of their colour. Tap a bus to drive it
//! the way it faces; with a clear road it drives round to a free bay at
//! the stop, and passengers of its colour board until it is full and
//! leaves. Five bays: fill them with buses nobody at the front wants and
//! the queue stops. 30 levels.
mod art;
mod glyphs;
mod puzzle;

use puzzle::{Bus, Lot, Way, DX, DY, LEVELS};
use wg::controls::{self, Icon, Row};
use wg::{hex, mix, tr, Game, Image, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 1;

const PLAY: i32 = 0;
const CLEARED: i32 = 1;
const DONE: i32 = 2;
/// every bay taken and nobody at the front of the queue fits
const STUCK: i32 = 3;

const BAYS: usize = 5;

/// the board's tilt: the ground's depth shows at this fraction of its width
const TILT: f32 = 0.848;
/// model units in a square
const CELL: f32 = 1.6;
/// the ring road's middle, squares out from the depot's edge, its width
/// and its kerb
const RING: f32 = 0.95;
const ROAD: f32 = 1.3;
const KERB: f32 = 0.12;
/// the bays: their open end by the ring road and their far end
const BAY_Y1: f32 = -(RING + ROAD / 2.0 + KERB + 0.08);
const BAY_Y0: f32 = BAY_Y1 - 2.7;
/// the pavement above the bays, and the queue along it
const WALK_Y0: f32 = BAY_Y0 - 1.15;
const QUEUE_Y: f32 = BAY_Y0 - 0.55;
const QUEUE_GAP: f32 = 0.5;
/// a minibus's and a bus's length, squares
const BUS_LEN: [f32; 2] = [2.9 * 0.9 / CELL, 4.4 * 0.9 / CELL];
/// how fast a bus drives, squares a second, and how fast it starts
const SPEED: f32 = 11.0;
const PULL: f32 = 0.18;
/// a bump: there and back, ms
const BUMP_MS: f32 = 420.0;
/// ms between passengers stepping out of the queue, and how fast they walk,
/// squares a second
const BOARD_MS: f32 = 140.0;
const WALK: f32 = 7.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 1],
    level: i32,
    phase: i32,
    lot: Lot,
    /// passengers gone from the queue
    head: i32,
    /// the bus in each bay (-1 for none) and its passengers
    bay_bus: [i8; 8],
    bay_fill: [i8; 8],
    retries: i32,
    level_ms: i32,
    time_ms: i32,
    all_retries: i32,
    all_boarded: i32,
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
unsafe impl wg::Plain for Bus {}

/// a bus driving to a bay (`bay`) or away (-1): its path and how far along
/// it is
struct Drive {
    colour: i8,
    kind: usize,
    bay: i32,
    path: Vec<Leg>,
    s: f32,
    /// ms since it set off
    t: f32,
}

/// One straight stretch of a drive: where it starts, its way and length,
/// and the way the bus faces along it (backing up, the other way).
#[derive(Clone, Copy)]
struct Leg {
    x: f32,
    y: f32,
    d: usize,
    len: f32,
    face: f32,
}

/// a passenger walking from the front of the queue onto a bus
struct Walker {
    colour: i8,
    body: usize,
    from: (f32, f32),
    to: (f32, f32),
    bay: usize,
    /// 0 to 1 along the way
    t: f32,
}

/// a bus bumping into another and back
#[derive(Clone, Copy)]
struct Bump {
    bus: usize,
    reach: f32,
    by: i32,
    t: f32,
}

/// where the depot is drawn: its top left and a square's width in plan units
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
    Bus(usize),
    Button,
    Ctl(Icon),
}

struct Boarding {
    s: Save,
    hover: Hit,
    keys: bool,
    sprites: Vec<Image>,
    drives: Vec<Drive>,
    walkers: Vec<Walker>,
    bump: Option<Bump>,
    /// ms until the next passenger may step out
    wait: f32,
    /// how far the queue still has to shuffle forward, places
    shift: f32,
    /// the bays flash red, 1 to 0, when a bus finds none free
    full: f32,
    /// ms, for walking feet
    clock: f32,
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

/// the way from a to b along a straight leg
fn way_to(a: (f32, f32), b: (f32, f32)) -> usize {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    if dx.abs() > dy.abs() {
        if dx > 0.0 { 1 } else { 3 }
    } else if dy > 0.0 {
        2
    } else {
        0
    }
}

fn kind(b: &Bus) -> usize {
    (b.len == 3) as usize
}

/// a bus's squares as a box on the ground, in squares
fn bus_box(b: &Bus) -> (f32, f32, f32, f32) {
    let (ax, ay) = (b.x as i32, b.y as i32);
    let (bx, by) = b.front();
    (ax.min(bx) as f32, ay.min(by) as f32, ax.max(bx) as f32 + 1.0, ay.max(by) as f32 + 1.0)
}

const PLAN: (f32, f32) = (300.0, 400.0);

impl Boarding {
    /// Fits the stop, the depot and its ring road between the top bar and
    /// the bottom of the screen.
    fn view(&self) -> View {
        let (w, h) = PLAN;
        let lot = &self.s.lot;
        let (top, bottom) = (40.0, h - 8.0);
        let edge = RING + ROAD / 2.0 + KERB + 0.1;
        let (gx0, gx1) = (-edge, lot.w as f32 + edge);
        let (gy0, gy1) = (WALK_Y0 - 0.1, lot.h as f32 + edge);
        // the passengers stand up out of the pavement by about a square
        let rise = 0.9;
        let s = ((w - 12.0) / (gx1 - gx0)).min((bottom - top) / ((gy1 - gy0) * TILT + rise));
        let x = w / 2.0 - (gx0 + gx1) / 2.0 * s;
        let y = top + rise * s - gy0 * s * TILT + ((bottom - top) - ((gy1 - gy0) * TILT + rise) * s) / 2.0;
        View { x, y, s }
    }

    /// bay k's middle across, squares
    fn bay_x(&self, k: usize) -> f32 {
        let (x0, x1) = bay_span(&self.s.lot);
        x0 + (k as f32 + 0.5) * (x1 - x0) / BAYS as f32
    }
    /// where a bus of this kind stands in a bay, squares down
    fn bay_y(kind: usize) -> f32 {
        BAY_Y0 + BUS_LEN[kind] / 2.0 + 0.12
    }
    /// the front of the queue, squares across
    fn queue_x(&self) -> f32 {
        bay_span(&self.s.lot).1 - 0.35
    }
    /// passengers standing in sight; the rest show as a number
    fn in_sight(&self) -> i32 {
        let (x0, _) = bay_span(&self.s.lot);
        ((self.queue_x() - x0 - 0.7) / QUEUE_GAP) as i32 + 1
    }

    fn card(&self) -> (f32, f32, f32, f32) {
        let (w, h) = PLAN;
        let (pw, ph) = (248.0, if self.s.phase == DONE { 96.0 } else { 158.0 });
        // low, so the stop and the queue show above it
        (w / 2.0 - pw / 2.0, h * 0.64 - ph / 2.0, pw, ph)
    }
    fn button(&self) -> (f32, f32, f32, f32) {
        let (px, py, pw, ph) = self.card();
        let (bw, bh) = (pw - 48.0, 42.0);
        (px + (pw - bw) / 2.0, py + ph - bh - 20.0, bw, bh)
    }

    /// The bus under a point: its squares, reaching up by its height,
    /// the nearest one wins.
    fn bus_at(&self, px: f32, py: f32) -> Option<usize> {
        let v = self.view();
        let (gx, gy) = v.ground(px, py);
        let lot = &self.s.lot;
        let mut best: Option<(usize, f32)> = None;
        for i in 0..lot.n as usize {
            let b = &lot.buses[i];
            if b.out != 0 {
                continue;
            }
            let (x0, y0, x1, y1) = bus_box(b);
            // the roof shows above the squares
            if gx >= x0 && gx < x1 && gy >= y0 - 0.7 && gy < y1 && best.is_none_or(|b| y1 > b.1) {
                best = Some((i, y1));
            }
        }
        best.map(|b| b.0)
    }

    /// Restart, in the top bar while a level is on
    fn controls(&self) -> Row {
        let row = controls::row(&[Icon::Restart], PLAN.0 - 8.0, 20.0);
        if self.s.phase == PLAY { row } else { row.hidden() }
    }
    fn can_restart(&self) -> bool {
        let lot = &self.s.lot;
        self.s.phase == PLAY && lot.buses[..lot.n as usize].iter().any(|b| b.out != 0)
    }
    /// The lot as dealt; counted as a retry, as getting stuck is.
    fn restart(&mut self) -> u32 {
        if !self.can_restart() {
            return 0;
        }
        self.retry();
        wg::EV_BLIP
    }
    /// The level again, counted as a retry.
    fn retry(&mut self) {
        self.s.retries += 1;
        self.s.all_retries += 1;
        self.begin(self.s.level);
    }

    fn hit(&self, x: f32, y: f32) -> Hit {
        if let Some(i) = self.controls().hit(x, y) {
            return Hit::Ctl(i);
        }
        let inside = |(bx, by, bw, bh): (f32, f32, f32, f32)| x >= bx && x < bx + bw && y >= by && y < by + bh;
        match self.s.phase {
            PLAY => self.bus_at(x, y).map_or(Hit::None, Hit::Bus),
            CLEARED | STUCK if inside(self.button()) => Hit::Button,
            _ => Hit::None,
        }
    }

    fn begin(&mut self, level: i32) {
        let s = &mut self.s;
        s.level = level;
        s.phase = PLAY;
        s.lot = puzzle::deal(level);
        s.head = 0;
        s.bay_bus = [-1; 8];
        s.bay_fill = [0; 8];
        s.level_ms = 0;
        s.cx = s.cx.clamp(0, s.lot.w - 1);
        s.cy = s.cy.clamp(0, s.lot.h - 1);
        self.drives.clear();
        self.walkers.clear();
        self.bump = None;
        self.wait = 0.0;
        self.shift = 0.0;
        self.full = 0.0;
    }

    /// Sends bus i the way it faces: round to the first free bay, or into
    /// what is in its way.
    fn send(&mut self, i: usize) -> u32 {
        let lot = &self.s.lot;
        if self.s.phase != PLAY || i >= lot.n as usize || lot.buses[i].out != 0 || self.bump.is_some_and(|b| b.bus == i) {
            return 0;
        }
        let b = lot.buses[i];
        match lot.way(i) {
            Way::Out => {
                let Some(k) = (0..BAYS).find(|&k| self.s.bay_bus[k] < 0) else {
                    self.full = 1.0;
                    return wg::EV_HIT;
                };
                self.s.lot.buses[i].out = 1;
                self.s.bay_bus[k] = i as i8;
                self.s.bay_fill[k] = 0;
                let path = self.route_in(&b, k);
                self.drives.push(Drive { colour: b.colour, kind: kind(&b), bay: k as i32, path, s: 0.0, t: 0.0 });
                wg::EV_BLIP
            }
            Way::Blocked { gap, by } => {
                // a bump finishes before the next one starts
                if self.bump.is_some() {
                    return 0;
                }
                self.bump = Some(Bump { bus: i, reach: gap as f32 + 0.16, by, t: 0.0 });
                wg::EV_HIT
            }
        }
    }

    /// The way to bay k: straight off the depot onto the ring road, round
    /// it the shorter way to below the bay, and up into it.
    fn route_in(&self, b: &Bus, k: usize) -> Vec<Leg> {
        let lot = &self.s.lot;
        let (w, h) = (lot.w as f32, lot.h as f32);
        let (x0, y0, x1, y1) = bus_box(b);
        let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let d = b.dir as usize;
        let at = match d {
            0 => (cx, -RING),
            1 => (w + RING, cy),
            2 => (cx, h + RING),
            _ => (-RING, cy),
        };
        let mut legs = vec![Leg { x: cx, y: cy, d, len: (at.0 - cx).abs() + (at.1 - cy).abs(), face: heading(d) }];
        let bx = self.bay_x(k);
        ring(&mut legs, w, h, at, (bx, -RING));
        let stop = Self::bay_y(kind(b));
        legs.push(Leg { x: bx, y: -RING, d: 0, len: -RING - stop, face: 0.0 });
        legs
    }

    /// The way out of bay k: back down onto the ring road, along to the
    /// top right corner and up the road out.
    fn route_out(&self, k: usize, kind: usize) -> Vec<Leg> {
        let lot = &self.s.lot;
        let (bx, by) = (self.bay_x(k), Self::bay_y(kind));
        let corner = lot.w as f32 + RING;
        vec![
            Leg { x: bx, y: by, d: 2, len: -RING - by, face: 0.0 },
            Leg { x: bx, y: -RING, d: 1, len: corner - bx, face: 90.0 },
            Leg { x: corner, y: -RING, d: 0, len: 14.0, face: 0.0 },
        ]
    }

    /// whether the bus in bay k is standing in it, not still driving there
    fn arrived(&self, k: usize) -> bool {
        self.s.bay_bus[k] >= 0 && !self.drives.iter().any(|d| d.bay == k as i32)
    }
    /// the bay with the fullest bus of colour c that has a seat left
    fn bay_for(&self, c: i8) -> Option<usize> {
        (0..BAYS)
            .filter(|&k| self.arrived(k))
            .filter(|&k| {
                let b = &self.s.lot.buses[self.s.bay_bus[k] as usize];
                b.colour == c && (self.s.bay_fill[k] as i32) < b.seats()
            })
            .max_by_key(|&k| self.s.bay_fill[k])
    }

    /// Full buses leave, the front passenger boards a bus of their colour,
    /// and the level clears or sticks once everything has stopped moving.
    fn step(&mut self, dt: f32) -> u32 {
        let mut ev = 0;
        for k in 0..BAYS {
            let i = self.s.bay_bus[k];
            if !self.arrived(k) || self.walkers.iter().any(|w| w.bay == k) {
                continue;
            }
            let b = self.s.lot.buses[i as usize];
            if self.s.bay_fill[k] as i32 >= b.seats() {
                self.s.bay_bus[k] = -1;
                self.s.bay_fill[k] = 0;
                let path = self.route_out(k, kind(&b));
                self.drives.push(Drive { colour: b.colour, kind: kind(&b), bay: -1, path, s: 0.0, t: 0.0 });
                ev |= wg::EV_BLIP;
            }
        }
        self.wait -= dt;
        let lot = &self.s.lot;
        let front = (self.s.head < lot.qn).then(|| lot.queue[self.s.head as usize]);
        if let Some(c) = front {
            if let Some(k) = self.bay_for(c) {
                if self.wait <= 0.0 {
                    self.s.bay_fill[k] += 1;
                    self.s.all_boarded += 1;
                    let body = body(self.s.head);
                    self.s.head += 1;
                    let door = (self.bay_x(k) + 0.32, BAY_Y0 + 0.2);
                    self.walkers.push(Walker { colour: c, body, from: (self.queue_x(), QUEUE_Y), to: door, bay: k, t: 0.0 });
                    self.shift = (self.shift + 1.0).min(3.0);
                    self.wait = BOARD_MS;
                }
            } else if self.drives.is_empty() && self.walkers.is_empty() && self.s.bay_bus[..BAYS].iter().all(|&i| i >= 0) {
                self.s.phase = STUCK;
                return ev | wg::EV_LOSE;
            }
        } else if self.s.lot.left() == 0 && self.drives.is_empty() && self.walkers.is_empty() && self.s.bay_bus[..BAYS].iter().all(|&i| i < 0) {
            ev |= self.clear_level();
        }
        ev
    }

    fn clear_level(&mut self) -> u32 {
        let s = &mut self.s;
        let n = s.lot.qn;
        let score = (20 * n - 100 * s.retries).max(5 * n);
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

    /// The card's button: the next level, or the same one again.
    fn press_button(&mut self) -> u32 {
        match self.s.phase {
            CLEARED => {
                self.s.retries = 0;
                self.begin(self.s.level + 1);
            }
            STUCK => self.retry(),
            _ => return 0,
        }
        wg::EV_BLIP
    }

    fn cursor_move(&mut self, dx: i32, dy: i32) {
        self.s.cx = (self.s.cx + dx).clamp(0, self.s.lot.w - 1);
        self.s.cy = (self.s.cy + dy).clamp(0, self.s.lot.h - 1);
    }

    fn bus_sprite(colour: i8, kind: usize, face: f32) -> usize {
        // 8 views, a turn of 45 degrees apart
        let k = ((face.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8;
        art::MINI_0_0 + colour as usize * 16 + kind * 8 + k
    }
    /// a passenger: 0 standing, 1 and 2 the steps of a walk
    fn person_sprite(colour: i8, body: usize, pose: usize) -> usize {
        art::FEMALE_A_0_STAND + colour as usize * 6 + body * 3 + pose
    }

    /// Draws sprite `i` with the model's origin at ground point (gx, gy),
    /// mirrored left to right if `flip`.
    fn put(&self, s: &mut Screen, v: &View, i: usize, gx: f32, gy: f32, flip: bool) {
        let a = &art::ARTS[i];
        let (px, py) = v.at(gx, gy);
        let k = v.s / CELL / a.ppu;
        let (w, h) = (a.w as f32 * k, a.h as f32 * k);
        if flip {
            s.image(&self.sprites[i], px - (a.w as f32 - a.ax) * k, py - a.ay * k, -w, h, 1.0);
        } else {
            s.image(&self.sprites[i], px - a.ax * k, py - a.ay * k, w, h, 1.0);
        }
    }

    /// A spot of a passenger's colour on the ground under them, so the
    /// colours read from afar.
    fn spot(&self, s: &mut Screen, v: &View, c: i8, gx: f32, gy: f32) {
        let (px, py) = v.at(gx, gy);
        s.ellipse(px, py, v.s * 0.24, v.s * 0.24 * TILT * 0.7, mix(hex(0xffffff), colour(c), 0.15), 0.9);
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

    /// The seats on a bus in a bay, two rows of dots on its roof: filled
    /// for each passenger aboard.
    fn seats(&self, s: &mut Screen, v: &View, k: usize) {
        let i = self.s.bay_bus[k] as usize;
        let b = &self.s.lot.buses[i];
        let (px, py) = v.at(self.bay_x(k), Self::bay_y(kind(b)));
        let rows = b.seats() / 2;
        let gap = v.s * 0.26;
        let r = v.s * 0.085;
        // the roof stands about half a square up
        let top = py - v.s * 0.62 - (rows - 1) as f32 * gap * TILT / 2.0;
        let c = colour(b.colour);
        for n in 0..b.seats() {
            let (x, y) = (px + (n % 2) as f32 * gap - gap / 2.0, top + (n / 2) as f32 * gap * TILT);
            s.disc(x, y + 1.0, r + 1.2, hex(0x000000), 0.25);
            if n < self.s.bay_fill[k] as i32 {
                s.disc(x, y, r + 1.0, hex(0xffffff), 1.0);
                s.disc(x, y, r, mix(hex(0xffffff), c, 0.1), 1.0);
            } else {
                s.disc(x, y, r + 1.0, hex(0xffffff), 0.9);
                s.disc(x, y, r, mix(hex(0x1d2330), c, 0.55), 1.0);
            }
        }
    }
}

/// the bays' span across, squares: to short of the road out, from where
/// the first bay's middle is on the ring road's line, so buses drive
/// straight up into it
fn bay_span(lot: &Lot) -> (f32, f32) {
    let x1 = lot.w as f32 + RING - ROAD / 2.0 - KERB - 0.2;
    // x0 + (x1 - x0) / (2 * BAYS) = -RING
    let k = 1.0 / (2.0 * BAYS as f32);
    ((-RING - x1 * k) / (1.0 - k), x1)
}

/// a passenger's body, the same for the same place in the queue
fn body(n: i32) -> usize {
    ((n * 5 + n / 3) % 2) as usize
}

/// Adds legs round the ring road from `at` to `to`, both on it, the
/// shorter way.
fn ring(legs: &mut Vec<Leg>, w: f32, h: f32, at: (f32, f32), to: (f32, f32)) {
    let (sw, sh) = (w + 2.0 * RING, h + 2.0 * RING);
    let perim = 2.0 * (sw + sh);
    // distance clockwise from the top left corner
    let along = |(x, y): (f32, f32)| -> f32 {
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
    let corners = [(-RING, -RING), (w + RING, -RING), (w + RING, h + RING), (-RING, h + RING)];
    let marks = [0.0, sw, sw + sh, 2.0 * sw + sh];
    let cw = (along(to) - along(at)).rem_euclid(perim);
    let clockwise = cw <= perim - cw;
    let mut left = if clockwise { cw } else { perim - cw };
    let mut here = at;
    let mut p = along(at);
    while left > 0.001 {
        // the next corner the chosen way
        let (k, gap) = (0..4)
            .map(|k| (k, if clockwise { (marks[k] - p).rem_euclid(perim) } else { (p - marks[k]).rem_euclid(perim) }))
            .filter(|&(_, g)| g > 0.001)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap();
        let next = if gap >= left { to } else { corners[k] };
        let len = gap.min(left);
        let d = way_to(here, next);
        legs.push(Leg { x: here.0, y: here.1, d, len, face: heading(d) });
        left -= len;
        here = next;
        p = marks[k];
    }
}

fn colour(c: i8) -> u32 {
    hex([0xe5484d, 0xf28c28, 0xf5c518, 0x3fb950, 0x3d7be0, 0x9b5de5][c as usize])
}

const COLOUR_NAMES: [[&str; 10]; 6] = [
    ["red", "红色", "赤", "빨강", "rojo", "vermelho", "rouge", "rot", "красный", "紅色"],
    ["orange", "橙色", "オレンジ", "주황", "naranja", "laranja", "orange", "orange", "оранжевый", "橙色"],
    ["yellow", "黄色", "黄", "노랑", "amarillo", "amarelo", "jaune", "gelb", "жёлтый", "黃色"],
    ["green", "绿色", "緑", "초록", "verde", "verde", "vert", "grün", "зелёный", "綠色"],
    ["blue", "蓝色", "青", "파랑", "azul", "azul", "bleu", "blau", "синий", "藍色"],
    ["purple", "紫色", "紫", "보라", "morado", "roxo", "violet", "lila", "фиолетовый", "紫色"],
];

impl Game for Boarding {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let sheet = Image::png(art::SHEET);
        let sprites = art::ARTS.iter().map(|a| sheet.crop(a.x, a.y, a.w, a.h)).collect();
        Boarding {
            s: blank(),
            hover: Hit::None,
            keys: false,
            sprites,
            drives: Vec::new(),
            walkers: Vec::new(),
            bump: None,
            wait: 0.0,
            shift: 0.0,
            full: 0.0,
            clock: 0.0,
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
    fn plan(&self, _w: i32, _h: i32) -> (f32, f32) {
        PLAN
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        self.clock += dt;
        for d in self.drives.iter_mut() {
            d.t += dt;
            // pulls away, then drives on at full speed
            let v = SPEED * (d.t / 1000.0 / PULL).min(1.0);
            d.s += v * dt / 1000.0;
        }
        let total = |d: &Drive| d.path.iter().map(|l| l.len).sum::<f32>();
        self.drives.retain(|d| d.s < total(d));
        for w in self.walkers.iter_mut() {
            let len = ((w.to.0 - w.from.0).powi(2) + (w.to.1 - w.from.1).powi(2)).sqrt().max(0.1);
            w.t += WALK * dt / 1000.0 / len;
        }
        self.walkers.retain(|w| w.t < 1.0);
        self.shift = (self.shift - dt / 1000.0 * 9.0).max(0.0);
        self.full = (self.full - dt / 600.0).max(0.0);
        if let Some(b) = &mut self.bump {
            b.t += dt / BUMP_MS;
            if b.t >= 1.0 {
                self.bump = None;
            }
        }
        if self.s.phase == PLAY {
            ev |= self.step(dt);
        }
        if self.s.phase == PLAY && self.s.lot.left() < self.s.lot.n {
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
            if input.pressed & wg::A != 0 {
                self.keys = true;
                if let Some(i) = self.s.lot.bus_at(self.s.cx, self.s.cy) {
                    ev |= self.send(i);
                }
            }
        } else if input.pressed & wg::A != 0 {
            ev |= self.press_button();
        }
        ev
    }

    fn key(&mut self, code: i32) -> bool {
        controls::is_restart_key(code) && self.restart() != 0
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.hover = Hit::None;
            return 0;
        }
        let h = self.hit(p.x, p.y);
        self.hover = if p.touch { Hit::None } else { h };
        if p.pressed & wg::M_LEFT == 0 {
            return 0;
        }
        self.keys = false;
        match h {
            Hit::Bus(i) => {
                let (x0, y0, ..) = bus_box(&self.s.lot.buses[i]);
                self.s.cx = x0 as i32;
                self.s.cy = y0 as i32;
                self.send(i)
            }
            Hit::Button => self.press_button(),
            Hit::Ctl(_) => self.restart(),
            Hit::None => 0,
        }
    }

    fn picture(&mut self) -> u32 {
        let hv = match self.hover {
            Hit::None => -1,
            Hit::Bus(i) => i as i32,
            Hit::Button => 1000,
            Hit::Ctl(i) => 1001 + i as i32,
        };
        let shown = [hv, self.keys as i32];
        let mut c = self.s;
        c.level_ms = 0;
        c.time_ms = 0;
        c.rep = KeyRepeat(0);
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        !self.drives.is_empty() || !self.walkers.is_empty() || self.bump.is_some() || self.shift > 0.0 || self.full > 0.0
    }

    fn draw(&mut self, s: &mut Screen) {
        let v = self.view();
        let (w, _) = PLAN;
        let lot = self.s.lot;
        let key = wg::hash(0, wg::bytes_of(&[self.s.level, lot.w, lot.h]));
        s.background(key, |s| {
            draw_ground(s, &v, &lot);
            self.draw_trees(s, &v, &lot);
        });

        // the bays flash when a bus finds none free
        if self.full > 0.0 {
            let (x0, x1) = bay_span(&lot);
            let half = (x1 - x0) / BAYS as f32 / 2.0 - 0.06;
            for k in 0..BAYS {
                let (ax, ay) = v.at(self.bay_x(k) - half, BAY_Y0);
                let (bx, by) = v.at(self.bay_x(k) + half, BAY_Y1);
                s.rect(ax, ay, bx - ax, by - ay, 4.0, hex(0xff4d4d), 0.35 * self.full);
            }
        }

        // everything standing on the ground, back to front
        let mut items: Vec<(f32, Item)> = Vec::new();
        for i in 0..lot.n as usize {
            let b = &lot.buses[i];
            if b.out == 0 {
                let (_, y0, _, y1) = bus_box(b);
                items.push(((y0 + y1) / 2.0, Item::Bus(i)));
            }
        }
        for k in 0..BAYS {
            if self.arrived(k) {
                let b = &lot.buses[self.s.bay_bus[k] as usize];
                items.push((Self::bay_y(kind(b)), Item::Bay(k)));
            }
        }
        for (k, d) in self.drives.iter().enumerate() {
            items.push((Self::drive_at(d).1, Item::Drive(k)));
        }
        let waiting = lot.qn - self.s.head;
        // the rest of the queue as a number, in front of the last in sight
        for n in (0..waiting.min(self.in_sight() + 1)).rev() {
            let d = if n == self.in_sight() { 0.01 } else { -0.001 * n as f32 };
            items.push((QUEUE_Y + d, Item::Queue(n)));
        }
        for (k, wk) in self.walkers.iter().enumerate() {
            items.push((wk.from.1 + (wk.to.1 - wk.from.1) * wk.t, Item::Walker(k)));
        }
        items.sort_by(|a, b| a.0.total_cmp(&b.0));

        let marked = match (self.hover, self.keys && self.s.phase == PLAY) {
            (Hit::Bus(i), _) => Some(i),
            (_, true) => lot.bus_at(self.s.cx, self.s.cy),
            _ => None,
        };
        if self.keys && self.s.phase == PLAY {
            // the cursor's square
            let (x, y) = v.at(self.s.cx as f32, self.s.cy as f32);
            s.rect(x + 1.0, y + 1.0, v.s - 2.0, v.s * TILT - 2.0, 4.0, hex(0xffe066), 0.35);
        }
        let step = (self.clock / 130.0) as usize % 2;
        for (_, item) in items {
            match item {
                Item::Bus(i) => {
                    let b = &lot.buses[i];
                    let (x0, y0, x1, y1) = bus_box(b);
                    let (mut gx, mut gy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                    if let Some(bp) = self.bump {
                        let d = lot.buses[bp.bus].dir as usize;
                        // there and back, quick out and slower back
                        if bp.bus == i {
                            let out = if bp.t < 0.45 { wg::ease_out(bp.t / 0.45) } else { 1.0 - wg::ease_out((bp.t - 0.45) / 0.55) };
                            gx += DX[d] as f32 * bp.reach * out;
                            gy += DY[d] as f32 * bp.reach * out;
                        } else if bp.by == i as i32 && bp.t > 0.4 {
                            // the bus hit rocks a little
                            let k = (1.0 - (bp.t - 0.4) / 0.6) * ((bp.t - 0.4) * 60.0).sin() * 0.05;
                            gx += DX[d] as f32 * k;
                            gy += DY[d] as f32 * k;
                        }
                    }
                    if marked == Some(i) {
                        let (px, py) = v.at(x0, y0);
                        s.shadow(px, py, (x1 - x0) * v.s, (y1 - y0) * v.s * TILT, 6.0, 7.0, hex(0xffffff), if self.keys { 0.8 } else { 0.55 });
                    }
                    self.put(s, &v, Self::bus_sprite(b.colour, kind(b), heading(b.dir as usize)), gx, gy, false);
                    if marked == Some(i) && self.bump.is_none() {
                        arrow(s, &v, b);
                    }
                }
                Item::Bay(k) => {
                    let b = &lot.buses[self.s.bay_bus[k] as usize];
                    self.put(s, &v, Self::bus_sprite(b.colour, kind(b), 0.0), self.bay_x(k), Self::bay_y(kind(b)), false);
                    self.seats(s, &v, k);
                }
                Item::Drive(k) => {
                    let d = &self.drives[k];
                    let (gx, gy, face) = Self::drive_at(d);
                    self.put(s, &v, Self::bus_sprite(d.colour, d.kind, face), gx, gy, false);
                }
                Item::Queue(n) => {
                    let at = self.s.head + n;
                    let x = self.queue_x() - (n as f32 + self.shift) * QUEUE_GAP;
                    if n == self.in_sight() {
                        // the rest of the queue, as a number
                        let (px, py) = v.at(x + QUEUE_GAP * 0.6, QUEUE_Y);
                        let label = format!("+{}", waiting - n);
                        let size = (v.s * 0.42).max(11.0);
                        let (tw, th) = (s.text_width(&label, size) + size * 0.9, size * 1.5);
                        s.rect(px - tw / 2.0, py - v.s * 0.5 - th / 2.0, tw, th, th / 2.0, hex(0x1d2330), 0.9);
                        s.text_c(&label, px, py - v.s * 0.5, size, hex(0xffffff), 1.0);
                    } else {
                        let pose = if self.shift > 0.0 { 1 + (step + n as usize) % 2 } else { 0 };
                        let c = lot.queue[at as usize];
                        self.spot(s, &v, c, x, QUEUE_Y);
                        self.put(s, &v, Self::person_sprite(c, body(at), pose), x, QUEUE_Y, false);
                    }
                }
                Item::Walker(k) => {
                    let wk = &self.walkers[k];
                    let (x, y) = (wk.from.0 + (wk.to.0 - wk.from.0) * wk.t, wk.from.1 + (wk.to.1 - wk.from.1) * wk.t);
                    self.spot(s, &v, wk.colour, x, y);
                    self.put(s, &v, Self::person_sprite(wk.colour, wk.body, 1 + step), x, y, wk.to.0 < wk.from.0);
                }
            }
        }
        if let Some(bp) = self.bump {
            // a red mark where it hit
            if bp.t > 0.35 && bp.t < 0.9 {
                let b = &lot.buses[bp.bus];
                let d = b.dir as usize;
                let (x, y) = b.front();
                let (fx, fy) = (x as f32 + 0.5 + DX[d] as f32 * (0.5 + bp.reach), y as f32 + 0.5 + DY[d] as f32 * (0.5 + bp.reach));
                let (px, py) = v.at(fx, fy);
                let a = 1.0 - (bp.t - 0.35) / 0.55;
                let r = v.s * (0.18 + (bp.t - 0.35) * 0.5);
                s.disc(px, py - v.s * 0.35, r, hex(0xff5a4e), 0.85 * a);
                s.text_c("!", px, py - v.s * 0.35, r * 1.5, hex(0xffffff), a);
            }
        }

        // the top bar
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let left_l = tr(&["Waiting", "等候", "待ち", "대기", "Esperan", "Esperando", "En attente", "Wartend", "Ждут", "等候"]);
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
        let hover = if let Hit::Ctl(i) = self.hover { Some(i) } else { None };
        let right = self.controls().draw(s, hover, |_| self.can_restart(), w);
        shaded(s, &format!("{} {}", left_l, waiting), right - 14.0, 20.0, 15.0, th.dim, true);
        if self.s.phase != PLAY {
            self.draw_card(s, w);
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
            1 => s.all_boarded,
            2 => s.all_retries,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let lot = &s.lot;
        let bays_ok = (0..BAYS).all(|k| {
            let i = s.bay_bus[k];
            i == -1 && s.bay_fill[k] == 0
                || i >= 0 && i < lot.n as i8 && lot.buses[i as usize].out == 1 && (0..=lot.buses[i as usize].seats() as i8).contains(&s.bay_fill[k])
        });
        let ok = s.version == VERSION
            && (0..LEVELS).contains(&s.opt[0])
            && (0..LEVELS).contains(&s.level)
            && (PLAY..=STUCK).contains(&s.phase)
            && lot.valid()
            && (0..=lot.qn).contains(&s.head)
            && bays_ok
            && (0..lot.w).contains(&s.cx)
            && (0..lot.h).contains(&s.cy);
        self.drives.clear();
        self.walkers.clear();
        self.bump = None;
        self.hover = Hit::None;
        self.shift = 0.0;
        ok
    }
}

/// An arrow on the ground ahead of a bus, the way it drives on a tap.
fn arrow(s: &mut Screen, v: &View, b: &Bus) {
    let d = b.dir as usize;
    let (x, y) = b.front();
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

fn blank() -> Save {
    Save {
        version: VERSION,
        opt: [0],
        level: 0,
        phase: PLAY,
        lot: Lot::empty(4, 4),
        head: 0,
        bay_bus: [-1; 8],
        bay_fill: [0; 8],
        retries: 0,
        level_ms: 0,
        time_ms: 0,
        all_retries: 0,
        all_boarded: 0,
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
    Bus(usize),
    Bay(usize),
    Drive(usize),
    Queue(i32),
    Walker(usize),
}

struct Theme {
    grass: u32,
    grass2: u32,
    road: u32,
    lot: u32,
    paint: u32,
    kerb: u32,
    pavement: u32,
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
    pavement: 0xd9d4c7,
    text: 0xffffff,
    dim: 0xe4f2dc,
    card: 0x2f3540,
    accent: 0x3d7be0,
};

/// The grass, the ring road and the road out, the depot, the bays and the
/// pavement: drawn once a level.
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
    // the pavement along the top, the kerb under it
    let (_, py0) = v.at(0.0, WALK_Y0);
    let (_, py1) = v.at(0.0, BAY_Y0);
    s.rect(x0, py0, x1 - x0, py1 - py0 + 3.0, 0.0, hex(0xa9a497), 1.0);
    s.rect(x0, py0, x1 - x0, py1 - py0, 0.0, hex(th.pavement), 1.0);
    // paving slabs
    let slab = v.s * 0.75;
    let mut x = v.x - (((v.x - x0) / slab).ceil() * slab);
    while x < x1 {
        s.line(x, py0, x, py1, 0.6, hex(0xb9b3a4), 0.6);
        x += slab;
    }
    s.line(x0, (py0 + py1) / 2.0, x1, (py0 + py1) / 2.0, 0.6, hex(0xb9b3a4), 0.6);
    let ring_rect = |s: &mut Screen, grow: f32, c: u32| {
        let (ax, ay) = v.at(-RING - half - grow, -RING - half - grow);
        let (bx, by) = v.at(w + RING + half + grow, h + RING + half + grow);
        s.rect(ax, ay, bx - ax, by - ay, v.s * (0.9 + grow), hex(c), 1.0);
    };
    // the kerb and the road, and the bays' tarmac from it up to the pavement
    ring_rect(s, KERB, th.kerb);
    ring_rect(s, 0.0, th.road);
    let (bx0, bx1) = bay_span(lot);
    let (ax, ay) = v.at(bx0 - 0.1, BAY_Y0);
    let (bx, by) = v.at(bx1 + 0.1, -RING);
    s.rect(ax, ay, bx - ax, by - ay, 0.0, hex(th.road), 1.0);
    // a line between bays, and a yellow box at the head of each
    let bw = (bx1 - bx0) / BAYS as f32;
    for k in 0..=BAYS {
        let gx = bx0 + k as f32 * bw;
        let (px, pa) = v.at(gx, BAY_Y0 + 0.08);
        let (_, pb) = v.at(gx, BAY_Y1);
        s.line(px, pa, px, pb, v.s * 0.03, hex(th.paint), 0.85);
    }
    for k in 0..BAYS {
        let (ax, ay) = v.at(bx0 + k as f32 * bw + 0.18, BAY_Y0 + 0.12);
        let (bx, _) = v.at(bx0 + (k + 1) as f32 * bw - 0.18, 0.0);
        s.line(ax, ay, bx, ay, v.s * 0.03, hex(0xf3d36b), 0.9);
    }
    // the road out, up from the top right corner across the pavement
    let (rx, _) = v.at(w + RING - half - KERB, 0.0);
    let (_, ry) = v.at(0.0, -RING);
    s.rect(rx, y0 - 2.0, (ROAD + 2.0 * KERB) * v.s, ry - y0 + 2.0, 0.0, hex(th.kerb), 1.0);
    s.rect(rx + KERB * v.s, y0 - 2.0, ROAD * v.s, ry - y0 + 2.0, 0.0, hex(th.road), 1.0);
    // a zebra crossing where it meets the pavement
    let stripes = 5;
    for i in 0..stripes {
        let sx = rx + KERB * v.s + (i as f32 + 0.2) * ROAD * v.s / stripes as f32;
        s.rect(sx, py0 + 2.0, ROAD * v.s / stripes as f32 * 0.6, py1 - py0 - 4.0, 1.0, hex(th.paint), 0.9);
    }
    // the depot, a step up
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
    // bays: a faint grid
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
}

impl Boarding {
    /// Trees and bushes on the grass, clear of the roads, the stop and the
    /// top bar.
    fn draw_trees(&self, s: &mut Screen, v: &View, lot: &Lot) {
        let (x0, y0, x1, y1) = s.visible();
        let (gx0, gy0) = v.ground(x0, y0);
        let (gx1, gy1) = v.ground(x1, y1);
        let (w, h) = (lot.w as f32, lot.h as f32);
        let out = RING + ROAD / 2.0 + 1.1;
        let road_x = w + RING;
        let (_, bar0) = v.ground(0.0, -6.0);
        let (_, bar1) = v.ground(0.0, 46.0);
        let mut rng = wg::Rng::new(4242);
        const STEP: f32 = 1.5;
        let kinds = [art::TREE_DEFAULT, art::TREE_OAK, art::TREE_DETAILED, art::TREE_FAT, art::BUSHLARGE, art::BUSHLARGE];
        let mut y = (gy0 / STEP).floor() * STEP;
        while y < gy1 + 2.0 {
            let mut x = (gx0 / STEP).floor() * STEP;
            while x < gx1 + 1.0 {
                let (tx, ty) = (x + rng.float() * STEP * 0.8, y + rng.float() * STEP * 0.8);
                let kind = kinds[rng.below(kinds.len() as u32) as usize];
                let near_depot = tx > -out && tx < w + out && ty > WALK_Y0 - 0.3 && ty < h + out + 1.2;
                let near_exit = (tx - road_x).abs() < ROAD / 2.0 + 0.9 && ty < 0.0;
                // a tree reaches about 1.4 squares up from its foot
                let under_bar = ty - 1.4 < bar1 && ty > bar0;
                if rng.float() < 0.6 && !near_depot && !near_exit && !under_bar {
                    self.put(s, v, kind, tx, ty, false);
                }
                x += STEP;
            }
            y += STEP;
        }
    }

    fn draw_card(&self, s: &mut Screen, w: f32) {
        let th = THEME;
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0x10141a), 0.45);
        let (px, py, pw, ph) = self.card();
        s.shadow(px, py + 4.0, pw, ph, 16.0, 14.0, 0, 0.45);
        s.rect(px, py, pw, ph, 16.0, hex(th.card), 1.0);
        let stuck = self.s.phase == STUCK;
        let title = if stuck {
            tr(&["Loading bays full", "车站没位置了", "乗り場が満杯", "정류장이 꽉 찼어요", "La parada está llena", "O ponto está cheio", "L'arrêt est plein", "Die Haltestelle ist voll", "Остановка заполнена", "車站沒位置了"])
        } else {
            tr(&["Level cleared", "过关", "クリア", "레벨 클리어", "Nivel superado", "Nível concluído", "Niveau réussi", "Level geschafft", "Уровень пройден", "過關"])
        };
        let size = s.fit_size(title, 21.0, pw - 32.0);
        s.text_c(title, w / 2.0, py + 34.0, size, hex(th.text), 1.0);
        let line = if stuck {
            let left_l = tr(&["Still waiting", "仍在等候", "まだ待っている人", "아직 기다리는 사람", "Aún esperan", "Ainda esperando", "Encore en attente", "Noch wartend", "Ещё ждут", "仍在等候"]);
            format!("{} {}", left_l, self.s.lot.qn - self.s.head)
        } else {
            let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
            let tries_l = tr(&["Retries", "重试", "リトライ", "재시도", "Reintentos", "Tentativas", "Essais", "Neustarts", "Повторы", "重試"]);
            format!("{} +{}    {} {}", score_l, self.s.cleared_score, tries_l, self.s.retries)
        };
        let size = s.fit_size(&line, 14.0, pw - 32.0);
        s.text_c(&line, w / 2.0, py + 64.0, size, hex(0xb9c2cf), 1.0);
        if self.s.phase != DONE {
            let b = self.button();
            let label = if stuck {
                tr(&["Try again", "再试一次", "もう一度", "다시 하기", "Reintentar", "Tentar de novo", "Réessayer", "Nochmal", "Ещё раз", "再試一次"])
            } else {
                tr(&["Next level", "下一关", "次のレベル", "다음 레벨", "Siguiente nivel", "Próximo nível", "Niveau suivant", "Nächstes Level", "Следующий уровень", "下一關"])
            };
            let hv = self.hover == Hit::Button;
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

/// The accessible board: the depot's squares, each saying which bus stands
/// on it.
impl wg::Board for Boarding {
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
        let Some(i) = lot.bus_at(x, y) else {
            out.push_str(tr(&["Empty", "空", "空き", "비어 있음", "Vacío", "Vazio", "Vide", "Leer", "Пусто", "空"]));
            return;
        };
        let b = &lot.buses[i];
        let what = if b.len == 3 {
            tr(&["# bus facing @", "朝@的#大巴", "@向きの#のバス", "@ 방향 # 버스", "Autobús # mirando @", "Ônibus # virado @", "Bus # tourné @", "#er Bus, Front @", "Автобус (#) носом @", "朝@的#大巴"])
        } else {
            tr(&["# minibus facing @", "朝@的#小巴", "@向きの#のミニバス", "@ 방향 # 미니버스", "Microbús # mirando @", "Micro-ônibus # virado @", "Minibus # tourné @", "#er Kleinbus, Front @", "Микроавтобус (#) носом @", "朝@的#小巴"])
        };
        out.push_str(&what.replace('#', tr(&COLOUR_NAMES[b.colour as usize])).replace('@', tr(&WAYS[b.dir as usize])));
        if b.front() == (x, y) {
            out.push_str(", ");
            out.push_str(tr(&["front", "车头", "前", "앞", "delante", "frente", "avant", "vorn", "перед", "車頭"]));
        }
        if lot.way(i) == Way::Out {
            out.push_str(", ");
            out.push_str(tr(&["clear path ahead", "前方畅通", "前が空いている", "앞길이 비어 있음", "vía libre delante", "caminho livre à frente", "voie libre devant", "vorne frei", "впереди свободно", "前方暢通"]));
        }
    }
    fn status(&self, out: &mut String) {
        let s = &self.s;
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let waiting_l = tr(&["waiting", "人等候", "人待ち", "명 대기", "esperan", "esperando", "en attente", "wartend", "ждут", "人等候"]);
        out.push_str(&format!("{} {}, {} {}", level_l, s.level + 1, s.lot.qn - s.head, waiting_l));
        // the next few in the queue, then the bays
        let next = (s.head..(s.head + 4).min(s.lot.qn)).map(|n| tr(&COLOUR_NAMES[s.lot.queue[n as usize] as usize])).collect::<Vec<_>>();
        if !next.is_empty() {
            out.push_str(". ");
            out.push_str(tr(&["Next", "接下来", "次", "다음", "Siguen", "Próximos", "Ensuite", "Als Nächstes", "Дальше", "接下來"]));
            out.push_str(": ");
            out.push_str(&next.join(", "));
        }
        out.push_str(". ");
        out.push_str(tr(&["Bays", "车位", "乗り場", "승차장", "Andenes", "Vagas", "Quais", "Buchten", "Места", "車位"]));
        out.push_str(": ");
        let bays = (0..BAYS)
            .map(|k| {
                let i = s.bay_bus[k];
                if i < 0 {
                    tr(&["free", "空", "空き", "빈자리", "libre", "livre", "libre", "frei", "свободно", "空"]).to_string()
                } else {
                    let b = &s.lot.buses[i as usize];
                    format!("{} {}/{}", tr(&COLOUR_NAMES[b.colour as usize]), s.bay_fill[k], b.seats())
                }
            })
            .collect::<Vec<_>>();
        out.push_str(&bays.join(", "));
    }
    fn keys(&self) -> &'static str {
        tr(&[
            "Arrow keys move between cells, Space dispatches the selected bus forward, I announces the level, passenger queue, and loading bays. R restarts the level.",
            "方向键在格子间移动，空格让该格的巴士向前开，I 读出关卡、队伍和车位。R 重新开始本关。",
            "矢印でマスを移動、スペースでそのマスのバスを前へ、I でレベル、列、乗り場を読み上げ。R でレベルをやり直し。",
            "화살표로 칸 이동, 스페이스로 그 칸의 버스를 앞으로, I로 레벨, 줄, 승차장 읽기. R로 레벨 다시 시작.",
            "Flechas para moverse entre casillas, Espacio conduce el autobús de esa casilla hacia delante, I lee nivel, fila y andenes. R reinicia el nivel.",
            "Setas movem entre as casas, Espaço dirige o ônibus daquela casa para a frente, I lê nível, fila e vagas. R reinicia o nível.",
            "Flèches pour changer de case, Espace avance le bus de cette case, I lit le niveau, la file et les quais. R recommence le niveau.",
            "Pfeiltasten wechseln das Feld, Leertaste fährt den Bus dort vorwärts, I liest Level, Schlange und Buchten vor. R startet das Level neu.",
            "Стрелки переходят по клеткам, пробел ведёт автобус на клетке вперёд, I читает уровень, очередь и места. R начинает уровень заново.",
            "方向鍵在格子間移動，空白鍵讓該格的巴士向前開，I 讀出關卡、隊伍和車位。R 重新開始本關。",
        ])
    }
    fn act(&mut self, x: i32, y: i32, _a: i32, said: &mut String) -> u32 {
        if self.s.phase != PLAY {
            let ev = self.press_button();
            self.status(said);
            return ev;
        }
        self.focus(x, y);
        let Some(i) = self.s.lot.bus_at(x, y) else {
            said.push_str(tr(&["No bus here", "这里没有巴士", "ここにバスはありません", "여기에 버스가 없습니다", "Aquí no hay autobús", "Não há ônibus aqui", "Pas de bus ici", "Hier steht kein Bus", "Здесь нет автобуса", "這裡沒有巴士"]));
            return 0;
        };
        let way = self.s.lot.way(i);
        let ev = self.send(i);
        match way {
            _ if ev == 0 => said.push_str(tr(&["Wait a moment", "请稍等", "少し待って", "잠시 기다리세요", "Espera un momento", "Espere um pouco", "Un instant", "Einen Moment", "Подождите", "請稍等"])),
            Way::Out if self.s.lot.buses[i].out == 0 => said.push_str(tr(&["All loading bays are full", "车位全满", "乗り場が全部埋まっている", "승차장이 모두 찼습니다", "Todos los andenes están ocupados", "Todas as vagas estão ocupadas", "Tous les quais sont pris", "Alle Buchten sind belegt", "Все места заняты", "車位全滿"])),
            Way::Out => {
                said.push_str(tr(&["Pulled into loading bay", "开到了车站", "乗り場へ向かった", "정류장으로 출발", "Fue a la parada", "Foi para o ponto", "Part vers l'arrêt", "Zur Haltestelle gefahren", "Поехал к остановке", "開到了車站"]));
                said.push_str(". ");
                self.status(said);
            }
            Way::Blocked { .. } => said.push_str(tr(&["Blocked by another bus", "撞到了巴士", "バスにぶつかった", "버스에 부딪힘", "Chocó con un autobús", "Bateu num ônibus", "Choc contre un bus", "Gegen einen Bus gestoßen", "Врезался в автобус", "撞到了巴士"])),
        }
        ev
    }
    fn rev(&mut self) -> u32 {
        let v = [self.s.phase, self.s.level, self.s.head, wg::lang() as i32];
        let bays = [self.s.bay_bus, self.s.bay_fill];
        wg::hash(wg::hash(wg::hash(0, wg::bytes_of(&self.s.lot)), wg::bytes_of(&v)), wg::bytes_of(&bays))
    }
}

wg::game!(Boarding);
wg::board!(Boarding);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Boarding, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    /// Plays a level the way its queue was dealt: the buses in the order
    /// it was made for, each as soon as a bay is free.
    fn play(g: &mut Boarding) -> u32 {
        let (_, order) = puzzle::deal_order(g.s.level);
        let mut next = 0;
        let mut ev = 0;
        for _ in 0..4000 {
            if g.s.phase != PLAY {
                break;
            }
            while next < order.len() && g.s.lot.buses[order[next]].out != 0 {
                next += 1;
            }
            if next < order.len() && (0..BAYS).any(|k| g.s.bay_bus[k] < 0) {
                assert_eq!(g.send(order[next]), wg::EV_BLIP);
                next += 1;
            }
            ev |= run(g, 48);
        }
        ev
    }

    #[test]
    fn plays_every_level() {
        let mut g = Boarding::new();
        g.start(1);
        for level in 0..LEVELS {
            assert_eq!(g.s.level, level);
            let ev = play(&mut g);
            assert!(ev & wg::EV_LEVEL != 0, "level {} not cleared, phase {}", level + 1, g.s.phase);
            if level + 1 < LEVELS {
                assert_eq!(g.s.phase, CLEARED);
                g.press_button();
            } else {
                assert!(g.over());
            }
        }
        assert_eq!(g.s.done, LEVELS);
    }

    #[test]
    fn sticks_when_the_bays_fill_wrong() {
        let mut g = Boarding::new();
        g.start(1);
        g.begin(12);
        // five buses nobody at the front wants fill the bays
        let lot = g.s.lot;
        let front = lot.queue[0];
        let others: Vec<usize> = (0..lot.n as usize).filter(|&i| lot.buses[i].colour != front).take(BAYS).collect();
        assert_eq!(others.len(), BAYS);
        for (k, &i) in others.iter().enumerate() {
            g.s.lot.buses[i].out = 1;
            g.s.bay_bus[k] = i as i8;
        }
        assert!(run(&mut g, 16) & wg::EV_LOSE != 0);
        assert_eq!(g.s.phase, STUCK);
        g.press_button();
        assert_eq!((g.s.phase, g.s.retries, g.s.head), (PLAY, 1, 0));
    }

    #[test]
    fn leaves_after_loading_mid_boarding() {
        let mut g = Boarding::new();
        g.start(1);
        play_until(&mut g, |g| !g.walkers.is_empty());
        assert!(g.loaded());
        // the bus whose last passenger was walking still leaves when full
        let ev = play(&mut g);
        assert!(ev & wg::EV_LEVEL != 0);
    }

    fn play_until(g: &mut Boarding, done: impl Fn(&Boarding) -> bool) {
        let lot = g.s.lot;
        let c = lot.queue[0];
        let i = (0..lot.n as usize).find(|&i| lot.way(i) == Way::Out && lot.buses[i].colour == c).expect("a free bus for the first passenger");
        g.send(i);
        for _ in 0..500 {
            if done(g) {
                return;
            }
            run(g, 16);
        }
        panic!("never got there");
    }

    #[test]
    fn every_route_ends_in_its_bay() {
        let mut g = Boarding::new();
        g.start(1);
        g.begin(20);
        let lot = g.s.lot;
        for i in 0..lot.n as usize {
            for k in 0..BAYS {
                let path = g.route_in(&lot.buses[i], k);
                let last = path.last().unwrap();
                let end = (last.x + DX[last.d] as f32 * last.len, last.y + DY[last.d] as f32 * last.len);
                assert!((end.0 - g.bay_x(k)).abs() < 0.01 && (end.1 - Boarding::bay_y(kind(&lot.buses[i]))).abs() < 0.01);
                assert!(path.len() <= 7);
                // each leg starts where the last one ended
                for p in path.windows(2) {
                    let e = (p[0].x + DX[p[0].d] as f32 * p[0].len, p[0].y + DY[p[0].d] as f32 * p[0].len);
                    assert!((e.0 - p[1].x).abs() < 0.01 && (e.1 - p[1].y).abs() < 0.01);
                }
            }
        }
    }
}
