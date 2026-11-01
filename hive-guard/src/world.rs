//! The physics: each drawn piece of ink as a solid bar that falls, turns
//! and rests, and the bees that find their way to the dog and shove what
//! is in it.

use crate::levels::Level;
use wg::Rng;

/// the line's half thickness, plan units
pub const LINE_R: f32 = 3.0;
/// one physics step, ms
pub const STEP_MS: f32 = 1000.0 / 120.0;
const DT: f32 = STEP_MS / 1000.0;
/// plan units a second, each second
const GRAVITY: f32 = 520.0;
const BOUNCE: f32 = 0.1;
const FRICTION: f32 = 0.6;
/// a bee's radius, and how often a hive lets one out, ms
const BEE_R: f32 = 4.0;
const SPAWN_MS: f32 = 240.0;
/// ms before the first bee
const WAKE_MS: f32 = 500.0;
/// a bee's weight against one drawn point of ink: what it shoves the ink with
const BEE_MASS: f32 = 3.0;
/// and boxed in at its hive, where it pushes its hardest
const BOXED_MASS: f32 = 16.0;
/// how near its hive a bee with no way to the dog pushes out and up from
/// the hive, at BOXED_MASS. Pressing toward the dog alone shoves a cap on a
/// ledge or the ground into what it rests on, so it holds; and bees
/// pressing as hard everywhere tip a dome over the dog.
const BOXED: f32 = 80.0;
/// how hard a bee flies at its way, and how much it wanders off it
const SEEK: f32 = 300.0;
const WANDER: f32 = 230.0;

/// p turned by the angle whose sine and cosine are `sc`
fn rot(p: (f32, f32), (s, c): (f32, f32)) -> (f32, f32) {
    (p.0 * c - p.1 * s, p.0 * s + p.1 * c)
}
fn cross(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.1 - a.1 * b.0
}
fn dot(a: (f32, f32), b: (f32, f32)) -> f32 {
    a.0 * b.0 + a.1 * b.1
}

/// The nearest point to p of the segment a b, and how far it is.
pub fn nearest(a: (f32, f32), b: (f32, f32), p: (f32, f32)) -> ((f32, f32), f32) {
    let ab = (b.0 - a.0, b.1 - a.1);
    let t = (dot((p.0 - a.0, p.1 - a.1), ab) / dot(ab, ab).max(1e-6)).clamp(0.0, 1.0);
    let q = (a.0 + ab.0 * t, a.1 + ab.1 * t);
    (q, ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt())
}

/// Ink once the bees are let out: the strokes that touch, as one rigid body
/// of evenly spaced points.
pub struct Bar {
    local: Vec<(f32, f32)>,
    /// where each stroke starts in the points, and where they end
    starts: Vec<usize>,
    /// the points where they are now, and the box around them
    pts: Vec<(f32, f32)>,
    bounds: (f32, f32, f32, f32),
    /// its middle, turn, and how they move
    c: (f32, f32),
    a: f32,
    v: (f32, f32),
    w: f32,
    inv_m: f32,
    inv_i: f32,
}

impl Bar {
    pub fn new(strokes: &[&[(f32, f32)]]) -> Bar {
        let mut pts = Vec::new();
        let mut starts = Vec::new();
        for s in strokes {
            starts.push(pts.len());
            pts.extend_from_slice(s);
        }
        starts.push(pts.len());
        let n = pts.len() as f32;
        let c = (pts.iter().map(|p| p.0).sum::<f32>() / n, pts.iter().map(|p| p.1).sum::<f32>() / n);
        let local: Vec<(f32, f32)> = pts.iter().map(|p| (p.0 - c.0, p.1 - c.1)).collect();
        let inertia: f32 = local.iter().map(|r| dot(*r, *r)).sum::<f32>() + n * LINE_R * LINE_R;
        let mut bar = Bar { local, starts, pts, bounds: (0.0, 0.0, 0.0, 0.0), c, a: 0.0, v: (0.0, 0.0), w: 0.0, inv_m: 1.0 / n, inv_i: 1.0 / inertia };
        bar.place();
        bar
    }

