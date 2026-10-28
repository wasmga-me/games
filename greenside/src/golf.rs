//! The rules: aiming, the ball rolling, rails, bumpers, sand, water, slopes,
//! windmills, gates and the cup. A save is a byte copy of `State`. Units are
//! the plan's (480 x 360), time is milliseconds.

use crate::holes::{Area, Hole, COUNT, HOLES};
use wg::{EV_BLIP, EV_GAMEOVER, EV_HIT, EV_LEVEL, EV_LOSE, EV_WIN};

pub const PW: f32 = 480.0;
pub const PH: f32 = 360.0;
pub const BALL_R: f32 = 5.0;
pub const CUP_R: f32 = 7.5;
/// half the thickness of a rail
pub const RAIL: f32 = 4.0;
pub const HUB_R: f32 = 6.0;

pub const VERSION: u32 = 1;

/// options, in catalog order: the hole to start at, the theme
pub const O_HOLE: usize = 0;
pub const O_THEME: usize = 1;
pub const NOPT: usize = 2;
pub const N_THEMES: i32 = 4;
pub const CHOICES: [i32; NOPT] = [COUNT as i32, N_THEMES];

// phases
pub const AIM: i32 = 0;
pub const ROLL: i32 = 1;
/// the ball dropped; the next hole comes up
pub const SUNK: i32 = 2;
/// the ball went in the water and comes back where it was hit from
pub const SPLASH: i32 = 3;
/// out of strokes on this hole
pub const PICKED: i32 = 4;
pub const OVER: i32 = 5;

/// the banner after a hole, sunk or picked up
pub const SUNK_MS: i32 = 1500;
pub const SPLASH_MS: i32 = 900;
/// a hole ends after this many strokes, penalties included
pub const MAX_STROKES: i32 = 8;
/// the fastest a putt leaves, units/s
pub const MAX_SPEED: f32 = 720.0;
/// a drag this long (plan units) is a full-power putt
pub const DRAG_FULL: f32 = 140.0;
/// faster than this over the cup and the ball lips out
const SINK_SPEED: f32 = 230.0;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct State {
    pub version: u32,
    pub opt: [i32; NOPT],
    pub phase: i32,
    pub phase_ms: i32,
    pub hole: i32,
    /// strokes on this hole, penalties included
    pub strokes: i32,
    /// strokes and par over the holes finished
    pub total: i32,
    pub par_total: i32,
    pub score: i32,
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// where the last putt was hit from (a splash comes back here)
    pub from_x: f32,
    pub from_y: f32,
    /// the aim, radians, and the power, 0..1
    pub aim: f32,
    pub power: f32,
    /// how long the ball has been barely moving
    pub still_ms: i32,
    /// time on this hole, which turns the windmills and slides the gates
    pub hole_ms: i32,
    pub time_ms: i32,
    pub played: i32,
    pub aces: i32,
    pub splashes: i32,
    /// the hole cleared last (-1 for none) and the strokes it took
    pub cleared: i32,
    pub last_strokes: i32,
}
unsafe impl wg::Plain for State {}

/// What the rules tell the picture about (sparks, splashes, flashes).
pub trait Fx {
    fn bump(&mut self, _i: usize, _x: f32, _y: f32) {}
    fn rail(&mut self, _x: f32, _y: f32, _speed: f32) {}
    fn splash(&mut self, _x: f32, _y: f32) {}
    fn sunk(&mut self, _x: f32, _y: f32) {}
}
pub struct NoFx;
impl Fx for NoFx {}

/// A pull under way (pointer input, not saved): where it began and where it is.
#[derive(Clone, Copy, Default)]
pub struct Drag {
    pub on: bool,
    pub x0: f32,
    pub y0: f32,
    pub x: f32,
    pub y: f32,
}

impl Drag {
    /// Aim opposite the pull and power by its length; None for a pull too
    /// short to count.
    pub fn pull(&self) -> Option<(f32, f32)> {
        let (dx, dy) = (self.x0 - self.x, self.y0 - self.y);
        let l = (dx * dx + dy * dy).sqrt();
        (l >= 8.0).then(|| (dy.atan2(dx), ((l - 8.0) / (DRAG_FULL - 8.0)).clamp(0.02, 1.0)))
    }
}

pub fn hole(i: i32) -> &'static Hole {
    &HOLES[(i.max(0) as usize).min(COUNT - 1)]
}

