//! The physics: the drawn line as a solid bar that falls, turns and rests,
//! and the bees that fly at the dog and bounce off it.

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

/// The line once drawn: a rigid body of evenly spaced points.
pub struct Bar {
    local: Vec<(f32, f32)>,
    /// its middle, turn, and how they move
    c: (f32, f32),
    a: f32,
    v: (f32, f32),
    w: f32,
    inv_m: f32,
    inv_i: f32,
}

impl Bar {
    pub fn new(pts: &[(f32, f32)]) -> Bar {
        let n = pts.len() as f32;
        let c = (pts.iter().map(|p| p.0).sum::<f32>() / n, pts.iter().map(|p| p.1).sum::<f32>() / n);
        let local: Vec<(f32, f32)> = pts.iter().map(|p| (p.0 - c.0, p.1 - c.1)).collect();
        let inertia: f32 = local.iter().map(|r| dot(*r, *r)).sum::<f32>() + n * LINE_R * LINE_R;
        Bar { local, c, a: 0.0, v: (0.0, 0.0), w: 0.0, inv_m: 1.0 / n, inv_i: 1.0 / inertia }
    }
    pub fn points(&self) -> impl Iterator<Item = (f32, f32)> + '_ {
        let sc = self.a.sin_cos();
        self.local.iter().map(move |&r| {
            let q = rot(r, sc);
            (self.c.0 + q.0, self.c.1 + q.1)
        })
    }

    /// Pushes the bar at world point p by impulse j.
    pub fn push(&mut self, p: (f32, f32), j: (f32, f32)) {
        self.apply((p.0 - self.c.0, p.1 - self.c.1), j);
    }
    /// Impulse j at r from the middle.
    fn apply(&mut self, r: (f32, f32), j: (f32, f32)) {
        self.v.0 += j.0 * self.inv_m;
        self.v.1 += j.1 * self.inv_m;
        self.w += cross(r, j) * self.inv_i;
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
                let vp = (self.v.0 - self.w * r.1, self.v.1 + self.w * r.0);
                let vn = dot(vp, n);
                if vn >= 0.0 {
                    continue;
                }
                let rn = cross(r, n);
                let j = -(1.0 + BOUNCE) * vn / (self.inv_m + rn * rn * self.inv_i);
                self.apply(r, (n.0 * j, n.1 * j));
                // friction along the surface, no more than the push allows
                let t = (-n.1, n.0);
                let vp = (self.v.0 - self.w * r.1, self.v.1 + self.w * r.0);
                let rt = cross(r, t);
                let jt = (-dot(vp, t) / (self.inv_m + rt * rt * self.inv_i)).clamp(-FRICTION * j, FRICTION * j);
                self.apply(r, (t.0 * jt, t.1 * jt));
            }
        }
        self.w *= 0.995;
    }

    /// The nearest point of the bar, at `pts`, to p, if within `reach`: the way out
    /// from it and how deep p is.
    fn touch(pts: &[(f32, f32)], p: (f32, f32), reach: f32) -> Option<((f32, f32), (f32, f32), f32)> {
        let mut best: Option<((f32, f32), (f32, f32), f32)> = None;
        let mut try_seg = |a: (f32, f32), b: (f32, f32)| {
            let ab = (b.0 - a.0, b.1 - a.1);
            let len2 = dot(ab, ab).max(1e-6);
            let t = (dot((p.0 - a.0, p.1 - a.1), ab) / len2).clamp(0.0, 1.0);
            let q = (a.0 + ab.0 * t, a.1 + ab.1 * t);
            let d = ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
            if d < reach && best.is_none_or(|b| reach - d > b.2) {
                let n = if d > 1e-4 { ((p.0 - q.0) / d, (p.1 - q.1) / d) } else { (0.0, -1.0) };
                best = Some((q, n, reach - d));
            }
        };
        if pts.len() == 1 {
            try_seg(pts[0], pts[0]);
        }
        for w in pts.windows(2) {
            try_seg(w[0], w[1]);
        }
        best
    }
}

pub struct Bee {
    pub p: (f32, f32),
    pub v: (f32, f32),
    pub age: f32,
    /// its own top speed and the way its wandering points
    speed: f32,
    wander: f32,
}

/// The bees out so far, and how many each hive has let out.
pub struct Swarm {
    pub bees: Vec<Bee>,
    out: Vec<u32>,
    rng: Rng,
    /// the line's points this step, kept to save allocating them each step
    pts: Vec<(f32, f32)>,
}

impl Swarm {
    pub fn new(seed: u32) -> Swarm {
        Swarm { bees: Vec::new(), out: Vec::new(), rng: Rng::new(0xBEE + seed * 131), pts: Vec::new() }
    }

    /// One step at `clock` ms into the swarm: new bees, then every bee
    /// flies. True when one reached the dog.
    pub fn step(&mut self, lv: &Level, mut bar: Option<&mut Bar>, clock: f32) -> bool {
        self.out.resize(lv.hives.len(), 0);
        for h in 0..lv.hives.len() {
            let due = ((clock - WAKE_MS) / SPAWN_MS).floor() as i32 + 1;
            if (self.out[h] as i32) < due.min(lv.bees as i32) {
                self.out[h] += 1;
                let rng = &mut self.rng;
                let (x, y) = lv.door(h);
                let v = ((rng.float() - 0.5) * 80.0, -60.0 - rng.float() * 50.0);
                self.bees.push(Bee { p: (x, y), v, age: 0.0, speed: 92.0 + rng.float() * 30.0, wander: rng.float() * 6.3 });
            }
        }
        let (dx, dy, dr) = lv.dog_circle();
        // the line where it is this step, and the box around it a bee must be in to touch it
        let reach = BEE_R + LINE_R;
        self.pts.clear();
        if let Some(b) = bar.as_deref() {
            self.pts.extend(b.points());
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for &(x, y) in &self.pts {
            (x0, y0, x1, y1) = (x0.min(x - reach), y0.min(y - reach), x1.max(x + reach), y1.max(y + reach));
        }
        for b in self.bees.iter_mut() {
            b.age += STEP_MS;
            b.wander += (self.rng.float() - 0.5) * 0.5;
            let (tx, ty) = (dx - b.p.0, dy - b.p.1);
            let d = (tx * tx + ty * ty).sqrt().max(1.0);
            let acc = (tx / d * 300.0 + b.wander.cos() * 230.0, ty / d * 300.0 + b.wander.sin() * 230.0);
            b.v.0 += acc.0 * DT;
            b.v.1 += acc.1 * DT;
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
            let near = b.p.0 > x0 && b.p.0 < x1 && b.p.1 > y0 && b.p.1 < y1;
            if let Some(bar) = bar.as_deref_mut().filter(|_| near) {
                if let Some((q, n, depth)) = Bar::touch(&self.pts, b.p, reach) {
                    let vn = dot(b.v, n);
                    bounce(b, n, depth);
                    // a bee knocks the line a little
                    if vn < 0.0 {
                        bar.push(q, (n.0 * vn * 0.04, n.1 * vn * 0.04));
                    }
                }
            }
            // inside the sky above the ground
            if b.p.0 < -8.0 || b.p.0 > 308.0 {
                b.v.0 = -b.v.0;
                b.p.0 = b.p.0.clamp(-8.0, 308.0);
            }
            if b.p.1 < 40.0 {
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
