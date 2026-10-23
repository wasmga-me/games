//! Laying the pier ahead of the squad, a chunk at a time: gate pairs, the
//! tan army in ranks with mechs and giants among it, barrels, gun crates
//! and ships firing from the sea, to the biggest army that ends a level. A
//! level is laid from its own seed, so it is the same every time; the
//! endless pier from the run's.
//!
//! As it lays the pier it keeps `expect`, the squad a player who picks well
//! would have by then, and sizes everything on it by that.
use crate::*;

/// how far ahead of the squad the pier is laid
const AHEAD: f32 = 95.0;
/// seconds of the squad expected's fire a mech or giant at the end of a
/// level soaks up, and how strong one in an army before is against that
const BIG_SECONDS: f32 = 16.0;
const LESSER: f32 = 0.25;
/// how many soldiers a mech or giant knocks out before it falls to its
/// stamps' toll alone, as a share of the squad expected to meet it: most
/// of that squad, so it has to be shot as it comes; and the share of that
/// squad the plan counts one costing a squad that shoots it
const PLAN_SHARE: f32 = 0.8;
const BIG_TOLL: f32 = 0.2;
/// how far ahead of the plan a squad gets before the pier ahead is laid for
/// it, and how many times a squad far behind the plan is laid for
const SLACK: f32 = 1.25;
const BEHIND: f32 = 2.0;
/// seconds of the squad's fire an army soaks up on its way in, and how
/// many more on each level harder
const FIGHT: f32 = 1.4;
const FIGHT_UP: f32 = 0.045;
/// metres more room after a level's opening gates on each level harder, so
/// the squad they make opens fire before an army running faster and
/// tougher there reaches it
const OPENING: f32 = 0.6;
/// the most an army puts on the pier, a level's and far down the endless pier's
const ARMY: f32 = 120.0;
const ARMY_FAR: f32 = 220.0;
/// a gate pair: each gate half the pier wide
const GATE_X: f32 = HALF * 0.5;
const GATE_W: f32 = HALF - 0.6;
/// where crates and things on one side stand
const SIDE_X: f32 = HALF * 0.5;
/// how far out a ship stands off the pier, in water scene.rs put_decor() leaves clear
const SHIP_X: f32 = HALF + 8.0;
/// how far apart the tan army stands in its ranks, across and along the
/// pier, and the least room it leaves round a mech or giant
const RANK_X: f32 = 1.5;
const RANK_D: f32 = 1.4;
const CROWD_GAP: f32 = 2.6;

const C_GATES: i32 = 0;
const C_ENEMY: i32 = 1;
const C_CRATE: i32 = 2;
const C_BARREL: i32 = 3;
const C_SHIP: i32 = 4;