    /// The strokes the bar is made of, where they are now.
    pub fn strokes(&self) -> impl Iterator<Item = &[(f32, f32)]> + '_ {
        self.starts.windows(2).map(|w| &self.pts[w[0]..w[1]])
    }
    /// The pieces of the bar as segments (a lone point is a segment to itself).
    fn segments(&self) -> impl Iterator<Item = ((f32, f32), (f32, f32))> + '_ {
        self.strokes().flat_map(|s| s.windows(2).map(|w| (w[0], w[1])).chain((s.len() == 1).then(|| (s[0], s[0]))))
    }

    /// The points from the middle and turn.
    fn place(&mut self) {
        let sc = self.a.sin_cos();
        let mut b = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for (q, &r) in self.pts.iter_mut().zip(&self.local) {
            let r = rot(r, sc);
            *q = (self.c.0 + r.0, self.c.1 + r.1);
            b = (b.0.min(q.0), b.1.min(q.1), b.2.max(q.0), b.3.max(q.1));
        }
        self.bounds = b;
    }

    /// whether p is within `reach` of the box around the bar
    fn near(&self, p: (f32, f32), reach: f32) -> bool {
        let b = self.bounds;
        p.0 > b.0 - reach && p.0 < b.2 + reach && p.1 > b.1 - reach && p.1 < b.3 + reach
    }

    /// The velocity of the bar's point at r from the middle.
    fn vel(&self, r: (f32, f32)) -> (f32, f32) {
        (self.v.0 - self.w * r.1, self.v.1 + self.w * r.0)
    }
    /// Impulse j at r from the middle.
    fn apply(&mut self, r: (f32, f32), j: (f32, f32)) {
        self.v.0 += j.0 * self.inv_m;
        self.v.1 += j.1 * self.inv_m;
        self.w += cross(r, j) * self.inv_i;
    }
    /// how hard it is to move at r along n
    fn give(&self, r: (f32, f32), n: (f32, f32)) -> f32 {
        let rn = cross(r, n);
        self.inv_m + rn * rn * self.inv_i
    }

    /// One step: falls, then pushes out of whatever it hits, with a little
    /// bounce and friction.
    pub fn step(&mut self, lv: &Level) {
        if self.c.1 > 900.0 {
            return;
        }
        self.v.1 += GRAVITY * DT;
        self.c.0 += self.v.0 * DT;
        self.c.1 += self.v.1 * DT;
        self.a += self.w * DT;
        let sc = self.a.sin_cos();
        for _ in 0..3 {
            for i in 0..self.local.len() {
                let r = rot(self.local[i], sc);
                let q = (self.c.0 + r.0, self.c.1 + r.1);
                let Some((n, depth)) = lv.contact(q, LINE_R, true, true) else { continue };
                // out of it, mostly by moving
                let k = (depth - 0.05).max(0.0) * 0.35;
                self.c.0 += n.0 * k;
                self.c.1 += n.1 * k;
                let vn = dot(self.vel(r), n);
                if vn >= 0.0 {
                    continue;
                }
                let j = -(1.0 + BOUNCE) * vn / self.give(r, n);
                self.apply(r, (n.0 * j, n.1 * j));
                // friction along the surface, no more than the push allows
                let t = (-n.1, n.0);
                let jt = (-dot(self.vel(r), t) / self.give(r, t)).clamp(-FRICTION * j, FRICTION * j);
                self.apply(r, (t.0 * jt, t.1 * jt));
            }
        }
        self.w *= 0.995;
        self.place();
    }

    /// The nearest point of the bar to p, if within `reach`: that point, the
    /// way out from it and how deep p is.
    fn touch(&self, p: (f32, f32), reach: f32) -> Option<((f32, f32), (f32, f32), f32)> {
        if !self.near(p, reach) {
            return None;
        }
        let mut best: Option<((f32, f32), (f32, f32), f32)> = None;
        for (a, b) in self.segments() {
            let (q, d) = nearest(a, b, p);
            if d < reach && best.is_none_or(|b| reach - d > b.2) {
                let n = if d > 1e-4 { ((p.0 - q.0) / d, (p.1 - q.1) / d) } else { (0.0, -1.0) };
                best = Some((q, n, reach - d));
            }
        }
        best
    }

    /// Keeps bar a's points out of bar b, pushing both apart.
    fn collide(a: &mut Bar, b: &mut Bar) {
        let reach = 2.0 * LINE_R;
        let (ba, bb) = (a.bounds, b.bounds);
        if ba.0 > bb.2 + reach || bb.0 > ba.2 + reach || ba.1 > bb.3 + reach || bb.1 > ba.3 + reach {
            return;
        }
        for i in 0..a.pts.len() {
            let p = a.pts[i];
            let Some((q, n, depth)) = b.touch(p, reach) else { continue };
            let (ra, rb) = ((p.0 - a.c.0, p.1 - a.c.1), (q.0 - b.c.0, q.1 - b.c.1));
            // apart, shared by weight
            let (wa, wb) = (a.inv_m, b.inv_m);
            let k = depth * 0.5 / (wa + wb);
            a.c = (a.c.0 + n.0 * k * wa, a.c.1 + n.1 * k * wa);
            b.c = (b.c.0 - n.0 * k * wb, b.c.1 - n.1 * k * wb);
            let (va, vb) = (a.vel(ra), b.vel(rb));
            let vn = dot((va.0 - vb.0, va.1 - vb.1), n);
            if vn < 0.0 {
                let j = -(1.0 + BOUNCE) * vn / (a.give(ra, n) + b.give(rb, n));
                a.apply(ra, (n.0 * j, n.1 * j));
                b.apply(rb, (-n.0 * j, -n.1 * j));
            }
        }
        a.place();
        b.place();
    }
}

