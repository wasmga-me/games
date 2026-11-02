//! The bodies and their gravity: leapfrog steps of a fixed size, softened
//! pull, and bodies that touch merge with their momentum kept.

use wg::Rng;

pub const MAX_BODIES: usize = 48;
/// Gravity times a sun's mass: a body 120 units out goes round in about 10 s.
pub const SUN_GM: f32 = 675_000.0;
/// Seconds per physics step.
pub const STEP: f32 = 1.0 / 240.0;
/// Softening, so close passes don't fling bodies off at absurd speeds.
const SOFT2: f32 = 9.0;
pub const SUN_R: f32 = 16.0;

pub const KIND_STAR: i32 = 1;
pub const KIND_PLANET: i32 = 2;

/// One body; kind 0 is an empty slot.
#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub struct Body {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// gravity times mass
    pub gm: f32,
    pub r: f32,
    pub kind: i32,
    pub colour: i32,
    /// the order bodies came in (set by `add`), for taking out the newest
    pub born: u32,
}

/// Everything a save keeps: options, the bodies and counters.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct State {
    pub version: u32,
    /// options, in the catalog `setup` order: start, trails
    pub opt: [i32; 2],
    pub rng: Rng,
    pub bodies: [Body; MAX_BODIES],
    /// time not yet stepped, in seconds
    pub carry: f32,
    /// the colour the next planet gets
    pub next_colour: i32,
    /// what `born` the next body gets
    pub next_born: u32,
}
unsafe impl wg::Plain for Body {}
unsafe impl wg::Plain for State {}

pub const VERSION: u32 = 1;
pub const O_START: usize = 0;
pub const O_TRAILS: usize = 1;
pub const N_COLOURS: i32 = 7;

/// Merges in a step, for the drawing: where the smaller body was, and its colour.
pub type Merges = Vec<(f32, f32, i32)>;

impl State {
    pub fn zero() -> State {
        State { version: VERSION, opt: [0, 0], rng: Rng::new(1), bodies: [Body::default(); MAX_BODIES], carry: 0.0, next_colour: 0, next_born: 0 }
    }

    /// A new system for the start option, around 0, 0.
    pub fn start(&mut self, seed: u32) {
        let opt = self.opt;
        *self = State::zero();
        self.opt = opt;
        self.rng = Rng::new(seed);
        match self.opt[O_START] {
            // two suns going round each other
            1 => {
                let gm = SUN_GM * 0.5;
                let d = 70.0;
                let v = (gm / (2.0 * d)).sqrt();
                self.add(Body { x: -d / 2.0, y: 0.0, vx: 0.0, vy: -v, gm, r: SUN_R * 0.8, kind: KIND_STAR, colour: 0, born: 0 });
                self.add(Body { x: d / 2.0, y: 0.0, vx: 0.0, vy: v, gm, r: SUN_R * 0.8, kind: KIND_STAR, colour: 1, born: 0 });
            }
            // a sun with planets, one with a moon
            2 => {
                self.add(Body { gm: SUN_GM, r: SUN_R, kind: KIND_STAR, ..Body::default() });
                for (i, d) in [50.0f32, 82.0, 120.0, 220.0].iter().enumerate() {
                    let a = self.rng.float() * core::f32::consts::TAU;
                    let gm = if i == 3 { 5000.0 } else { 600.0 + 900.0 * self.rng.float() };
                    let slot = self.planet_at(d * a.cos(), d * a.sin(), gm);
                    if let (3, Some(slot)) = (i, slot) {
                        // a moon, well inside the planet's reach
                        let p = self.bodies[slot];
                        let md = 11.0;
                        let (mx, my) = (p.x + md * a.cos(), p.y + md * a.sin());
                        let mv = (p.gm * md * md / (md * md + SOFT2).powf(1.5)).sqrt();
                        let moon = Body { x: mx, y: my, vx: p.vx - mv * a.sin(), vy: p.vy + mv * a.cos(), gm: 30.0, r: 2.4, kind: KIND_PLANET, colour: self.take_colour(), born: 0 };
                        self.add(moon);
                    }
                }
            }
            _ => {
                self.add(Body { gm: SUN_GM, r: SUN_R, kind: KIND_STAR, ..Body::default() });
            }
        }
        // no drift: the whole system's momentum is zero
        let (vx, vy) = self.mean(|b| b.kind != 0, |b| (b.vx, b.vy));
        for b in self.bodies.iter_mut().filter(|b| b.kind != 0) {
            b.vx -= vx;
            b.vy -= vy;
        }
    }

