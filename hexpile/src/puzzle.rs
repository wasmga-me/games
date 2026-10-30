//! The board of hex stacks: where the cells are, which touch, and how tiles
//! slide from stack to stack once a stack is placed.
use wg::Rng;

/// cells on the large board (radius 3); the small one (radius 2) has 19
pub const MAXC: usize = 37;
/// tiles a stack can hold
pub const MAXH: usize = 40;
/// tiles a stack from the tray can have
pub const TRAYH: usize = 8;
/// a run of this many tiles of one colour clears
pub const CLEAR: usize = 10;
pub const COLOURS: u8 = 7;

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct Board {
    pub radius: i32,
    /// each cell's height, and its tiles from the bottom up (colours 1..=7)
    pub h: [u8; MAXC],
    pub tile: [[u8; MAXH]; MAXC],
    pub _pad: [u8; 3],
}
// SAFETY: #[repr(C)], integers only, padding spelled out
unsafe impl wg::Plain for Board {}

/// one thing happening on the board, in the order it happens
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Step {
    /// the top `n` tiles of colour `c` slide from one stack onto another
    Move { from: usize, to: usize, n: usize, c: u8 },
    /// the top `n` tiles of a stack clear, scoring `points`
    Clear { cell: usize, n: usize, c: u8, points: i32 },
}

/// the cells as axial coordinates (q, r), row by row from the back
pub fn cells(radius: i32) -> Vec<(i32, i32)> {
    let mut v = Vec::new();
    for r in -radius..=radius {
        for q in -radius..=radius {
            if (q + r).abs() <= radius {
                v.push((q, r));
            }
        }
    }
    v
}

pub fn count(radius: i32) -> usize {
    (3 * radius * (radius + 1) + 1) as usize
}

pub fn neighbours(radius: i32, i: usize) -> Vec<usize> {
    let all = cells(radius);
    let (q, r) = all[i];
    const AROUND: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];
    AROUND
        .iter()
        .filter_map(|&(dq, dr)| all.iter().position(|&c| c == (q + dq, r + dr)))
        .collect()
}

impl Board {
    pub fn new(radius: i32) -> Board {
        Board { radius, h: [0; MAXC], tile: [[0; MAXH]; MAXC], _pad: [0; 3] }
    }
    pub fn cells(&self) -> usize {
        count(self.radius)
    }
    pub fn top(&self, i: usize) -> u8 {
        let h = self.h[i] as usize;
        if h == 0 { 0 } else { self.tile[i][h - 1] }
    }
    /// how many tiles of the top colour sit together on top
    pub fn run(&self, i: usize) -> usize {
        let h = self.h[i] as usize;
        let c = self.top(i);
        (0..h).rev().take_while(|&k| self.tile[i][k] == c).count()
    }
    pub fn pure(&self, i: usize) -> bool {
        self.run(i) == self.h[i] as usize
    }
    pub fn empty_cells(&self) -> usize {
        (0..self.cells()).filter(|&i| self.h[i] == 0).count()
    }
    pub fn put(&mut self, i: usize, tiles: &[u8]) {
        for &t in tiles {
            let h = self.h[i] as usize;
            self.tile[i][h] = t;
            self.h[i] += 1;
        }
    }
    fn take(&mut self, i: usize, n: usize) {
        for _ in 0..n {
            self.h[i] -= 1;
            let h = self.h[i] as usize;
            self.tile[i][h] = 0;
        }
    }
    /// moves the top run of `from` onto `to`, if it fits
    fn slide(&mut self, from: usize, to: usize) -> Option<Step> {
        let n = self.run(from);
        let c = self.top(from);
        if self.h[to] as usize + n > MAXH {
            return None;
        }
        self.take(from, n);
        self.put(to, &vec![c; n]);
        Some(Step::Move { from, to, n, c })
    }
    pub fn apply(&mut self, step: &Step) {
        match *step {
            Step::Move { from, to, n, c } => {
                self.take(from, n);
                self.put(to, &vec![c; n]);
            }
            Step::Clear { cell, n, .. } => self.take(cell, n),
        }
    }

