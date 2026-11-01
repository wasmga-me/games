//! The physics: water and lava as a particle fluid (each drop pushed by the
//! crowding round it, after Clavet, Beaudoin and Poulin's double density
//! relaxation, and slowed by its neighbours' viscosity), coins as grains
//! that pile; lava that water turns to stone, the stones made together one
//! rigid lump; and the orc, a heavy body that falls when what holds it is
//! pulled away.

use crate::levels::{Level, DROP_R, GOLD, GRAIN_R, GROUND, LAVA, PIN_R, ROW_RISE, SPACING, STONE, WALL_R, WATER};

/// one physics step, ms
pub const STEP_MS: f32 = 1000.0 / 120.0;
const DT: f32 = STEP_MS / 1000.0;
const GRAVITY: f32 = 700.0;
/// how long a pin takes to slide out, and shakes when another holds it, ms
const PULL_MS: f32 = 240.0;
pub const JAM_MS: f32 = 260.0;
/// ms a new stone joins the stones it touches into one lump
const HEAT_MS: f32 = 220.0;
/// drops of connected lava each drop of water sets to stone
const COLD: usize = 3;
/// a drop of water that boiled away
const GONE: u8 = 255;
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
/// steps between the marks that tell whether things still move, and how
/// many drops may shiver in place at rest
const MARK: u32 = 12;
const SHIVER: usize = 4;
/// how near two drops' middles are when they touch
const TOUCH: f32 = 2.0 * DROP_R + 1.0;
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
    /// ms left of joining other stones, for a stone just made
    heat: f32,
    /// where it was at the last mark (see `World::moved`)
    mark: (f32, f32),
    /// how low it fell to this step before anything pushed it back, and
    /// whether something held it up (it is not falling freely)
    fell: f32,
    held: bool,
    /// the lump of stone it belongs to, or `LOOSE`
    lump: u32,
}

const LOOSE: u32 = u32::MAX;

/// Stones made together, which keep the shape they were made in: their
/// grains, and each one's place from the middle when the lump was made.
struct Lump {
    of: Vec<u32>,
    shape: Vec<(f32, f32)>,
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
    /// how far out it has slid, 0 to 1 (out of the dungeon for good), and
    /// how far the player wants it
    pub out: f32,
    pub goal: f32,
    /// ms left of the shake that shows another pin holds it
    pub jam: f32,
    /// whether it has moved at all
    moved: bool,
}

/// the ring on a pin's handle: its middle this far past the handle end, and
/// how far round it other pins keep
pub const RING: f32 = 7.5;
pub const RING_R: f32 = 10.0;

impl Pin {
    /// where it is now
    pub fn now(&self) -> ((f32, f32), (f32, f32)) {
        self.at(self.out)
    }
    /// how far and which way it slides, all the way out
    pub fn travel(&self) -> (f32, f32) {
        ((self.b.0 - self.a.0) * 1.05, (self.b.1 - self.a.1) * 1.05)
    }
    /// where it is when this far out
    fn at(&self, out: f32) -> ((f32, f32), (f32, f32)) {
        let t = self.travel();
        let d = (t.0 * out, t.1 * out);
        ((self.a.0 + d.0, self.a.1 + d.1), (self.b.0 + d.0, self.b.1 + d.1))
    }
    /// its ring's middle when this far out
    pub fn ring(&self, out: f32) -> (f32, f32) {
        let (a, b) = self.at(out);
        let l = dist(a, b).max(1.0);
        (b.0 + (b.0 - a.0) / l * RING, b.1 + (b.1 - a.1) / l * RING)
    }
    pub fn gone(&self) -> bool {
        self.out >= 1.0
    }
    /// out for good, or where the player wants it
    fn settled(&self) -> bool {
        self.gone() || self.out == self.goal
    }
    /// whether it has moved at all, even if it is back in now
    pub fn touched(&self) -> bool {
        self.moved
    }
    /// How far this pin, this far out, keeps clear of pin q where it is now
    /// (below 0 where they overlap): rod on rod, ring on rod, ring on ring.
    fn clearance(&self, out: f32, q: &Pin) -> f32 {
        let ((a, b), c) = (self.at(out), self.ring(out));
        let ((qa, qb), qc) = (q.now(), q.ring(q.out));
        let rod = seg_seg_dist(a, b, qa, qb) - 2.0 * PIN_R;
        let rings = dist(c, qc) - 2.0 * RING_R;
        rod.min(seg_dist(c, qa, qb) - RING_R - PIN_R).min(seg_dist(qc, a, b) - RING_R - PIN_R).min(rings)
    }
}