pub fn inside(a: &Area, x: f32, y: f32) -> bool {
    let (cx, cy) = (x.clamp(a.x + a.r, a.x + a.w - a.r), y.clamp(a.y + a.r, a.y + a.h - a.r));
    let (dx, dy) = (x - cx, y - cy);
    dx * dx + dy * dy <= a.r * a.r
}

/// The closest point to (px, py) on the segment a-b.
pub fn closest(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> (f32, f32) {
    let (dx, dy) = (bx - ax, by - ay);
    let l = dx * dx + dy * dy;
    let t = if l > 0.0 { (((px - ax) * dx + (py - ay) * dy) / l).clamp(0.0, 1.0) } else { 0.0 };
    (ax + dx * t, ay + dy * t)
}

/// A windmill's bars at time t: (x0, y0, x1, y1) each.
pub fn bars(s: &crate::holes::Spinner, t: f32) -> impl Iterator<Item = (f32, f32, f32, f32)> + '_ {
    let n = s.bars.max(1);
    (0..n).map(move |k| {
        let th = s.speed * t / 1000.0 + k as f32 * core::f32::consts::PI / n as f32;
        let (c, sn) = (th.cos() * s.len, th.sin() * s.len);
        (s.x - c, s.y - sn, s.x + c, s.y + sn)
    })
}

/// A gate's rail at time t, and how fast it is moving (units/s).
pub fn gate(m: &crate::holes::Mover, t: f32) -> ((f32, f32, f32, f32), (f32, f32)) {
    let w = core::f32::consts::TAU / m.period;
    let ph = (t * w + m.phase * core::f32::consts::TAU).sin() * 0.5 + 0.5;
    let v = (t * w + m.phase * core::f32::consts::TAU).cos() * 0.5 * w * 1000.0;
    ((m.x0 + m.dx * ph, m.y0 + m.dy * ph, m.x1 + m.dx * ph, m.y1 + m.dy * ph), (m.dx * v, m.dy * v))
}

impl State {
    pub fn zero() -> State {
        State {
            version: VERSION,
            opt: [0, 0],
            phase: AIM,
            phase_ms: 0,
            hole: 0,
            strokes: 0,
            total: 0,
            par_total: 0,
            score: 0,
            x: 0.0,
            y: 0.0,
            vx: 0.0,
            vy: 0.0,
            from_x: 0.0,
            from_y: 0.0,
            aim: 0.0,
            power: 0.5,
            still_ms: 0,
            hole_ms: 0,
            time_ms: 0,
            played: 0,
            aces: 0,
            splashes: 0,
            cleared: -1,
            last_strokes: 0,
        }
    }

    pub fn start(&mut self) {
        let opt = self.opt;
        *self = State::zero();
        self.opt = opt;
        self.load(opt[O_HOLE].clamp(0, COUNT as i32 - 1));
    }

    /// Puts the ball on hole i's tee, aimed at the cup.
    pub fn load(&mut self, i: i32) {
        let h = hole(i);
        self.hole = i;
        self.strokes = 0;
        self.phase = AIM;
        self.phase_ms = 0;
        self.hole_ms = 0;
        self.still_ms = 0;
        (self.x, self.y) = h.tee;
        (self.from_x, self.from_y) = h.tee;
        (self.vx, self.vy) = (0.0, 0.0);
        self.aim = (h.cup.1 - h.tee.1).atan2(h.cup.0 - h.tee.0);
        self.power = 0.5;
    }

    pub fn over(&self) -> bool {
        self.phase == OVER
    }

    pub fn to_par(&self) -> i32 {
        self.total - self.par_total
    }

    /// Points for a hole: 100 for each stroke under par + 5, and 500 more
    /// for a hole in one.
    pub fn points(strokes: i32, par: i32) -> i32 {
        100 * (par + 5 - strokes).max(0) + if strokes == 1 { 500 } else { 0 }
    }

    /// Something on the hole moves on its own.
    pub fn busy(&self) -> bool {
        let h = hole(self.hole);
        !h.spinners.is_empty() || !h.movers.is_empty()
    }

    /// Hits the ball with the aim and power set.
    pub fn putt(&mut self) -> u32 {
        if self.phase != AIM || self.power < 0.02 {
            return 0;
        }
        let v = MAX_SPEED * self.power;
        self.vx = self.aim.cos() * v;
        self.vy = self.aim.sin() * v;
        (self.from_x, self.from_y) = (self.x, self.y);
        self.strokes += 1;
        self.phase = ROLL;
        self.phase_ms = 0;
        self.still_ms = 0;
        EV_BLIP
    }

