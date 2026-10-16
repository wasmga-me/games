//! The rules: paddle, balls, bricks, boosts. Everything a save needs lives in
//! one plain struct (`State`), so a save is a byte copy of it. Units are the
//! plan's (480 x 400, see `draw.rs`), time is milliseconds.

use crate::levels::{self, BOMB, COLS, EMPTY, GOLD, ROWS, SILVER};
use crate::math::{absf, clampf, cosf, sinf, sqrtf};

pub const PW: f32 = 480.0;
pub const PH: f32 = 400.0;
pub const FIELD_L: f32 = 12.0;
pub const FIELD_R: f32 = 468.0;
pub const FIELD_T: f32 = 38.0;
pub const BRICK_X0: f32 = 16.0;
pub const BRICK_Y0: f32 = 60.0;
pub const CELL_W: f32 = 32.0;
pub const CELL_H: f32 = 16.0;
pub const PADDLE_Y: f32 = 368.0; // the paddle's middle
pub const PADDLE_H: f32 = 10.0;
pub const SHIELD_Y: f32 = 390.0;
pub const BALL_R: f32 = 5.0;
pub const ITEM_W: f32 = 26.0;
pub const ITEM_H: f32 = 12.0;

pub const MAX_BALLS: usize = 12;
pub const MAX_ITEMS: usize = 6;
pub const MAX_SHOTS: usize = 16;

pub const MAGIC: u32 = 0x4257_4c31; // "BWL1"
pub const VERSION: u32 = 4;

/// options, in catalog order: speed, starting level, theme
pub const O_SPEED: usize = 0;
pub const O_LEVEL: usize = 1;
pub const O_THEME: usize = 2;
pub const NOPT: usize = 3;
pub const CHOICES: [i32; NOPT] = [3, levels::COUNT as i32, 4];
const SPEEDS: [f32; 3] = [230.0, 275.0, 320.0];
const MAX_SPEED_UP: f32 = 1.45;

pub const PADDLE_WIDTHS: [f32; 4] = [44.0, 66.0, 94.0, 124.0];

// phases
pub const PLAY: i32 = 0;
pub const MISS: i32 = 1; // a ball was lost, lives are left
pub const DYING: i32 = 2; // the last ball was lost
pub const CLEAR: i32 = 3; // the level is cleared, the next one comes up
pub const OVER: i32 = 4;

const MISS_MS: i32 = 900;
const CLEAR_MS: i32 = 1400;

// boosts
pub const B_WIDE: i32 = 0;
pub const B_SHRINK: i32 = 1;
pub const B_MULTI: i32 = 2;
pub const B_SLOW: i32 = 3;
pub const B_LASER: i32 = 4;
pub const B_CATCH: i32 = 5;
pub const B_FIRE: i32 = 6;
pub const B_LIFE: i32 = 7;
pub const B_SHIELD: i32 = 8;
pub const B_FAST: i32 = 9;
pub const NBOOST: usize = 10;
const BOOST_WEIGHT: [u32; NBOOST] = [14, 6, 14, 9, 10, 9, 6, 3, 10, 5];

pub const LASER_MS: i32 = 12000;
pub const CATCH_MS: i32 = 15000;
pub const FIRE_MS: i32 = 9000;
pub const SLOW_MS: i32 = 12000;

// events returned from wg_frame (wg.h)
pub const EV_BLIP: i32 = 1;
pub const EV_HIT: i32 = 2;
pub const EV_LOSE: i32 = 4;
pub const EV_GAMEOVER: i32 = 8;
pub const EV_LEVEL: i32 = 16;
pub const EV_WIN: i32 = 32;

