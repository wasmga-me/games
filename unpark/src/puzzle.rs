//! The car park: who stands where, which way each car can leave, and the
//! levels, dealt so every car gets out.

use wg::Rng;

pub const LEVELS: i32 = 30;
pub const MAXW: usize = 8;
pub const MAXH: usize = 10;
pub const MAXCARS: usize = 40;

/// Facing: up, right, down, left the screen.
pub const DX: [i32; 4] = [0, 1, 0, -1];
pub const DY: [i32; 4] = [-1, 0, 1, 0];

/// What stands on a square besides cars.
pub const EMPTY: u8 = 0;
pub const CONE: u8 = 1;
pub const CRATE: u8 = 2;

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Car {
    /// the square of its back end
    pub x: i8,
    pub y: i8,
    /// 2 for a car, 3 for a bus
    pub len: i8,
    /// the way its front faces, 0..4
    pub dir: i8,
    /// which model it is
    pub model: i8,
    /// 1 once it has left
    pub out: i8,
}

impl Car {
    /// its squares, back end first
    pub fn cells(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let d = self.dir as usize;
        (0..self.len as i32).map(move |i| (self.x as i32 + DX[d] * i, self.y as i32 + DY[d] * i))
    }
    /// the square at its back (way 1) or front (way 0) end
    pub fn end(&self, back: bool) -> (i32, i32) {
        let k = if back { 0 } else { self.len as i32 - 1 };
        let d = self.dir as usize;
        (self.x as i32 + DX[d] * k, self.y as i32 + DY[d] * k)
    }
}

/// A park: w x h squares, cones and crates, barriers along parts of its edge
/// and the cars.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Lot {
    pub w: i32,
    pub h: i32,
    pub props: [u8; MAXW * MAXH],
    /// a barrier past each edge square: top row, right column, bottom row,
    /// left column, MAXH or MAXW apart
    pub fence: [u8; 2 * (MAXW + MAXH)],
    pub n: i32,
    pub cars: [Car; MAXCARS],
}

/// Where a car going one way ends up: out of the park, or stopped by
/// something `gap` squares on.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Way {
    Out,
    /// squares it can move before the thing in its way, and that thing: a
    /// car, or -1 for a prop or a barrier
    Blocked { gap: i32, by: i32 },
}

impl Lot {
    pub fn empty(w: i32, h: i32) -> Lot {
        Lot { w, h, props: [EMPTY; MAXW * MAXH], fence: [0; 2 * (MAXW + MAXH)], n: 0, cars: [Car::default(); MAXCARS] }
    }
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h
    }
    pub fn prop(&self, x: i32, y: i32) -> u8 {
        self.props[(y * MAXW as i32 + x) as usize]
    }
    /// the car on a square, if any (cars that left are gone)
    pub fn car_at(&self, x: i32, y: i32) -> Option<usize> {
        (0..self.n as usize).find(|&i| self.cars[i].out == 0 && self.cars[i].cells().any(|c| c == (x, y)))
    }
    /// the barrier slot past edge square (x, y) going way d
    pub fn fence_slot(&self, x: i32, y: i32, d: usize) -> usize {
        let (mw, mh) = (MAXW as i32, MAXH as i32);
        (match d {
            0 => x,
            1 => mw + y,
            2 => mw + mh + x,
            _ => 2 * mw + mh + y,
        }) as usize
    }
    pub fn fenced(&self, x: i32, y: i32, d: usize) -> bool {
        self.fence[self.fence_slot(x, y, d)] != 0
    }
    /// what happens when car i drives its front's way (back = false) or backs up
    pub fn way(&self, i: usize, back: bool) -> Way {
        let c = &self.cars[i];
        let d = if back { (c.dir as usize + 2) % 4 } else { c.dir as usize };
        let (mut x, mut y) = c.end(back);
        // which car is on each square, looked up once for the whole way
        let mut on = [-1i8; MAXW * MAXH];
        for (j, car) in self.cars[..self.n as usize].iter().enumerate().filter(|(_, c)| c.out == 0) {
            for (cx, cy) in car.cells() {
                on[(cy * MAXW as i32 + cx) as usize] = j as i8;
            }
        }
        let mut gap = 0;
        loop {
            let (nx, ny) = (x + DX[d], y + DY[d]);
            if !self.inside(nx, ny) {
                return if self.fenced(x, y, d) { Way::Blocked { gap, by: -1 } } else { Way::Out };
            }
            if self.prop(nx, ny) != EMPTY {
                return Way::Blocked { gap, by: -1 };
            }
            let j = on[(ny * MAXW as i32 + nx) as usize];
            if j >= 0 {
                return Way::Blocked { gap, by: j as i32 };
            }
            (x, y) = (nx, ny);
            gap += 1;
        }
    }
    pub fn free(&self, i: usize) -> bool {
        self.way(i, false) == Way::Out || self.way(i, true) == Way::Out
    }
    pub fn left(&self) -> i32 {
        self.cars[..self.n as usize].iter().filter(|c| c.out == 0).count() as i32
    }
    /// a save's park holds together: sizes, cars inside and apart
    pub fn valid(&self) -> bool {
        if !(2..=MAXW as i32).contains(&self.w) || !(2..=MAXH as i32).contains(&self.h) || !(0..=MAXCARS as i32).contains(&self.n) {
            return false;
        }
        let mut used = [false; MAXW * MAXH];
        for y in 0..self.h {
            for x in 0..self.w {
                let p = self.prop(x, y);
                if p > CRATE {
                    return false;
                }
                used[(y * MAXW as i32 + x) as usize] = p != EMPTY;
            }
        }
        self.cars[..self.n as usize].iter().all(|c| {
            // a car's model, or a bus's, from the right set
            let models = models(c.len);
            if !(2..=3).contains(&c.len) || !(0..4).contains(&c.dir) || !(0..=1).contains(&c.out) || !models.contains(&c.model) {
                return false;
            }
            c.out == 1
                || c.cells().all(|(x, y)| {
                    let k = (y * MAXW as i32 + x) as usize;
                    let ok = self.inside(x, y) && !used[k];
                    if ok {
                        used[k] = true;
                    }
                    ok
                })
        })
    }
}