    fn take_colour(&mut self) -> i32 {
        let c = self.next_colour;
        self.next_colour = (c + 1) % N_COLOURS;
        c
    }

    /// Puts a body in the first empty slot and says which; none when full.
    pub fn add(&mut self, b: Body) -> Option<usize> {
        let i = self.bodies.iter().position(|s| s.kind == 0)?;
        self.bodies[i] = Body { born: self.next_born, ..b };
        self.next_born = self.next_born.wrapping_add(1);
        Some(i)
    }

    pub fn count(&self, kind: i32) -> usize {
        self.bodies.iter().filter(|b| b.kind == kind).count()
    }

    /// The suns' centre of mass, where the view looks.
    pub fn centre(&self) -> (f32, f32) {
        self.mean(|b| b.kind == KIND_STAR, |b| (b.x, b.y))
    }

    /// The mass-weighted mean of `f` over the bodies `pick` takes.
    fn mean(&self, pick: impl Fn(&Body) -> bool, f: impl Fn(&Body) -> (f32, f32)) -> (f32, f32) {
        let (mut x, mut y, mut m) = (0.0, 0.0, 0.0);
        for b in self.bodies.iter().filter(|b| pick(b)) {
            let (fx, fy) = f(b);
            x += fx * b.gm;
            y += fy * b.gm;
            m += b.gm;
        }
        if m > 0.0 {
            (x / m, y / m)
        } else {
            (0.0, 0.0)
        }
    }

    /// The radius a planet of this gravity gets.
    pub fn radius(gm: f32) -> f32 {
        (2.2 + gm.cbrt() * 0.32).min(SUN_R * 0.75)
    }

    /// A planet at x, y going round the system's suns on a circle.
    pub fn planet_at(&mut self, x: f32, y: f32, gm: f32) -> Option<usize> {
        let (cx, cy) = self.centre();
        let suns: f32 = self.bodies.iter().filter(|b| b.kind == KIND_STAR).map(|b| b.gm).sum();
        let (dx, dy) = (x - cx, y - cy);
        let d = (dx * dx + dy * dy).sqrt().max(1.0);
        let v = (suns / d).sqrt();
        // the suns' own drift, so the orbit is round in their frame
        let (svx, svy) = self.mean(|b| b.kind == KIND_STAR, |b| (b.vx, b.vy));
        let colour = self.take_colour();
        self.add(Body { x, y, vx: svx - v * dy / d, vy: svy + v * dx / d, gm, r: State::radius(gm), kind: KIND_PLANET, colour, born: 0 })
    }

    /// A planet flung from x, y at vx, vy.
    pub fn fling(&mut self, x: f32, y: f32, vx: f32, vy: f32, gm: f32) -> Option<usize> {
        let colour = self.take_colour();
        self.add(Body { x, y, vx, vy, gm, r: State::radius(gm), kind: KIND_PLANET, colour, born: 0 })
    }

    /// Takes out the newest planet.
    pub fn remove_last(&mut self) {
        if let Some(b) = self.bodies.iter_mut().filter(|b| b.kind == KIND_PLANET).max_by_key(|b| b.born) {
            *b = Body::default();
        }
    }