    /// After a stack lands on `start`: tiles of the same colour on touching
    /// stacks slide together, runs of ten or more clear, and the stacks
    /// that changed are looked at again until nothing more moves. Each
    /// gathering merges two runs into one, so it ends.
    pub fn settle(&mut self, start: usize) -> Vec<Step> {
        let mut steps = Vec::new();
        let mut queue = vec![start];
        let mut chain = 0;
        while !queue.is_empty() {
            let i = queue.remove(0);
            if self.h[i] == 0 {
                continue;
            }
            let c = self.top(i);
            let near: Vec<usize> = neighbours(self.radius, i).into_iter().filter(|&j| self.h[j] > 0 && self.top(j) == c).collect();
            if near.is_empty() {
                continue;
            }
            // a mixed stack next to one of just this colour gives its tiles
            // away, and its lower colours come free
            let target = if near.len() == 1 && self.pure(near[0]) && !self.pure(i) {
                let j = near[0];
                if let Some(s) = self.slide(i, j) {
                    steps.push(s);
                    queue.push(i);
                }
                j
            } else {
                for &j in &near {
                    if let Some(s) = self.slide(j, i) {
                        steps.push(s);
                        queue.push(j);
                    }
                }
                i
            };
            let n = self.run(target);
            if n >= CLEAR {
                chain += 1;
                let points = n as i32 * chain;
                steps.push(Step::Clear { cell: target, n, c, points });
                self.take(target, n);
                queue.push(target);
            }
        }
        steps
    }

    pub fn valid(&self) -> bool {
        let n = self.cells();
        (2..=3).contains(&self.radius)
            && self._pad == [0; 3]
            && (0..MAXC).all(|i| {
                let h = self.h[i] as usize;
                (i < n || h == 0)
                    && h <= MAXH
                    && self.tile[i].iter().enumerate().all(|(k, &t)| if k < h { (1..=COLOURS).contains(&t) } else { t == 0 })
            })
    }
}

/// colours in play: more as the run goes on
pub fn colours(radius: i32, cleared: i32) -> u8 {
    let base = if radius == 2 { 3 } else { 5 };
    (base + cleared / 120).min(COLOURS as i32) as u8
}

/// a new stack for the tray: 2 to 7 tiles (8 late in a run) in one to
/// three colour bands
pub fn deal_stack(rng: &mut Rng, colours: u8, cleared: i32) -> ([u8; TRAYH], u8) {
    // taller stacks in more bands as the run goes on
    let late = (cleared / 200).min(6) as u32;
    let top = 5 + late.min(3);
    let h = 2 + rng.below(top - 1) as usize;
    let bands = match rng.below(20) + late * 2 {
        0..=6 => 1,
        7..=15 => 2,
        _ => 3,
    }
    .min(h);
    // band sizes: each at least one tile
    let mut size = vec![1usize; bands];
    for _ in bands..h {
        size[rng.below(bands as u32) as usize] += 1;
    }
    let mut tiles = [0u8; TRAYH];
    let mut k = 0;
    let mut last = 0u8;
    for s in size {
        let mut c = 1 + rng.below(colours as u32) as u8;
        while c == last {
            c = 1 + rng.below(colours as u32) as u8;
        }
        last = c;
        for _ in 0..s {
            tiles[k] = c;
            k += 1;
        }
    }
    (tiles, h as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout() {
        assert_eq!(cells(2).len(), 19);
        assert_eq!(cells(3).len(), count(3));
        assert_eq!(neighbours(3, 18).len(), 6);
        assert_eq!(neighbours(3, 0).len(), 3);
    }

    #[test]
    fn gathers_and_clears() {
        let mut b = Board::new(2);
        let mid = 9;
        let near = neighbours(2, mid);
        b.put(near[0], &[2, 1, 1, 1, 1]);
        b.put(near[1], &[1, 1, 1, 1]);
        b.put(near[2], &[3, 3]);
        b.put(mid, &[3, 1, 1]);
        let steps = b.settle(mid);
        // both 1-runs come to the middle, ten clear, then the 3s meet
        assert!(steps.iter().any(|s| matches!(s, Step::Clear { n: 10, c: 1, .. })));
        assert_eq!((b.h[mid], b.top(mid)), (3, 3));
        assert_eq!((b.h[near[0]], b.h[near[1]], b.h[near[2]]), (1, 0, 0));
        assert!(b.valid());
    }
}
