//! The depot and the queue: where each bus stands, which way it can leave,
//! who waits for which colour, and the levels, dealt so everyone gets a
//! seat.

use wg::Rng;

pub const LEVELS: i32 = 30;
pub const MAXW: usize = 6;
pub const MAXH: usize = 6;
pub const MAXBUSES: usize = 24;
/// passengers in the longest queue: every bus a big one
pub const MAXQ: usize = MAXBUSES * 6;
/// the colours: red, orange, yellow, green, blue, purple
pub const COLOURS: i8 = 6;
/// bays at the stop
pub const BAYS: usize = 5;

/// Facing: up, right, down, left the screen.
pub const DX: [i32; 4] = [0, 1, 0, -1];
pub const DY: [i32; 4] = [-1, 0, 1, 0];

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Bus {
    /// the square of its back end
    pub x: i8,
    pub y: i8,
    /// 2 for a minibus, 3 for a bus
    pub len: i8,
    /// the way its front faces, 0..4
    pub dir: i8,
    pub colour: i8,
    /// 1 once it has left the depot
    pub out: i8,
}

impl Bus {
    /// its squares, back end first
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let d = self.dir as usize;
        (0..self.len as i32).map(move |i| (self.x as i32 + DX[d] * i, self.y as i32 + DY[d] * i))
    }
    /// the square at its front end
    pub fn front(&self) -> (i32, i32) {
        let k = self.len as i32 - 1;
        let d = self.dir as usize;
        (self.x as i32 + DX[d] * k, self.y as i32 + DY[d] * k)
    }
    /// passengers it seats
    pub fn seats(&self) -> i32 {
        if self.len == 3 { 6 } else { 4 }
    }
}

/// A depot of w x h squares and its buses, and the queue at the stop.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Lot {
    pub w: i32,
    pub h: i32,
    pub n: i32,
    pub buses: [Bus; MAXBUSES],
    /// the queue's colours, front first, and how many
    pub qn: i32,
    pub queue: [i8; MAXQ],
}

/// Where a bus driving forward ends up: out of the depot, or stopped by
/// the bus `gap` squares on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Way {
    Out,
    Blocked { gap: i32, by: i32 },
}

impl Lot {
    pub fn empty(w: i32, h: i32) -> Lot {
        Lot { w, h, n: 0, buses: [Bus::default(); MAXBUSES], qn: 0, queue: [0; MAXQ] }
    }
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }
    /// the bus on a square, if any (buses that left are gone)
    pub fn bus_at(&self, x: i32, y: i32) -> Option<usize> {
        (0..self.n as usize).find(|&i| self.buses[i].out == 0 && self.buses[i].cells().any(|c| c == (x, y)))
    }
    pub fn way(&self, i: usize) -> Way {
        let b = &self.buses[i];
        let d = b.dir as usize;
        let (mut x, mut y) = b.front();
        // which bus is on each square, looked up once for the whole way
        let mut on = [-1i8; MAXW * MAXH];
        for (j, bus) in self.buses[..self.n as usize].iter().enumerate().filter(|(_, b)| b.out == 0) {
            for (cx, cy) in bus.cells() {
                on[(cy * MAXW as i32 + cx) as usize] = j as i8;
            }
        }
        let mut gap = 0;
        loop {
            (x, y) = (x + DX[d], y + DY[d]);
            if !self.inside(x, y) {
                return Way::Out;
            }
            let j = on[(y * MAXW as i32 + x) as usize];
            if j >= 0 {
                return Way::Blocked { gap, by: j as i32 };
            }
            gap += 1;
        }
    }
    /// whether bus i is still in the depot with a clear road out
    pub fn leavable(&self, i: usize) -> bool {
        self.buses[i].out == 0 && self.way(i) == Way::Out
    }
    /// Of the bays, as (bay, bus, passengers on it), the one a passenger
    /// of colour c boards: the fullest bus of that colour with a seat left.
    pub fn bay_for(&self, bays: impl Iterator<Item = (usize, usize, i32)>, c: i8) -> Option<usize> {
        bays.filter(|&(_, i, f)| self.buses[i].colour == c && f < self.buses[i].seats()).max_by_key(|&(_, _, f)| f).map(|(k, _, _)| k)
    }
    pub fn left(&self) -> i32 {
        self.buses[..self.n as usize].iter().filter(|b| b.out == 0).count() as i32
    }
    /// a save's depot holds together: sizes, buses inside and apart, as
    /// many passengers of each colour as seats
    pub fn valid(&self) -> bool {
        if !(2..=MAXW as i32).contains(&self.w) || !(2..=MAXH as i32).contains(&self.h) || !(0..=MAXBUSES as i32).contains(&self.n) {
            return false;
        }
        if !(0..=MAXQ as i32).contains(&self.qn) {
            return false;
        }
        let mut used = [false; MAXW * MAXH];
        let mut seats = [0; COLOURS as usize];
        let ok = self.buses[..self.n as usize].iter().all(|b| {
            if !(2..=3).contains(&b.len) || !(0..4).contains(&b.dir) || !(0..=1).contains(&b.out) || !(0..COLOURS).contains(&b.colour) {
                return false;
            }
            seats[b.colour as usize] += b.seats();
            b.out == 1
                || b.cells().all(|(x, y)| {
                    let k = (y * MAXW as i32 + x) as usize;
                    let ok = self.inside(x, y) && !used[k];
                    if ok {
                        used[k] = true;
                    }
                    ok
                })
        });
        for &c in &self.queue[..self.qn as usize] {
            if !(0..COLOURS).contains(&c) {
                return false;
            }
            seats[c as usize] -= 1;
        }
        ok && seats.iter().all(|&s| s == 0)
    }
}