    /// Mouse or touch: press anywhere, pull back, let go to putt. A pull
    /// goes on off the screen, and putts when let go there too; a pull
    /// the page cancels (the pointer gone) is called off.
    pub fn pointer(&mut self, d: &mut Drag, x: f32, y: f32, inside: bool, pressed: bool, released: bool) -> u32 {
        if self.phase != AIM || !inside {
            d.on = false;
            return 0;
        }
        if pressed {
            *d = Drag { on: true, x0: x, y0: y, x, y };
        }
        if !d.on {
            return 0;
        }
        (d.x, d.y) = (x, y);
        let pull = d.pull();
        if let Some((aim, power)) = pull {
            self.aim = aim;
            self.power = power;
        }
        if released {
            d.on = false;
            if pull.is_some() {
                return self.putt();
            }
        }
        0
    }

    /// One frame: arrows aim and set the power, A putts, and the ball rolls.
    /// `dragging` holds the arrows off while a pull is under way; `hold_ms`
    /// is how long an arrow has been held, for turning quicker.
    pub fn frame(&mut self, held: u32, pressed: u32, dt: i32, dragging: bool, hold_ms: &mut i32, fx: &mut dyn Fx) -> u32 {
        if self.phase == OVER || dt <= 0 {
            return 0;
        }
        let dts = dt as f32 / 1000.0;
        self.time_ms += dt;
        self.hole_ms += dt;
        self.phase_ms += dt;
        let mut ev = 0;
        match self.phase {
            AIM if self.busy() && self.struck(self.hole_ms as f32) => {
                // a windmill bar or gate comes round into the ball at rest: it rolls
                (self.from_x, self.from_y) = (self.x, self.y);
                self.phase = ROLL;
                self.phase_ms = 0;
                self.still_ms = 0;
                ev |= self.roll(dt, fx);
            }
            AIM => {
                if !dragging {
                    // fine at first, quicker the longer an arrow is held
                    let turn = if *hold_ms > 600 { 1.6 } else { 0.7 };
                    if held & wg::LEFT != 0 {
                        self.aim -= turn * dts;
                    }
                    if held & wg::RIGHT != 0 {
                        self.aim += turn * dts;
                    }
                    *hold_ms = if held & (wg::LEFT | wg::RIGHT) != 0 { *hold_ms + dt } else { 0 };
                    if held & wg::UP != 0 {
                        self.power = (self.power + 0.6 * dts).min(1.0);
                    }
                    if held & wg::DOWN != 0 {
                        self.power = (self.power - 0.6 * dts).max(0.05);
                    }
                    self.aim = self.aim.rem_euclid(core::f32::consts::TAU);
                    if pressed & wg::A != 0 {
                        ev |= self.putt();
                    }
                }
            }
            ROLL => ev |= self.roll(dt, fx),
            SPLASH if self.phase_ms >= SPLASH_MS => {
                (self.x, self.y) = (self.from_x, self.from_y);
                ev |= self.rest();
            }
            SUNK | PICKED if self.phase_ms >= SUNK_MS => {
                if self.hole + 1 >= COUNT as i32 {
                    self.phase = OVER;
                    ev |= EV_WIN | EV_GAMEOVER;
                } else {
                    self.load(self.hole + 1);
                }
            }
            _ => {}
        }
        ev
    }

    /// The hole is done in `strokes`: score it and show the banner.
    fn finish(&mut self, strokes: i32) -> u32 {
        let par = hole(self.hole).par;
        let pts = Self::points(strokes, par);
        self.total += strokes;
        self.par_total += par;
        self.score += pts;
        self.played += 1;
        if strokes == 1 {
            self.aces += 1;
        }
        self.cleared = self.hole;
        self.last_strokes = strokes;
        self.phase = if strokes > MAX_STROKES { PICKED } else { SUNK };
        self.phase_ms = 0;
        EV_LEVEL
    }

    /// The ball is at rest: aim again, unless the strokes ran out.
    fn rest(&mut self) -> u32 {
        self.phase = AIM;
        self.phase_ms = 0;
        if self.strokes >= MAX_STROKES {
            return self.finish(MAX_STROKES + 1);
        }
        0
    }

    /// The points made on the hole cleared last.
    pub fn cleared_points(&self) -> i32 {
        Self::points(self.last_strokes, hole(self.cleared).par)
    }

