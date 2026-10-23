//! The rules: a puff that bounces off every ledge it lands on, steered left
//! and right, climbing as high as it can before it falls off the bottom of
//! the screen. A save is a byte copy of `State`. Heights ("alt") count up
//! from the ground in plan units (10 to the metre); time is milliseconds.

use wg::Rng;

pub const PW: f32 = 360.0;
pub const PH: f32 = 560.0;
/// the puff's radius
pub const R: f32 = 12.0;
pub const LEDGE_W: f32 = 62.0;
pub const LEDGE_H: f32 = 10.0;

pub const GRAVITY: f32 = 1150.0;
pub const JUMP: f32 = 640.0;
pub const SPRING: f32 = 1100.0;
/// how high a plain bounce goes
pub const BOUNCE: f32 = JUMP * JUMP / (2.0 * GRAVITY);
const GUST: f32 = 860.0;
pub const GUST_MS: i32 = 1600;
pub const MAX_VX: f32 = 300.0;
/// the puff keeps this far below the top of the screen on the way up
const CAM_LEAD: f32 = PH * 0.42;

pub const VERSION: u32 = 1;

/// options, in catalog order: pace, theme
pub const O_PACE: usize = 0;
pub const O_THEME: usize = 1;
pub const NOPT: usize = 2;
pub const N_THEMES: i32 = 4;
/// The longest step the world moves in at once (ms).
const STEP_MS: i32 = 20;
pub const N_PACES: i32 = 3;
pub const CHOICES: [i32; NOPT] = [N_PACES, N_THEMES];

// ledges
pub const GROUND: i32 = 0;
pub const PLAIN: i32 = 1;
pub const SLIDING: i32 = 2;
/// takes one bounce, then crumbles
pub const CRUMBLE: i32 = 3;

pub const N_LEDGES: usize = 64;
pub const N_GEMS: usize = 16;
pub const N_BIRDS: usize = 4;
pub const N_GUSTS: usize = 2;

pub const PLAY: i32 = 0;
/// knocked by a bird: falls, no more bounces
pub const TUMBLE: i32 = 1;
pub const OVER: i32 = 2;

pub const GEM_POINTS: i32 = 25;
pub const BIRD_POINTS: i32 = 50;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Ledge {
    pub x: f32,
    pub alt: f32,
    pub w: f32,
    pub vx: f32,
    pub kind: i32,
    /// 0 none, else ms since it began to crumble
    pub crumble_ms: i32,
    /// a spring on it, this far from its left end (negative for none)
    pub spring: f32,
    /// ms since its spring was last sprung
    pub sprung_ms: i32,
}

/// a time long past, for clocks that only count up to show something
const LONG_AGO: i32 = 10_000;

/// the most a gap between ledges grows to on each pace, inside a bounce
const GAP_CAP: [f32; N_PACES as usize] = [118.0, 142.0, 156.0];
const _: () = assert!(GAP_CAP[2] < BOUNCE - 20.0);
/// the gaps between ledges at the ground on each pace (shortest, longest),
/// growing to (FAR, GAP_CAP) at the pace's hardest
const GAP_START: [(f32, f32); N_PACES as usize] = [(28.0, 72.0), (70.0, 110.0), (105.0, 140.0)];
const FAR: [f32; N_PACES as usize] = [78.0, 120.0, 140.0];
/// how often a spare crumbling ledge sits between two, at the ground and at the hardest
const SPARE: [(f32, f32); N_PACES as usize] = [(0.12, 0.42), (0.06, 0.15), (0.0, 0.0)];
/// the height at which each pace is at its hardest
const HARDEST: [f32; N_PACES as usize] = [20_000.0, 16_000.0, 9_000.0];
/// how fast the screen rises on its own (plan units a second) on each pace,
/// at first and at its hardest, and from what height
const RISE: [(f32, f32, f32); N_PACES as usize] = [(0.0, 0.0, 0.0), (40.0, 100.0, 2_000.0), (70.0, 150.0, 300.0)];
/// birds: from what height, and how many more than on Normal
const BIRDS: [(f32, f32); N_PACES as usize] = [(4_000.0, 0.7), (1_500.0, 1.2), (500.0, 1.8)];
/// how much more often ledges slide and how much faster than on Normal,
/// and how often a ledge on the way up crumbles at the hardest
const SLIDES: [(f32, f32); N_PACES as usize] = [(0.6, 1.0), (1.1, 1.0), (1.6, 1.3)];
const CRUMBLES: [f32; N_PACES as usize] = [0.0, 0.12, 0.3];
/// how much the ledges narrow by at the hardest
const NARROW: [f32; N_PACES as usize] = [0.0, 0.1, 0.25];

