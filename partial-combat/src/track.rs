//! Laying the pier ahead of the squad, a chunk at a time: gate pairs, enemy
//! groups, barrels, gun crates, holes, narrow stretches, ships firing from
//! the sea, and the boss arena that ends a level. A level is laid from its
//! own seed, so it is the same every time; the endless pier from the run's.
//!
//! As it lays the pier it keeps `expect`, the squad a player who picks well
//! would have by then, and sizes enemies, barrels and the boss by it.
use crate::*;

/// how far ahead of the squad the pier is laid
const AHEAD: f32 = 95.0;
/// seconds a squad of the size expected takes to beat a boss
const BOSS_SECONDS: f32 = 13.0;

const C_GATES: i32 = 0;
const C_ENEMY: i32 = 1;
const C_CRATE: i32 = 2;
const C_BARREL: i32 = 3;
const C_HAZARD: i32 = 4;

impl Combat {
    pub fn lay_track(&mut self) {
        while self.s.arena_d == 0.0 && self.s.next_d < self.s.d + AHEAD {
            self.lay_chunk();
        }
    }

    /// How hard the pier is where it is being laid.
    fn hard_at(&self) -> f32 {
        if self.s.endless() {
            self.s.next_d / 160.0 + self.s.level as f32 * 1.0
        } else {
            self.s.level as f32
        }
    }

    fn rand(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.s.track_rng.float()
    }
    fn chance(&mut self, p: f32) -> bool {
        self.s.track_rng.float() < p
    }

    fn add_thing(&mut self, t: Thing) -> usize {
        let n = self.s.nthing as usize;
        if n >= MAXO {
            return MAXO;
        }
        self.s.thing[n] = t;
        self.s.nthing += 1;
        n
    }

    fn lay_chunk(&mut self) {
        let hard = self.hard_at();
        let c = self.s.chunk;
        self.s.chunk += 1;
        let total = if self.s.endless() { 10 } else { 8 + (self.s.level / 2).min(5) };
        let k = if self.s.endless() { c % (total + 1) } else { c };
        if k >= total {
            return self.lay_arena();
        }
        // the last stretch before a boss is a gate pair, for one more boost
        let kind = if k == total - 1 {
            C_GATES
        } else {
            match k {
                0 | 3 | 6 | 10 => C_GATES,
                1 | 5 | 8 | 11 => C_ENEMY,
                2 => C_CRATE,
                4 | 9 => {
                    if hard >= 1.0 {
                        C_HAZARD
                    } else {
                        C_BARREL
                    }
                }
                7 => {
                    if self.s.expect_gun < 3 && hard >= 3.0 {
                        C_CRATE
                    } else {
                        C_BARREL
                    }
                }
                _ => C_ENEMY,
            }
        };
        let d0 = self.s.next_d;
        let len = match kind {
            C_GATES => self.lay_gates(d0, hard),
            C_ENEMY => self.lay_enemies(d0, hard),
            C_CRATE => self.lay_crate(d0, hard),
            C_BARREL => self.lay_barrels(d0, hard),
            _ => self.lay_hazard(d0, hard),
        };
        self.s.next_d = d0 + len;
    }

    /// Two gates side by side: a sum that helps and one that doesn't, or two that help unequally.
    fn lay_gates(&mut self, d0: f32, hard: f32) -> f32 {
        let e = self.s.expect.max(1.0);
        let add = |p: &mut Self| (OP_ADD, (3.0 + hard * 1.2 + e * p.rand(0.25, 0.6)).round().min(90.0) as i32);
        let mul = |p: &mut Self| (OP_MUL, if hard >= 4.0 && e < 40.0 && p.chance(0.4) { 3 } else { 2 });
        let sub = |p: &mut Self| (OP_SUB, (2.0 + e * p.rand(0.3, 0.7) + hard).round().min(90.0) as i32);
        let div = |p: &mut Self| (OP_DIV, if hard >= 6.0 && p.chance(0.4) { 3 } else { 2 });
        let good = |p: &mut Self| if e >= 5.0 && p.chance(0.45) { mul(p) } else { add(p) };
        let bad = |p: &mut Self| if e >= 8.0 && p.chance(0.4) { div(p) } else { sub(p) };
        // a level opens with two gates that both help, so a wrong guess there isn't the end
        let first = self.s.chunk == 1;
        let pair = if first {
            (add(self), if e >= 3.0 { mul(self) } else { add(self) })
        } else if self.chance((0.55 - hard * 0.02).max(0.3)) {
            (good(self), bad(self))
        } else if hard >= 5.0 && e >= 15.0 && self.chance(0.2) {
            // two bad ones: take the lesser, or shoot one up
            (sub(self), div(self))
        } else {
            (add(self), mul(self))
        };
        let (left, right) = if self.chance(0.5) { (pair.0, pair.1) } else { (pair.1, pair.0) };
        let d = d0 + 6.0;
        let a = self.add_thing(Thing { kind: K_GATE, x: -2.5, d, w: 4.7, a: left.0, value: left.1, ..NO_THING });
        let b = self.add_thing(Thing { kind: K_GATE, x: 2.5, d, w: 4.7, a: right.0, value: right.1, ..NO_THING });
        if a < MAXO && b < MAXO {
            self.s.thing[a].b = b as i32;
            self.s.thing[b].b = a as i32;
        }
        let apply = |(op, v): (i32, i32)| match op {
            OP_ADD => e + v as f32,
            OP_SUB => e - v as f32,
            OP_MUL => e * v as f32,
            _ => e / v as f32,
        };
        self.s.expect = (apply(left).max(apply(right)) * 0.95).clamp(1.0, MAX_COUNT as f32);
        16.0
    }

