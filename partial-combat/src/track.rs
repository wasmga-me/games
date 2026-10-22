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
/// a gate pair: each gate half the pier wide
const GATE_X: f32 = HALF * 0.5;
const GATE_W: f32 = HALF - 0.6;
/// where crates and things on one side stand
const SIDE_X: f32 = HALF * 0.5;
/// the most soldiers in a row of a boss's army
const ARMY_ROW: i32 = 12;

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
        self.s.hardness_at(self.s.next_d)
    }

    fn rand(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.s.track_rng.float()
    }
    fn chance(&mut self, p: f32) -> bool {
        self.s.track_rng.float() < p
    }

    /// Puts a thing on the pier; returns where it is in the list, or None when the list is full.
    fn add_thing(&mut self, t: Thing) -> Option<usize> {
        let n = self.s.nthing as usize;
        if n >= MAXO {
            return None;
        }
        self.s.thing[n] = t;
        self.s.nthing += 1;
        Some(n)
    }

    fn lay_chunk(&mut self) {
        let hard = self.hard_at();
        let c = self.s.chunk;
        self.s.chunk += 1;
        let total = if self.s.endless() {
            10
        } else {
            8 + (self.s.level / 2).min(5)
        };
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
        let g = gains(e);
        let add = |p: &mut Self| {
            (
                OP_ADD,
                (3.0 + hard * 1.2 + g * p.rand(0.25, 0.6)).round() as i32,
            )
        };
        // far down the endless pier a big squad no longer doubles
        let doubles = !self.s.endless() || e.max(self.s.count as f32) < 200.0 - hard * 3.0;
        let mul = |p: &mut Self| {
            if !doubles {
                return add(p);
            }
            (
                OP_MUL,
                if hard >= 4.0 && e < 40.0 && p.chance(0.4) {
                    3
                } else {
                    2
                },
            )
        };
        let sub = |p: &mut Self| {
            (
                OP_SUB,
                (2.0 + g * p.rand(0.3, 0.7) * (1.0 + hard * 0.04) + hard).round() as i32,
            )
        };
        let div = |p: &mut Self| (OP_DIV, if hard >= 6.0 && p.chance(0.4) { 3 } else { 2 });
        let good = |p: &mut Self| {
            if e >= 5.0 && p.chance(0.45) {
                mul(p)
            } else {
                add(p)
            }
        };
        let bad = |p: &mut Self| {
            if e >= 8.0 && p.chance(0.4) {
                div(p)
            } else {
                sub(p)
            }
        };
        // a level opens with two gates that both help, so a wrong guess there isn't the end
        let first = self.s.chunk == 1;
        let pair = if first {
            (add(self), if e >= 3.0 { mul(self) } else { add(self) })
        } else if self.chance((0.55 - hard * 0.02).max(0.3)) {
            (good(self), bad(self))
        } else if hard >= 5.0 && e >= 15.0 && self.chance(0.1 + hard * 0.012) {
            // two bad ones: take the lesser, or shoot one up
            (sub(self), div(self))
        } else {
            (add(self), mul(self))
        };
        let (left, right) = if self.chance(0.5) {
            (pair.0, pair.1)
        } else {
            (pair.1, pair.0)
        };
        // the squad expected, firing at a gate all the way in, counts it pts,
        // a share of its gains, less on later levels; a squad that fires
        // harder counts it at most twice that
        let pts = (1.0 - hard * 0.04).max(0.25) * g;
        let gate = Thing {
            kind: K_GATE,
            d: d0 + 6.0,
            w: GATE_W,
            hp: ((self.burst() / pts).round() as i32).max(1),
            hp_max: (pts * 2.0).round() as i32,
            ..NO_THING
        };
        let a = self.add_thing(Thing {
            x: -GATE_X,
            a: left.0,
            value: left.1,
            ..gate
        });
        let b = self.add_thing(Thing {
            x: GATE_X,
            a: right.0,
            value: right.1,
            ..gate
        });
        if let (Some(a), Some(b)) = (a, b) {
            self.s.thing[a].b = b as i32;
            self.s.thing[b].b = a as i32;
        }
        // the size expected after the gate, unrounded (gate_sum rounds), a
        // + gate counted up by about half what shooting it all the way in would
        let apply = |(op, v): (i32, i32)| match op {
            OP_ADD => e + v as f32 + pts * 0.5,
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
        // the share of the squad a group costs, more on later levels
        let loss = (0.19 + hard * 0.016).min(0.5);
        let hp = enemy_hp(E_GRUNT, hard);
        // the endless pier's groups keep growing past what any squad holds off
        let most = if self.s.endless() {
            (70.0 + hard * 1.5).min(110.0) as i32
        } else {
            70
        };
        let n = ((e * loss / reach(hp)).round() as i32 + 2).clamp(3, most);
        let x = self.rand(-HALF * 0.4, HALF * 0.4);
        let d = d0 + 14.0;
        let group = self.lay_group(x, d, n);
        let runners = if hard >= 2.0 { 0.2 } else { 0.0 };
        let rifles = if hard >= 4.0 { 0.15 } else { 0.0 };
        let brutes = if hard >= 5.0 {
            1 + (hard / 6.0) as i32
        } else {
            0
        };
        for i in 0..n {
            let (ox, oz) = slot(i as usize);
            let roll = self.s.track_rng.float();
            let kind = if i < brutes {
                E_BRUTE
            } else if roll < rifles && oz > 0.0 {
                E_RIFLE
            } else if roll < rifles + runners {
                E_RUNNER
            } else {
                E_GRUNT
            };
            self.spawn_enemy(
                kind,
                (x + ox * 1.6).clamp(-HALF + 0.4, HALF - 0.4),
                d + oz * 1.4,
                group,
                ES_WAIT,
            );
        }
        if hard >= 1.0 && self.chance(0.5) {
            let tx = (x + self.rand(-1.5, 1.5)).clamp(-HALF + 1.5, HALF - 1.5);
            self.add_thing(Thing {
                kind: K_TNT,
                x: tx,
                d: d - 4.5,
                hp: 3,
                hp_max: 3,
                ..NO_THING
            });
        }
        self.s.expect = after_group(e, n, hp).max(1.0);
        26.0
    }

    /// The counter over a group of n enemies at x, d; its index, or -1.
    fn lay_group(&mut self, x: f32, d: f32, n: i32) -> i32 {
        self.add_thing(Thing {
            kind: K_GROUP,
            x,
            d,
            value: n,
            ..NO_THING
        })
        .map_or(-1, |g| g as i32)
    }

    /// Damage a second the squad expected puts into what it aims at.
    fn dps(&self) -> f32 {
        squad_dps(self.s.expect.max(1.0), self.s.expect_gun)
    }

    /// Damage the squad expected puts into a thing it runs at, firing the
    /// in_range() seconds it gets: halfway, on a log scale, between what it
    /// does with the pistol and with the gun it is expected to have by now,
    /// so each better gun breaks things sooner.
    fn burst(&self) -> f32 {
        (squad_dps(self.s.expect.max(1.0), 0) * self.dps()).sqrt() * self.in_range()
    }

    /// Seconds of fire a squad running at a thing gets into it: from lining
    /// up a little inside its guns' range to stepping round it, less the
    /// bullets' flight.
    fn in_range(&self) -> f32 {
        ((WEAPONS[self.s.expect_gun as usize].range - 6.0) / self.s.speed - 0.4).max(1.0)
    }

    /// A gun crate, shot open for a better gun, sometimes with a barrel beside it.
    fn lay_crate(&mut self, d0: f32, hard: f32) -> f32 {
        let gun = (self.s.expect_gun + 1).min(4);
        // a share of that time, more on later levels
        let share = (0.35 + hard * 0.02).min(0.7);
        let hp = (4.0 + self.burst() * share).round() as i32;
        let x = if self.chance(0.5) { -SIDE_X } else { SIDE_X };
        self.add_thing(Thing {
            kind: K_CRATE,
            x,
            d: d0 + 8.0,
            value: gun,
            hp,
            hp_max: hp,
            ..NO_THING
        });
        if self.chance(0.6) {
            self.lay_barrel(-x, d0 + 8.0, hard);
        }
        self.s.expect_gun = gun;
        16.0
    }

    fn lay_barrels(&mut self, d0: f32, hard: f32) -> f32 {
        let x = self.rand(-HALF + 2.0, HALF - 2.0);
        self.lay_barrel(x, d0 + 6.0, hard);
        if self.chance(0.5) {
            let x2 = if x < 0.0 { x + 6.0 } else { x - 6.0 };
            if self.chance(0.4) && hard >= 2.0 {
                self.add_thing(Thing {
                    kind: K_TNT,
                    x: x2,
                    d: d0 + 9.0,
                    hp: 3,
                    hp_max: 3,
                    ..NO_THING
                });
            } else {
                self.lay_barrel(x2, d0 + 9.0, hard);
            }
        }
        14.0
    }

    fn lay_barrel(&mut self, x: f32, d: f32, hard: f32) {
        let e = self.s.expect;
        let value = (3.0 + gains(e) * 0.25 + hard).round() as i32;
        let share = (0.3 + hard * 0.02).min(0.7);
        let hp = (3.0 + self.burst() * share).round() as i32;
        self.add_thing(Thing {
            kind: K_BARREL,
            x,
            d,
            value,
            hp,
            hp_max: hp,
            ..NO_THING
        });
        self.s.expect = (e + value as f32 * 0.8).min(MAX_COUNT as f32);
    }

    /// Holes in the deck, a narrow stretch, or a ship firing shells.
    fn lay_hazard(&mut self, d0: f32, hard: f32) -> f32 {
        let row0 = (d0 / TILE).ceil() * TILE + TILE;
        let roll = self.s.track_rng.below(if hard >= 5.0 {
            3
        } else if hard >= 3.0 {
            2
        } else {
            1
        });
        let hole = |p: &mut Self, lane: i32, lanes: i32, row: f32, rows: f32| {
            p.add_thing(Thing {
                kind: K_HOLE,
                x: -HALF + lane as f32 * TILE,
                w: lanes as f32 * TILE,
                d: row,
                len: rows * TILE,
                ..NO_THING
            });
        };
        // what comes next starts 7.5 m past the last hole
        let past = |end: f32| end - d0 + 7.5;
        let len = match roll {
            0 => {
                // holes to steer round, half the pier wide: one side, then the other
                let first = self.s.track_rng.below(3) as i32;
                hole(self, first, 2, row0, 1.0);
                let second = match first {
                    0 => 2,
                    2 => 0,
                    _ if self.chance(0.5) => 0,
                    _ => 2,
                };
                hole(self, second, 2, row0 + TILE * 3.0, 1.0);
                if hard >= 4.0 && self.chance(0.5) {
                    hole(self, first, 2, row0 + TILE * 6.0, 1.0);
                    past(row0 + TILE * 7.0)
                } else {
                    past(row0 + TILE * 4.0)
                }
            }
            1 => {
                // a narrow stretch, that may step aside half way
                let rows = if hard >= 5.0 { 3.0 } else { 2.0 };
                let open = if hard >= 7.0 { 1 } else { 2 };
                let lane = self.s.track_rng.below((LANES - open + 1) as u32) as i32;
                let side = |p: &mut Self, lane: i32, row: f32, rows: f32| {
                    if lane > 0 {
                        hole(p, 0, lane, row, rows);
                    }
                    if lane + open < LANES {
                        hole(p, lane + open, LANES - lane - open, row, rows);
                    }
                };
                side(self, lane, row0, rows);
                if open > 1 && hard >= 5.0 && self.chance(0.6) {
                    // by less than its width, so the two stretches meet
                    let shift = 1 + self.s.track_rng.below(open as u32 - 1) as i32;
                    let next = if lane + shift > LANES - open || (lane >= shift && self.chance(0.5))
                    {
                        lane - shift
                    } else {
                        lane + shift
                    };
                    side(self, next, row0 + rows * TILE, rows);
                    past(row0 + rows * 2.0 * TILE)
                } else {
                    past(row0 + rows * TILE)
                }
            }
            _ => {
                // a ship off the pier, firing shells at the squad as it passes
                let x = if self.chance(0.5) {
                    -HALF - 8.0
                } else {
                    HALF + 8.0
                };
                self.add_thing(Thing {
                    kind: K_SHIP,
                    x,
                    d: d0 + 18.0,
                    cool: 0.8,
                    ..NO_THING
                });
                if self.chance(0.5) {
                    let bx = -x.signum() * SIDE_X;
                    self.lay_barrel(bx, d0 + 12.0, hard);
                }
                30.0
            }
        };
        self.s.expect *= 0.9;
        len
    }

    /// The end of a level: the squad stops at arena_d and fights the boss,
    /// and the boss's army stands in a wedge beside it, waiting.
    fn lay_arena(&mut self) {
        let d0 = self.s.next_d;
        let arena = d0 + 6.0;
        self.s.arena_d = arena;
        self.add_thing(Thing {
            kind: K_ARENA,
            d: arena,
            len: 30.0,
            ..NO_THING
        });
        let hard = self.hard_at();
        let side = if self.chance(0.5) { -1.0 } else { 1.0 };
        let e = self.s.expect.max(3.0);
        // the army beside the boss: rows from its point towards the squad, one more each row up to ARMY_ROW
        let n = ((e * (0.6 + hard * 0.015).min(0.9)).round() as i32 + 6).clamp(10, 160);
        // what is left of the groups before it still stands, and the boss waves more on
        let n = n.min(MAXE as i32 - self.s.nen - 24).max(0);
        let ax = -side * 3.0;
        let group = self.lay_group(ax, arena + 9.0, n);
        let (mut row, mut k) = (0, 0);
        while k < n {
            let wide = (row + 1).min(ARMY_ROW);
            for i in 0..wide {
                if k >= n {
                    break;
                }
                let x = ax + (i as f32 - (wide - 1) as f32 * 0.5) * 0.85;
                let kind = if row >= 3 && hard >= 3.0 && i % 5 == 2 {
                    E_RIFLE
                } else if hard >= 5.0 && row >= 5 && i == 0 {
                    E_BRUTE
                } else {
                    E_GRUNT
                };
                self.spawn_enemy(
                    kind,
                    x.clamp(-HALF + 0.4, HALF - 0.4),
                    arena + 6.0 + row as f32 * 0.75,
                    group,
                    ES_WAIT,
                );
                k += 1;
            }
            row += 1;
        }
        // the endless pier goes on past the boss for the squad left
        self.s.expect = after_group(e, n, enemy_hp(E_GRUNT, hard)).max(3.0);
        let toughness = if self.s.endless() {
            0.6 + hard * 0.035
        } else {
            (0.4 + hard * 0.04).min(1.2)
        };
        let hp = (BOSS_SECONDS * toughness * self.dps()).round().max(30.0) as i32;
        let home = arena + BOSS_HOME;
        self.s.boss = Boss {
            state: 1,
            kind: boss_kind(self.s.level),
            x: side * 4.5,
            d: home,
            hp,
            hp_max: hp,
            flinch_at: hp * 3 / 4,
            ..NO_BOSS
        };
        self.s.next_d = arena + 32.0;
    }
}

/// What sums on gates and barrels are sized by for a squad of e: e itself up
/// to a hundred, then ten times its square root, so a big squad still grows
/// by them but not past everything that thins it out.
fn gains(e: f32) -> f32 {
    e.min(10.0 * e.sqrt())
}

/// The squad expected of e after a group of n, each taking hp damage, comes
/// at it: it loses about those that reach it, as the rest fall to its fire first.
fn after_group(e: f32, n: i32, hp: i32) -> f32 {
    (e - n as f32 * reach(hp)).max(e * 0.45)
}

/// The share of a group of the tan army, each taking hp damage, that gets to the squad.
fn reach(hp: i32) -> f32 {
    1.0 - 0.55 / hp as f32
}
