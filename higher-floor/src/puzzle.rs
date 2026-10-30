//! The tower: its rooms, what waits in each, which the hero can reach, and
//! the levels, dealt along a way up that wins.

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

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Room {
    pub kind: i8,
    pub look: i8,
    pub pad: [i8; 2],
    /// a monster's power, or what a potion adds
    pub val: i32,
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

/// the power after taking a room, or None when its monster wins
pub fn take(power: i32, room: &Room) -> Option<i32> {
    match room.kind {
        MONSTER if power > room.val => Some(power.saturating_add(room.val)),
        MONSTER => None,
        POTION => Some(power.saturating_add(room.val)),
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
    /// Whether room r can be entered with these rooms taken: the ground
    /// floor always, and any room next to one taken.
    pub fn reachable(&self, taken: u32, r: usize) -> bool {
        if r >= self.n() || taken & 1 << r != 0 || self.rooms[r].kind == EMPTY {
            return false;
        }
        let (f, c) = self.place(r);
        f == 0
            || [(f - 1, c), (f + 1, c), (f, c - 1), (f, c + 1)]
                .iter()
                .any(|&(g, d)| g >= 0 && g < self.floors && d >= 0 && d < self.cols && taken & 1 << (g * self.cols + d) != 0)
    }
    /// the monsters not yet beaten
    pub fn left(&self, taken: u32) -> usize {
        (0..self.n()).filter(|&r| self.rooms[r].kind == MONSTER && taken & 1 << r == 0).count()
    }
    /// every monster beaten
    pub fn won(&self, taken: u32) -> bool {
        self.left(taken) == 0
    }
    /// the room in column x, y floors from the top
    pub fn room_at(&self, x: i32, y: i32) -> usize {
        ((self.floors - 1 - y) * self.cols + x) as usize
    }
    pub fn monsters(&self) -> i32 {
        self.rooms[..self.n()].iter().filter(|r| r.kind == MONSTER).count() as i32
    }
    pub fn valid(&self) -> bool {
        (1..=MAXCOLS as i32).contains(&self.cols)
            && (1..=MAXFLOORS as i32).contains(&self.floors)
            && self.start > 0
            && self.rooms[..self.n()].iter().all(|r| (EMPTY..=TRAP).contains(&r.kind) && (0..LOOKS).contains(&r.look) && r.val >= 0)
    }
    /// Whether some order of rooms from here beats every monster.
    #[cfg(test)]
    pub fn solvable(&self, taken: u32, power: i32) -> bool {
        let mut seen = std::collections::HashSet::new();
        self.search(taken, power, &mut seen)
    }
    #[cfg(test)]
    fn search(&self, taken: u32, power: i32, seen: &mut std::collections::HashSet<(u32, i32)>) -> bool {
        if self.won(taken) {
            return true;
        }
        if !seen.insert((taken, power)) {
            return false;
        }
        (0..self.n()).any(|r| self.reachable(taken, r) && take(power, &self.rooms[r]).is_some_and(|p| self.search(taken | 1 << r, p, seen)))
    }
}

/// a level's tower: columns, floors, potions, chests, traps
fn shape(level: i32) -> (i32, i32, usize, usize, usize) {
    let l = level;
    let floors = (2 + l * 4 / 26).min(MAXFLOORS as i32);
    let cols = 2;
    let potions = (l >= 1) as usize + (l >= 12) as usize;
    let chests = (l >= 3) as usize + (l >= 16) as usize;
    let traps = (l >= 6) as usize + (l >= 20) as usize;
    (cols, floors, potions, chests, traps)
}

/// Deals a level: the same level always deals the same tower.
pub fn deal(level: i32) -> Tower {
    let (cols, floors, potions, chests, traps) = shape(level);
    let mut rng = Rng::new(0x70E7 + level as u32 * 7919);
    let mut best: Option<(i32, Tower)> = None;
    // the share of random climbs that should win: most at first, few at
    // the end
    let want = 48.0 * (0.6 - 0.55 * level as f32 / (LEVELS - 1) as f32);
    for _ in 0..60 {
        let t = build(&mut rng, cols, floors, potions, chests, traps);
        let mut wins = 0;
        for _ in 0..48 {
            wins += wander(&mut rng, &t) as i32;
        }
        let off = (wins as f32 - want).abs() as i32;
        if best.as_ref().is_none_or(|b| off < b.0) {
            best = Some((off, t));
        }
    }
    best.unwrap().1
}

/// One tower: a random way up through every room, its monsters each a
/// little weaker than the hero when the way reaches them.
fn build(rng: &mut Rng, cols: i32, floors: i32, potions: usize, chests: usize, traps: usize) -> Tower {
    let mut t = Tower { cols, floors, start: 4 + rng.below(5) as i32, rooms: [Room::default(); MAXROOMS] };
    let n = t.n();
    let mut kinds = vec![MONSTER; n];
    let mut k = 0;
    for (count, kind) in [(potions, POTION), (chests, CHEST), (traps, TRAP)] {
        for _ in 0..count.min(n.saturating_sub(k + 2)) {
            kinds[k] = kind;
            k += 1;
        }
    }
    rng.shuffle(&mut kinds[..n]);
    for r in 0..n {
        t.rooms[r].kind = kinds[r];
    }
    let mut power = t.start;
    let mut taken = 0u32;
    for _ in 0..n {
        let open: Vec<usize> = (0..n).filter(|&r| t.reachable(taken, r)).collect();
        let r = open[rng.below(open.len() as u32) as usize];
        let room = &mut t.rooms[r];
        match room.kind {
            MONSTER => room.val = (power as f32 * (0.45 + 0.5 * rng.float())) as i32,
            POTION => room.val = 1 + (power as f32 * (0.25 + 0.35 * rng.float())) as i32,
            _ => {}
        }
        room.val = room.val.clamp(if room.kind == MONSTER || room.kind == POTION { 1 } else { 0 }, power.max(2) - 1);
        power = take(power, room).unwrap_or(power);
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
    t
}

/// A climb taking open rooms at random, skipping traps when it can:
/// whether it beats every monster.
fn wander(rng: &mut Rng, t: &Tower) -> bool {
    let mut power = t.start;
    let mut taken = 0u32;
    loop {
        if t.won(taken) {
            return true;
        }
        let open: Vec<usize> = (0..t.n()).filter(|&r| t.reachable(taken, r)).collect();
        let good: Vec<usize> = open.iter().copied().filter(|&r| t.rooms[r].kind != TRAP).collect();
        let pick = if good.is_empty() { &open } else { &good };
        if pick.is_empty() {
            return false;
        }
        let r = pick[rng.below(pick.len() as u32) as usize];
        match take(power, &t.rooms[r]) {
            Some(p) => power = p,
            None => return false,
        }
        taken |= 1 << r;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_can_be_won() {
        let mut rng = Rng::new(9);
        for level in 0..LEVELS {
            let t = deal(level);
            assert!(t.valid(), "level {level} invalid");
            assert!(t.solvable(0, t.start), "level {level} cannot be won");
            let wins = (0..200).filter(|_| wander(&mut rng, &t)).count();
            let line: Vec<String> = (0..t.n()).map(|r| format!("{}:{}", t.rooms[r].kind, t.rooms[r].val)).collect();
            println!("level {:2}: {}x{} start {} random wins {:3}/200  {}", level + 1, t.cols, t.floors, t.start, wins, line.join(" "));
        }
    }
}