    /// An enemy group across the pier, waiting, sometimes behind a TNT barrel.
    fn lay_enemies(&mut self, d0: f32, hard: f32) -> f32 {
        let e = self.s.expect;
        let share = (0.42 + hard * 0.03).min(0.85);
        // the endless pier's groups keep growing past what any squad holds off
        let most = if self.s.endless() { (60.0 + hard * 1.5).min(100.0) as i32 } else { 60 };
        let n = ((e * share).round() as i32 + 2).clamp(3, most);
        let x = self.rand(-1.8, 1.8);
        let d = d0 + 14.0;
        let group = self.add_thing(Thing { kind: K_GROUP, x, d, value: n, ..NO_THING });
        let runners = if hard >= 2.0 { 0.2 } else { 0.0 };
        let rifles = if hard >= 4.0 { 0.15 } else { 0.0 };
        let brutes = if hard >= 5.0 { 1 + (hard / 6.0) as i32 } else { 0 };
        for i in 0..n {
            let (ox, oz) = slot(i as usize);
            let roll = self.s.track_rng.float();
            let kind = if i < brutes { E_BRUTE } else if roll < rifles && oz > 0.0 { E_RIFLE } else if roll < rifles + runners { E_RUNNER } else { E_GRUNT };
            self.spawn_enemy(kind, (x + ox * 1.5).clamp(-HALF + 0.4, HALF - 0.4), d + oz * 1.4, group as i32, ES_WAIT);
        }
        if hard >= 1.0 && self.chance(0.5) {
            let tx = (x + self.rand(-1.0, 1.0)).clamp(-3.5, 3.5);
            self.add_thing(Thing { kind: K_TNT, x: tx, d: d - 4.5, hp: 3, hp_max: 3, ..NO_THING });
        }
        // the squad loses about those that reach it: the rest fall to its fire first
        self.s.expect = (e - n as f32 * 0.45).max(e * 0.45).max(1.0);
        26.0
    }

    /// Damage a second the squad expected puts into something lined up in front of it:
    /// only the soldiers in line with it hit it.
    fn dps_in_line(&self) -> f32 {
        let e = self.s.expect.max(1.0);
        e.min(MAX_SHOOTERS) * gun_dps(self.s.expect_gun) * (0.76 / radius(e as i32)).min(1.0)
    }

    /// A gun crate, shot open for a better gun, sometimes with a barrel beside it.
    fn lay_crate(&mut self, d0: f32, hard: f32) -> f32 {
        let gun = (self.s.expect_gun + 1).min(4);
        let hp = (4.0 + self.dps_in_line() * (1.8 + hard * 0.05)).round() as i32;
        let x = if self.chance(0.5) { -2.5 } else { 2.5 };
        self.add_thing(Thing { kind: K_CRATE, x, d: d0 + 8.0, value: gun, hp, hp_max: hp, ..NO_THING });
        if self.chance(0.6) {
            self.lay_barrel(-x, d0 + 8.0, hard);
        }
        self.s.expect_gun = gun;
        16.0
    }

    fn lay_barrels(&mut self, d0: f32, hard: f32) -> f32 {
        let x = self.rand(-2.8, 2.8);
        self.lay_barrel(x, d0 + 6.0, hard);
        if self.chance(0.5) {
            let x2 = if x < 0.0 { x + 3.6 } else { x - 3.6 };
            if self.chance(0.4) && hard >= 2.0 {
                self.add_thing(Thing { kind: K_TNT, x: x2, d: d0 + 9.0, hp: 3, hp_max: 3, ..NO_THING });
            } else {
                self.lay_barrel(x2, d0 + 9.0, hard);
            }
        }
        14.0
    }

