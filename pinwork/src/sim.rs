//! The physics: grains of gold, lava, water and stone that fall, pile and
//! flow round the walls and pins; lava that water turns to stone; and the
//! orc, a heavy body that falls when what holds it is pulled away.

use crate::levels::{Level, GRAIN_R, GROUND, LAVA, PIN_R, STONE, WALL_R, WATER};

/// one physics step, ms
pub const STEP_MS: f32 = 1000.0 / 120.0;
const DT: f32 = STEP_MS / 1000.0;
const GRAVITY: f32 = 700.0;
/// how long a pin takes to slide out, ms
const PULL_MS: f32 = 240.0;
/// ms a new stone turns the lava it touches to stone too
const HEAT_MS: f32 = 220.0;
/// the orc's body: a circle this big, its middle this far above its feet
pub const ORC_R: f32 = 12.0;
pub const ORC_UP: f32 = 13.0;
/// the knight's half width and height
pub const KNIGHT_W: f32 = 9.0;
pub const KNIGHT_H: f32 = 30.0;
/// how fast a living orc walks at the knight
const ORC_WALK: f32 = 45.0;
/// a grain's top speed, plan units a second
const TOP_SPEED: f32 = 320.0;
/// the share of a push between grains that does not become speed
const SOAK: f32 = 0.5;
/// below this speed, plan units a second, a grain is at rest
const REST: f32 = 14.0;

#[derive(Clone, Copy)]
pub struct Grain {
    pub p: (f32, f32),
    prev: (f32, f32),
    pub kind: u8,
    /// ms left of turning lava to stone, for a stone just made
    heat: f32,
}

#[derive(Clone, Copy)]
pub struct Pin {
    /// tip and handle where it started
    pub a: (f32, f32),
    pub b: (f32, f32),
    /// how far out it has slid, 0 to 1, and whether it is sliding
    pub out: f32,
    pub pulled: bool,
}

impl Pin {
    /// where it is now
    pub fn now(&self) -> ((f32, f32), (f32, f32)) {
        let k = self.out * 1.05;
        let d = ((self.b.0 - self.a.0) * k, (self.b.1 - self.a.1) * k);
        ((self.a.0 + d.0, self.a.1 + d.1), (self.b.0 + d.0, self.b.1 + d.1))
    }
    pub fn gone(&self) -> bool {
        self.out >= 1.0
    }
}

#[derive(Clone, Copy)]
pub struct Orc {
    /// the middle of its body
    pub p: (f32, f32),
    prev: (f32, f32),
    pub alive: bool,
    /// where it was before the last step, as its walk moves `prev` too
    start: (f32, f32),
}

pub struct World {
    pub grains: Vec<Grain>,
    pub pins: Vec<Pin>,
    pub orc: Option<Orc>,
    /// lava got the knight; the orc got the knight
    pub burned: bool,
    pub caught: bool,
    head: Vec<i32>,
    next: Vec<i32>,
    /// each grain's cell in `head`
    cells: Vec<usize>,
    /// the pins still in the way this step
    cut: Vec<((f32, f32), (f32, f32))>,
}

/// the grid the grains are sorted into, to find neighbours
const CELL: f32 = 2.0 * GRAIN_R;
const GX0: f32 = -40.0;
const GY0: f32 = -40.0;
const COLS: i32 = 48;
const ROWS: i32 = 62;

fn cell(p: (f32, f32)) -> (i32, i32) {
    (((p.0 - GX0) / CELL).clamp(0.0, (COLS - 1) as f32) as i32, ((p.1 - GY0) / CELL).clamp(0.0, (ROWS - 1) as f32) as i32)
}

/// The way out of a capsule from a to b of radius `r` for a circle at p of
/// radius `pr`, and how deep it is in.
pub fn capsule(p: (f32, f32), pr: f32, a: (f32, f32), b: (f32, f32), r: f32) -> Option<((f32, f32), f32)> {
    let ab = (b.0 - a.0, b.1 - a.1);
    let len2 = (ab.0 * ab.0 + ab.1 * ab.1).max(1e-6);
    let t = (((p.0 - a.0) * ab.0 + (p.1 - a.1) * ab.1) / len2).clamp(0.0, 1.0);
    let q = (a.0 + ab.0 * t, a.1 + ab.1 * t);
    let (dx, dy) = (p.0 - q.0, p.1 - q.1);
    let d2 = dx * dx + dy * dy;
    let reach = r + pr;
    if d2 >= reach * reach {
        return None;
    }
    let d = d2.sqrt();
    Some(if d > 1e-4 { ((dx / d, dy / d), reach - d) } else { ((0.0, -1.0), reach) })
}