    /// The ball rolls for dt ms, in steps short enough not to pass through
    /// a rail.
    fn roll(&mut self, dt: i32, fx: &mut dyn Fx) -> u32 {
        let mut ev = 0;
        let mut left = dt as f32 / 1000.0;
        let t0 = (self.hole_ms - dt) as f32;
        while left > 0.0 && self.phase == ROLL {
            let speed = (self.vx * self.vx + self.vy * self.vy).sqrt();
            let step = (1.5 / speed.max(1.0)).clamp(0.0005, 0.004).min(left);
            left -= step;
            let t = t0 + (dt as f32 / 1000.0 - left) * 1000.0;
            ev |= self.step(step, t, fx);
        }
        ev
    }

    /// A windmill bar or gate touches the ball at time t.
    fn struck(&self, t: f32) -> bool {
        let h = hole(self.hole);
        let near = |(x0, y0, x1, y1): (f32, f32, f32, f32), r: f32| {
            let (px, py) = closest(self.x, self.y, x0, y0, x1, y1);
            (px - self.x).powi(2) + (py - self.y).powi(2) < (r + BALL_R).powi(2)
        };
        h.spinners.iter().any(|sp| bars(sp, t).into_iter().any(|b| near(b, 3.0)))
            || h.movers.iter().any(|m| near(gate(m, t).0, RAIL + 1.0))
    }

    fn step(&mut self, dt: f32, t: f32, fx: &mut dyn Fx) -> u32 {
        let h = hole(self.hole);
        let mut ev = 0;
        // the ground: slopes push, sand drags
        let (mut ax, mut ay) = (0.0, 0.0);
        let mut on_slope = false;
        for s in h.slopes.iter().filter(|s| inside(&s.at, self.x, self.y)) {
            ax += s.ax;
            ay += s.ay;
            on_slope = true;
        }
        let in_sand = h.sand.iter().any(|a| inside(a, self.x, self.y));
        let (drag, k) = if in_sand { (380.0, 3.2) } else { (42.0, 0.85) };
        // the cup's lip draws a slow ball in
        let (cx, cy) = (h.cup.0 - self.x, h.cup.1 - self.y);
        let cd = (cx * cx + cy * cy).sqrt();
        if cd < CUP_R + 6.0 && cd > 0.01 {
            ax += cx / cd * 160.0;
            ay += cy / cd * 160.0;
        }
        self.vx += ax * dt;
        self.vy += ay * dt;
        let sp = (self.vx * self.vx + self.vy * self.vy).sqrt();
        let slow = (drag + k * sp) * dt;
        if slow >= sp {
            self.vx = 0.0;
            self.vy = 0.0;
        } else {
            let f = (sp - slow) / sp;
            self.vx *= f;
            self.vy *= f;
        }
        self.x += self.vx * dt;
        self.y += self.vy * dt;

        // rails: the outline, the inner rails, windmill bars and gates
        for (x0, y0, x1, y1) in h.rails() {
            ev |= self.rail(x0, y0, x1, y1, RAIL, 0.0, 0.0, 0.72, fx);
        }
        for sp in h.spinners {
            let w = sp.speed;
            for (x0, y0, x1, y1) in bars(sp, t) {
                let (px, py) = closest(self.x, self.y, x0, y0, x1, y1);
                // the bar's own speed where the ball meets it
                let (ux, uy) = (-(py - sp.y) * w, (px - sp.x) * w);
                ev |= self.rail(x0, y0, x1, y1, 3.0, ux, uy, 0.6, fx);
            }
            ev |= self.round(sp.x, sp.y, HUB_R, 0.7, 0.0, None, fx);
        }
        for m in h.movers {
            let ((x0, y0, x1, y1), (ux, uy)) = gate(m, t);
            ev |= self.rail(x0, y0, x1, y1, RAIL + 1.0, ux, uy, 0.6, fx);
        }
        for (i, &(bx, by, r)) in h.bumpers.iter().enumerate() {
            ev |= self.round(bx, by, r, 1.0, 110.0, Some(i), fx);
        }

        // the cup
        let (cx, cy) = (h.cup.0 - self.x, h.cup.1 - self.y);
        let cd = (cx * cx + cy * cy).sqrt();
        let sp = (self.vx * self.vx + self.vy * self.vy).sqrt();
        if cd < CUP_R - 1.5 {
            if sp < SINK_SPEED {
                (self.x, self.y) = h.cup;
                (self.vx, self.vy) = (0.0, 0.0);
                fx.sunk(h.cup.0, h.cup.1);
                return ev | EV_HIT | self.finish(self.strokes);
            }
            // too fast: past the middle, the far lip kicks it up and away a little
            if self.vx * cx + self.vy * cy < 0.0 {
                let side = (self.vx * cy - self.vy * cx).signum();
                let turn = -0.12 * side;
                let (c, s) = (turn.cos(), turn.sin());
                let (vx, vy) = (self.vx * c - self.vy * s, self.vx * s + self.vy * c);
                self.vx = vx * 0.94;
                self.vy = vy * 0.94;
            }
        }

        if h.water.iter().any(|a| inside(a, self.x, self.y)) {
            fx.splash(self.x, self.y);
            self.strokes += 1;
            self.splashes += 1;
            self.phase = SPLASH;
            self.phase_ms = 0;
            (self.vx, self.vy) = (0.0, 0.0);
            return ev | EV_LOSE;
        }

        // at rest: barely moving for a moment (a ball held against a rail on
        // a slope stops too)
        if sp < 5.0 {
            self.still_ms += (dt * 1000.0) as i32 + 1;
        } else {
            self.still_ms = 0;
        }
        if sp == 0.0 && !on_slope || self.still_ms > 400 {
            (self.vx, self.vy) = (0.0, 0.0);
            ev |= self.rest();
        }
        ev
    }

