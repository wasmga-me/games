//! Laying the pier ahead of the squad, a beat at a time: rows of gates a
//! lane wide, buffs, ships firing from the sea, and down all three lanes the
//! enemy army marching on the squad without a break, mechs and giants among
//! it, the strongest in a level's last stretch. A level is laid from its own
//! seed, so it is the same every time; the endless pier from the run's.
//!
//! As it lays the pier it keeps `expect`, the squad a player who picks well
//! would have by then, and sizes everything on it by that. The army is laid
//! by where it meets the squad: each soldier starts as much further on as it
//! marches while the squad runs there, so it comes when the plan has it.
use crate::*;

/// how far ahead of the squad the pier is laid
const AHEAD: f32 = 95.0;
/// a beat of the pier: its share of the army, then at most one of a gate
/// row, a buff or a ship; a level's stretches of STRETCH beats, then LAST
/// beats with the strongest mech or giant
const BEAT: f32 = 10.0;
const STRETCH: i32 = 5;
const LAST: i32 = 3;
/// seconds of the squad expected's fire a mech or giant at the end of a
/// level soaks up, and how strong one in the stretches before is against that
const BIG_SECONDS: f32 = 5.0;
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
/// the share of the squad's fire as it runs a beat that the beat's army
/// soaks up on its way in, how much less on each level harder (where it
/// comes faster and fires more), and how much more each 100 m down the
/// endless pier, so no squad holds it off for ever
const FIGHT: f32 = 0.4;
const FIGHT_DOWN: f32 = 0.003;
const FIGHT_FAR: f32 = 0.02;
/// the share of the squad expected a beat's army costs, how much more on
/// each level harder, the most, and how much more a mech's or giant's beat costs
const LOSS: f32 = 0.08;
const LOSS_UP: f32 = 0.004;
const LOSS_MOST: f32 = 0.2;
const BIG_LOSS: f32 = 0.15;
/// metres more room after a level's opening gates on each level harder, so
/// the squad they make opens fire before an army running faster and
/// tougher there reaches it
const OPENING: f32 = 0.6;
/// the most a beat's army numbers, a level's and far down the endless pier's, and the least
const ARMY: f32 = 30.0;
const ARMY_FAR: f32 = 40.0;
const ARMY_LEAST: f32 = 9.0;
/// a gate: a lane wide, less room between gates
const GATE_W: f32 = LANE_W - 0.5;
/// how far out a ship stands off the pier, in water scene.rs put_decor() leaves clear
const SHIP_X: f32 = HALF + 8.0;
/// the least room the army leaves round a mech or giant
const CROWD_GAP: f32 = 2.6;
/// the most riflemen or grenadiers in a block of a lane's army, and the
/// squad expected before any come
const FEW: usize = 3;
const FIRING_FROM: f32 = 30.0;