/// The way out of a box for a circle, and how deep it is in.
fn boxed(p: (f32, f32), pr: f32, r: (f32, f32, f32, f32)) -> Option<((f32, f32), f32)> {
    let q = (p.0.clamp(r.0, r.2), p.1.clamp(r.1, r.3));
    let (dx, dy) = (p.0 - q.0, p.1 - q.1);
    let d2 = dx * dx + dy * dy;
    if d2 >= pr * pr {
        return None;
    }
    if d2 > 1e-6 {
        let d = d2.sqrt();
        return Some(((dx / d, dy / d), pr - d));
    }
    // inside: out the top or the nearer side
    let sides = [(p.1 - r.1, (0.0, -1.0)), (p.0 - r.0, (-1.0, 0.0)), (r.2 - p.0, (1.0, 0.0))];
    let (gap, n) = sides.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
    Some((n, gap + pr))
}

/// the knight's box: x0, y0, x1, y1
pub fn knight_box(x: f32) -> (f32, f32, f32, f32) {
    (x - KNIGHT_W, GROUND - KNIGHT_H, x + KNIGHT_W, GROUND)
}

impl World {
    pub fn new(lv: &Level) -> World {
        let grains = lv.grains.iter().map(|&(x, y, kind)| Grain { p: (x, y), prev: (x, y), kind, heat: 0.0 }).collect();
        let pins = lv.pins.iter().map(|&(ax, ay, bx, by)| Pin { a: (ax, ay), b: (bx, by), out: 0.0, pulled: false }).collect();
        let orc = lv.orc.map(|(x, feet)| Orc { p: (x, feet - ORC_UP), prev: (x, feet - ORC_UP), alive: true, start: (x, feet - ORC_UP) });
        World { grains, pins, orc, burned: false, caught: false, head: vec![-1; (COLS * ROWS) as usize], next: Vec::new(), cells: Vec::new(), cut: Vec::new() }
    }

    /// Starts pin i sliding out; false if it is already out.
    pub fn pull(&mut self, i: usize) -> bool {
        let Some(pin) = self.pins.get_mut(i) else { return false };
        if pin.pulled {
            return false;
        }
        pin.pulled = true;
        true
    }

    /// Whether everything has come to rest.
    pub fn resting(&self) -> bool {
        let still = |p: (f32, f32), q: (f32, f32)| ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt() / DT < REST;
        self.pins.iter().all(|p| !p.pulled || p.gone())
            && self.grains.iter().all(|g| g.p.1 > 600.0 || still(g.p, g.prev))
            && self.orc.is_none_or(|o| o.p.1 > 600.0 || still(o.p, o.start))
    }

    /// One step.
    pub fn step(&mut self, lv: &Level) {
        for pin in self.pins.iter_mut().filter(|p| p.pulled) {
            pin.out = (pin.out + STEP_MS / PULL_MS).min(1.0);
        }
        let fall = |p: &mut (f32, f32), prev: &mut (f32, f32)| {
            let mut v = ((p.0 - prev.0) * 0.996, (p.1 - prev.1) * 0.996);
            // no faster than a top speed
            let sp = (v.0 * v.0 + v.1 * v.1).sqrt();
            if sp > TOP_SPEED * DT {
                v = (v.0 * TOP_SPEED * DT / sp, v.1 * TOP_SPEED * DT / sp);
            }
            *prev = *p;
            *p = (p.0 + v.0, p.1 + v.1 + GRAVITY * DT * DT);
        };
        for g in self.grains.iter_mut() {
            fall(&mut g.p, &mut g.prev);
            g.heat = (g.heat - STEP_MS).max(0.0);
        }
        if let Some(o) = &mut self.orc {
            o.start = o.p;
            fall(&mut o.p, &mut o.prev);
            // a living orc on its feet walks at the knight
            if o.alive && (o.p.1 - o.prev.1).abs() < 0.2 && (lv.knight - o.p.0).abs() > 1.0 {
                let step = (lv.knight - o.p.0).signum() * ORC_WALK * DT;
                o.p.0 += step;
                o.prev.0 += step;
            }
        }
        let mut pins = std::mem::take(&mut self.cut);
        pins.clear();
        pins.extend(self.pins.iter().filter(|p| !p.gone()).map(|p| p.now()));
        let solid = |p: (f32, f32), pr: f32| -> Option<((f32, f32), f32)> {
            let mut best: Option<((f32, f32), f32)> = None;
            let mut keep = |c: Option<((f32, f32), f32)>| {
                if let Some(c) = c {
                    if best.is_none_or(|b| c.1 > b.1) {
                        best = Some(c);
                    }
                }
            };
            for &(ax, ay, bx, by) in &lv.walls {
                keep(capsule(p, pr, (ax, ay), (bx, by), WALL_R));
            }
            for &(a, b) in &pins {
                keep(capsule(p, pr, a, b, PIN_R));
            }
            best
        };
        let knight = knight_box(lv.knight);
        for _ in 0..3 {
            self.collide_grains();
            for g in self.grains.iter_mut() {
                // a few passes, as a grain can sit in a corner
                for _ in 0..2 {
                    match solid(g.p, GRAIN_R) {
                        Some((n, d)) => {
                            g.p = (g.p.0 + n.0 * d, g.p.1 + n.1 * d);
                            // friction along the wall: gold and stone more than the liquids
                            let keep = if g.kind == WATER || g.kind == LAVA { 0.97 } else { 0.8 };
                            let v = (g.p.0 - g.prev.0, g.p.1 - g.prev.1);
                            let vn = v.0 * n.0 + v.1 * n.1;
                            let vt = (v.0 - vn * n.0, v.1 - vn * n.1);
                            g.prev = (g.p.0 - vt.0 * keep, g.p.1 - vt.1 * keep);
                        }
                        None => break,
                    }
                }
                if let Some((n, d)) = boxed(g.p, GRAIN_R, knight) {
                    g.p = (g.p.0 + n.0 * d, g.p.1 + n.1 * d);
                }
                if let Some(o) = &self.orc {
                    let (dx, dy) = (g.p.0 - o.p.0, g.p.1 - o.p.1);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < ORC_R + GRAIN_R && d > 1e-4 {
                        let k = (ORC_R + GRAIN_R - d) / d;
                        g.p = (g.p.0 + dx * k, g.p.1 + dy * k);
                    }
                }
            }
            if let Some(o) = &mut self.orc {
                if let Some((n, d)) = solid(o.p, ORC_R) {
                    o.p = (o.p.0 + n.0 * d, o.p.1 + n.1 * d);
                    let v = (o.p.0 - o.prev.0, o.p.1 - o.prev.1);
                    o.prev = (o.p.0 - v.0 * 0.5, o.p.1 - v.1 * 0.5);
                }
            }
        }
        self.cut = pins;
        // what touches the knight and the orc
        for g in &self.grains {
            if g.kind != LAVA {
                continue;
            }
            if boxed(g.p, GRAIN_R + 0.8, knight).is_some() {
                self.burned = true;
            }
            if let Some(o) = &mut self.orc {
                if ((g.p.0 - o.p.0).powi(2) + (g.p.1 - o.p.1).powi(2)).sqrt() < ORC_R + GRAIN_R + 0.8 {
                    o.alive = false;
                }
            }
        }
        if let Some(o) = &self.orc {
            if o.alive && boxed(o.p, ORC_R + 1.0, knight).is_some() {
                self.caught = true;
            }
        }
    }