/// a level's depot: width, height, how full, colours, buses mixed in, and
/// how many buses' passengers wait mixed together
fn shape(level: i32) -> (i32, i32, f32, i8, bool, usize) {
    let l = level;
    let w = (5 + l / 12).min(MAXW as i32);
    let h = (5 + (l + 6) / 12).min(MAXH as i32);
    let fill = (0.62 + 0.01 * l as f32).min(0.82);
    let colours = (4 + l / 8).min(COLOURS as i32) as i8;
    let mixed = if l < 10 { 3 } else { 4 };
    (w, h, fill, colours, l >= 2, mixed)
}

/// Deals a level. The same level always deals the same depot and queue.
pub fn deal(level: i32) -> Lot {
    // a retry deals the same level again: keep the last one
    thread_local!(static LAST: std::cell::Cell<Option<(i32, Lot)>> = const { std::cell::Cell::new(None) });
    if let Some((l, lot)) = LAST.get() {
        if l == level {
            return lot;
        }
    }
    let lot = deal_order(level).0;
    LAST.set(Some((level, lot)));
    lot
}

/// A level, and the order of sending its buses that its queue was made for.
pub fn deal_order(level: i32) -> (Lot, Vec<usize>) {
    let (w, h, fill, colours, buses, mixed) = shape(level);
    let mut rng = Rng::new(0xB05 + level as u32 * 7919);
    // the share of tries that a hasty player, sending any bus that can
    // leave, should lose: now and then at first, mostly at the end
    let want = TRIALS as f32 * (0.2 + 0.75 * level as f32 / (LEVELS - 1) as f32);
    let mut best: Option<(f32, Lot, Vec<usize>)> = None;
    for _ in 0..DEALS {
        let mut lot = park(&mut rng, w, h, fill, colours, buses);
        let order = queue(&mut rng, &mut lot, mixed);
        let lost = (0..TRIALS).filter(|_| !hasty(&lot, &mut rng)).count();
        let off = (lost as f32 - want).abs();
        if best.as_ref().is_none_or(|b| off < b.0) {
            best = Some((off, lot, order));
        }
    }
    let (_, lot, order) = best.unwrap();
    (lot, order)
}

const DEALS: usize = 24;
const TRIALS: usize = 32;

/// A hasty player: any bus that can leave, whenever a bay is free, and
/// everyone who can board does before the next. Whether the queue empties.
fn hasty(lot: &Lot, rng: &mut Rng) -> bool {
    let mut g = *lot;
    let mut bays: [Option<(usize, i32)>; BAYS] = [None; BAYS];
    let mut head = 0;
    loop {
        while head < g.qn as usize {
            let open = bays.iter().enumerate().filter_map(|(k, b)| b.map(|(i, f)| (k, i, f)));
            let Some(k) = g.bay_for(open, g.queue[head]) else {
                break;
            };
            let (i, f) = bays[k].as_mut().unwrap();
            *f += 1;
            if *f == g.buses[*i].seats() {
                bays[k] = None;
            }
            head += 1;
        }
        if head == g.qn as usize {
            return true;
        }
        let Some(k) = (0..BAYS).find(|&k| bays[k].is_none()) else {
            return false;
        };
        let free: Vec<usize> = (0..g.n as usize).filter(|&i| g.leavable(i)).collect();
        if free.is_empty() {
            return false;
        }
        let i = free[rng.below(free.len() as u32) as usize];
        g.buses[i].out = 1;
        bays[k] = Some((i, 0));
    }
}