/// Steps all the ink: each bar falls, then the bars keep out of each other.
pub fn step_bars(lv: &Level, bars: &mut [Bar]) {
    for b in bars.iter_mut() {
        b.step(lv);
    }
    for i in 1..bars.len() {
        let (head, tail) = bars.split_at_mut(i);
        for a in head {
            Bar::collide(&mut tail[0], a);
            Bar::collide(a, &mut tail[0]);
        }
    }
}

/// the sky the bees fly in: left, right and top
const SKY: (f32, f32, f32) = (-8.0, 308.0, 40.0);
/// a cell of the bees' map, plan units, and the map's size: the sky down to
/// the ground
const CELL: f32 = 4.0;
const MAP_W: usize = ((SKY.1 - SKY.0) / CELL) as usize + 1;
const MAP_H: usize = ((crate::levels::GROUND - SKY.2) / CELL) as usize + 1;
const FAR: u32 = u32::MAX;
/// physics steps between redrawing the bees' map
const MAP_EVERY: u32 = 6;
const NEXT: [(i32, i32, u32); 8] = [(1, 0, 10), (-1, 0, 10), (0, 1, 10), (0, -1, 10), (1, 1, 14), (1, -1, 14), (-1, 1, 14), (-1, -1, 14)];

fn cell_mid(x: usize, y: usize) -> (f32, f32) {
    (SKY.0 + (x as f32 + 0.5) * CELL, SKY.2 + (y as f32 + 0.5) * CELL)
}
/// the cell under p, unclamped
fn cell_of(p: (f32, f32)) -> (i32, i32) {
    (((p.0 - SKY.0) / CELL).floor() as i32, ((p.1 - SKY.2) / CELL).floor() as i32)
}
fn index(x: i32, y: i32) -> Option<usize> {
    ((0..MAP_W as i32).contains(&x) && (0..MAP_H as i32).contains(&y)).then(|| y as usize * MAP_W + x as usize)
}

/// Where a bee should fly: along the map, straight at the dog (next to it
/// or off the map), or nowhere, shut off from the dog by the ink.
#[derive(PartialEq)]
enum Way {
    Go((f32, f32)),
    Straight,
    Shut,
}

/// The bees' map of the sky: how far each cell is from the dog, going
/// around the ground, the ledges and the ink.
struct Map {
    /// cells a bee cannot be in for the ground and the ledges, and with the ink
    wall: Vec<bool>,
    blocked: Vec<bool>,
    /// the cells around the dog
    dog: Vec<usize>,
    /// distance to the dog: 10 a cell across, 14 a cell corner to corner
    dist: Vec<u32>,
    queue: std::collections::BinaryHeap<std::cmp::Reverse<(u32, u32)>>,
}

