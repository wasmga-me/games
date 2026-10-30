//! The physics: water and lava as a particle fluid (each drop pushed by the
//! crowding round it, after Clavet, Beaudoin and Poulin's double density
//! relaxation, and slowed by its neighbours' viscosity), coins and stones
//! as grains that pile; lava that water turns to stone; and the orc, a heavy
//! body that falls when what holds it is pulled away.

use crate::levels::{Level, DROP_R, GOLD, GRAIN_R, GROUND, LAVA, PIN_R, ROW_RISE, SPACING, STONE, WALL_R, WATER};

/// one physics step, ms
pub const STEP_MS: f32 = 1000.0 / 120.0;
const DT: f32 = STEP_MS / 1000.0;
const GRAVITY: f32 = 700.0;
/// how long a pin takes to slide out, ms
const PULL_MS: f32 = 240.0;
/// ms a new stone turns the lava it touches to stone too
const HEAT_MS: f32 = 220.0;
/// ms a puff of steam lasts
pub const STEAM_MS: f32 = 650.0;
/// the orc's body: a circle this big, its middle this far above its feet
pub const ORC_R: f32 = 12.0;
pub const ORC_UP: f32 = 13.0;
/// the knight's half width and height
pub const KNIGHT_W: f32 = 9.0;
pub const KNIGHT_H: f32 = 30.0;
/// how fast a living orc walks at the knight
const ORC_WALK: f32 = 45.0;
/// a particle's top speed, plan units a second
const TOP_SPEED: f32 = 320.0;
/// below this speed, plan units a second, a particle is at rest
const REST: f32 = 14.0;
/// steps between the marks that tell whether things still move
const MARK: u32 = 12;
/// how near lava's middle must come to burn the knight or the orc
const BURN: f32 = DROP_R + 1.5;

/// how far a drop feels its neighbours
const H: f32 = 2.0 * SPACING;
/// the fluid's stiffness, and its near stiffness that keeps drops apart and
/// gives the surface its tension, as a step's push
const STIFF: f32 = 0.35;
const NEAR_STIFF: f32 = 1.4;

#[derive(Clone, Copy)]
pub struct Grain {
    pub p: (f32, f32),
    prev: (f32, f32),
    pub kind: u8,
    /// ms left of turning lava to stone, for a stone just made
    heat: f32,
    /// where it was at the last mark (see `World::moved`)
    mark: (f32, f32),
}

fn radius(kind: u8) -> f32 {
    if kind == GOLD {
        GRAIN_R
    } else {
        DROP_R
    }
}
fn liquid(kind: u8) -> bool {
    kind == WATER || kind == LAVA
}
/// how heavy it is, for who gives way when two meet
fn mass(kind: u8) -> f32 {
    match kind {
        GOLD => 3.0,
        STONE => 2.0,
        _ => 1.0,
    }
}
/// the viscosity: linear and quadratic, as a share of the speed two
/// neighbouring drops close in at
fn viscosity(kind: u8) -> (f32, f32) {
    if kind == LAVA {
        (0.25, 0.4)
    } else {
        (0.02, 0.1)
    }
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

/// A puff of steam where water met lava.
#[derive(Clone, Copy)]
pub struct Steam {
    pub p: (f32, f32),
    pub age: f32,
}

/// a wall or pin: its ends, its half thickness and its box
type Seg = ((f32, f32), (f32, f32), f32, (f32, f32, f32, f32));

pub struct World {
    pub grains: Vec<Grain>,
    pub pins: Vec<Pin>,
    pub orc: Option<Orc>,
    pub steam: Vec<Steam>,
    /// lava got the knight; the orc got the knight
    pub burned: bool,
    pub caught: bool,
    /// the neighbour grid: each cell's first particle, and each particle's next
    head: Vec<i32>,
    next: Vec<i32>,
    /// the cells filled last time, to empty
    cells: Vec<usize>,
    /// pairs of particles near each other: both liquid, and the rest
    wet: Vec<(u32, u32)>,
    dry: Vec<(u32, u32)>,
    /// steps so far, and whether anything moved between the last two marks
    steps: u32,
    moved: bool,
    /// the walls and the pins still in, each with the box round it
    segs: Vec<Seg>,
    /// each drop's crowding and near crowding
    density: Vec<(f32, f32)>,
    /// the crowding a drop has at rest
    rest_density: f32,
}

/// the neighbour grid, cells as wide as a drop's reach
const CELL: f32 = H;
const GX0: f32 = -40.0;
const GY0: f32 = -40.0;
const COLS: i32 = 39;
const ROWS: i32 = 50;

fn cell(p: (f32, f32)) -> (i32, i32) {
    (((p.0 - GX0) / CELL).clamp(0.0, (COLS - 1) as f32) as i32, ((p.1 - GY0) / CELL).clamp(0.0, (ROWS - 1) as f32) as i32)
}

/// The point of the segment a-b nearest p.
fn nearest(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    let ab = (b.0 - a.0, b.1 - a.1);
    let len2 = (ab.0 * ab.0 + ab.1 * ab.1).max(1e-6);
    let t = (((p.0 - a.0) * ab.0 + (p.1 - a.1) * ab.1) / len2).clamp(0.0, 1.0);
    (a.0 + ab.0 * t, a.1 + ab.1 * t)
}

/// How far p is from the segment a-b.
pub fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let q = nearest(p, a, b);
    ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt()
}

