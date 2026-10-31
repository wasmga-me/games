//! The tower: its rooms, what waits in each, which the hero can reach, and
//! the towers, dealt along a way up that wins.

use std::collections::HashMap;
use wg::Rng;

pub const LEVELS: i32 = 30;
pub const MAXCOLS: usize = 2;
pub const MAXFLOORS: usize = 6;
pub const MAXROOMS: usize = MAXCOLS * MAXFLOORS;
/// monster looks, weakest first: skeleton, zombie, ghost, orc, vampire
pub const LOOKS: i8 = 5;

/// What waits in a room.
pub const EMPTY: i8 = 0;
pub const MONSTER: i8 = 1;
/// adds its number to the hero's power
pub const POTION: i8 = 2;
/// doubles it
pub const CHEST: i8 = 3;
/// halves it
pub const TRAP: i8 = 4;
/// takes its number from it; the hero falls if that leaves nothing
pub const POISON: i8 = 5;
/// a monster that grows by `grow` with every room the hero takes
pub const BRUTE: i8 = 6;
/// opens the locked rooms
pub const KEY: i8 = 7;

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Room {
    pub kind: i8,
    pub look: i8,
    /// a brute's growth per room taken
    pub grow: i8,
    /// shut until the key is taken
    pub locked: i8,
    /// a monster's power (a brute's before the hero's first room), what a
    /// potion adds or poison takes
    pub val: i32,
}

impl Room {
    pub fn fights(&self) -> bool {
        self.kind == MONSTER || self.kind == BRUTE
    }
    /// whether a fight here with this power is a tie, which fells both
    pub fn ties(&self, power: i32, moves: u32) -> bool {
        self.fights() && self.strength(moves) == power
    }
    /// a monster's power once the hero has taken `moves` rooms
    pub fn strength(&self, moves: u32) -> i32 {
        if self.kind == BRUTE { self.val.saturating_add(self.grow as i32 * moves as i32) } else { self.val }
    }
}

/// The tower: `cols` rooms across, `floors` up, numbered from the bottom
/// left along each floor; the hero's power at the start.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Tower {
    pub cols: i32,
    pub floors: i32,
    pub start: i32,
    pub rooms: [Room; MAXROOMS],
}

/// The power after taking a room with `moves` rooms taken before it, or
/// None when the hero falls: to a monster as strong or stronger (a tie
/// fells both), or to poison that leaves him nothing.
pub fn take(power: i32, room: &Room, moves: u32) -> Option<i32> {
    match room.kind {
        MONSTER | BRUTE => {
            let v = room.strength(moves);
            (power > v).then(|| power.saturating_add(v))
        }
        POTION => Some(power.saturating_add(room.val)),
        POISON => Some(power - room.val).filter(|&p| p > 0),
        CHEST => Some(power.saturating_mul(2)),
        TRAP => Some((power + 1) / 2),
        _ => Some(power),
    }
}

impl Tower {
    pub fn empty() -> Tower {
        Tower { cols: 1, floors: 1, start: 1, rooms: [Room::default(); MAXROOMS] }
    }
    pub fn n(&self) -> usize {
        (self.cols * self.floors) as usize
    }
    /// room r's floor (0 at the bottom) and column
    pub fn place(&self, r: usize) -> (i32, i32) {
        (r as i32 / self.cols, r as i32 % self.cols)
    }
    /// whether the locked rooms are open: no key left to take
    pub fn unlocked(&self, taken: u32) -> bool {
        (0..self.n()).all(|r| self.rooms[r].kind != KEY || taken & 1 << r != 0)
    }
    /// whether room r is shut: locked, with the key not yet taken
    pub fn locked(&self, taken: u32, r: usize) -> bool {
        self.rooms[r].locked != 0 && !self.unlocked(taken)
    }
    /// Whether room r can be entered with these rooms taken: the ground
    /// floor always, and any room next to one taken; locked rooms only
    /// once the key is taken.
    pub fn reachable(&self, taken: u32, r: usize) -> bool {
        if r >= self.n() || taken & 1 << r != 0 || self.rooms[r].kind == EMPTY {
            return false;
        }
        if self.locked(taken, r) {
            return false;
        }
        let (f, c) = self.place(r);
        f == 0
            || [(f - 1, c), (f + 1, c), (f, c - 1), (f, c + 1)]
                .iter()
                .any(|&(g, d)| g >= 0 && g < self.floors && d >= 0 && d < self.cols && taken & 1 << (g * self.cols + d) != 0)
    }
    /// the rooms not yet taken
    pub fn left(&self, taken: u32) -> usize {
        (0..self.n()).filter(|&r| self.rooms[r].kind != EMPTY && taken & 1 << r == 0).count()
    }
    /// every room taken
    pub fn won(&self, taken: u32) -> bool {
        self.left(taken) == 0
    }
    /// the room in column x, y floors from the top
    pub fn room_at(&self, x: i32, y: i32) -> usize {
        ((self.floors - 1 - y) * self.cols + x) as usize
    }
    pub fn monsters(&self) -> i32 {
        self.rooms[..self.n()].iter().filter(|r| r.fights()).count() as i32
    }
    pub fn valid(&self) -> bool {
        (1..=MAXCOLS as i32).contains(&self.cols)
            && (1..=MAXFLOORS as i32).contains(&self.floors)
            && self.start > 0
            && self.rooms[..self.n()]
                .iter()
                .all(|r| (EMPTY..=KEY).contains(&r.kind) && (0..LOOKS).contains(&r.look) && r.val >= 0 && r.grow >= 0 && (0..=1).contains(&r.locked))
    }