impl Map {
    fn new(lv: &Level) -> Map {
        let cells = || (0..MAP_H).flat_map(|y| (0..MAP_W).map(move |x| cell_mid(x, y)));
        let wall: Vec<bool> = cells().map(|p| lv.contact(p, BEE_R, false, false).is_some()).collect();
        let (dx, dy, dr) = lv.dog_circle();
        let dog = cells().enumerate().filter(|&(i, (x, y))| !wall[i] && (x - dx).powi(2) + (y - dy).powi(2) < (dr + BEE_R).powi(2)).map(|(i, _)| i).collect();
        Map { blocked: wall.clone(), wall, dog, dist: vec![FAR; MAP_W * MAP_H], queue: Default::default() }
    }

    /// Finds every cell's way to the dog with the ink where it is now.
    fn update(&mut self, bars: &[Bar]) {
        self.blocked.copy_from_slice(&self.wall);
        let reach = BEE_R + LINE_R - 0.5;
        for (a, b) in bars.iter().flat_map(Bar::segments) {
            let (x0, y0) = cell_of((a.0.min(b.0) - reach, a.1.min(b.1) - reach));
            let (x1, y1) = cell_of((a.0.max(b.0) + reach, a.1.max(b.1) + reach));
            for y in y0.max(0)..=y1.min(MAP_H as i32 - 1) {
                for x in x0.max(0)..=x1.min(MAP_W as i32 - 1) {
                    if nearest(a, b, cell_mid(x as usize, y as usize)).1 < reach {
                        self.blocked[y as usize * MAP_W + x as usize] = true;
                    }
                }
            }
        }
        self.dist.fill(FAR);
        for &i in &self.dog {
            if !self.blocked[i] {
                self.dist[i] = 0;
                self.queue.push(std::cmp::Reverse((0, i as u32)));
            }
        }
        let open = |x: i32, y: i32| index(x, y).filter(|&j| !self.blocked[j]);
        while let Some(std::cmp::Reverse((d, i))) = self.queue.pop() {
            let i = i as usize;
            if d > self.dist[i] {
                continue;
            }
            let (x, y) = ((i % MAP_W) as i32, (i / MAP_W) as i32);
            for (ox, oy, cost) in NEXT {
                // no cutting a corner of a wall or the ink
                let Some(j) = open(x + ox, y + oy).filter(|_| open(x + ox, y).is_some() && open(x, y + oy).is_some()) else { continue };
                if d + cost < self.dist[j] {
                    self.dist[j] = d + cost;
                    self.queue.push(std::cmp::Reverse((d + cost, j as u32)));
                }
            }
        }
    }

    /// The way to go from p to reach the dog around things.
    fn way(&self, p: (f32, f32)) -> Way {
        let (x, y) = cell_of(p);
        let at = |x: i32, y: i32| index(x, y).map_or(FAR, |i| self.dist[i]);
        let own = at(x, y);
        if own == 0 || index(x, y).is_none() {
            return Way::Straight;
        }
        let mut sum = (0.0, 0.0);
        if own == FAR {
            // pressed against something: toward the nearest cell with a way
            let (d, ox, oy) = NEXT.iter().map(|&(ox, oy, c)| (at(x + ox, y + oy).saturating_add(c), ox, oy)).min().unwrap();
            if d == FAR {
                return Way::Shut;
            }
            sum = (ox as f32, oy as f32);
        } else {
            // downhill, each neighbour by how much nearer it is
            for (ox, oy, cost) in NEXT {
                let d = at(x + ox, y + oy);
                if d < own {
                    let k = (own - d) as f32 / (cost * cost) as f32;
                    sum = (sum.0 + ox as f32 * k, sum.1 + oy as f32 * k);
                }
            }
        }
        let len = (sum.0 * sum.0 + sum.1 * sum.1).sqrt();
        if len > 1e-6 {
            Way::Go((sum.0 / len, sum.1 / len))
        } else {
            Way::Straight
        }
    }
}

pub struct Bee {
    pub p: (f32, f32),
    pub v: (f32, f32),
    pub age: f32,
    /// its hive's door
    home: (f32, f32),
    /// its own top speed and the way its wandering points
    speed: f32,
    wander: f32,
}

/// The bees out so far, how many each hive has let out, and their map.
pub struct Swarm {
    pub bees: Vec<Bee>,
    out: Vec<u32>,
    rng: Rng,
    map: Map,
    steps: u32,
}