    /// Bounces the ball off the rail a-b (half-thickness r) moving at (ux, uy).
    #[allow(clippy::too_many_arguments)]
    fn rail(&mut self, ax: f32, ay: f32, bx: f32, by: f32, r: f32, ux: f32, uy: f32, e: f32, fx: &mut dyn Fx) -> u32 {
        let (px, py) = closest(self.x, self.y, ax, ay, bx, by);
        let (dx, dy) = (self.x - px, self.y - py);
        let d2 = dx * dx + dy * dy;
        let reach = r + BALL_R;
        if d2 >= reach * reach {
            return 0;
        }
        let d = d2.sqrt();
        let (nx, ny) = if d > 1e-4 {
            (dx / d, dy / d)
        } else {
            // dead on the line: push it back the way it came
            let l = (self.vx * self.vx + self.vy * self.vy).sqrt().max(1e-4);
            (-self.vx / l, -self.vy / l)
        };
        self.x = px + nx * reach;
        self.y = py + ny * reach;
        let (rvx, rvy) = (self.vx - ux, self.vy - uy);
        let vn = rvx * nx + rvy * ny;
        if vn >= 0.0 {
            return 0;
        }
        self.vx -= (1.0 + e) * vn * nx;
        self.vy -= (1.0 + e) * vn * ny;
        if -vn > 60.0 {
            fx.rail(px, py, -vn);
            return EV_BLIP;
        }
        0
    }

    /// Bounces the ball off a round post; bumper i adds a kick.
    #[allow(clippy::too_many_arguments)]
    fn round(&mut self, cx: f32, cy: f32, r: f32, e: f32, kick: f32, bumper: Option<usize>, fx: &mut dyn Fx) -> u32 {
        let (dx, dy) = (self.x - cx, self.y - cy);
        let d2 = dx * dx + dy * dy;
        let reach = r + BALL_R;
        if d2 >= reach * reach || d2 < 1e-6 {
            return 0;
        }
        let d = d2.sqrt();
        let (nx, ny) = (dx / d, dy / d);
        self.x = cx + nx * reach;
        self.y = cy + ny * reach;
        let vn = self.vx * nx + self.vy * ny;
        if vn >= 0.0 {
            return 0;
        }
        self.vx -= (1.0 + e) * vn * nx - nx * kick;
        self.vy -= (1.0 + e) * vn * ny - ny * kick;
        if let Some(i) = bumper {
            fx.bump(i, cx + nx * r, cy + ny * r);
            return EV_HIT;
        }
        EV_BLIP
    }

    /// Checks a save just copied in.
    pub fn valid(&self) -> bool {
        let f = |v: f32| v.is_finite();
        self.version == VERSION
            && (0..COUNT as i32).contains(&self.hole)
            && (AIM..=OVER).contains(&self.phase)
            && (0..=MAX_STROKES + 1).contains(&self.strokes)
            && (0..=COUNT as i32).contains(&self.played)
            && (-1..COUNT as i32).contains(&self.cleared)
            && self.total >= 0
            && self.score >= 0
            && self.time_ms >= 0
            && self.hole_ms >= 0
            && [self.x, self.y, self.vx, self.vy, self.from_x, self.from_y, self.aim, self.power].into_iter().all(f)
            && (0.0..=PW).contains(&self.x)
            && (0.0..=PH).contains(&self.y)
    }
}