/// The distance between segments a-b and c-d.
fn seg_seg_dist(a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) -> f32 {
    let cross = |o: (f32, f32), p: (f32, f32), q: (f32, f32)| (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0);
    let (d1, d2, d3, d4) = (cross(c, d, a), cross(c, d, b), cross(a, b, c), cross(a, b, d));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return 0.0;
    }
    seg_dist(a, c, d).min(seg_dist(b, c, d)).min(seg_dist(c, a, b)).min(seg_dist(d, a, b))
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

impl Orc {
    fn moving(&self) -> bool {
        self.p.1 < 600.0 && dist(self.p, self.start) / DT >= REST
    }
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
    /// lumps of stone; one that joined another is left empty
    lumps: Vec<Lump>,
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

/// The distance between points p and q.
pub fn dist(p: (f32, f32), q: (f32, f32)) -> f32 {
    ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt()
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

/// First contact along a circle's entire motion, including the round ends
/// of a wall. Overlap-only collision misses particles pushed through a pin
/// by fluid pressure or a rigid stone's shape correction.
fn sweep(from: (f32, f32), to: (f32, f32), pr: f32, seg: &Seg) -> Option<(f32, (f32, f32))> {
    let (a, b, r, (x0, y0, x1, y1)) = *seg;
    if from.0.max(to.0) + pr < x0 || from.0.min(to.0) - pr > x1 || from.1.max(to.1) + pr < y0 || from.1.min(to.1) - pr > y1 {
        return None;
    }
    let radius = pr + r;
    let v = (to.0 - from.0, to.1 - from.1);
    let vv = v.0 * v.0 + v.1 * v.1;
    if vv < 1e-10 {
        return None;
    }
    let mut hit: Option<(f32, (f32, f32))> = None;
    let mut keep = |t: f32, n: (f32, f32)| {
        if (0.0..=1.0).contains(&t) && v.0 * n.0 + v.1 * n.1 < -1e-6 && hit.is_none_or(|h| t < h.0) {
            hit = Some((t, n));
        }
    };
    let len = dist(a, b);
    if len > 1e-6 {
        let axis = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let n = (-axis.1, axis.0);
        let d = (from.0 - a.0) * n.0 + (from.1 - a.1) * n.1;
        let speed = v.0 * n.0 + v.1 * n.1;
        if speed.abs() > 1e-6 {
            for sign in [-1.0, 1.0] {
                let t = (sign * radius - d) / speed;
                let along = (from.0 + v.0 * t - a.0) * axis.0 + (from.1 + v.1 * t - a.1) * axis.1;
                if (0.0..=len).contains(&along) {
                    keep(t, (sign * n.0, sign * n.1));
                }
            }
        }
    }
    for c in [a, b] {
        let q = (from.0 - c.0, from.1 - c.1);
        let dot = q.0 * v.0 + q.1 * v.1;
        let disc = dot * dot - vv * (q.0 * q.0 + q.1 * q.1 - radius * radius);
        if disc >= 0.0 {
            let t = (-dot - disc.sqrt()) / vv;
            keep(t, ((q.0 + t * v.0) / radius, (q.1 + t * v.1) / radius));
        }
    }
    hit
}

fn seal(g: &mut Grain, safe: &mut (f32, f32), segs: &[Seg]) {
    let r = radius(g.kind);
    for _ in 0..4 {
        let hit = segs.iter().filter_map(|s| sweep(*safe, g.p, r, s)).min_by(|a, b| a.0.total_cmp(&b.0));
        let Some((t, n)) = hit else { break };
        let v = (g.p.0 - safe.0, g.p.1 - safe.1);
        let contact = (safe.0 + v.0 * t + n.0 * 0.001, safe.1 + v.1 * t + n.1 * 0.001);
        let remain = (v.0 * (1.0 - t), v.1 * (1.0 - t));
        let into = (remain.0 * n.0 + remain.1 * n.1).min(0.0);
        g.p = (contact.0 + remain.0 - into * n.0, contact.1 + remain.1 - into * n.1);
        // Remove the normal velocity as well as the positional error.
        slide(g, n, 0.0);
        *safe = contact;
    }
    *safe = g.p;
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
        let grains =
            lv.grains.iter().map(|&(x, y, kind)| Grain { p: (x, y), prev: (x, y), kind, heat: 0.0, mark: (x, y), fell: y, held: false, lump: LOOSE }).collect();
        let pins = lv.pins.iter().map(|&(ax, ay, bx, by)| Pin { a: (ax, ay), b: (bx, by), out: 0.0, goal: 0.0, jam: 0.0, moved: false }).collect();
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
            lumps: Vec::new(),
        }
    }