impl Combat {
    /// Lays the pier on ahead, on and on: past a level's end the army keeps
    /// coming till the level is won.
    pub fn lay_track(&mut self) {
        while self.s.next_d < self.s.d + AHEAD {
            self.lay_beat();
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
    fn lane(&mut self) -> i32 {
        self.s.track_rng.below(3) as i32
    }

    /// Puts a thing on the pier, unless the list is full.
    fn add_thing(&mut self, t: Thing) {
        let n = self.s.nthing as usize;
        if n < MAXO {
            self.s.thing[n] = t;
            self.s.nthing += 1;
        }
    }

    fn lay_beat(&mut self) {
        // the plan, past the last beat the squad got through, is kept from
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
        let c = self.s.beat;
        self.s.beat += 1;
        let stretches = if self.s.endless() {
            4
        } else {
            4 + (self.s.level / 3).min(3)
        };
        let total = stretches * STRETCH;
        let k = if self.s.endless() {
            c % (total + LAST)
        } else {
            c
        };
        let d0 = self.s.next_d;
        // a level opens with gates that all help, and its army comes on past
        // them, with room for the squad they make to open fire; each round of
        // the endless pier opens with them too, the army still coming
        if k == 0 {
            self.lay_gates(c, d0 + 4.0, hard, true);
        }
        let (from, len) = if c == 0 {
            let from = d0 + 8.0 + hard * OPENING;
            (from, from - d0 + BEAT * 0.5)
        } else {
            (d0, BEAT)
        };
        // the army down every lane, sized for the squad expected so far: with
        // the strongest mech or giant at the start of the last stretch, and
        // later on a lesser one now and then, once the squad expected is big
        // enough to take one, one at a time and none just before the last
        let alone = self.s.big.iter().all(|b| b.state == BIG_NONE);
        let big = if k == total {
            1.0
        } else if k < total - STRETCH
            && self.s.expect >= 30.0
            && alone
            && self.chance((0.045 + hard * 0.009).min(0.18))
        {
            LESSER
        } else {
            0.0
        };
        self.lay_army(from, d0 + len, hard, big);
        // then what the beat holds: gates each stretch, the last before the
        // last stretch one more boost; a buff, a gun now and then; a ship
        let at = d0 + len - 2.0;
        match k % STRETCH {
            _ if k == 0 || k >= total => {}
            0 => self.lay_gates(c, at, hard, k == total - STRETCH),
            2 => {
                // a gun in the first stretch, the third from level 4 and the sixth from level 10
                let st = k / STRETCH;
                let gun = st == 0 || (st == 2 && hard >= 3.0) || (st == 5 && hard >= 9.0);
                self.lay_buff(at, hard, gun);
            }
            4 if hard >= 3.0 && self.chance((0.2 + hard * 0.02).min(0.5)) => self.lay_ship(at),
            _ => {}
        }
        self.s.next_d = d0 + len;
        self.plan(c, self.s.next_d);
        if k == total + LAST - 1 && !self.s.endless() {
            self.s.end_d = self.s.next_d;
        }
    }

    /// Notes what the squad is expected to be past beat c, which ends at d.
    fn plan(&mut self, c: i32, d: f32) {
        let s = &mut self.s;
        s.plan_at[c as usize % PLANS] = [d, s.expect, s.expect_gun as f32];
    }

    /// A row of gates at d, one in a lane, as row `row`: a sum that helps
    /// and one that doesn't, or two that help unequally, sometimes a third
    /// that doesn't; on later levels now and then three that don't; three
    /// that help if `boost`.
    fn lay_gates(&mut self, row: i32, d: f32, hard: f32, boost: bool) {
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
        let third = |p: &mut Self, odds: f32| p.chance(odds).then(|| bad(p));
        // a level opens with gates that all help, so a wrong guess there
        // isn't the end, and the last before its last stretch is one more boost
        let mut sums = if boost {
            let mid = if e >= 3.0 { mul(self) } else { add(self) };
            [Some(add(self)), Some(mid), Some(add(self))]
        } else if self.chance((0.55 - hard * 0.02).max(0.3)) {
            [Some(good(self)), Some(bad(self)), third(self, 0.4)]
        } else if hard >= 5.0 && e >= 40.0 && self.chance(0.1 + hard * 0.012) {
            // none that help: take the least bad, or shoot one up
            [Some(sub(self)), Some(div(self)), Some(bad(self))]
        } else {
            [Some(add(self)), Some(mul(self)), third(self, 0.5)]
        };
        self.s.track_rng.shuffle(&mut sums);
        // the squad expected, firing at a gate all the way in, counts it pts,
        // a share of its gains, less on later levels; a squad that fires
        // harder counts it at most twice that
        let pts = (1.0 - hard * 0.04).max(0.25) * g;
        let gate = Thing {
            kind: K_GATE,
            w: GATE_W,
            b: row,
            hp: ((self.burst() / pts).round() as i32).max(1),
            hp_max: (pts * 2.0).round() as i32,
            ..NO_THING
        };
        // a row's gates needn't stand level: from the second level on, now
        // and then each a little further on than the next
        let stagger = if hard >= 1.0 && self.chance(0.4) {
            3.0
        } else {
            0.0
        };
        for (lane, sum) in sums.iter().enumerate() {
            if let Some((op, v)) = *sum {
                let d = d + self.rand(0.0, stagger);
                self.add_thing(Thing {
                    x: lane_x(lane as i32),
                    d,
                    a: op,
                    value: v,
                    ..gate
                });
            }
        }
        // the size expected after the row, unrounded (gate_sum rounds): the
        // best gate, or an open lane; a + gate counted up by about half what
        // shooting it all the way in would
        let apply = |(op, v): (i32, i32)| match op {
            OP_ADD => e + v as f32 + pts * 0.5,
            OP_SUB => e - v as f32,
            OP_MUL => e * v as f32,
            _ => e / v as f32,
        };
        let best = sums
            .iter()
            .map(|s| s.map_or(e, apply))
            .fold(f32::MIN, f32::max);
        self.s.expect = (best * 0.95).clamp(1.0, MAX_COUNT as f32);
    }

    /// The army that meets the squad from `from` to `to`, down every lane, as
    /// many as the squad expected cuts down as it runs there; with a mech or
    /// giant in it as strong as `big` of the strongest, if more than 0 and one
    /// more fits on the pier, the army then bigger to match.
    fn lay_army(&mut self, from: f32, to: f32, hard: f32, big: f32) {
        let e = self.s.expect.max(1.0);
        let slot = self
            .s
            .big
            .iter()
            .position(|b| b.state == BIG_NONE)
            .filter(|_| big > 0.0);
        let big = if slot.is_some() { big } else { 0.0 };
        let span = (to - from) / BEAT;
        // the share of the squad it costs, more on later levels
        let loss = ((LOSS + hard * LOSS_UP).min(LOSS_MOST) + BIG_LOSS * big) * span;
        // the endless pier's army keeps growing past what any squad holds
        // off, as far as there is room beside what is left of the army before
        let most = if self.s.endless() {
            (ARMY + hard).min(ARMY_FAR)
        } else {
            ARMY
        };
        let room = (MAXE as i32 - self.s.nen - 8).max(0) as f32;
        let most = (most * span * (1.0 + big)).min(room);
        let up = if self.s.endless() {
            FIGHT_FAR
        } else {
            -FIGHT_DOWN
        };
        let fight = (FIGHT + hard * up) * (to - from) / self.s.speed * (1.0 + 0.3 * big);
        let n = self.army_size(hard, fight, ARMY_LEAST * span, most);
        // room left round a mech's or giant's legs, which stand about half its width out
        let kind = big_kind(self.s.level + self.s.beat);
        let clear = slot.map_or(0.0, |_| (BODIES[kind as usize].half() + 1.0).max(CROWD_GAP));
        let (bx, bd) = (lane_x(self.lane()), (from + to) * 0.5);
        let first = self.s.nen as usize;
        for lane in 0..3 {
            // each lane its share, in a block of one kind or two
            let m = (n + 2 - lane) / 3;
            let blocks = if m >= 6 { 2 } else { 1 };
            let part = (to - from) / blocks as f32;
            for b in 0..blocks {
                let lo = from + part * b as f32;
                let count = (m + blocks - 1 - b) / blocks;
                self.lay_block(count, lane as i32, lo, lo + part, hard, (bx, bd, clear));
            }
        }
        let lost = self.send_army(first, e * loss, fight);
        self.s.expect = (e - lost).max(1.0);
        if let Some(i) = slot {
            self.lay_big(i, kind, bx, bd, hard, big);
            self.s.expect = (self.s.expect - e * big * BIG_TOLL).max(1.0);
        }
    }

    /// Puts down `count` of the army in a lane, to meet the squad evenly from
    /// `from` to `to`: of a kind by mix(), or grunts with a few brutes,
    /// riflemen or grenadiers behind them; none within `clear` of (x, d) of
    /// `big`, where a mech or giant comes.
    fn lay_block(
        &mut self,
        count: usize,
        lane: i32,
        from: f32,
        to: f32,
        hard: f32,
        big: (f32, f32, f32),
    ) {
        if count == 0 {
            return;
        }
        let brutes = (self.s.expect / 20.0) as usize;
        let (kind, few) = if hard >= 2.0 && brutes > 0 && self.chance(0.1 + hard * 0.005) {
            (E_BRUTE, (count / 3).clamp(1, brutes.min(3)))
        } else {
            let mut roll = self.s.track_rng.float();
            let kind = mix(hard)
                .iter()
                .find(|&&(_, odds)| {
                    roll -= odds;
                    roll < 0.0
                })
                .map_or(E_GRUNT, |k| k.0);
            match kind {
                // none that fire on a squad too small to take it
                E_RIFLE | E_GRENADIER if self.s.expect < FIRING_FROM => (E_GRUNT, count),
                E_RIFLE | E_GRENADIER => (kind, count.min(FEW)),
                _ => (kind, count),
            }
        };
        let mid = from + (to - from) * (count - few) as f32 / count as f32;
        self.lay_file(E_GRUNT, count - few, lane, from, mid, big);
        self.lay_file(kind, few, lane, mid, to, big);
    }

    /// Puts down `count` of kind in a lane, evenly from `from` to `to`.
    fn lay_file(
        &mut self,
        kind: i32,
        count: usize,
        lane: i32,
        from: f32,
        to: f32,
        big: (f32, f32, f32),
    ) {
        if count == 0 {
            return;
        }
        // in a file, or as many abreast as keep its rows close
        let across = count.div_ceil(6).min(4);
        let rows = count.div_ceil(across);
        let step = (to - from) / rows as f32;
        let wide = LANE_W - 1.4;
        let (bx, bd, clear) = big;
        for i in 0..count {
            let (row, j) = (i / across, i % across);
            let x = lane_x(lane) + ((j as f32 + 0.5) / across as f32 - 0.5) * wide;
            let x = (x + self.rand(-0.25, 0.25)).clamp(-HALF + 0.4, HALF - 0.4);
            let m = from + (row as f32 + 0.5) * step + self.rand(-0.3, 0.3);
            if (x - bx).hypot(m - bd) < clear {
                continue;
            }
            let pace = march_speed(kind, self.s.hardness_at(m));
            let (d, meant) = (self.start_at(m, pace), self.meant_at(m, pace));
            self.spawn_enemy(kind, x, d, meant);
        }
    }

    /// Where one marching at `pace` starts to meet the squad at m.
    fn start_at(&self, m: f32, pace: f32) -> f32 {
        m + (m - self.s.d).max(0.0) * pace / self.s.speed
    }

    /// The fire the plan has for the squad as one marching at `pace` to
    /// meet it at m comes THIN from it.
    fn meant_at(&self, m: f32, pace: f32) -> f32 {
        let v = self.s.speed;
        self.s.planned_fire(m - THIN * v / (v + pace))
    }

    /// A mech or giant of kind at x in slot i, to meet the squad at m, as
    /// tough as `big` of BIG_SECONDS of the fire of the squad expected past its army.
    fn lay_big(&mut self, i: usize, kind: i32, x: f32, m: f32, hard: f32, big: f32) {
        let toughness = if self.s.endless() {
            0.6 + hard * 0.035
        } else {
            (1.2 + hard * 0.005).min(1.3)
        };
        let hp = (BIG_SECONDS * big * toughness * self.dps())
            .round()
            .max(30.0) as i32;
        let e = self.s.expect * big;
        let pace = march_speed(E_GRUNT, hard);
        self.s.big[i] = Big {
            state: BIG_MARCH,
            kind,
            x,
            d: self.start_at(m, pace),
            hp,
            hp_max: hp,
            worth: ((e * PLAN_SHARE / share(e)).round() as i32).max(3),
            flinch_at: hp * 3 / 4,
            meant: self.meant_at(m, pace),
            ..NO_BIG
        };
    }

    /// How many an army numbers: as many as soak up `fight` seconds of the
    /// squad expected's fire with the hp they have, from `least` to `most`.
    fn army_size(&self, hard: f32, fight: f32, least: f32, most: f32) -> usize {
        let grunt = enemy_hp(E_GRUNT, hard) as f32;
        let hp = grunt
            + mix(hard)
                .iter()
                .map(|&(kind, odds)| odds * (enemy_hp(kind, hard) as f32 - grunt))
                .sum::<f32>();
        (self.dps() * fight / hp).round().max(least).min(most) as usize
    }

    /// Sizes the army put down from first on to cost the squad expected about
    /// `cost` soldiers: as many of it get through as that takes (from one in
    /// fifty to four in five), each made tough enough that the rest fall to
    /// `fight` seconds of the squad's fire on the way in. Returns what it
    /// should cost.
    fn send_army(&mut self, first: usize, cost: f32, fight: f32) -> f32 {
        let stake = self.s.en[first..self.s.nen as usize]
            .iter()
            .map(|e| worth(e.kind))
            .sum::<i32>() as f32
            * share(self.s.expect);
        let through = (cost / stake.max(1.0)).clamp(0.02, 0.8);
        self.toughen(first, self.dps() * fight / (1.0 - through));
        through * stake
    }

    /// Damage a second the squad expected puts into what it aims at.
    fn dps(&self) -> f32 {
        squad_dps(self.s.expect.max(1.0), self.s.expect_gun)
    }

    /// Damage the squad expected puts into a gate it runs at, firing the
    /// in_range() seconds it gets: halfway, on a log scale, between what it
    /// does with the pistol and with the gun it is expected to have by now,
    /// so each better gun counts gates up sooner.
    fn burst(&self) -> f32 {
        (squad_dps(self.s.expect.max(1.0), 0) * self.dps()).sqrt() * self.in_range()
    }

    /// Seconds of fire a squad running at a gate gets into it: from lining
    /// up a little inside its guns' range to reaching it, less the bullets' flight.
    fn in_range(&self) -> f32 {
        ((WEAPONS[self.s.expect_gun as usize].range - 6.0) / self.s.speed - 0.4).max(1.0)
    }

    /// A buff at d in a lane: the next gun if `gun` and there is one, with
    /// soldiers in another lane now and then as the other pick; or soldiers.
    fn lay_buff(&mut self, d: f32, hard: f32, gun: bool) {
        let lane = self.lane();
        if !gun || self.s.expect_gun >= 4 {
            return self.lay_men(lane, d, hard);
        }
        let gun = self.s.expect_gun + 1;
        self.add_thing(Thing {
            kind: K_BUFF,
            x: lane_x(lane),
            d,
            a: BUFF_GUN,
            value: gun,
            ..NO_THING
        });
        // the plan takes the gun
        if self.chance(0.6) {
            let (e, other) = (
                self.s.expect,
                (lane + 1 + self.s.track_rng.below(2) as i32) % 3,
            );
            self.lay_men(other, d, hard);
            self.s.expect = e;
        }
        self.s.expect_gun = gun;
    }

    /// Soldiers to join the squad that runs into them.
    fn lay_men(&mut self, lane: i32, d: f32, hard: f32) {
        let e = self.s.expect;
        let value = (3.0 + gains(e) * 0.25 + hard).round() as i32;
        self.add_thing(Thing {
            kind: K_BUFF,
            x: lane_x(lane),
            d,
            a: BUFF_MEN,
            value,
            ..NO_THING
        });
        self.s.expect = (e + value as f32 * 0.8).min(MAX_COUNT as f32);
    }

    /// A ship off the pier, firing shells at the squad as it passes.
    fn lay_ship(&mut self, d: f32) {
        let x = if self.chance(0.5) { -SHIP_X } else { SHIP_X };
        self.add_thing(Thing {
            kind: K_SHIP,
            x,
            d,
            cool: 0.8,
            ..NO_THING
        });
        self.s.expect *= 0.9;
    }
}

/// Each kind's share of the army where the pier is that hard (grunts the
/// rest): every kind from the first level, most of its share there and all
/// of it by the sixth.
fn mix(hard: f32) -> [(i32, f32); 4] {
    let k = (0.7 + hard * 0.06).min(1.0);
    [
        (E_RIFLE, 0.15),
        (E_GRENADIER, 0.08),
        (E_HEAVY, 0.15),
        (E_RUNNER, 0.25),
    ]
    .map(|(kind, odds)| (kind, odds * k))
}

/// What sums on gates and buffs are sized by for a squad of e: e itself up
/// to a hundred, then ten times its square root, so a big squad still grows
/// by them but not past everything that thins it out.
fn gains(e: f32) -> f32 {
    e.min(10.0 * e.sqrt())
}

/// How many soldiers each one shown of a squad of e stands for.
fn share(e: f32) -> f32 {
    (e / shooters(e)).max(1.0)
}
