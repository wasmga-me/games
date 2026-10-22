//! Laying the pier ahead of the squad, a chunk at a time: gate pairs, enemy
//! groups, barrels, gun crates, ships firing from the sea, and the boss in
//! its crowd that ends a level. A level is laid from its own seed, so it is
//! the same every time; the endless pier from the run's.
//!
//! As it lays the pier it keeps `expect`, the squad a player who picks well
//! would have by then, and sizes enemies, barrels and the boss by it.
use crate::*;

/// how far ahead of the squad the pier is laid
const AHEAD: f32 = 95.0;
/// seconds a squad of the size expected takes to beat a boss
const BOSS_SECONDS: f32 = 13.0;
/// how far ahead of the plan a squad gets before the pier ahead is laid for
/// it, and how many times a squad far behind the plan is laid for
const SLACK: f32 = 1.25;
const BEHIND: f32 = 2.0;
/// seconds of the squad's fire a group of the tan army soaks up on its way in
const FIGHT: f32 = 2.0;
/// a gate pair: each gate half the pier wide
const GATE_X: f32 = HALF * 0.5;
const GATE_W: f32 = HALF - 0.6;
/// where crates and things on one side stand
const SIDE_X: f32 = HALF * 0.5;
/// how far out a ship sails, in water scene.rs put_decor() leaves clear
const SHIP_X: f32 = HALF + 5.0;
/// how far apart the boss's crowd stands, across and along the pier, and
/// the least room it leaves round the boss
const CROWD_X: f32 = 0.95;
const CROWD_D: f32 = 0.9;
const CROWD_GAP: f32 = 2.6;

const C_GATES: i32 = 0;
const C_ENEMY: i32 = 1;
const C_CRATE: i32 = 2;
const C_BARREL: i32 = 3;
const C_SHIP: i32 = 4;