    /// Moves time on by `secs`, in whole steps.
    pub fn advance(&mut self, secs: f32, far: f32, merges: &mut Merges) {
        self.carry = (self.carry + secs).min(0.25);
        while self.carry >= STEP {
            self.carry -= STEP;
            step(&mut self.bodies, STEP);
            self.collide(merges);
        }
        // bodies that left for good
        let (cx, cy) = self.centre();
        for b in self.bodies.iter_mut().filter(|b| b.kind == KIND_PLANET) {
            if (b.x - cx).abs() > far || (b.y - cy).abs() > far {
                *b = Body::default();
            }
        }
    }

    /// Bodies that touch become one, keeping mass and momentum.
    fn collide(&mut self, merges: &mut Merges) {
        for i in 0..MAX_BODIES {
            if self.bodies[i].kind == 0 {
                continue;
            }
            for j in i + 1..MAX_BODIES {
                let (a, b) = (self.bodies[i], self.bodies[j]);
                if b.kind == 0 {
                    continue;
                }
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let rr = (a.r + b.r) * 0.85;
                if dx * dx + dy * dy > rr * rr {
                    continue;
                }
                let m = a.gm + b.gm;
                // the heavier one (a sun over a planet) keeps its kind and colour
                let (big, small) = if a.kind == KIND_STAR && b.kind != KIND_STAR || (a.kind == b.kind && a.gm >= b.gm) { (a, b) } else { (b, a) };
                let r = if big.kind == KIND_STAR { big.r } else { (big.r.powi(3) + small.r.powi(3)).cbrt().min(SUN_R * 0.75) };
                let merged = Body {
                    x: (a.x * a.gm + b.x * b.gm) / m,
                    y: (a.y * a.gm + b.y * b.gm) / m,
                    vx: (a.vx * a.gm + b.vx * b.gm) / m,
                    vy: (a.vy * a.gm + b.vy * b.gm) / m,
                    gm: m,
                    r,
                    kind: big.kind,
                    colour: big.colour,
                    born: big.born,
                };
                merges.push((small.x, small.y, small.colour));
                self.bodies[i] = merged;
                self.bodies[j] = Body::default();
            }
        }
    }

    pub fn valid(&self) -> bool {
        self.version == VERSION
            && self.carry.is_finite()
            && (0..N_COLOURS).contains(&self.next_colour)
            && self.bodies.iter().all(|b| {
                (0..=KIND_PLANET).contains(&b.kind)
                    && [b.x, b.y, b.vx, b.vy, b.gm, b.r].iter().all(|v| v.is_finite())
                    && (b.kind == 0 || (b.gm > 0.0 && b.r > 0.0 && (0..N_COLOURS).contains(&b.colour)))
            })
            && self.count(KIND_STAR) > 0
    }
}

/// One leapfrog (kick, drift, kick) step of every body.
pub fn step(bodies: &mut [Body], dt: f32) {
    let mut ax = [0.0f32; MAX_BODIES];
    let mut ay = [0.0f32; MAX_BODIES];
    accel(bodies, &mut ax, &mut ay);
    for (i, b) in bodies.iter_mut().enumerate().filter(|(_, b)| b.kind != 0) {
        b.vx += ax[i] * dt * 0.5;
        b.vy += ay[i] * dt * 0.5;
        b.x += b.vx * dt;
        b.y += b.vy * dt;
    }
    accel(bodies, &mut ax, &mut ay);
    for (i, b) in bodies.iter_mut().enumerate().filter(|(_, b)| b.kind != 0) {
        b.vx += ax[i] * dt * 0.5;
        b.vy += ay[i] * dt * 0.5;
    }
}

fn accel(bodies: &[Body], ax: &mut [f32; MAX_BODIES], ay: &mut [f32; MAX_BODIES]) {
    ax.fill(0.0);
    ay.fill(0.0);
    for i in 0..bodies.len() {
        let a = bodies[i];
        if a.kind == 0 {
            continue;
        }
        for j in i + 1..bodies.len() {
            let b = bodies[j];
            if b.kind == 0 {
                continue;
            }
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let d2 = dx * dx + dy * dy + SOFT2;
            let inv = 1.0 / (d2 * d2.sqrt());
            ax[i] += dx * inv * b.gm;
            ay[i] += dy * inv * b.gm;
            ax[j] -= dx * inv * a.gm;
            ay[j] -= dy * inv * a.gm;
        }
    }
}