/// a level's size, cars and props: width, height, how full, crate and cone
/// count, barrier squares, buses
fn shape(level: i32) -> (i32, i32, f32, i32, i32, bool) {
    let l = level;
    let w = (4 + l / 6).min(MAXW as i32);
    let h = (w + 1 + (l % 3 == 2) as i32).min(MAXH as i32);
    let fill = 0.62 + 0.012 * l as f32;
    let props = if l >= 3 { 1 + l / 8 } else { 0 };
    let fence = if l >= 6 { 2 + l / 5 } else { 0 };
    (w, h, fill.min(0.86), props, fence, l >= 9)
}

/// Car models drawn: 10 cars, then the buses.
pub const CAR_MODELS: i8 = 10;
pub const BUS_MODELS: i8 = 2;
/// the models for a car (2 long) or a bus (3 long)
fn models(len: i8) -> std::ops::Range<i8> {
    if len == 3 { CAR_MODELS..CAR_MODELS + BUS_MODELS } else { 0..CAR_MODELS }
}

/// Deals a level. The same level always deals the same park.
pub fn deal(level: i32) -> Lot {
    let (w, h, fill, props, fence, buses) = shape(level);
    let mut rng = Rng::new(0x5EED + level as u32 * 7919);
    let mut best: Option<(i32, Lot)> = None;
    for _ in 0..24 {
        let lot = attempt(&mut rng, w, h, fill, props, fence, buses);
        // the fuller the better, and the fewer cars free from the start
        let free = (0..lot.n as usize).filter(|&i| lot.free(i)).count() as i32;
        let cover: i32 = lot.cars[..lot.n as usize].iter().map(|c| c.len as i32).sum();
        let mark = cover * 4 - free * 3;
        if best.as_ref().is_none_or(|b| mark > b.0) {
            best = Some((mark, lot));
        }
    }
    best.unwrap().1
}

fn attempt(rng: &mut Rng, w: i32, h: i32, fill: f32, props: i32, fence: i32, buses: bool) -> Lot {
    let mut lot = Lot::empty(w, h);
    for _ in 0..props {
        let (x, y) = (rng.range(0, w - 1), rng.range(0, h - 1));
        lot.props[(y * MAXW as i32 + x) as usize] = if rng.below(2) == 0 { CONE } else { CRATE };
    }
    for _ in 0..fence {
        let d = rng.below(4) as usize;
        let (x, y) = match d {
            0 => (rng.range(0, w - 1), 0),
            1 => (w - 1, rng.range(0, h - 1)),
            2 => (rng.range(0, w - 1), h - 1),
            _ => (0, rng.range(0, h - 1)),
        };
        let slot = lot.fence_slot(x, y, d);
        lot.fence[slot] = 1;
    }
    let want = ((w * h) as f32 * fill) as i32;
    let mut cover = 0;
    let mut tries = 0;
    // Each car parks only where it could drive straight out past the cars
    // already parked, so taking them away last first always works.
    while cover < want && tries < 3000 && (lot.n as usize) < MAXCARS {
        tries += 1;
        let len = if buses && rng.below(6) == 0 { 3 } else { 2 };
        let dir = rng.below(4) as i8;
        let car = Car { x: rng.range(0, w - 1) as i8, y: rng.range(0, h - 1) as i8, len, dir, model: 0, out: 0 };
        if !car.cells().all(|(x, y)| lot.inside(x, y) && lot.prop(x, y) == EMPTY && lot.car_at(x, y).is_none()) {
            continue;
        }
        let i = lot.n as usize;
        lot.cars[i] = car;
        lot.n += 1;
        if lot.free(i) {
            cover += len as i32;
            let m = models(len);
            lot.cars[i].model = m.start + rng.below((m.end - m.start) as u32) as i8;
        } else {
            lot.n -= 1;
        }
    }
    // the last parked can leave first; shuffle nothing else, so the order
    // of the array means nothing to the player
    lot
}

#[cfg(test)]
mod tests {
    use super::*;

    /// every car leaves when the free ones go first, again and again
    fn clears(mut lot: Lot) -> bool {
        loop {
            let Some(i) = (0..lot.n as usize).find(|&i| lot.cars[i].out == 0 && lot.free(i)) else {
                return lot.left() == 0;
            };
            lot.cars[i].out = 1;
        }
    }

    #[test]
    fn every_level_clears() {
        for level in 0..LEVELS {
            let lot = deal(level);
            assert!(lot.valid(), "level {level} invalid");
            assert!(clears(lot), "level {level} stuck");
            let free = (0..lot.n as usize).filter(|&i| lot.free(i)).count();
            let cover: i32 = lot.cars[..lot.n as usize].iter().map(|c| c.len as i32).sum();
            println!("level {:2}: {}x{} cars {:2} free {:2} cover {:.0}%", level + 1, lot.w, lot.h, lot.n, free, 100.0 * cover as f32 / (lot.w * lot.h) as f32);
        }
    }
}