    /// The chance a careful climb wins: one that picks at random among the
    /// open rooms it survives.
    pub fn odds(&self) -> f32 {
        self.odds_from(0, self.start, &mut HashMap::new())
    }
    fn odds_from(&self, taken: u32, power: i32, seen: &mut HashMap<(u32, i32), f32>) -> f32 {
        if self.won(taken) {
            return 1.0;
        }
        if let Some(&p) = seen.get(&(taken, power)) {
            return p;
        }
        let moves = taken.count_ones();
        let (mut open, mut sum) = (0, 0.0);
        for r in 0..self.n() {
            if !self.reachable(taken, r) {
                continue;
            }
            if let Some(p) = take(power, &self.rooms[r], moves) {
                open += 1;
                sum += self.odds_from(taken | 1 << r, p, seen);
            }
        }
        let p = if open == 0 { 0.0 } else { sum / open as f32 };
        seen.insert((taken, power), p);
        p
    }
}

/// The rooms besides monsters a tower can hold, each from the level (from
/// 0) it joins at; a kind listed twice can come twice from the later level.
const ITEMS: [(i8, i32); 12] = [
    (TRAP, 0),
    (POTION, 1),
    (CHEST, 2),
    (POISON, 4),
    (BRUTE, 6),
    (KEY, 9),
    (TRAP, 12),
    (POTION, 14),
    (BRUTE, 16),
    (CHEST, 18),
    (POISON, 21),
    (TRAP, 24),
];

/// A tower's floors at difficulty `d` (the level from 0, going on past the
/// last level in endless mode): a floor more about every seven levels.
fn floors(d: i32) -> i32 {
    (2 + d * 4 / 26).min(MAXFLOORS as i32)
}

/// What fills a tower's rooms besides monsters, at most `room`: always the
/// newest kind yet, then others at random.
fn items(rng: &mut Rng, d: i32, room: usize) -> Vec<i8> {
    let mut can: Vec<(i8, i32)> = ITEMS.iter().copied().filter(|&(_, at)| at <= d).collect();
    // ITEMS is in joining order, so the newest is last
    let newest = can.len() - 1;
    can.swap(0, newest);
    rng.shuffle(&mut can[1..]);
    can.iter().take(room).map(|&(k, _)| k).collect()
}

/// The share of careful climbs (any room they survive, picked at random)
/// a tower at difficulty `d` should let win: about half at first, few by
/// the last level, fewer still in endless.
fn careful_wins(d: i32) -> f32 {
    (0.5 - 0.42 * d as f32 / (LEVELS - 1) as f32).max(0.04)
}

/// Deals a level: the same level always deals the same tower.
pub fn deal(level: i32) -> Tower {
    deal_with(&mut Rng::new(0x70E7 + level as u32 * 7919), level)
}

/// Deals tower `k` (from 0) of an endless climb from this seed.
pub fn deal_endless(seed: u32, k: i32) -> Tower {
    // a little ahead of the levels at first, then on past them
    deal_with(&mut Rng::new(seed ^ (k as u32 + 1).wrapping_mul(0x9E37_79B9)), 2 + k * 3 / 2)
}

/// Of a few towers built at difficulty d, the one whose careful climbs win
/// nearest the share wanted, and which a careful climb can lose.
fn deal_with(rng: &mut Rng, d: i32) -> Tower {
    let want = careful_wins(d);
    let mut best: Option<(f32, Tower)> = None;
    let mut built = 0;
    // a build can get stuck (behind a lock, or with nothing left to beat);
    // those don't count
    for _ in 0..400 {
        if built == 40 {
            break;
        }
        let Some(t) = build(rng, d) else { continue };
        built += 1;
        let p = t.odds();
        let off = if p > 0.9 { 1.0 + p } else { (p - want).abs() };
        if best.as_ref().is_none_or(|b| off < b.0) {
            best = Some((off, t));
        }
    }
    match best {
        Some((_, t)) => t,
        // never seen, but a tower one step easier always deals
        None => deal_with(rng, d - 1),
    }
}