impl Swarm {
    pub fn new(lv: &Level, seed: u32) -> Swarm {
        Swarm { bees: Vec::new(), out: vec![0; lv.hives.len()], rng: Rng::new(0xBEE + seed * 131), map: Map::new(lv), steps: 0 }
    }
    /// No bees out yet, keeping the level's map.
    pub fn reset(&mut self, seed: u32) {
        self.bees.clear();
        self.out.fill(0);
        self.rng = Rng::new(0xBEE + seed * 131);
        self.steps = 0;
    }

    /// One step at `clock` ms into the swarm: new bees, then every bee
    /// flies. True when one reached the dog.
    pub fn step(&mut self, lv: &Level, bars: &mut [Bar], clock: f32) -> bool {
        if self.steps % MAP_EVERY == 0 {
            self.map.update(bars);
        }
        self.steps += 1;
        let due = (((clock - WAKE_MS) / SPAWN_MS).floor() as i32 + 1).min(lv.bees as i32);
        for (h, out) in self.out.iter_mut().enumerate() {
            if (*out as i32) < due {
                *out += 1;
                let rng = &mut self.rng;
                let (x, y) = lv.door(h);
                let v = ((rng.float() - 0.5) * 80.0, -60.0 - rng.float() * 50.0);
                self.bees.push(Bee { p: (x, y), v, age: 0.0, home: (x, y), speed: 92.0 + rng.float() * 30.0, wander: rng.float() * 6.3 });
            }
        }
        let (dx, dy, dr) = lv.dog_circle();
        let reach = BEE_R + LINE_R;
        for b in self.bees.iter_mut() {
            b.age += STEP_MS;
            b.wander += (self.rng.float() - 0.5) * 0.5;
            // around things when there is a way; boxed in at the hive, out
            // and up from it; else straight at the dog
            let way = self.map.way(b.p);
            let (hx, hy) = (b.p.0 - b.home.0, b.p.1 - b.home.1);
            let boxed = way == Way::Shut && hx * hx + hy * hy < BOXED * BOXED;
            let way = if let Way::Go(w) = way { w } else {
                let (tx, ty) = if boxed { (hx, hy - BOXED * 0.5) } else { (dx - b.p.0, dy - b.p.1) };
                let d = (tx * tx + ty * ty).sqrt().max(1.0);
                (tx / d, ty / d)
            };
            let mass = if boxed { BOXED_MASS } else { BEE_MASS };
            b.v.0 += (way.0 * SEEK + b.wander.cos() * WANDER) * DT;
            b.v.1 += (way.1 * SEEK + b.wander.sin() * WANDER) * DT;
            let sp = (b.v.0 * b.v.0 + b.v.1 * b.v.1).sqrt();
            if sp > b.speed {
                b.v = (b.v.0 * b.speed / sp, b.v.1 * b.speed / sp);
            }
            b.p.0 += b.v.0 * DT;
            b.p.1 += b.v.1 * DT;
            // off the ground and the ledges (not the hives they come from)
            if let Some((n, depth)) = lv.contact(b.p, BEE_R, false, false) {
                bounce(b, n, depth);
            }
            for bar in bars.iter_mut() {
                if let Some((q, n, depth)) = bar.touch(b.p, reach) {
                    let vn = dot(b.v, n);
                    bounce(b, n, depth);
                    // the bee shoves the ink as hard as it bounced off it
                    if vn < 0.0 {
                        let r = (q.0 - bar.c.0, q.1 - bar.c.1);
                        bar.apply(r, (n.0 * vn * 1.6 * mass, n.1 * vn * 1.6 * mass));
                    }
                }
            }
            // inside the sky above the ground
            if b.p.0 < SKY.0 || b.p.0 > SKY.1 {
                b.v.0 = -b.v.0;
                b.p.0 = b.p.0.clamp(SKY.0, SKY.1);
            }
            if b.p.1 < SKY.2 {
                b.v.1 = b.v.1.abs();
            }
            if ((b.p.0 - dx).powi(2) + (b.p.1 - dy).powi(2)).sqrt() < dr + BEE_R * 0.5 {
                return true;
            }
        }
        false
    }
}

fn bounce(b: &mut Bee, n: (f32, f32), depth: f32) {
    b.p.0 += n.0 * depth;
    b.p.1 += n.1 * depth;
    let vn = dot(b.v, n);
    if vn < 0.0 {
        b.v.0 -= 1.6 * vn * n.0;
        b.v.1 -= 1.6 * vn * n.1;
    }
}