/// Where a body flung from x, y at vx, vy goes over the next `secs`, among
/// the bodies as they move: points every `every` seconds, until it hits one.
/// Runs every frame of a drag, so it steps only the bodies there are, at
/// half the rate of the real thing (close enough for a dotted line).
pub fn path(bodies: &[Body; MAX_BODIES], x: f32, y: f32, vx: f32, vy: f32, secs: f32, every: f32, out: &mut Vec<(f32, f32)>) {
    out.clear();
    // the test body first; it has no pull of its own
    let mut bs = [Body::default(); MAX_BODIES];
    bs[0] = Body { x, y, vx, vy, gm: 0.0, r: 1.0, kind: KIND_PLANET, colour: 0, born: 0 };
    let mut n = 1;
    for b in bodies.iter().filter(|b| b.kind != 0) {
        if n == MAX_BODIES {
            return;
        }
        bs[n] = *b;
        n += 1;
    }
    let bs = &mut bs[..n];
    let dt = STEP * 2.0;
    let steps = (secs / dt) as i32;
    let per = ((every / dt) as i32).max(1);
    out.push((x, y));
    for k in 1..=steps {
        step(bs, dt);
        let p = bs[0];
        if k % per == 0 {
            out.push((p.x, p.y));
        }
        if bs[1..].iter().any(|b| (b.x - p.x).powi(2) + (b.y - p.y).powi(2) < (b.r * 0.9).powi(2)) {
            out.push((p.x, p.y));
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The planets start keeps all five bodies for ten minutes, and nothing drifts.
    #[test]
    fn planets_start_is_stable() {
        for seed in [1, 7, 42, 99, 1234] {
            let mut s = State::zero();
            s.opt[O_START] = 2;
            s.start(seed);
            let mut ev = Merges::new();
            for _ in 0..600 * 60 {
                s.advance(1.0 / 60.0, 1600.0, &mut ev);
            }
            assert_eq!(s.count(KIND_PLANET), 5, "seed {seed}");
            assert!(ev.is_empty(), "seed {seed}");
            let (cx, cy) = s.centre();
            assert!(cx.abs() < 10.0 && cy.abs() < 10.0, "seed {seed}: {cx}, {cy}");
            assert!(s.valid());
        }
    }

    #[test]
    fn removes_the_newest_planet() {
        let mut s = State::zero();
        s.start(1);
        let a = s.fling(100.0, 0.0, 0.0, 0.0, 500.0).unwrap();
        let b = s.fling(200.0, 0.0, 0.0, 0.0, 500.0).unwrap();
        s.bodies[a] = Body::default();
        // the newest takes the free slot below the older one
        let c = s.fling(300.0, 0.0, 0.0, 0.0, 500.0).unwrap();
        assert!(c < b);
        s.remove_last();
        assert_eq!(s.bodies[c].kind, 0);
        assert_eq!(s.bodies[b].kind, KIND_PLANET);
    }

    #[test]
    fn merging_keeps_momentum() {
        let mut s = State::zero();
        s.add(Body { x: 0.0, y: 0.0, vx: 1.0, vy: 0.0, gm: 100.0, r: 5.0, kind: KIND_STAR, colour: 0, born: 0 });
        s.add(Body { x: 4.0, y: 0.0, vx: -3.0, vy: 2.0, gm: 50.0, r: 3.0, kind: KIND_PLANET, colour: 3, born: 0 });
        let mut ev = Merges::new();
        s.collide(&mut ev);
        assert_eq!(s.count(KIND_STAR), 1);
        assert_eq!(s.count(KIND_PLANET), 0);
        let b = s.bodies[0];
        assert!((b.vx - (100.0 - 150.0) / 150.0).abs() < 1e-5 && (b.vy - 100.0 / 150.0).abs() < 1e-5);
        assert_eq!(ev.len(), 1);
    }
}