/// One tower at difficulty d: a random way up through every room, its
/// monsters each a little weaker than the hero when the way reaches them.
/// None when the way gets stuck behind a locked room.
fn build(rng: &mut Rng, d: i32) -> Option<Tower> {
    let mut t = Tower { cols: MAXCOLS as i32, floors: floors(d), start: 4 + rng.below(5) as i32, rooms: [Room::default(); MAXROOMS] };
    let n = t.n();
    // at least a third of the rooms (and two) are monsters
    let mut kinds = items(rng, d, n - (n / 3).max(2));
    kinds.resize(n, MONSTER);
    rng.shuffle(&mut kinds);
    for r in 0..n {
        t.rooms[r].kind = kinds[r];
    }
    if kinds.contains(&KEY) {
        // locks go on rooms above the ground floor that aren't the key
        let mut can: Vec<usize> = (t.cols as usize..n).filter(|&r| kinds[r] != KEY).collect();
        rng.shuffle(&mut can);
        for &r in can.iter().take(1 + (d >= 20) as usize) {
            t.rooms[r].locked = 1;
        }
    }
    let mut power = t.start;
    let mut taken = 0u32;
    for moves in 0..n as u32 {
        let open: Vec<usize> = (0..n).filter(|&r| t.reachable(taken, r)).collect();
        if open.is_empty() {
            return None;
        }
        let r = open[rng.below(open.len() as u32) as usize];
        let room = &mut t.rooms[r];
        let frac = |rng: &mut Rng, lo: f32, hi: f32| (power as f32 * (lo + (hi - lo) * rng.float())) as i32;
        match room.kind {
            MONSTER => room.val = frac(rng, 0.55, 0.98).clamp(1, power.max(2) - 1),
            BRUTE => {
                // as strong as a monster when the way reaches it, having
                // grown by `grow` for every room taken before
                let v = frac(rng, 0.6, 0.98).clamp(1, power.max(2) - 1);
                let grow = (1 + rng.below(2 + d as u32 / 10) as i32).min((v - 1) / moves.max(1) as i32).max(0);
                room.grow = grow as i8;
                room.val = v - grow * moves as i32;
            }
            POTION => room.val = 1 + frac(rng, 0.25, 0.6),
            POISON => room.val = frac(rng, 0.2, 0.6).clamp(1, (power - 1).max(1)),
            _ => {}
        }
        power = take(power, room, moves)?;
        taken |= 1 << r;
    }
    // the looks by strength: the weakest a skeleton, the strongest a vampire
    let mut vals: Vec<i32> = t.rooms[..n].iter().filter(|r| r.kind == MONSTER).map(|r| r.val).collect();
    vals.sort();
    for r in 0..n {
        if t.rooms[r].kind == MONSTER {
            let rank = vals.iter().position(|&v| v == t.rooms[r].val).unwrap();
            t.rooms[r].look = (rank * LOOKS as usize / vals.len().max(1)) as i8;
        }
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether some order of rooms from here takes them all.
    fn solvable(t: &Tower, taken: u32, power: i32) -> bool {
        t.won(taken)
            || (0..t.n()).any(|r| t.reachable(taken, r) && take(power, &t.rooms[r], taken.count_ones()).is_some_and(|p| solvable(t, taken | 1 << r, p)))
    }

    fn check(name: &str, t: &Tower) {
        assert!(t.valid(), "{name} invalid");
        assert!(solvable(t, 0, t.start), "{name} cannot be won");
        let careful = t.odds();
        assert!(careful < 0.9, "{name}: a careful climb wins {careful}");
        let line: Vec<String> = (0..t.n())
            .map(|r| {
                let m = t.rooms[r];
                format!("{}{}:{}{}", if m.locked != 0 { "L" } else { "" }, m.kind, m.val, if m.grow > 0 { format!("+{}", m.grow) } else { String::new() })
            })
            .collect();
        println!("{name}: {}x{} start {} careful {:.2}  {}", t.cols, t.floors, t.start, careful, line.join(" "));
    }

    #[test]
    fn every_level_can_be_won_and_lost() {
        for level in 0..LEVELS {
            check(&format!("level {:2}", level + 1), &deal(level));
        }
    }

    #[test]
    fn endless_towers_can_be_won_and_lost() {
        for k in 0..40 {
            check(&format!("endless {k:2}"), &deal_endless(12345, k));
        }
    }

    /// Many endless climbs, as players' random seeds deal them: run with
    /// --ignored.
    #[test]
    #[ignore]
    fn endless_deals_for_any_seed() {
        for seed in 0..150u32 {
            for k in 0..24 {
                let t = deal_endless(seed.wrapping_mul(2654435761), k);
                assert!(t.valid() && solvable(&t, 0, t.start), "seed {seed} tower {k}");
            }
        }
    }

    #[test]
    fn ties_fell_both_and_brutes_grow() {
        let m = Room { kind: MONSTER, val: 8, ..Room::default() };
        assert_eq!(take(8, &m, 0), None);
        assert_eq!(take(9, &m, 0), Some(17));
        let b = Room { kind: BRUTE, val: 3, grow: 2, ..Room::default() };
        assert_eq!(take(8, &b, 2), Some(15));
        assert_eq!(take(8, &b, 3), None);
        let p = Room { kind: POISON, val: 5, ..Room::default() };
        assert_eq!((take(6, &p, 0), take(5, &p, 0)), (Some(1), None));
    }
}