    fn lay_barrel(&mut self, x: f32, d: f32, hard: f32) {
        let e = self.s.expect;
        let value = (3.0 + e * 0.25 + hard).round().min(60.0) as i32;
        let hp = (3.0 + self.dps_in_line() * (1.2 + hard * 0.04)).round() as i32;
        self.add_thing(Thing { kind: K_BARREL, x, d, value, hp, hp_max: hp, ..NO_THING });
        self.s.expect = (e + value as f32 * 0.8).min(MAX_COUNT as f32);
    }

    /// Holes in the deck, a narrow stretch, or a ship firing shells.
    fn lay_hazard(&mut self, d0: f32, hard: f32) -> f32 {
        let row0 = (d0 / TILE).ceil() * TILE + TILE * 2.0;
        let roll = self.s.track_rng.below(if hard >= 5.0 { 3 } else if hard >= 3.0 { 2 } else { 1 });
        let hole = |p: &mut Self, lane: i32, lanes: i32, row: f32, rows: f32| {
            p.add_thing(Thing { kind: K_HOLE, x: -HALF + lane as f32 * TILE, w: lanes as f32 * TILE, d: row, len: rows * TILE, ..NO_THING });
        };
        let len = match roll {
            0 => {
                // holes to steer round: one side, then the other
                let first = self.s.track_rng.below(3) as i32;
                hole(self, first, 2, row0, 2.0);
                let second = if first == 0 { 2 } else if first == 2 { 0 } else if self.chance(0.5) { 0 } else { 2 };
                hole(self, second, 2, row0 + TILE * 5.0, 2.0);
                if hard >= 4.0 && self.chance(0.5) {
                    hole(self, first.min(2), 2, row0 + TILE * 10.0, 2.0);
                    TILE * 17.0
                } else {
                    TILE * 12.0
                }
            }
            1 => {
                // a narrow stretch: two lanes, that may step aside half way
                let rows = 4.0 + (hard / 3.0).min(3.0).floor();
                let lane = self.s.track_rng.below(3) as i32;
                let side = |p: &mut Self, lane: i32, row: f32, rows: f32| {
                    if lane > 0 {
                        hole(p, 0, lane, row, rows);
                    }
                    if lane + 2 < LANES {
                        hole(p, lane + 2, LANES - lane - 2, row, rows);
                    }
                };
                side(self, lane, row0, rows);
                if hard >= 5.0 && self.chance(0.6) {
                    let next = if lane == 1 { if self.chance(0.5) { 0 } else { 2 } } else { 1 };
                    side(self, next, row0 + rows * TILE, rows);
                    (rows * 2.0 + 5.0) * TILE
                } else {
                    (rows + 5.0) * TILE
                }
            }
            _ => {
                // a ship off the pier, firing shells at the squad as it passes
                let x = if self.chance(0.5) { -13.0 } else { 13.0 };
                self.add_thing(Thing { kind: K_SHIP, x, d: d0 + 18.0, cool: 0.8, ..NO_THING });
                if self.chance(0.5) {
                    let bx = -x.signum() * 2.5;
                    self.lay_barrel(bx, d0 + 12.0, hard);
                }
                30.0
            }
        };
        self.s.expect *= 0.9;
        len
    }

    /// The end of a level: the squad stops at arena_d and fights the boss.
    fn lay_arena(&mut self) {
        let d0 = self.s.next_d;
        let arena = d0 + 6.0;
        self.s.arena_d = arena;
        self.add_thing(Thing { kind: K_ARENA, d: arena, len: 26.0, ..NO_THING });
        let e = self.s.expect.max(3.0);
        let dps = e.min(MAX_SHOOTERS) * gun_dps(self.s.expect_gun);
        let hard = self.hard_at();
        let toughness = if self.s.endless() { 0.6 + hard * 0.035 } else { (0.55 + hard * 0.025).min(1.1) };
        let hp = (BOSS_SECONDS * toughness * dps).round().max(30.0) as i32;
        let kind = if self.s.endless() { self.s.level % 4 } else { self.s.level % 4 };
        self.s.boss = Boss { state: 1, kind, x: 0.0, d: arena + 17.0, hp, hp_max: hp, t: 0.0, cool: 2.0, atk: 0, atk_t: 0.0, flash: 0.0, sway: 0.0 };
        self.s.next_d = arena + 28.0;
    }
}