/// The way out of a capsule from a to b of radius `r` for a circle at p of
/// radius `pr`, and how deep it is in.
fn capsule(p: (f32, f32), pr: f32, a: (f32, f32), b: (f32, f32), r: f32) -> Option<((f32, f32), f32)> {
    let q = nearest(p, a, b);
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

/// Moves a particle d along n out of a wall (or the knight), keeping only
/// its speed along the wall, less some friction: coins and stones more
/// than the liquids.
fn slide(g: &mut Grain, n: (f32, f32), d: f32) {
    g.p = (g.p.0 + n.0 * d, g.p.1 + n.1 * d);
    let keep = match g.kind {
        WATER => 0.98,
        LAVA => 0.9,
        _ => 0.8,
    };
    let v = (g.p.0 - g.prev.0, g.p.1 - g.prev.1);
    let vn = v.0 * n.0 + v.1 * n.1;
    let vt = (v.0 - vn * n.0, v.1 - vn * n.1);
    g.prev = (g.p.0 - vt.0 * keep, g.p.1 - vt.1 * keep);
}

/// The crowding a drop feels from neighbours this far away, as a share of
/// its reach: (1 - q)² and (1 - q)³.
fn crowding(q: f32) -> (f32, f32) {
    let w = 1.0 - q;
    (w * w, w * w * w)
}

/// The crowding of a drop inside a body of still fluid, packed as the
/// levels pack it: rows `SPACING` apart, every other one shifted half a drop.
fn lattice_density() -> f32 {
    let mut sum = 0.0;
    for row in -3i32..=3 {
        for col in -3i32..=3 {
            let x = col as f32 * SPACING + if row.rem_euclid(2) == 1 { SPACING / 2.0 } else { 0.0 };
            let y = row as f32 * ROW_RISE;
            let d = (x * x + y * y).sqrt();
            if d > 1e-4 && d < H {
                sum += crowding(d / H).0;
            }
        }
    }
    sum
}

impl World {
    pub fn new(lv: &Level) -> World {
        let grains = lv.grains.iter().map(|&(x, y, kind)| Grain { p: (x, y), prev: (x, y), kind, heat: 0.0, mark: (x, y) }).collect();
        let pins = lv.pins.iter().map(|&(ax, ay, bx, by)| Pin { a: (ax, ay), b: (bx, by), out: 0.0, pulled: false }).collect();
        let orc = lv.orc.map(|(x, feet)| Orc { p: (x, feet - ORC_UP), prev: (x, feet - ORC_UP), alive: true, start: (x, feet - ORC_UP) });
        World {
            grains,
            pins,
            orc,
            steam: Vec::new(),
            burned: false,
            caught: false,
            head: vec![-1; (COLS * ROWS) as usize],
            next: Vec::new(),
            cells: Vec::new(),
            wet: Vec::new(),
            dry: Vec::new(),
            segs: Vec::new(),
            steps: 0,
            moved: true,
            density: Vec::new(),
            rest_density: lattice_density(),
        }
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

    /// Whether everything has come to rest: the orc still, and each coin
    /// and drop no further than a slow crawl from where it was at the last
    /// mark (crowded drops and coins shiver in place from step to step).
    pub fn resting(&self) -> bool {
        let still = |p: (f32, f32), q: (f32, f32), dt: f32| ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt() / dt < REST;
        self.pins.iter().all(|p| !p.pulled || p.gone())
            && self.steam.is_empty()
            && !self.moved
            && self.orc.is_none_or(|o| o.p.1 > 600.0 || still(o.p, o.start, DT))
    }

    /// One step.
    pub fn step(&mut self, lv: &Level) {
        for pin in self.pins.iter_mut().filter(|p| p.pulled) {
            pin.out = (pin.out + STEP_MS / PULL_MS).min(1.0);
        }
        for s in self.steam.iter_mut() {
            s.age += STEP_MS;
            s.p.1 -= 18.0 * DT;
        }
        self.steam.retain(|s| s.age < STEAM_MS);
        // every MARK steps, whether anything moved since the last mark
        self.steps += 1;
        if self.steps % MARK == 0 {
            let far = REST * MARK as f32 * DT;
            self.moved = self.grains.iter().any(|g| g.p.1 < 600.0 && (g.p.0 - g.mark.0).powi(2) + (g.p.1 - g.mark.1).powi(2) > far * far);
            self.grains.iter_mut().for_each(|g| g.mark = g.p);
        }
        let fall = |p: &mut (f32, f32), prev: &mut (f32, f32), calm: f32| {
            let mut v = ((p.0 - prev.0) * 0.996, (p.1 - prev.1) * 0.996);
            // no faster than a top speed; a drop slower than `calm` stops,
            // so a pool comes to rest instead of shivering
            let sp = (v.0 * v.0 + v.1 * v.1).sqrt();
            if sp > TOP_SPEED * DT {
                v = (v.0 * TOP_SPEED * DT / sp, v.1 * TOP_SPEED * DT / sp);
            } else if sp < calm * DT {
                v = (v.0 * 0.5, v.1 * 0.5);
            }
            *prev = *p;
            *p = (p.0 + v.0, p.1 + v.1 + GRAVITY * DT * DT);
        };
        // the drops' viscosity, from where they were (the pairs found last
        // step)
        self.viscosity();
        for g in self.grains.iter_mut() {
            // water stops shivering; lava is thick enough to settle by
            // itself, and creeps on slowly
            fall(&mut g.p, &mut g.prev, if g.kind == WATER { 2.0 * REST } else { 0.0 });
            g.heat = (g.heat - STEP_MS).max(0.0);
        }
        if let Some(o) = &mut self.orc {
            o.start = o.p;
            fall(&mut o.p, &mut o.prev, 0.0);
            // a living orc on its feet walks at the knight
            if o.alive && (o.p.1 - o.prev.1).abs() < 0.2 && (lv.knight - o.p.0).abs() > 1.0 {
                let step = (lv.knight - o.p.0).signum() * ORC_WALK * DT;
                o.p.0 += step;
                o.prev.0 += step;
            }
        }
        // then the fluid pushes apart where it is crowded, and lava meeting
        // water turns to stone
        self.find_pairs();
        self.react();
        self.relax();
        // the walls and the pins still in, with the boxes round them
        let walls = lv.walls.iter().map(|&(ax, ay, bx, by)| ((ax, ay), (bx, by), WALL_R));
        let pins = self.pins.iter().filter(|p| !p.gone()).map(|p| (p.now().0, p.now().1, PIN_R));
        let mut segs = std::mem::take(&mut self.segs);
        segs.clear();
        segs.extend(walls.chain(pins).map(|(a, b, r)| (a, b, r, (a.0.min(b.0) - r, a.1.min(b.1) - r, a.0.max(b.0) + r, a.1.max(b.1) + r))));
        let solid = |p: (f32, f32), pr: f32| -> Option<((f32, f32), f32)> {
            let mut best: Option<((f32, f32), f32)> = None;
            for &(a, b, r, (x0, y0, x1, y1)) in &segs {
                if p.0 + pr < x0 || p.0 - pr > x1 || p.1 + pr < y0 || p.1 - pr > y1 {
                    continue;
                }
                if let Some(c) = capsule(p, pr, a, b, r) {
                    if best.is_none_or(|b| c.1 > b.1) {
                        best = Some(c);
                    }
                }
            }
            best
        };
        let knight = knight_box(lv.knight);
        for _ in 0..3 {
            self.collide();
            for g in self.grains.iter_mut() {
                let r = radius(g.kind);
                // a few passes, as a particle can sit in a corner
                for _ in 0..2 {
                    match solid(g.p, r) {
                        Some((n, d)) => slide(g, n, d),
                        None => break,
                    }
                }
                if let Some((n, d)) = boxed(g.p, r, knight) {
                    slide(g, n, d);
                }
                if let Some(o) = &self.orc {
                    let (dx, dy) = (g.p.0 - o.p.0, g.p.1 - o.p.1);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < ORC_R + r && d > 1e-4 {
                        let k = (ORC_R + r - d) / d;
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
        self.segs = segs;
        // what touches the knight and the orc
        for g in &self.grains {
            if g.kind != LAVA {
                continue;
            }
            if boxed(g.p, BURN, knight).is_some() {
                self.burned = true;
            }
            if let Some(o) = &mut self.orc {
                if ((g.p.0 - o.p.0).powi(2) + (g.p.1 - o.p.1).powi(2)).sqrt() < ORC_R + BURN {
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

    /// Sorts the particles into the grid and lists every pair nearer than
    /// a drop's reach.
    fn find_pairs(&mut self) {
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
        self.wet.clear();
        self.dry.clear();
        for i in 0..self.grains.len() {
            let (p, c) = (self.grains[i].p, self.cells[i] as i32);
            let (cx, cy) = (c % COLS, c / COLS);
            // its own cell and the four ahead, so each pair is met once
            for (dx, dy) in [(0, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || nx >= COLS || ny >= ROWS {
                    continue;
                }
                let mut j = self.head[(ny * COLS + nx) as usize];
                while j >= 0 {
                    let ju = j as usize;
                    j = self.next[ju];
                    if dx == 0 && dy == 0 && ju <= i {
                        continue;
                    }
                    let q = self.grains[ju].p;
                    if (q.0 - p.0).powi(2) + (q.1 - p.1).powi(2) < H * H {
                        let wet = liquid(self.grains[i].kind) && liquid(self.grains[ju].kind);
                        (if wet { &mut self.wet } else { &mut self.dry }).push((i as u32, ju as u32));
                    }
                }
            }
        }
    }

    /// Drops closing in on each other give up some of that speed, lava much
    /// more than water.
    fn viscosity(&mut self) {
        for &(i, j) in &self.wet {
            let (a, b) = (self.grains[i as usize], self.grains[j as usize]);
            if !liquid(a.kind) || a.kind != b.kind {
                continue;
            }
            let (dx, dy) = (b.p.0 - a.p.0, b.p.1 - a.p.1);
            let d = (dx * dx + dy * dy).sqrt();
            if d < 1e-4 || d >= H {
                continue;
            }
            let n = (dx / d, dy / d);
            // speeds as moves a step: how fast they close in
            let u = ((a.p.0 - a.prev.0) - (b.p.0 - b.prev.0)) * n.0 + ((a.p.1 - a.prev.1) - (b.p.1 - b.prev.1)) * n.1;
            if u <= 0.0 {
                continue;
            }
            let (lin, quad) = viscosity(a.kind);
            let k = ((1.0 - d / H) * (lin * u + quad * u * u)).min(u) * 0.5;
            // less speed: move where they came from
            self.grains[i as usize].prev.0 += n.0 * k;
            self.grains[i as usize].prev.1 += n.1 * k;
            self.grains[j as usize].prev.0 -= n.0 * k;
            self.grains[j as usize].prev.1 -= n.1 * k;
        }
    }

    /// Each drop's crowding, then pushes that spread the crowded ones out
    /// (and a lighter push that pulls a thin surface together).
    fn relax(&mut self) {
        self.density.clear();
        self.density.resize(self.grains.len(), (0.0, 0.0));
        for &(i, j) in &self.wet {
            let (a, b) = (&self.grains[i as usize], &self.grains[j as usize]);
            if !liquid(a.kind) || !liquid(b.kind) {
                continue;
            }
            let d = ((b.p.0 - a.p.0).powi(2) + (b.p.1 - a.p.1).powi(2)).sqrt();
            let (w, near) = crowding((d / H).min(1.0));
            for k in [i, j] {
                let c = &mut self.density[k as usize];
                c.0 += w;
                c.1 += near;
            }
        }
        let rest = self.rest_density;
        let pressure = |c: (f32, f32)| (STIFF * (c.0 - rest), NEAR_STIFF * c.1);
        for &(i, j) in &self.wet {
            let (a, b) = (self.grains[i as usize], self.grains[j as usize]);
            if !liquid(a.kind) || !liquid(b.kind) {
                continue;
            }
            let (dx, dy) = (b.p.0 - a.p.0, b.p.1 - a.p.1);
            let d = (dx * dx + dy * dy).sqrt();
            if d < 1e-4 || d >= H {
                continue;
            }
            let q = 1.0 - d / H;
            let (pa, na) = pressure(self.density[i as usize]);
            let (pb, nb) = pressure(self.density[j as usize]);
            let push = ((pa + pb) * 0.5 * q + (na + nb) * 0.5 * q * q) * 0.5 / d;
            self.grains[i as usize].p.0 -= dx * push;
            self.grains[i as usize].p.1 -= dy * push;
            self.grains[j as usize].p.0 += dx * push;
            self.grains[j as usize].p.1 += dy * push;
        }
    }

    /// Pushes overlapping particles apart where one of them is a coin or a
    /// stone, the lighter one more.
    fn collide(&mut self) {
        for &(i, j) in &self.dry {
            let (a, b) = (self.grains[i as usize], self.grains[j as usize]);
            let two = radius(a.kind) + radius(b.kind);
            let (dx, dy) = (b.p.0 - a.p.0, b.p.1 - a.p.1);
            let d2 = dx * dx + dy * dy;
            if d2 >= two * two {
                continue;
            }
            let d = d2.sqrt().max(1e-4);
            let (ma, mb) = (mass(a.kind), mass(b.kind));
            let k = (two - d) / d;
            let (ka, kb) = (k * mb / (ma + mb), k * ma / (ma + mb));
            let pa = (a.p.0 - dx * ka, a.p.1 - dy * ka);
            let pb = (b.p.0 + dx * kb, b.p.1 + dy * kb);
            let (prev_a, prev_b) = if liquid(a.kind) || liquid(b.kind) {
                // a drop on a coin or stone: apart, and no longer closing
                // in, so a pool on them comes to rest
                let n = (dx / d, dy / d);
                let (va, vb) = ((a.p.0 - a.prev.0, a.p.1 - a.prev.1), (b.p.0 - b.prev.0, b.p.1 - b.prev.1));
                let closing = ((va.0 - vb.0) * n.0 + (va.1 - vb.1) * n.1).max(0.0);
                let (ca, cb) = (closing * mb / (ma + mb), closing * ma / (ma + mb));
                ((pa.0 - va.0 + n.0 * ca, pa.1 - va.1 + n.1 * ca), (pb.0 - vb.0 - n.0 * cb, pb.1 - vb.1 - n.1 * cb))
            } else {
                // coins and stones: apart, with half the push kept as
                // speed: no splashes
                ((a.prev.0 - dx * ka * 0.5, a.prev.1 - dy * ka * 0.5), (b.prev.0 + dx * kb * 0.5, b.prev.1 + dy * kb * 0.5))
            };
            let gi = &mut self.grains[i as usize];
            (gi.p, gi.prev) = (pa, prev_a);
            let gj = &mut self.grains[j as usize];
            (gj.p, gj.prev) = (pb, prev_b);
        }
    }

    /// Lava touching water, or a stone still hot from it, turns to stone,
    /// with a puff of steam where it met water.
    fn react(&mut self) {
        let reach = 2.0 * DROP_R + 1.0;
        let World { grains, wet, dry, steam, .. } = self;
        for &(i, j) in wet.iter().chain(dry.iter()) {
            let (i, j) = (i as usize, j as usize);
            let (a, b) = (grains[i], grains[j]);
            if (b.p.0 - a.p.0).powi(2) + (b.p.1 - a.p.1).powi(2) >= reach * reach {
                continue;
            }
            for (l, o) in [(i, j), (j, i)] {
                let (lava, other) = (grains[l], grains[o]);
                if lava.kind != LAVA || !(other.kind == WATER || (other.kind == STONE && other.heat > 0.0)) {
                    continue;
                }
                grains[l].kind = STONE;
                grains[l].heat = HEAT_MS;
                if other.kind == WATER && steam.len() < 40 {
                    steam.push(Steam { p: lava.p, age: 0.0 });
                }
            }
        }
        // pairs with a new stone collide from now on
        wet.retain(|&(i, j)| {
            let both = liquid(grains[i as usize].kind) && liquid(grains[j as usize].kind);
            if !both {
                dry.push((i, j));
            }
            both
        });
    }
}