impl Combat {
    pub fn lay_track(&mut self) {
        while self.s.stop_d == 0.0 && self.s.next_d < self.s.d + AHEAD {
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
        // the plan, past the last chunk the squad got through, is kept from
        // SLACK under the squad to BEHIND times it: a squad doing better gets
        // a pier laid for it, and one far behind still has a chance
        let s = &mut self.s;
        let past = s.plan_at.iter().filter(|p| p[0] > 0.0 && p[0] <= s.d);
        if let Some(p) = past.max_by(|a, b| a[0].total_cmp(&b[0])).copied() {
            let n = s.count.max(1) as f32;
            let lift = (n / (p[1] * SLACK)).max(1.0).min(n * BEHIND / p[1]);
            s.expect = (s.expect * lift).min(MAX_COUNT as f32);
            for p in &mut s.plan_at {
                p[1] *= lift;
            }
        }
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
            self.lay_boss();
            self.s.plan_at[c as usize % PLANS] = [self.s.next_d, self.s.expect];
            return;
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
                    if hard >= 3.0 && self.chance((0.3 + hard * 0.03).min(0.8)) {
                        C_SHIP
                    } else if hard >= 1.0 {
                        C_ENEMY
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
            _ => self.lay_ship(d0, hard),
        };
        self.s.next_d = d0 + len;
        self.s.plan_at[c as usize % PLANS] = [self.s.next_d, self.s.expect];
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
        } else if hard >= 5.0 && e >= 40.0 && self.chance(0.1 + hard * 0.012) {
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
        let e = self.s.expect.max(1.0);
        // the share of the squad a group costs, more on later levels
        let loss = (0.19 + hard * 0.016).min(0.5);
        // the endless pier's groups keep growing past what any squad holds off
        let most = if self.s.endless() {
            (70.0 + hard * 1.5).min(110.0) as i32
        } else {
            70
        };
        let n = ((e * loss / share(e) * 2.0).round() as i32 + 2).clamp(3, most);
        let x = self.rand(-HALF * 0.4, HALF * 0.4);
        let d = d0 + 14.0;
        // each kind's share of the group, from the level it turns up; riflemen
        // and grenadiers first, as they stand at the back
        let mix = [
            (E_RIFLE, 4.0, 0.15),
            (E_GRENADIER, 7.0, 0.08),
            (E_HEAVY, 3.0, 0.1),
            (E_RUNNER, 2.0, 0.2),
        ]
        .map(|(kind, from, odds)| (kind, if hard >= from { odds } else { 0.0 }));
        // a brute in a small group, more in a big one, but none against a
        // squad that one would cut down to nothing
        let brutes = if hard >= 5.0 {
            (1 + n / 30 + (hard / 12.0) as i32).min((e / 20.0) as i32)
        } else {
            0
        };
        let first = self.s.nen as usize;
        for i in 0..n {
            let (ox, oz) = slot(i as usize);
            let mut roll = self.s.track_rng.float();
            let kind = if i < brutes {
                E_BRUTE
            } else {
                let mut kinds = mix.iter();
                if oz <= 0.0 {
                    kinds.nth(1);
                }
                kinds
                    .find(|&&(_, odds)| {
                        roll -= odds;
                        roll < 0.0
                    })
                    .map_or(E_GRUNT, |k| k.0)
            };
            let ex = (x + ox * 1.6).clamp(-HALF + 0.4, HALF - 0.4);
            self.spawn_enemy(kind, ex, d + oz * 1.4, ES_WAIT);
        }
        let lost = self.send_army(first, e * loss);
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
        self.s.expect = (e - lost).max(1.0);
        26.0
    }

    /// Sizes the army put down from first on to cost the squad expected about
    /// `cost` soldiers: as many of it get through as that takes (from one in
    /// twenty to four in five), each made tough enough that the rest fall to
    /// the squad's fire on the way in. Returns what it should cost.
    fn send_army(&mut self, first: usize, cost: f32) -> f32 {
        let army = first..self.s.nen as usize;
        let stake = self.s.en[army.clone()]
            .iter()
            .map(|e| worth(e.kind))
            .sum::<i32>() as f32
            * share(self.s.expect);
        let through = (cost / stake.max(1.0)).clamp(0.05, 0.8);
        let meant = self.dps();
        self.toughen(first, meant * FIGHT / (1.0 - through));
        for e in &mut self.s.en[army] {
            e.meant = meant;
        }
        through * stake
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

    /// A ship off the pier, firing shells at the squad as it passes.
    fn lay_ship(&mut self, d0: f32, hard: f32) -> f32 {
        let x = if self.chance(0.5) { -SHIP_X } else { SHIP_X };
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
        self.s.expect *= 0.9;
        30.0
    }

    /// The end of a level: the boss comes on in the middle of its crowd, and
    /// the squad stops at stop_d to fight them.
    fn lay_boss(&mut self) {
        let stop = self.s.next_d + 6.0;
        self.s.stop_d = stop;
        let hard = self.hard_at();
        let e = self.s.expect.max(3.0);
        let cost = e * (0.3 + hard * 0.012).min(0.55);
        let n = ((cost / share(e) * 2.0).round() as i32 + 6).clamp(10, 160);
        // what is left of the groups before it still stands, and the boss waves more on
        let n = n.min(MAXE as i32 - self.s.nen - 24).max(0);
        // in rows across a stretch of the pier the wider the more there are,
        // as far in front of the boss as behind it, with room left round it
        let home = stop + BOSS_HOME;
        let wide = ((n as f32).sqrt() * 1.6).clamp(6.0, HALF * 2.0 - 1.0);
        let bx = self.rand(-1.0, 1.0) * (HALF - 0.5 - wide * 0.5).min(3.0);
        let half = (wide * 0.5 / CROWD_X) as i32;
        // clear of its legs, which stand about half its width out
        let m = &BODIES[boss_kind(self.s.level) as usize];
        let clear = (m.width * m.scale * 0.5 + 1.0).max(CROWD_GAP);
        let gap = (core::f32::consts::PI * clear * clear / (CROWD_X * CROWD_D)) as i32;
        let rows = (n + gap) / (half * 2 + 1) + 1;
        let front = (home - rows as f32 * CROWD_D * 0.5).max(stop + 6.0);
        let first = self.s.nen as usize;
        let (mut row, mut left) = (0, n);
        while left > 0 {
            let d = front + row as f32 * CROWD_D;
            // from the middle out, so a row part filled stays round the boss
            for j in 0..half * 2 + 1 {
                if left == 0 {
                    break;
                }
                let k = if j % 2 == 0 { j / 2 } else { -(j + 1) / 2 };
                let x = bx + k as f32 * CROWD_X;
                if (x - bx).hypot(d - home) < clear {
                    continue;
                }
                let kind = if row >= 3 && hard >= 3.0 && k.rem_euclid(5) == 2 {
                    E_RIFLE
                } else if row >= 3 && hard >= 7.0 && k.rem_euclid(6) == 4 {
                    E_GRENADIER
                } else if hard >= 5.0 && row >= 2 && k.abs() == half {
                    E_BRUTE
                } else if row >= 1 && hard >= 3.0 && k.rem_euclid(4) == 1 {
                    E_HEAVY
                } else {
                    E_GRUNT
                };
                let ex = (x + self.rand(-0.2, 0.2)).clamp(-HALF + 0.4, HALF - 0.4);
                let ed = d + self.rand(-0.25, 0.25);
                self.spawn_enemy(kind, ex, ed, ES_WAIT);
                left -= 1;
            }
            row += 1;
        }
        let lost = self.send_army(first, cost);
        // the endless pier goes on past the boss for the squad left
        self.s.expect = (e - lost).max(3.0);
        let toughness = if self.s.endless() {
            0.6 + hard * 0.035
        } else {
            (0.52 + hard * 0.042).min(1.3)
        };
        let hp = (BOSS_SECONDS * toughness * self.dps()).round().max(30.0) as i32;
        self.s.boss = Boss {
            state: 1,
            kind: boss_kind(self.s.level),
            x: bx,
            d: home,
            hp,
            hp_max: hp,
            flinch_at: hp * 3 / 4,
            meant: self.dps(),
            ..NO_BOSS
        };
        // the endless pier's next stretch starts past the crowd
        self.s.next_d = home + 20.0;
    }
}

/// What sums on gates and barrels are sized by for a squad of e: e itself up
/// to a hundred, then ten times its square root, so a big squad still grows
/// by them but not past everything that thins it out.
fn gains(e: f32) -> f32 {
    e.min(10.0 * e.sqrt())
}

/// How many soldiers each one shown of a squad of e stands for.
fn share(e: f32) -> f32 {
    (e / shooters(e)).max(1.0)
}