fn park(rng: &mut Rng, w: i32, h: i32, fill: f32, colours: i8, buses: bool) -> Lot {
    let mut lot = Lot::empty(w, h);
    let want = ((w * h) as f32 * fill) as i32;
    let mut cover = 0;
    let mut tries = 0;
    // Each bus parks only where it could drive straight out past the buses
    // already parked, so taking them away last first always works.
    while cover < want && tries < 3000 && (lot.n as usize) < MAXBUSES {
        tries += 1;
        let len = if buses && rng.below(4) == 0 { 3 } else { 2 };
        let bus = Bus { x: rng.range(0, w - 1) as i8, y: rng.range(0, h - 1) as i8, len, dir: rng.below(4) as i8, colour: 0, out: 0 };
        if !bus.cells().all(|(x, y)| lot.inside(x, y) && lot.bus_at(x, y).is_none()) {
            continue;
        }
        let i = lot.n as usize;
        lot.buses[i] = bus;
        lot.n += 1;
        if lot.way(i) == Way::Out {
            cover += len as i32;
        } else {
            lot.n -= 1;
        }
    }
    // the colours in turn, from a shuffled start, so each shows about as
    // often as the others
    let mut order: Vec<i8> = (0..colours).collect();
    rng.shuffle(&mut order);
    let mut picks: Vec<usize> = (0..lot.n as usize).collect();
    rng.shuffle(&mut picks);
    for (k, &i) in picks.iter().enumerate() {
        lot.buses[i].colour = order[k % order.len()];
    }
    lot
}

/// The queue: the buses taken out in some order that works, and their
/// passengers mixed between the next `mixed` of them. Sending the buses in
/// that order, at most `mixed` wait at the stop at once, fewer than the
/// bays, so the queue always moves.
fn queue(rng: &mut Rng, lot: &mut Lot, mixed: usize) -> Vec<usize> {
    let mut order = Vec::new();
    let mut gone = *lot;
    loop {
        let free: Vec<usize> = (0..gone.n as usize).filter(|&i| gone.leavable(i)).collect();
        if free.is_empty() {
            break;
        }
        let i = free[rng.below(free.len() as u32) as usize];
        gone.buses[i].out = 1;
        order.push(i);
    }
    let mut left: Vec<(usize, i32)> = Vec::new();
    let mut next = 0;
    lot.qn = 0;
    loop {
        while left.len() < mixed && next < order.len() {
            left.push((order[next], lot.buses[order[next]].seats()));
            next += 1;
        }
        if left.is_empty() {
            break;
        }
        let k = rng.below(left.len() as u32) as usize;
        lot.queue[lot.qn as usize] = lot.buses[left[k].0].colour;
        lot.qn += 1;
        left[k].1 -= 1;
        if left[k].1 == 0 {
            left.remove(k);
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_deals() {
        for level in 0..LEVELS {
            let lot = deal(level);
            assert!(lot.valid(), "level {level} invalid");
            let mut gone = lot;
            while let Some(i) = (0..gone.n as usize).find(|&i| gone.leavable(i)) {
                gone.buses[i].out = 1;
            }
            assert_eq!(gone.left(), 0, "level {level} stuck");
            let free = (0..lot.n as usize).filter(|&i| lot.way(i) == Way::Out).count();
            let mut rng = Rng::new(5);
            const N: usize = 400;
            let lost = (0..N).filter(|_| !hasty(&lot, &mut rng)).count();
            println!("level {:2}: {}x{} buses {:2} free {:2} queue {} hasty play loses {:.0}%", level + 1, lot.w, lot.h, lot.n, free, lot.qn, 100.0 * lost as f32 / N as f32);
            assert!(lost > 0, "level {} can't be lost", level + 1);
        }
    }
}