    /// Starts pin i sliding toward `goal` (1 all the way out, 0 all the
    /// way in); false if it is already out for good or already heading
    /// there.
    pub fn slide(&mut self, i: usize, goal: f32) -> bool {
        let Some(pin) = self.pins.get_mut(i) else { return false };
        let goal = goal.clamp(0.0, 1.0);
        if pin.gone() || pin.goal == goal {
            return false;
        }
        pin.goal = goal;
        true
    }

    /// Whether everything has come to rest: the orc still, and each coin
    /// and drop no further than a slow crawl from where it was at the last
    /// mark (crowded drops and coins shiver in place from step to step).
    pub fn resting(&self) -> bool {
        self.pins.iter().all(Pin::settled) && self.steam.is_empty() && !self.moved && self.orc.is_none_or(|o| !o.moving())
    }

    /// Whether a living orc is still on its way: a living orc on its feet
    /// always walks at the knight, so one standing still is walled off.
    pub fn orc_coming(&self) -> bool {
        self.orc.is_some_and(|o| o.alive && o.moving())
    }

    /// One step.
    pub fn step(&mut self, lv: &Level) {
        let mut safe: Vec<_> = self.grains.iter().map(|g| g.p).collect();
        // the pins slide toward where the player wants them, until one
        // runs into another
        for pin in self.pins.iter_mut() {
            pin.jam = (pin.jam - STEP_MS).max(0.0);
        }
        for i in 0..self.pins.len() {
            let pin = self.pins[i];
            if pin.settled() {
                continue;
            }
            let reach = STEP_MS / PULL_MS;
            let out = if pin.goal > pin.out { (pin.out + reach).min(pin.goal) } else { (pin.out - reach).max(pin.goal) };
            let blocked = self.pins.iter().enumerate().any(|(j, q)| {
                if j == i || q.gone() {
                    return false;
                }
                let now = pin.clearance(pin.out, q);
                let next = pin.clearance(out, q);
                next < 0.0 && next < now
            });
            let pin = &mut self.pins[i];
            if blocked {
                pin.goal = pin.out;
                pin.jam = JAM_MS;
            } else {
                pin.out = out;
                pin.moved = true;
            }
        }
        for s in self.steam.iter_mut() {
            s.age += STEP_MS;
            s.p.1 -= 18.0 * DT;
        }
        self.steam.retain(|s| s.age < STEAM_MS);
        // every MARK steps, whether anything moved since the last mark
        self.steps += 1;
        if self.steps % MARK == 0 {
            // a few drops shivering on a pin's round top don't count; one
            // that really travels does, and so does any coin or stone
            let far = REST * MARK as f32 * DT;
            let (mut some, mut most, mut solid) = (0, 0.0f32, false);
            for g in self.grains.iter().filter(|g| g.kind != GONE && g.p.1 < 600.0 && dist(g.p, g.mark) > far) {
                some += 1;
                most = most.max(dist(g.p, g.mark));
                solid |= !liquid(g.kind);
            }
            self.moved = solid || some > SHIVER || most > 4.0 * far;
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
        for g in self.grains.iter_mut().filter(|g| g.kind != GONE) {
            // water that rests on something stops shivering (water falling
            // freely must not: it would hang in the air); lava is thick
            // enough to settle by itself, and creeps on slowly
            fall(&mut g.p, &mut g.prev, if g.kind == WATER && g.held { 2.0 * REST } else { 0.0 });
            g.fell = g.p.1;
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
        // Rebuild before finding neighbours: a closed pin also separates
        // pressure, viscosity and water/lava reactions on its two sides.
        let walls = lv.walls.iter().map(|&(ax, ay, bx, by)| ((ax, ay), (bx, by), WALL_R));
        let pins = self.pins.iter().filter(|p| !p.gone()).map(|p| (p.now().0, p.now().1, PIN_R));
        self.segs.clear();
        self.segs.extend(walls.chain(pins).map(|(a, b, r)| (a, b, r, (a.0.min(b.0) - r, a.1.min(b.1) - r, a.0.max(b.0) + r, a.1.max(b.1) + r))));
        self.find_pairs();
        self.react();
        self.fuse();
        self.relax();
        // the walls and the pins still in, with the boxes round them
        let segs = std::mem::take(&mut self.segs);
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
            self.keep_shapes();
            for (i, g) in self.grains.iter_mut().enumerate().filter(|(_, g)| g.kind != GONE) {
                let r = radius(g.kind);
                seal(g, &mut safe[i], &segs);
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
                if let Some(o) = self.orc.as_ref().filter(|o| o.alive) {
                    let (dx, dy) = (g.p.0 - o.p.0, g.p.1 - o.p.1);
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < ORC_R + r && d > 1e-4 {
                        let k = (ORC_R + r - d) / d;
                        g.p = (g.p.0 + dx * k, g.p.1 + dy * k);
                    }
                }
                seal(g, &mut safe[i], &segs);
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
        for g in self.grains.iter_mut() {
            g.held = g.fell - g.p.1 > 0.5 * GRAVITY * DT * DT;
        }
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
            if self.grains[i].kind == GONE {
                continue;
            }
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
                    if (dx == 0 && dy == 0 && ju <= i) || self.grains[ju].kind == GONE {
                        continue;
                    }
                    let q = self.grains[ju].p;
                    if (q.0 - p.0).powi(2) + (q.1 - p.1).powi(2) < H * H && !self.segs.iter().any(|&(a, b, r, (x0, y0, x1, y1))| {
                        p.0.max(q.0) >= x0 && p.0.min(q.0) <= x1 && p.1.max(q.1) >= y0 && p.1.min(q.1) <= y1
                            && seg_seg_dist(p, q, a, b) <= r
                    }) {
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
            if a.lump != LOOSE && a.lump == b.lump {
                continue;
            }
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

    /// Cooling travels through a connected lava/stone pool. A crust must
    /// not insulate an arbitrary pocket just because it exceeds a radius.
    /// The neighbour graph already excludes contacts across closed barriers.
    fn react(&mut self) {
        if !self.grains.iter().any(|g| g.kind == LAVA) { return; }
        fn root(parent: &mut [usize], mut i: usize) -> usize {
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }
        let mut parent: Vec<_> = (0..self.grains.len()).collect();
        // Coins conduct heat too: they must not insulate trapped lava
        // from water sitting directly on the pile.
        let rock = |k| k == LAVA || k == STONE || k == GOLD;
        for &(i, j) in self.wet.iter().chain(&self.dry) {
            let (i, j) = (i as usize, j as usize);
            let (a, b) = (self.grains[i], self.grains[j]);
            if rock(a.kind) && rock(b.kind) && dist(a.p, b.p) < radius(a.kind) + radius(b.kind) + 1.0 {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                parent[a] = b;
            }
        }
        let mut pools = vec![Vec::new(); self.grains.len()];
        for (i, g) in self.grains.iter().enumerate() {
            if g.kind == LAVA { pools[root(&mut parent, i)].push(i); }
        }
        for &(i, j) in self.wet.iter().chain(&self.dry) {
            for (w, r) in [(i as usize, j as usize), (j as usize, i as usize)] {
                let (water, stone) = (self.grains[w], self.grains[r]);
                if water.kind != WATER || !rock(stone.kind) || dist(water.p, stone.p) >= DROP_R + radius(stone.kind) + 1.0 { continue; }
                let pool = &mut pools[root(&mut parent, r)];
                if pool.is_empty() { continue; }
                // Work inward from the contact. Each consumed water drop
                // still has a finite cooling capacity.
                pool.sort_by(|&a, &b| dist(self.grains[b].p, water.p).total_cmp(&dist(self.grains[a].p, water.p)));
                for _ in 0..COLD {
                    let Some(k) = pool.pop() else { break };
                    self.grains[k].kind = STONE;
                    self.grains[k].heat = HEAT_MS;
                }
                self.grains[w].kind = GONE;
                if self.steam.len() < 40 { self.steam.push(Steam { p: water.p, age: 0.0 }); }
            }
        }
        self.wet.retain(|&(i, j)| {
            let (a, b) = (self.grains[i as usize].kind, self.grains[j as usize].kind);
            let both = liquid(a) && liquid(b);
            if !both && a != GONE && b != GONE { self.dry.push((i, j)); }
            both
        });
    }

    /// Stones touching where one is still hot from being made join into one
    /// lump (stones long cold only pile on each other).
    fn fuse(&mut self) {
        let mut changed = Vec::new();
        for k in 0..self.dry.len() {
            let (i, j) = self.dry[k];
            let (a, b) = (self.grains[i as usize], self.grains[j as usize]);
            if a.kind != STONE || b.kind != STONE || (a.heat <= 0.0 && b.heat <= 0.0) || (a.lump != LOOSE && a.lump == b.lump) {
                continue;
            }
            if dist(a.p, b.p) >= TOUCH {
                continue;
            }
            // into a's lump (a new one if a is loose), b and its lump with it
            let into = if a.lump == LOOSE {
                self.lumps.push(Lump { of: vec![i], shape: Vec::new() });
                self.grains[i as usize].lump = (self.lumps.len() - 1) as u32;
                self.lumps.len() - 1
            } else {
                a.lump as usize
            };
            let joining = if b.lump == LOOSE { vec![j] } else { std::mem::take(&mut self.lumps[b.lump as usize].of) };
            for &g in &joining {
                self.grains[g as usize].lump = into as u32;
            }
            self.lumps[into].of.extend(joining);
            if !changed.contains(&into) {
                changed.push(into);
            }
        }
        // each lump that grew keeps the shape it has now
        for into in changed {
            let lump = &mut self.lumps[into];
            let c = centre(&self.grains, &lump.of);
            lump.shape = lump.of.iter().map(|&g| (self.grains[g as usize].p.0 - c.0, self.grains[g as usize].p.1 - c.1)).collect();
        }
    }

    /// Puts each lump's stones back in its shape, turned and moved to fit
    /// where they have been pushed (shape matching).
    fn keep_shapes(&mut self) {
        for lump in &self.lumps {
            if lump.of.len() < 2 {
                continue;
            }
            let c = centre(&self.grains, &lump.of);
            // the turn that best fits the shape to the stones
            let (mut dot, mut cross) = (0.0, 0.0);
            for (&g, q) in lump.of.iter().zip(&lump.shape) {
                let r = (self.grains[g as usize].p.0 - c.0, self.grains[g as usize].p.1 - c.1);
                dot += q.0 * r.0 + q.1 * r.1;
                cross += q.0 * r.1 - q.1 * r.0;
            }
            let l = (dot * dot + cross * cross).sqrt().max(1e-6);
            let (sin, cos) = (cross / l, dot / l);
            for (&g, q) in lump.of.iter().zip(&lump.shape) {
                self.grains[g as usize].p = (c.0 + q.0 * cos - q.1 * sin, c.1 + q.0 * sin + q.1 * cos);
            }
        }
    }
}

/// The middle of these grains.
fn centre(grains: &[Grain], of: &[u32]) -> (f32, f32) {
    let n = of.len() as f32;
    of.iter().fold((0.0, 0.0), |c, &g| (c.0 + grains[g as usize].p.0 / n, c.1 + grains[g as usize].p.1 / n))
}

#[cfg(test)]
mod collision_tests {
    use super::*;

    #[test]
    fn dead_orc_does_not_displace_liquid() {
        let lv = crate::levels::level(3);
        let mut corpse = World::new(&lv);
        let mut absent = World::new(&lv);
        corpse.orc.as_mut().unwrap().alive = false;
        absent.orc = None;
        for w in [&mut corpse, &mut absent] {
            w.pins.iter_mut().for_each(|p| { p.out = 1.0; p.goal = 1.0; });
        }
        for _ in 0..600 { corpse.step(&lv); absent.step(&lv); }
        for (a, b) in corpse.grains.iter().zip(&absent.grains) {
            assert_eq!(a.p, b.p, "a corpse must not act as a dam");
        }
    }

    #[test]
    fn chute_water_cools_the_whole_connected_lava_pool() {
        let lv = crate::levels::level(5);
        for order in [[0, 2], [2, 0]] {
            let mut w = World::new(&lv);
            for pin in order {
                w.slide(pin, 1.0);
                for _ in 0..600 { w.step(&lv); }
            }
            for _ in 0..1200 { w.step(&lv); }
            let lava = w.grains.iter().filter(|g| g.kind == LAVA).count();
            let water = w.grains.iter().filter(|g| g.kind == WATER).count();
            assert_eq!(lava, 0, "order {order:?}: {lava} lava remains with {water} water");
            assert_eq!(w.grains.iter().filter(|g| g.kind == GOLD).count(), lv.grains.iter().filter(|g| g.2 == GOLD).count());
        }
    }

    #[test]
    fn cooling_respects_barriers_and_water_capacity() {
        let mut lv = crate::levels::level(0);
        lv.grains = vec![(100.0, 97.5, WATER), (100.0, 102.5, LAVA)];
        let mut w = World::new(&lv);
        w.segs = vec![segment((80.0, 100.0), (120.0, 100.0), PIN_R)];
        w.find_pairs();
        w.react();
        assert_eq!(w.grains[0].kind, WATER);
        assert_eq!(w.grains[1].kind, LAVA);
        w.segs.clear();
        w.find_pairs();
        w.react();
        assert_eq!(w.grains[0].kind, GONE);
        assert_eq!(w.grains[1].kind, STONE);

        lv.grains = vec![(100.0, 100.0, WATER)];
        lv.grains.extend((1..=5).map(|i| (100.0 + i as f32 * 5.0, 100.0, LAVA)));
        let mut w = World::new(&lv);
        for _ in 0..5 { w.find_pairs(); w.react(); }
        assert_eq!(w.grains.iter().filter(|g| g.kind == STONE).count(), COLD);
        assert_eq!(w.grains.iter().filter(|g| g.kind == LAVA).count(), 5 - COLD);
    }

    fn segment(a: (f32, f32), b: (f32, f32), r: f32) -> Seg {
        (a, b, r, (a.0.min(b.0) - r, a.1.min(b.1) - r, a.0.max(b.0) + r, a.1.max(b.1) + r))
    }

    #[test]
    fn fast_motion_and_pressure_cannot_cross_a_pin() {
        for (a, b, from, to) in [
            ((0.0, 100.0), (100.0, 100.0), (50.0, 80.0), (50.0, 130.0)),
            ((100.0, 0.0), (100.0, 100.0), (80.0, 50.0), (130.0, 50.0)),
            ((0.0, 0.0), (100.0, 100.0), (20.0, 60.0), (70.0, 30.0)),
        ] {
            let barrier = segment(a, b, PIN_R);
            let (t, _) = sweep(from, to, DROP_R, &barrier).expect("swept contact");
            assert!(t > 0.0 && t < 1.0);
        }
        let bar = segment((0.0, 100.0), (100.0, 100.0), PIN_R);
        assert!(sweep((110.0, 80.0), (110.0, 130.0), DROP_R, &bar).is_none(), "open gap must pass fluid");
        assert!(sweep((103.0, 80.0), (103.0, 130.0), DROP_R, &bar).is_some(), "round pin tip must seal");
    }

    #[test]
    fn untouched_chambers_hold_their_contents() {
        for k in 0..crate::levels::LEVELS {
            let lv = crate::levels::level(k);
            let mut world = World::new(&lv);
            for _ in 0..360 {
                world.step(&lv);
            }
            assert!(!world.burned && !world.caught, "level {} fails without input", k + 1);
            assert!(world.grains.iter().all(|g| g.p.0.is_finite() && g.p.1.is_finite()));
            for g in &world.grains {
                for &(ax, ay, bx, by) in &lv.walls {
                    assert!(seg_dist(g.p, (ax, ay), (bx, by)) + 0.1 >= WALL_R + radius(g.kind), "level {} leaks into a wall", k + 1);
                }
            }
        }
    }
}