impl Ledge {
    pub fn new(x: f32, alt: f32, w: f32, vx: f32, kind: i32, spring: f32) -> Ledge {
        Ledge { x, alt, w, vx, kind, crumble_ms: 0, spring, sprung_ms: LONG_AGO }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Thing {
    pub x: f32,
    pub alt: f32,
    pub vx: f32,
    /// 1 there, 0 gone; birds: 2 knocked and falling
    pub alive: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct State {
    pub version: u32,
    pub opt: [i32; NOPT],
    pub rng: Rng,
    pub phase: i32,
    pub x: f32,
    pub alt: f32,
    pub vx: f32,
    pub vy: f32,
    /// the bottom of the screen, as a height
    pub cam: f32,
    pub best_alt: f32,
    /// the highest ledge laid so far, on the path up
    pub top: f32,
    pub gust_ms: i32,
    pub ledges: [Ledge; N_LEDGES],
    pub gems: [Thing; N_GEMS],
    pub birds: [Thing; N_BIRDS],
    pub gusts: [Thing; N_GUSTS],
    pub gem_count: i32,
    pub springs: i32,
    pub stomps: i32,
    pub time_ms: i32,
    /// ms since the last bounce, for the squash
    pub bounce_ms: i32,
}
unsafe impl wg::Plain for State {}

/// What the rules tell the picture about.
pub trait Fx {
    fn gem(&mut self, _x: f32, _alt: f32) {}
    fn crumble(&mut self, _l: &Ledge) {}
    fn stomp(&mut self, _x: f32, _alt: f32) {}
    fn hit(&mut self, _x: f32, _alt: f32) {}
}
pub struct NoFx;
impl Fx for NoFx {}

/// The shortest way from a to b round the wrapped screen.
pub fn wrap_dx(a: f32, b: f32) -> f32 {
    (b - a + PW * 1.5).rem_euclid(PW) - PW * 0.5
}

impl State {
    pub fn zero() -> State {
        State {
            version: VERSION,
            opt: [1, 0],
            rng: Rng::new(1),
            phase: PLAY,
            x: PW / 2.0,
            alt: R,
            vx: 0.0,
            vy: 0.0,
            cam: 0.0,
            best_alt: 0.0,
            top: 0.0,
            gust_ms: 0,
            ledges: [Ledge::default(); N_LEDGES],
            gems: [Thing::default(); N_GEMS],
            birds: [Thing::default(); N_BIRDS],
            gusts: [Thing::default(); N_GUSTS],
            gem_count: 0,
            springs: 0,
            stomps: 0,
            time_ms: 0,
            bounce_ms: LONG_AGO,
        }
    }

    pub fn start(&mut self, seed: u32) {
        let opt = self.opt;
        *self = State::zero();
        self.opt = opt;
        self.rng = Rng::new(seed);
        self.ledges[0] = Ledge::new(0.0, 0.0, PW, 0.0, GROUND, -1.0);
        self.vy = JUMP;
        self.lay();
    }

    pub fn over(&self) -> bool {
        self.phase == OVER
    }

    pub fn score(&self) -> i32 {
        self.height() + self.gem_count * GEM_POINTS + self.stomps * BIRD_POINTS
    }

    /// metres climbed
    pub fn height(&self) -> i32 {
        (self.best_alt / 10.0) as i32
    }

    /// the pace chosen, 0 Gentle to 2 Wild
    fn pace(&self) -> usize {
        self.opt[O_PACE].clamp(0, N_PACES - 1) as usize
    }

    /// 0 at the ground, 1 at the pace's hardest
    fn progress(&self) -> f32 {
        (self.top / HARDEST[self.pace()]).min(1.0)
    }

    fn free(items: &mut [Thing]) -> Option<&mut Thing> {
        items.iter_mut().find(|t| t.alive == 0)
    }

    /// Lays ledges, gems, birds and gusts up to a screen above the top.
    fn lay(&mut self) {
        let pace = self.pace();
        while self.top < self.cam + PH * 1.6 {
            let p = self.progress();
            // the gap to the next ledge grows with height
            let (lo0, hi0) = GAP_START[pace];
            let lo = lo0 + (FAR[pace] - lo0) * p;
            let hi = hi0 + (GAP_CAP[pace] - hi0) * p;
            let gap = lo + (hi - lo) * self.rng.float();
            // after a crumbling ledge on the way up, a still one
            let after_crumble = self.ledges.iter().any(|l| l.w > 0.0 && l.alt == self.top && l.kind == CRUMBLE);
            self.top += gap;
            let w = LEDGE_W * (1.0 - NARROW[pace] * p);
            let x = self.rng.float() * (PW - w);
            let (more, faster) = SLIDES[pace];
            let sliding = !after_crumble && self.rng.float() < (0.04 + 0.4 * p) * more;
            // one bounce on it, then on up: crumbling ledges on the way
            let crumbles = !sliding && !after_crumble && self.rng.float() < CRUMBLES[pace] * p;
            let spring = if !crumbles && self.rng.float() < 0.06 { 8.0 + self.rng.float() * (w - 16.0) } else { -1.0 };
            let vx = if sliding { (40.0 + 90.0 * p) * faster * if self.rng.below(2) == 0 { 1.0 } else { -1.0 } } else { 0.0 };
            let alt = self.top;
            let kind = if sliding { SLIDING } else if crumbles { CRUMBLE } else { PLAIN };
            self.put_ledge(Ledge::new(x, alt, w, vx, kind, spring));
            if self.rng.float() < 0.28 {
                let gx = x + w / 2.0;
                let galt = alt + 40.0 + self.rng.float() * 50.0;
                if let Some(g) = Self::free(&mut self.gems) {
                    *g = Thing { x: gx, alt: galt, vx: 0.0, alive: 1 };
                }
            }
            // a crumbling ledge between, never the only way up
            let (spare0, spare1) = SPARE[pace];
            if self.rng.float() < spare0 + (spare1 - spare0) * p {
                let cx = self.rng.float() * (PW - LEDGE_W);
                let calt = alt - gap * 0.5;
                self.put_ledge(Ledge::new(cx, calt, LEDGE_W, 0.0, CRUMBLE, -1.0));
            }
            let (birds_from, more) = BIRDS[pace];
            if alt > birds_from && self.rng.float() < (0.03 + 0.07 * p) * more {
                let left = self.rng.below(2) == 0;
                let v = (60.0 + 80.0 * p + 30.0 * self.rng.float()) * if left { 1.0 } else { -1.0 };
                let balt = alt + gap * 0.5;
                if let Some(b) = Self::free(&mut self.birds) {
                    *b = Thing { x: self.rng.float() * PW, alt: balt, vx: v, alive: 1 };
                }
            }
            if alt > 1500.0 && self.rng.float() < 0.012 {
                if let Some(g) = Self::free(&mut self.gusts) {
                    *g = Thing { x: x + w / 2.0, alt: alt + 45.0, vx: 0.0, alive: 1 };
                }
            }
        }
    }

    fn put_ledge(&mut self, l: Ledge) {
        // a slot below the screen, or the lowest of all
        let cam = self.cam;
        let i = (0..N_LEDGES)
            .find(|&i| self.ledges[i].w <= 0.0 || self.ledges[i].alt < cam - 60.0)
            .unwrap_or_else(|| (0..N_LEDGES).min_by(|&a, &b| self.ledges[a].alt.total_cmp(&self.ledges[b].alt)).unwrap_or(0));
        self.ledges[i] = l;
    }

    /// The arrows steer, or a finger or mouse steers to `steer`, the x it is at.
    pub fn frame(&mut self, held: u32, steer: Option<f32>, dt: i32, fx: &mut dyn Fx) -> u32 {
        if self.phase == OVER || dt <= 0 {
            return 0;
        }
        // slow frames go in short steps, so a fast puff can't pass through things
        if dt > STEP_MS {
            let (mut ev, mut left) = (0, dt);
            while left > 0 {
                ev |= self.frame(held, steer, left.min(STEP_MS), fx);
                left -= STEP_MS;
            }
            return ev;
        }
        let t = dt as f32 / 1000.0;
        self.time_ms += dt;
        self.bounce_ms = (self.bounce_ms + dt).min(LONG_AGO);
        let mut ev = 0;

        // steering
        let keys = held & (wg::LEFT | wg::RIGHT);
        let want = if self.phase == TUMBLE {
            self.vx * 0.5
        } else if let Some(to) = steer {
            (wrap_dx(self.x, to) * 9.0).clamp(-MAX_VX, MAX_VX)
        } else if keys == wg::LEFT {
            -MAX_VX
        } else if keys == wg::RIGHT {
            MAX_VX
        } else {
            0.0
        };
        let acc = 2400.0 * t;
        self.vx += (want - self.vx).clamp(-acc, acc);
        self.x = (self.x + self.vx * t).rem_euclid(PW);

        // up and down
        let was = self.alt;
        if self.gust_ms > 0 {
            self.gust_ms -= dt;
            self.vy = GUST;
        } else {
            self.vy -= GRAVITY * t;
        }
        self.alt += self.vy * t;
        self.best_alt = self.best_alt.max(self.alt);

        // the world moves
        for l in self.ledges.iter_mut().filter(|l| l.w > 0.0) {
            l.sprung_ms = (l.sprung_ms + dt).min(LONG_AGO);
            if l.vx != 0.0 {
                l.x += l.vx * t;
                if l.x < 0.0 || l.x + l.w > PW {
                    l.vx = -l.vx;
                    l.x = l.x.clamp(0.0, PW - l.w);
                }
            }
            if l.crumble_ms > 0 {
                l.crumble_ms += dt;
            }
        }
        for b in self.birds.iter_mut().filter(|b| b.alive != 0) {
            if b.alive == 2 {
                b.alt -= 300.0 * t;
            } else {
                b.x = (b.x + b.vx * t).rem_euclid(PW);
            }
        }

        // landing on a ledge: only falling, and only crossing its top
        if self.phase == PLAY && self.vy < 0.0 {
            let (foot0, foot1) = (was - R, self.alt - R);
            for l in self.ledges.iter_mut() {
                if l.w <= 0.0 || l.crumble_ms > 300 || foot0 < l.alt - 2.0 || foot1 > l.alt {
                    continue;
                }
                // round the wrapped edge too, as the puff is drawn
                let on = l.kind == GROUND || wrap_dx(l.x + l.w / 2.0, self.x).abs() < l.w / 2.0 + 6.0;
                if !on {
                    continue;
                }
                self.alt = l.alt + R;
                self.bounce_ms = 0;
                if l.spring >= 0.0 && (self.x - (l.x + l.spring)).abs() < 13.0 {
                    self.vy = SPRING;
                    l.sprung_ms = 0;
                    self.springs += 1;
                    ev |= wg::EV_HIT;
                } else {
                    self.vy = JUMP;
                    ev |= wg::EV_BLIP;
                }
                if l.kind == CRUMBLE && l.crumble_ms == 0 {
                    l.crumble_ms = 1;
                    fx.crumble(l);
                }
                break;
            }
        }

        // gems, gusts and birds (none once knocked out of the sky)
        if self.phase == PLAY {
            for g in self.gems.iter_mut().filter(|g| g.alive == 1) {
                if wrap_dx(self.x, g.x).abs() < R + 8.0 && (self.alt - g.alt).abs() < R + 8.0 {
                    g.alive = 0;
                    self.gem_count += 1;
                    fx.gem(g.x, g.alt);
                    ev |= wg::EV_BLIP;
                }
            }
            for g in self.gusts.iter_mut().filter(|g| g.alive == 1) {
                if wrap_dx(self.x, g.x).abs() < R + 12.0 && (self.alt - g.alt).abs() < R + 16.0 {
                    g.alive = 0;
                    self.gust_ms = GUST_MS;
                    ev |= wg::EV_HIT;
                }
            }
            for b in self.birds.iter_mut().filter(|b| b.alive == 1) {
                let (dx, dy) = (wrap_dx(self.x, b.x), self.alt - b.alt);
                if dx * dx + dy * dy > (R + 11.0) * (R + 11.0) {
                    continue;
                }
                if self.gust_ms > 0 || (self.vy < 0.0 && dy > 4.0) {
                    // from above, or carried by a gust: the bird is knocked away
                    b.alive = 2;
                    self.stomps += 1;
                    if self.gust_ms <= 0 {
                        self.vy = JUMP * 1.05;
                        self.bounce_ms = 0;
                    }
                    fx.stomp(b.x, b.alt);
                    ev |= wg::EV_HIT;
                } else {
                    self.phase = TUMBLE;
                    self.vy = self.vy.min(120.0);
                    fx.hit(self.x, self.alt);
                    ev |= wg::EV_LOSE;
                }
            }
        }

        // the screen follows on the way up, never down, and on the faster
        // paces it rises on its own once the climb is under way
        self.cam = self.cam.max(self.alt - (PH - CAM_LEAD));
        let (slow, fast, from) = RISE[self.pace()];
        if fast > 0.0 && self.cam > from && self.phase == PLAY {
            self.cam += (slow + (fast - slow) * self.progress()) * t;
        }
        for b in self.birds.iter_mut() {
            if b.alive != 0 && b.alt < self.cam - 60.0 {
                b.alive = 0;
            }
        }
        for g in self.gems.iter_mut().chain(self.gusts.iter_mut()) {
            if g.alive != 0 && g.alt < self.cam - 40.0 {
                g.alive = 0;
            }
        }
        self.lay();

        if self.alt < self.cam - R * 3.0 {
            self.phase = OVER;
            ev |= wg::EV_LOSE | wg::EV_GAMEOVER;
        }
        ev
    }

    pub fn valid(&self) -> bool {
        let f = |v: f32| v.is_finite();
        self.version == VERSION
            && (PLAY..=OVER).contains(&self.phase)
            && [self.x, self.alt, self.vx, self.vy, self.cam, self.best_alt, self.top].into_iter().all(f)
            && self.ledges.iter().all(|l| f(l.x) && f(l.alt) && f(l.w) && f(l.vx) && (GROUND..=CRUMBLE).contains(&l.kind))
            && self.gems.iter().chain(self.birds.iter()).chain(self.gusts.iter()).all(|g| f(g.x) && f(g.alt) && f(g.vx))
            && self.time_ms >= 0
    }
}