    /// Pushes overlapping grains apart; lava meeting water, or a stone just
    /// made, turns to stone.
    fn collide_grains(&mut self) {
        // empty only the cells filled last time
        for &c in &self.cells {
            self.head[c] = -1;
        }
        self.cells.clear();
        self.next.clear();
        self.next.resize(self.grains.len(), -1);
        for (i, g) in self.grains.iter().enumerate() {
            let (cx, cy) = cell(g.p);
            let c = (cy * COLS + cx) as usize;
            self.cells.push(c);
            self.next[i] = self.head[c];
            self.head[c] = i as i32;
        }
        let two = 2.0 * GRAIN_R;
        for i in 0..self.grains.len() {
            let (cx, cy) = cell(self.grains[i].p);
            for ny in (cy - 1).max(0)..=(cy + 1).min(ROWS - 1) {
                for nx in (cx - 1).max(0)..=(cx + 1).min(COLS - 1) {
                    let mut j = self.head[(ny * COLS + nx) as usize];
                    while j >= 0 {
                        let ju = j as usize;
                        j = self.next[ju];
                        if ju <= i {
                            continue;
                        }
                        let (a, b) = (self.grains[i], self.grains[ju]);
                        let (dx, dy) = (b.p.0 - a.p.0, b.p.1 - a.p.1);
                        let d2 = dx * dx + dy * dy;
                        if d2 >= two * two {
                            continue;
                        }
                        let d = d2.sqrt().max(1e-4);
                        let k = (two - d) / d * 0.5;
                        // apart, with half the push kept as speed: no splashes
                        let gi = &mut self.grains[i];
                        gi.p = (a.p.0 - dx * k, a.p.1 - dy * k);
                        gi.prev = (a.prev.0 - dx * k * SOAK, a.prev.1 - dy * k * SOAK);
                        let gj = &mut self.grains[ju];
                        gj.p = (b.p.0 + dx * k, b.p.1 + dy * k);
                        gj.prev = (b.prev.0 + dx * k * SOAK, b.prev.1 + dy * k * SOAK);
                        react(&mut self.grains, i, ju);
                    }
                }
            }
        }
    }
}

/// Lava touching water, or a stone still hot from it, turns to stone.
fn react(grains: &mut [Grain], i: usize, j: usize) {
    let turns = |lava: &Grain, other: &Grain| lava.kind == LAVA && (other.kind == WATER || (other.kind == STONE && other.heat > 0.0));
    for (a, b) in [(i, j), (j, i)] {
        if turns(&grains[a], &grains[b]) {
            grains[a].kind = STONE;
            grains[a].heat = HEAT_MS;
        }
    }
}