impl Combat {
    /// Lays the pier on ahead, to a level's last army or on and on.
    pub fn lay_track(&mut self) {
        while self.s.end_d == 0.0 && self.s.next_d < self.s.d + AHEAD {
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
        if let Some(p) = s.plan_by(s.d) {
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
        // the last stretch before a level's last army, or the endless pier's
        // every so often, is a gate pair for one more boost
        let kind = if k >= total {
            C_ENEMY
        } else if k == total - 1 {
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
            C_GATES if c == 0 => self.lay_gates(d0, hard, true) + hard * OPENING,
            C_GATES => self.lay_gates(d0, hard, k == total - 1),
            C_ENEMY => {
                // a mech or giant in a level's last army, and a lesser one
                // in more of the rest later on, once the squad expected is
                // big enough to take one, leaving a place on the pier for
                // the last one
                let free = self.s.big.iter().filter(|b| b.state == BIG_NONE).count();
                let big = if k >= total {
                    1.0
                } else if self.s.expect >= 30.0
                    && free > 1
                    && self.chance((0.15 + hard * 0.03).min(0.6))
                {
                    LESSER
                } else {
                    0.0
                };
                self.lay_enemies(d0, hard, big)
            }
            C_CRATE => self.lay_crate(d0, hard),
            C_BARREL => self.lay_barrels(d0, hard),
            _ => self.lay_ship(d0, hard),
        };
        self.s.next_d = d0 + len;
        self.plan(c, self.s.next_d);
        if k >= total && !self.s.endless() {
            self.s.end_d = self.s.next_d;
        }
    }

    /// Notes what the squad is expected to be past chunk c, which ends at d.
    fn plan(&mut self, c: i32, d: f32) {
        let s = &mut self.s;
        s.plan_at[c as usize % PLANS] = [d, s.expect, s.expect_gun as f32];
    }

    /// Two gates side by side: a sum that helps and one that doesn't, or two
    /// that help unequally, as both do if `boost`.
    fn lay_gates(&mut self, d0: f32, hard: f32, boost: bool) -> f32 {
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
        // a level opens with two gates that both help, so a wrong guess there
        // isn't the end, and the last before a level's last army is one more boost
        let pair = if boost {
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

    /// An army across the pier, as many as the squad expected cuts down on
    /// its way in, sometimes behind a TNT barrel; with a mech or giant in it
    /// as strong as `big` of the strongest, if more than 0 and one more fits
    /// on the pier, the army then bigger to match.
    fn lay_enemies(&mut self, d0: f32, hard: f32, big: f32) -> f32 {
        let e = self.s.expect.max(1.0);
        let slot = self
            .s
            .big
            .iter()
            .position(|b| b.state == BIG_NONE)
            .filter(|_| big > 0.0);
        let big = if slot.is_some() { big } else { 0.0 };
        // the share of the squad an army costs, more on later levels
        let loss = (0.19 + hard * 0.016).min(0.5);
        let loss = loss + ((0.3 + hard * 0.012).min(0.55) - loss) * big;
        // the endless pier's armies keep growing past what any squad holds
        // off, as far as there is room beside what is left of those before
        let most = if self.s.endless() {
            (ARMY + hard * 3.0).min(ARMY_FAR)
        } else {
            ARMY
        };
        let room = (MAXE as i32 - self.s.nen - 8).max(0) as f32;
        let (most, fight) = (
            most * (1.0 + big),
            (FIGHT + hard * FIGHT_UP) * (1.0 + 0.3 * big),
        );
        let n = self.army_size(hard, fight, most.min(room));
        // room left round a mech's or giant's legs, which stand about half its width out
        let kind = big_kind(self.s.level + self.s.chunk);
        let clear = slot.map_or(0.0, |_| (BODIES[kind as usize].half() + 1.0).max(CROWD_GAP));
        let x = self.rand(-HALF * 0.4, HALF * 0.4);
        let front = d0 + 14.0;
        let first = self.s.nen as usize;
        let (x, home, deep) = self.lay_army(n, x, front, clear, hard);
        let lost = self.send_army(first, e * loss, fight);
        if hard >= 1.0 && self.chance(0.5) {
            let tx = (x + self.rand(-1.5, 1.5)).clamp(-HALF + 1.5, HALF - 1.5);
            self.add_thing(Thing {
                kind: K_TNT,
                x: tx,
                d: front - 4.5,
                hp: 3,
                hp_max: 3,
                ..NO_THING
            });
        }
        self.s.expect = (e - lost).max(1.0);
        if let Some(i) = slot {
            self.lay_big(i, kind, x, home, hard, big);
            self.s.expect = (self.s.expect - e * big * BIG_TOLL).max(1.0);
        }
        front - d0 + deep + 8.0
    }

    /// A mech or giant of kind at x, d in slot i, as tough as `big` of
    /// BIG_SECONDS of the fire of the squad expected past its army.
    fn lay_big(&mut self, i: usize, kind: i32, x: f32, d: f32, hard: f32, big: f32) {
        let toughness = if self.s.endless() {
            0.6 + hard * 0.035
        } else {
            (0.8 + hard * 0.025).min(1.3)
        };
        let hp = (BIG_SECONDS * big * toughness * self.dps())
            .round()
            .max(30.0) as i32;
        let e = self.s.expect * big;
        self.s.big[i] = Big {
            state: BIG_WAIT,
            kind,
            x,
            d,
            hp,
            hp_max: hp,
            worth: ((e * PLAN_SHARE / share(e)).round() as i32).max(3),
            flinch_at: hp * 3 / 4,
            meant: self.s.planned_fire(d - THIN),
            ..NO_BIG
        };
    }

    /// How many an army numbers: as many as soak up `fight` seconds of the
    /// squad expected's fire with the hp they have, `most` at most.
    fn army_size(&self, hard: f32, fight: f32, most: f32) -> usize {
        let grunt = enemy_hp(E_GRUNT, hard) as f32;
        let hp = grunt
            + mix(hard)
                .iter()
                .map(|&(kind, odds)| odds * (enemy_hp(kind, hard) as f32 - grunt))
                .sum::<f32>();
        (self.dps() * fight / hp).round().max(8.0).min(most) as usize
    }

    /// Puts down an army of n in ranks from `front` on back, round x, as wide
    /// as their number calls for: brutes in the middle of its front, the rest
    /// by mix(), each rank from its middle out, with room left in the middle
    /// for a mech or giant if `clear` > 0. Returns its middle, where one stands
    /// in it, and how deep it stands.
    fn lay_army(&mut self, n: usize, x: f32, front: f32, clear: f32, hard: f32) -> (f32, f32, f32) {
        let wide = ((n as f32).sqrt() * RANK_X * 1.2).clamp(5.0, HALF * 2.0 - 1.0);
        let half = (wide * 0.5 / RANK_X) as i32;
        let per = half * 2 + 1;
        let gap = (core::f32::consts::PI * clear * clear / (RANK_X * RANK_D)) as i32;
        let rows = (n as i32 + gap + per - 1) / per;
        let side = HALF - 0.5 - half as f32 * RANK_X;
        let x = x.clamp(-side, side);
        let home = front + (rows - 1) as f32 * RANK_D * 0.5;
        // a brute in a small army, more in a big one, but none against a
        // squad that one would cut down to nothing
        let brutes = if hard >= 2.0 {
            (1 + n / 30 + (hard / 12.0) as usize).min((self.s.expect / 20.0) as usize)
        } else {
            0
        };
        let mix = mix(hard);
        let (mut row, mut laid) = (0, 0);
        while laid < n {
            let d = front + row as f32 * RANK_D;
            for j in 0..per {
                if laid == n {
                    break;
                }
                let k = if j % 2 == 0 { j / 2 } else { -(j + 1) / 2 };
                let px = x + k as f32 * RANK_X;
                if (px - x).hypot(d - home) < clear {
                    continue;
                }
                let mut roll = self.s.track_rng.float();
                let kind = if laid < brutes {
                    E_BRUTE
                } else {
                    // riflemen and grenadiers only in the back half
                    let mut kinds = mix.iter();
                    if row * 2 < rows {
                        kinds.nth(1);
                    }
                    kinds
                        .find(|&&(_, odds)| {
                            roll -= odds;
                            roll < 0.0
                        })
                        .map_or(E_GRUNT, |k| k.0)
                };
                let ex = (px + self.rand(-0.2, 0.2)).clamp(-HALF + 0.4, HALF - 0.4);
                let ed = d + self.rand(-0.25, 0.25);
                self.spawn_enemy(kind, ex, ed);
                laid += 1;
            }
            row += 1;
        }
        (x, home, row as f32 * RANK_D)
    }

    /// Sizes the army put down from first on to cost the squad expected about
    /// `cost` soldiers: as many of it get through as that takes (from one in
    /// fifty to four in five), each made tough enough that the rest fall to
    /// `fight` seconds of the squad's fire on the way in. Returns what it
    /// should cost.
    fn send_army(&mut self, first: usize, cost: f32, fight: f32) -> f32 {
        let army = first..self.s.nen as usize;
        let stake = self.s.en[army.clone()]
            .iter()
            .map(|e| worth(e.kind))
            .sum::<i32>() as f32
            * share(self.s.expect);
        let through = (cost / stake.max(1.0)).clamp(0.02, 0.8);
        self.toughen(first, self.dps() * fight / (1.0 - through));
        for k in army {
            self.s.en[k].meant = self.s.planned_fire(self.s.en[k].d - THIN);
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
        // a barrel across the pier from it is the other pick: the plan takes the gun
        if self.chance(0.6) {
            let e = self.s.expect;
            self.lay_barrel(-x, d0 + 8.0, hard);
            self.s.expect = e;
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
}

/// Each kind's share of an army where the pier is that hard: every kind
/// from the first level, half its share there and all of it by the sixth;
/// riflemen and grenadiers first, as they stand at the back.
fn mix(hard: f32) -> [(i32, f32); 4] {
    let k = (0.4 + hard * 0.1).min(1.0);
    [
        (E_RIFLE, 0.15),
        (E_GRENADIER, 0.08),
        (E_HEAVY, 0.1),
        (E_RUNNER, 0.2),
    ]
    .map(|(kind, odds)| (kind, odds * k))
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