// buttons (wg.h)
pub const K_UP: i32 = 1;
pub const K_LEFT: i32 = 4;
pub const K_RIGHT: i32 = 8;
pub const K_A: i32 = 16;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Ball {
    pub x: f32,
    pub y: f32,
    /// direction, a unit vector; the speed is the game's
    pub dx: f32,
    pub dy: f32,
    pub alive: i32,
    /// held on the paddle (serve or Catch), at this offset from its middle
    pub stuck: i32,
    pub off: f32,
    pub stuck_ms: i32,
    /// bounces off walls and gold since it last broke something or met the
    /// paddle: a ball caught in a loop gets a nudge
    pub idle: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Item {
    pub x: f32,
    pub y: f32,
    pub kind: i32,
    pub alive: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Shot {
    pub x: f32,
    pub y: f32,
    pub alive: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct State {
    pub magic: u32,
    pub version: u32,
    pub rng: u32,
    pub opt: [i32; NOPT],
    pub phase: i32,
    pub phase_ms: i32,
    pub level: i32,
    pub lives: i32,
    pub score: i32,
    pub won: i32,
    pub bricks: [[u8; COLS]; ROWS],
    pub hits: [[u8; COLS]; ROWS],
    pub left: i32,
    pub paddle_x: f32,
    pub paddle_size: i32,
    pub key_v: f32,
    pub target_x: f32,
    pub speed: f32,
    pub balls: [Ball; MAX_BALLS],
    pub items: [Item; MAX_ITEMS],
    pub shots: [Shot; MAX_SHOTS],
    pub laser_ms: i32,
    pub catch_ms: i32,
    pub fire_ms: i32,
    pub slow_ms: i32,
    pub shield: i32,
    pub laser_cd: i32,
    pub drop_luck: i32,
    pub streak: i32,
    pub paddle_hits: i32,
    // pointer
    pub pointer: i32, // 1 mouse, 2 touch: the paddle follows target_x
    pub mbtn: i32,
    pub touch_on: i32,
    pub touch_off: f32,
    pub touch_x0: f32,
    pub touch_ms: i32,
    pub touch_moved: i32,
    pub launch: i32,
    pub prev_buttons: i32,
    pub pend_ev: i32,
    // for the game over screen
    pub time_ms: i32,
    pub broken: i32,
    pub boosts: i32,
    pub best_streak: i32,
    pub lost: i32,
    /// the score when this level began, and the last level cleared (-1 for
    /// none) with what it scored, for the per-level scoreboards
    pub level_start: i32,
    pub cleared: i32,
    pub cleared_score: i32,
}

// SAFETY: #[repr(C)] structs of integers and floats only
unsafe impl wg::Plain for Ball {}
unsafe impl wg::Plain for Item {}
unsafe impl wg::Plain for Shot {}
unsafe impl wg::Plain for State {}

/// What happened this frame that only the picture cares about.
pub trait Fx {
    fn broke(&mut self, r: usize, c: usize, kind: u8);
    fn hit(&mut self, r: usize, c: usize);
    /// the brick at r, c changed in the grid (broken or cracked)
    fn changed(&mut self, r: usize, c: usize);
    fn caught(&mut self, kind: i32, x: f32);
    fn bounce(&mut self, x: f32, y: f32);
    fn level(&mut self);
}

pub fn xorshift(s: &mut u32) -> u32 {
    let mut x = if *s != 0 { *s } else { 0x9E37_79B9 };
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *s = x;
    x
}

impl State {
    pub const fn zero() -> State {
        // SAFETY: State is plain numbers, all-zero is a valid value
        unsafe { core::mem::transmute([0u8; core::mem::size_of::<State>()]) }
    }

    pub fn over(&self) -> bool {
        self.phase == DYING || self.phase == OVER || (self.phase == CLEAR && self.won != 0)
    }

    pub fn paddle_w(&self) -> f32 {
        PADDLE_WIDTHS[self.paddle_size as usize]
    }

    pub fn ball_speed(&self) -> f32 {
        self.speed * if self.slow_ms > 0 { 0.7 } else { 1.0 }
    }

    fn base_speed(&self) -> f32 {
        SPEEDS[self.opt[O_SPEED] as usize] * (1.0 + 0.01 * self.level as f32)
    }

    /// a new run with the options picked
    pub fn start(&mut self, seed: u32) {
        let keep = self.magic == MAGIC;
        let mut opt = [1, 0, 0];
        if keep {
            opt = self.opt;
        }
        *self = State::zero();
        self.magic = MAGIC;
        self.version = VERSION;
        self.rng = seed | 1;
        self.opt = opt;
        self.lives = 3;
        self.cleared = -1;
        self.load_level(self.opt[O_LEVEL]);
    }

    pub fn load_level(&mut self, n: i32) {
        self.level = n;
        self.level_start = self.score;
        self.left = 0;
        let silver = if n >= 15 { 3 } else { 2 };
        for r in 0..ROWS {
            for c in 0..COLS {
                let k = levels::brick(n as usize, r, c);
                self.bricks[r][c] = k;
                self.hits[r][c] = if k == SILVER { silver } else { 1 };
                if k != EMPTY && k != GOLD {
                    self.left += 1;
                }
            }
        }
        self.speed = self.base_speed();
        self.paddle_hits = 0;
        self.items = [Item { x: 0.0, y: 0.0, kind: 0, alive: 0 }; MAX_ITEMS];
        self.shots = [Shot { x: 0.0, y: 0.0, alive: 0 }; MAX_SHOTS];
        self.laser_ms = 0;
        self.catch_ms = 0;
        self.fire_ms = 0;
        self.slow_ms = 0;
        self.shield = 0;
        self.paddle_size = 1;
        self.paddle_x = PW / 2.0;
        self.target_x = self.paddle_x;
        self.serve();
        self.phase = PLAY;
        self.phase_ms = 0;
    }

    /// one ball, held on the paddle until launched
    fn serve(&mut self) {
        self.balls = [Ball { x: 0.0, y: 0.0, dx: 0.0, dy: 0.0, alive: 0, stuck: 0, off: 0.0, stuck_ms: 0, idle: 0 }; MAX_BALLS];
        let off = 10.0;
        self.balls[0] = Ball {
            x: self.paddle_x + off,
            y: PADDLE_Y - PADDLE_H / 2.0 - BALL_R,
            dx: 0.0,
            dy: -1.0,
            alive: 1,
            stuck: 1,
            off,
            stuck_ms: 0,
            idle: 0,
        };
        self.launch = 0;
    }

    fn clamp_paddle(&mut self) {
        let h = self.paddle_w() / 2.0;
        self.paddle_x = clampf(self.paddle_x, FIELD_L + h, FIELD_R - h);
    }

    /// the pointer, in plan units; buttons as wg_mouse has them
    pub fn mouse(&mut self, x: f32, _y: f32, b: i32, outside: bool) {
        let touch = b & 8 != 0;
        let down = b & 1 != 0;
        if outside && !touch {
            self.mbtn = 0;
            return;
        }
        if touch {
            // a finger drags the paddle from where it is, so it never hides
            // it; a short tap launches
            let finger = down && !outside;
            if finger && self.touch_on == 0 {
                self.touch_on = 1;
                self.touch_off = self.paddle_x - x;
                self.touch_x0 = x;
                self.touch_ms = 0;
                self.touch_moved = 0;
            } else if finger {
                if absf(x - self.touch_x0) > 6.0 {
                    self.touch_moved = 1;
                }
                self.target_x = x + self.touch_off;
                self.pointer = 2;
            } else if self.touch_on != 0 {
                self.touch_on = 0;
                if self.touch_moved == 0 && self.touch_ms < 350 {
                    self.launch = 1;
                }
            }
            return;
        }
        self.pointer = 1;
        self.target_x = x;
        if down && self.mbtn == 0 {
            self.launch = 1;
        }
        self.mbtn = down as i32;
    }

    pub fn frame(&mut self, buttons: i32, dt_ms: i32, fx: &mut dyn Fx) -> i32 {
        let pressed = buttons & !self.prev_buttons;
        self.prev_buttons = buttons;
        let dt_ms = dt_ms.clamp(0, 100);
        let dt = dt_ms as f32 / 1000.0;
        xorshift(&mut self.rng);
        if self.touch_on != 0 {
            self.touch_ms += dt_ms;
        }
        let mut ev = core::mem::take(&mut self.pend_ev);
        match self.phase {
            PLAY => {
                self.time_ms += dt_ms;
                self.move_paddle(buttons, dt);
                if pressed & (K_A | K_UP) != 0 {
                    self.launch = 1;
                }
                ev |= self.play(dt_ms, fx);
            }
            MISS => {
                self.move_paddle(buttons, dt);
                self.phase_ms += dt_ms;
                if self.phase_ms >= MISS_MS {
                    self.phase = PLAY;
                    self.serve();
                }
            }
            DYING => {
                self.phase_ms += dt_ms;
                if self.phase_ms >= MISS_MS {
                    self.phase = OVER;
                    ev |= EV_GAMEOVER;
                }
            }
            CLEAR => {
                self.move_paddle(buttons, dt);
                self.phase_ms += dt_ms;
                if self.phase_ms >= CLEAR_MS {
                    if self.won != 0 {
                        self.phase = OVER;
                        ev |= EV_GAMEOVER | EV_WIN;
                    } else {
                        self.load_level(self.level + 1);
                        fx.level();
                    }
                }
            }
            _ => {}
        }
        ev
    }

    fn move_paddle(&mut self, buttons: i32, dt: f32) {
        let dir = (buttons & K_RIGHT != 0) as i32 - (buttons & K_LEFT != 0) as i32;
        if dir != 0 {
            self.pointer = 0;
            // quick to get going, quick to stop
            let top = 560.0;
            self.key_v += dir as f32 * 4200.0 * dt;
            self.key_v = clampf(self.key_v, -top, top);
            if (self.key_v > 0.0) != (dir > 0) {
                self.key_v = dir as f32 * 120.0;
            }
            self.paddle_x += self.key_v * dt;
        } else {
            self.key_v = 0.0;
            if self.pointer != 0 {
                self.paddle_x = self.target_x;
            }
        }
        self.clamp_paddle();
    }

    fn play(&mut self, dt_ms: i32, fx: &mut dyn Fx) -> i32 {
        let mut ev = 0;
        let dt = dt_ms as f32 / 1000.0;
        for t in [&mut self.laser_ms, &mut self.catch_ms, &mut self.fire_ms, &mut self.slow_ms] {
            *t = (*t - dt_ms).max(0);
        }
        // balls on the paddle ride along; a held one lets go by itself in time
        let launch = core::mem::take(&mut self.launch) != 0;
        for i in 0..MAX_BALLS {
            let b = &mut self.balls[i];
            if b.alive == 0 || b.stuck == 0 {
                continue;
            }
            b.stuck_ms += dt_ms;
            b.x = self.paddle_x + b.off;
            b.y = PADDLE_Y - PADDLE_H / 2.0 - BALL_R;
            // 1 is a serve, which waits; 2 is a catch, which lets go in time
            if launch || (b.stuck == 2 && b.stuck_ms > 2500) {
                b.stuck = 0;
                let t = clampf(b.off / (PADDLE_WIDTHS[self.paddle_size as usize] / 2.0 + BALL_R), -1.0, 1.0);
                let a = t * 1.0;
                b.dx = sinf(a);
                b.dy = -cosf(a);
                ev |= EV_BLIP;
            }
        }
        // lasers
        if self.laser_ms > 0 {
            self.laser_cd -= dt_ms;
            if self.laser_cd <= 0 {
                self.laser_cd = 360;
                let h = self.paddle_w() / 2.0 - 7.0;
                for x in [self.paddle_x - h, self.paddle_x + h] {
                    if let Some(s) = self.shots.iter_mut().find(|s| s.alive == 0) {
                        *s = Shot { x, y: PADDLE_Y - PADDLE_H, alive: 1 };
                    }
                }
            }
        }
        for i in 0..MAX_SHOTS {
            if self.shots[i].alive == 0 {
                continue;
            }
            self.shots[i].y -= 560.0 * dt;
            let (x, y) = (self.shots[i].x, self.shots[i].y);
            if y < FIELD_T {
                self.shots[i].alive = 0;
                continue;
            }
            if let Some((r, c)) = cell_at(x, y) {
                if self.bricks[r][c] != EMPTY {
                    self.shots[i].alive = 0;
                    ev |= self.damage(r, c, false, fx);
                }
            }
        }
        // balls
        let speed = self.ball_speed();
        let paddle_w = self.paddle_w();
        for i in 0..MAX_BALLS {
            if self.balls[i].alive == 0 || self.balls[i].stuck != 0 {
                continue;
            }
            let dist = speed * dt;
            let steps = (dist / 2.0) as i32 + 1;
            let h = dist / steps as f32;
            for _ in 0..steps {
                ev |= self.step_ball(i, h, paddle_w, fx);
                if self.balls[i].alive == 0 || self.balls[i].stuck != 0 || self.left == 0 {
                    break;
                }
            }
            if self.left == 0 {
                break;
            }
        }
        // boosts falling
        for i in 0..MAX_ITEMS {
            let it = self.items[i];
            if it.alive == 0 {
                continue;
            }
            let y = it.y + 105.0 * dt;
            self.items[i].y = y;
            let half = paddle_w / 2.0;
            if y + ITEM_H / 2.0 >= PADDLE_Y - PADDLE_H / 2.0
                && y - ITEM_H / 2.0 <= PADDLE_Y + PADDLE_H / 2.0
                && it.x + ITEM_W / 2.0 >= self.paddle_x - half
                && it.x - ITEM_W / 2.0 <= self.paddle_x + half
            {
                self.items[i].alive = 0;
                self.boost(it.kind);
                fx.caught(it.kind, it.x);
                ev |= if it.kind == B_SHRINK || it.kind == B_FAST { EV_HIT } else { EV_BLIP };
            } else if y > PH + ITEM_H {
                self.items[i].alive = 0;
            }
        }
        if self.left == 0 {
            self.score += 100 * (self.level + 1);
            self.cleared = self.level;
            self.cleared_score = self.score - self.level_start;
            self.phase = CLEAR;
            self.phase_ms = 0;
            self.won = (self.level as usize + 1 >= levels::LEVELS.len()) as i32;
            for b in self.balls.iter_mut() {
                b.alive = 0;
            }
            for it in self.items.iter_mut() {
                it.alive = 0;
            }
            for s in self.shots.iter_mut() {
                s.alive = 0;
            }
            return ev | EV_LEVEL;
        }
        if !self.balls.iter().any(|b| b.alive != 0) {
            self.lost += 1;
            self.lives -= 1;
            self.streak = 0;
            self.phase_ms = 0;
            self.laser_ms = 0;
            self.catch_ms = 0;
            self.fire_ms = 0;
            self.slow_ms = 0;
            self.paddle_size = 1;
            self.clamp_paddle();
            for it in self.items.iter_mut() {
                it.alive = 0;
            }
            for s in self.shots.iter_mut() {
                s.alive = 0;
            }
            if self.lives <= 0 {
                self.phase = DYING;
            } else {
                self.phase = MISS;
            }
            ev |= EV_LOSE;
        }
        ev
    }

    fn step_ball(&mut self, i: usize, h: f32, paddle_w: f32, fx: &mut dyn Fx) -> i32 {
        let mut ev = 0;
        let mut b = self.balls[i];
        b.x += b.dx * h;
        b.y += b.dy * h;
        // walls
        if b.x - BALL_R < FIELD_L {
            b.x = FIELD_L + BALL_R;
            b.dx = absf(b.dx);
            b.idle += 1;
            fx.bounce(b.x - BALL_R, b.y);
        } else if b.x + BALL_R > FIELD_R {
            b.x = FIELD_R - BALL_R;
            b.dx = -absf(b.dx);
            b.idle += 1;
            fx.bounce(b.x + BALL_R, b.y);
        }
        if b.y - BALL_R < FIELD_T {
            b.y = FIELD_T + BALL_R;
            b.dy = absf(b.dy);
            b.idle += 1;
            fx.bounce(b.x, b.y - BALL_R);
        }
        // paddle: where it lands sets the angle, up to 60 degrees either way
        let half = paddle_w / 2.0;
        let top = PADDLE_Y - PADDLE_H / 2.0;
        if b.dy > 0.0
            && b.y + BALL_R >= top
            && b.y - BALL_R <= PADDLE_Y + PADDLE_H / 2.0
            && b.x >= self.paddle_x - half - BALL_R
            && b.x <= self.paddle_x + half + BALL_R
        {
            let t = clampf((b.x - self.paddle_x) / (half + BALL_R), -1.0, 1.0);
            let a = t * 1.05;
            b.dx = sinf(a);
            b.dy = -cosf(a);
            b.y = top - BALL_R;
            self.streak = 0;
            b.idle = 0;
            self.paddle_hits += 1;
            if self.paddle_hits % 10 == 0 {
                self.speed = (self.speed * 1.03).min(self.base_speed() * MAX_SPEED_UP);
            }
            if self.catch_ms > 0 {
                b.stuck = 2;
                b.off = clampf(b.x - self.paddle_x, -half, half);
                b.stuck_ms = 0;
            }
            ev |= EV_BLIP;
        }
        // shield
        if self.shield != 0 && b.dy > 0.0 && b.y + BALL_R >= SHIELD_Y {
            b.y = SHIELD_Y - BALL_R;
            b.dy = -absf(b.dy);
            self.shield = 0;
            fx.bounce(b.x, SHIELD_Y);
            ev |= EV_BLIP;
        }
        if b.y - BALL_R > PH {
            b.alive = 0;
        }
        // bricks: the one the ball sinks into deepest
        let c0 = ((b.x - BALL_R - BRICK_X0) / CELL_W) as i32 - (b.x - BALL_R < BRICK_X0) as i32;
        let c1 = ((b.x + BALL_R - BRICK_X0) / CELL_W) as i32;
        let r0 = ((b.y - BALL_R - BRICK_Y0) / CELL_H) as i32 - (b.y - BALL_R < BRICK_Y0) as i32;
        let r1 = ((b.y + BALL_R - BRICK_Y0) / CELL_H) as i32;
        let mut best: Option<(usize, usize, f32, f32, f32)> = None;
        for r in r0.max(0)..=r1.min(ROWS as i32 - 1) {
            for c in c0.max(0)..=c1.min(COLS as i32 - 1) {
                let (r, c) = (r as usize, c as usize);
                if self.bricks[r][c] == EMPTY {
                    continue;
                }
                let x0 = BRICK_X0 + c as f32 * CELL_W;
                let y0 = BRICK_Y0 + r as f32 * CELL_H;
                let px = clampf(b.x, x0, x0 + CELL_W);
                let py = clampf(b.y, y0, y0 + CELL_H);
                let (nx, ny) = (b.x - px, b.y - py);
                let d2 = nx * nx + ny * ny;
                if d2 >= BALL_R * BALL_R {
                    continue;
                }
                if best.map_or(true, |(_, _, d, _, _)| d2 < d) {
                    best = Some((r, c, d2, nx, ny));
                }
            }
        }
        if let Some((r, c, d2, mut nx, mut ny)) = best {
            let fire = self.fire_ms > 0 && self.bricks[r][c] != GOLD;
            if !fire {
                let mut d = sqrtf(d2);
                if d < 1e-4 {
                    // the middle went in: back out the way it came
                    nx = -b.dx;
                    ny = -b.dy;
                    d = 0.0;
                } else {
                    nx /= d;
                    ny /= d;
                }
                let dot = b.dx * nx + b.dy * ny;
                if dot < 0.0 {
                    b.dx -= 2.0 * dot * nx;
                    b.dy -= 2.0 * dot * ny;
                }
                b.x += nx * (BALL_R - d + 0.01);
                b.y += ny * (BALL_R - d + 0.01);
            }
            if self.bricks[r][c] == GOLD {
                b.idle += 1;
            } else {
                b.idle = 0;
            }
            self.balls[i] = b;
            ev |= self.damage(r, c, fire, fx);
            b = self.balls[i];
        }
        if b.idle >= 12 {
            // turn it a little, one way or the other, to break the loop
            let a = if xorshift(&mut self.rng) & 1 != 0 { 0.3f32 } else { -0.3 };
            let (sn, co) = (sinf(a), cosf(a));
            (b.dx, b.dy) = (b.dx * co - b.dy * sn, b.dx * sn + b.dy * co);
            b.idle = 0;
        }
        // never so flat that it takes forever to come back
        if absf(b.dy) < 0.28 {
            b.dy = if b.dy < 0.0 { -0.28 } else { 0.28 };
            let l = sqrtf(b.dx * b.dx + b.dy * b.dy);
            b.dx /= l;
            b.dy /= l;
        }
        self.balls[i] = b;
        ev
    }

    /// a ball, shot or blast hits the brick at r, c
    fn damage(&mut self, r: usize, c: usize, smash: bool, fx: &mut dyn Fx) -> i32 {
        let k = self.bricks[r][c];
        if k == EMPTY {
            return 0;
        }
        if k == GOLD {
            fx.hit(r, c);
            return EV_HIT;
        }
        if !smash && self.hits[r][c] > 1 {
            self.hits[r][c] -= 1;
            fx.hit(r, c);
            fx.changed(r, c);
            return EV_HIT;
        }
        self.bricks[r][c] = EMPTY;
        self.left -= 1;
        self.broken += 1;
        self.streak += 1;
        self.best_streak = self.best_streak.max(self.streak);
        let base = match k {
            SILVER => 50,
            BOMB => 20,
            _ => 10,
        };
        // bricks in a row without touching the paddle are worth more
        self.score += base + base * self.streak.min(20) / 5;
        fx.broke(r, c, k);
        fx.changed(r, c);
        self.drop_boost(r, c);
        if k == BOMB {
            for (dr, dc) in [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)] {
                let (nr, nc) = (r as i32 + dr, c as i32 + dc);
                if nr >= 0 && nc >= 0 && (nr as usize) < ROWS && (nc as usize) < COLS {
                    let (nr, nc) = (nr as usize, nc as usize);
                    if self.bricks[nr][nc] != GOLD {
                        self.damage(nr, nc, true, fx);
                    }
                }
            }
        }
        EV_HIT
    }

    fn drop_boost(&mut self, r: usize, c: usize) {
        // about one brick in seven drops something, and never a long drought
        self.drop_luck += 1;
        let roll = xorshift(&mut self.rng) % 100;
        if roll >= 13 && self.drop_luck < 14 {
            return;
        }
        let Some(slot) = self.items.iter().position(|it| it.alive == 0) else { return };
        self.drop_luck = 0;
        let total: u32 = BOOST_WEIGHT.iter().sum();
        let mut pick = xorshift(&mut self.rng) % total;
        let mut kind = 0;
        for (i, w) in BOOST_WEIGHT.iter().enumerate() {
            if pick < *w {
                kind = i as i32;
                break;
            }
            pick -= w;
        }
        // nothing useless: no Life at nine lives, no Multi with a crowd out
        if (kind == B_LIFE && self.lives >= 9) || (kind == B_MULTI && self.balls.iter().filter(|b| b.alive != 0).count() > 6) {
            kind = B_WIDE;
        }
        self.items[slot] = Item {
            x: BRICK_X0 + (c as f32 + 0.5) * CELL_W,
            y: BRICK_Y0 + (r as f32 + 0.5) * CELL_H,
            kind,
            alive: 1,
        };
    }

    pub fn boost(&mut self, kind: i32) {
        self.boosts += 1;
        self.score += 50;
        match kind {
            B_WIDE => self.paddle_size = (self.paddle_size + 1).min(3),
            B_SHRINK => self.paddle_size = (self.paddle_size - 1).max(0),
            B_MULTI => self.multiply(),
            B_SLOW => self.slow_ms = SLOW_MS,
            B_LASER => {
                self.laser_ms = LASER_MS;
                self.laser_cd = 0;
            }
            B_CATCH => self.catch_ms = CATCH_MS,
            B_FIRE => self.fire_ms = FIRE_MS,
            B_LIFE => self.lives = (self.lives + 1).min(9),
            B_SHIELD => self.shield = 1,
            B_FAST => self.speed = (self.speed * 1.15).min(self.base_speed() * MAX_SPEED_UP),
            _ => {}
        }
        self.clamp_paddle();
    }

    /// every ball in play becomes three
    fn multiply(&mut self) {
        let now: [Ball; MAX_BALLS] = self.balls;
        for b in now.iter() {
            if b.alive == 0 {
                continue;
            }
            let (dx, dy) = if b.stuck != 0 { (0.0, -1.0) } else { (b.dx, b.dy) };
            for a in [-0.45f32, 0.45] {
                let Some(slot) = self.balls.iter().position(|x| x.alive == 0) else { return };
                let (s, co) = (sinf(a), cosf(a));
                let mut nb = *b;
                nb.stuck = 0;
                nb.dx = dx * co - dy * s;
                nb.dy = dx * s + dy * co;
                if nb.dy > -0.28 && b.stuck != 0 {
                    nb.dy = -0.5;
                }
                self.balls[slot] = nb;
            }
        }
    }

    /// everything the game indexes with, so a bad save can't reach past a table
    pub fn valid(&self) -> bool {
        if self.magic != MAGIC || self.version != VERSION {
            return false;
        }
        for i in 0..NOPT {
            if self.opt[i] < 0 || self.opt[i] >= CHOICES[i] {
                return false;
            }
        }
        if !(PLAY..=OVER).contains(&self.phase) || self.level < 0 || self.level as usize >= levels::LEVELS.len() {
            return false;
        }
        if !(0..=3).contains(&self.paddle_size) || self.lives < 0 || self.lives > 9 {
            return false;
        }
        let ok = |v: f32| v == v && absf(v) < 1e5;
        if !ok(self.paddle_x) || !ok(self.target_x) || !ok(self.speed) || !ok(self.key_v) {
            return false;
        }
        if self.balls.iter().any(|b| !ok(b.x) || !ok(b.y) || !ok(b.dx) || !ok(b.dy) || !ok(b.off)) {
            return false;
        }
        if self.items.iter().any(|it| !ok(it.x) || !ok(it.y) || it.kind < 0 || it.kind >= NBOOST as i32) {
            return false;
        }
        if self.cleared < -1 || self.cleared >= self.level.max(0) + 1 || self.cleared_score < 0 {
            return false;
        }
        self.bricks.iter().flatten().all(|&k| k <= BOMB)
    }
}

/// a player that follows (tests and the thumbnail) the lowest ball falling, aiming a little off centre
pub fn bot(s: &mut State, aim: f32) {
    let mut best: Option<&Ball> = None;
    for b in s.balls.iter().filter(|b| b.alive != 0 && b.stuck == 0 && b.dy > 0.0) {
        if best.map_or(true, |o| b.y > o.y) {
            best = Some(b);
        }
    }
    let x = match best {
        Some(b) => b.x + aim,
        None => s.items.iter().find(|i| i.alive != 0).map_or(PW / 2.0, |i| i.x),
    };
    s.pointer = 1;
    s.target_x = x;
    s.launch = 1;
}

/// the grid cell under a point, if any
pub fn cell_at(x: f32, y: f32) -> Option<(usize, usize)> {
    if x < BRICK_X0 || y < BRICK_Y0 {
        return None;
    }
    let c = ((x - BRICK_X0) / CELL_W) as usize;
    let r = ((y - BRICK_Y0) / CELL_H) as usize;
    if r < ROWS && c < COLS {
        Some((r, c))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoFx;
    impl Fx for NoFx {
        fn broke(&mut self, _: usize, _: usize, _: u8) {}
        fn hit(&mut self, _: usize, _: usize) {}
        fn changed(&mut self, _: usize, _: usize) {}
        fn caught(&mut self, _: i32, _: f32) {}
        fn bounce(&mut self, _: f32, _: f32) {}
        fn level(&mut self) {}
    }

    #[test]
    fn a_bot_clears_every_level() {
        for n in 0..levels::LEVELS.len() {
            let mut s = State::zero();
            s.start(12345 + n as u32);
            s.load_level(n as i32);
            s.lives = 9;
            let mut ms = 0;
            let mut aim = 0.0;
            while s.level == n as i32 && s.phase != OVER && s.phase != DYING && ms < 20 * 60 * 1000 {
                if ms % 4000 == 0 {
                    aim = ((xorshift(&mut s.rng) % 41) as f32 - 20.0) * 0.6;
                }
                bot(&mut s, aim);
                s.frame(0, 16, &mut NoFx);
                s.lives = 9;
                ms += 16;
                assert!(s.valid(), "level {} went bad", n + 1);
            }
            assert!(s.level != n as i32 || s.phase == CLEAR || s.won != 0, "level {} not cleared in {} s, {} left", n + 1, ms / 1000, s.left);
            assert_eq!(s.cleared, n as i32, "level {} cleared as {}", n + 1, s.cleared);
            assert!(s.cleared_score > 0 && s.cleared_score <= s.score);
            println!("level {:2}: {:4} s, score {}", n + 1, ms / 1000, s.score);
        }
    }
}
