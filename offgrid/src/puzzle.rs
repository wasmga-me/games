//! The board of arrows: which way each points, whether its way off the
//! board is clear, and the level deals.
use wg::Rng;

pub const MAXN: usize = 9;
pub const LEVELS: i32 = 30;

/// up, right, down, left; 0 is an empty square
pub const DX: [i32; 5] = [0, 0, 1, 0, -1];
pub const DY: [i32; 5] = [0, -1, 0, 1, 0];

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct Board {
    pub n: i32,
    pub cell: [u8; MAXN * MAXN],
}
// SAFETY: #[repr(C)], integers only
unsafe impl wg::Plain for Board {}

impl Board {
    pub fn at(&self, x: i32, y: i32) -> u8 {
        if x < 0 || y < 0 || x >= self.n || y >= self.n {
            0
        } else {
            self.cell[(y * self.n + x) as usize]
        }
    }
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.n && y < self.n
    }
    /// how many empty squares the arrow on (x, y) can move before it
    /// meets another, or None when its way off the board is clear
    pub fn blocked(&self, x: i32, y: i32) -> Option<i32> {
        let d = self.at(x, y) as usize;
        let (mut cx, mut cy, mut steps) = (x + DX[d], y + DY[d], 0);
        while self.inside(cx, cy) {
            if self.at(cx, cy) != 0 {
                return Some(steps);
            }
            cx += DX[d];
            cy += DY[d];
            steps += 1;
        }
        None
    }
    /// squares the arrow on (x, y) travels to leave the board
    pub fn exit(&self, x: i32, y: i32) -> i32 {
        match self.at(x, y) {
            1 => y + 1,
            2 => self.n - x,
            3 => self.n - y,
            _ => x + 1,
        }
    }
    pub fn count(&self) -> i32 {
        self.cell[..(self.n * self.n) as usize].iter().filter(|&&c| c != 0).count() as i32
    }
    pub fn valid(&self) -> bool {
        (3..=MAXN as i32).contains(&self.n)
            && self.cell.iter().enumerate().all(|(i, &c)| c <= 4 && (i < (self.n * self.n) as usize || c == 0))
    }
    /// whether every arrow can leave, taking them in some order
    #[cfg(test)]
    pub fn clearable(&self) -> bool {
        let mut b = *self;
        loop {
            let mut moved = false;
            for y in 0..b.n {
                for x in 0..b.n {
                    if b.at(x, y) != 0 && b.blocked(x, y).is_none() {
                        b.cell[(y * b.n + x) as usize] = 0;
                        moved = true;
                    }
                }
            }
            if b.count() == 0 {
                return true;
            }
            if !moved {
                return false;
            }
        }
    }
}

/// the board's side in level l (0 for the first): 4 up to 9
pub fn side(level: i32) -> i32 {
    4 + (level * 5 + 14) / (LEVELS - 1)
}

/// The deal for a level, the same for everyone. Arrows are placed one at a
/// time, each pointing a way that is clear of those placed before it, so
/// taking them off in the reverse order always works.
pub fn deal(level: i32) -> Board {
    let n = side(level);
    let cells = (n * n) as usize;
    // fuller boards as the levels go on
    let want = ((cells as f32) * (0.6 + 0.3 * level as f32 / (LEVELS - 1) as f32)) as usize;
    let mut rng = Rng::new(0x4f46_4647 ^ (level as u32 + 1).wrapping_mul(2_246_822_519));
    loop {
        let mut b = Board { n, cell: [0; MAXN * MAXN] };
        // the middle first and the edges last: the early arrows' long ways
        // out then cross the squares filled after them
        let mut order: Vec<(i32, usize)> = (0..cells)
            .map(|i| {
                let (x, y) = ((i % n as usize) as i32, (i / n as usize) as i32);
                let edge = x.min(y).min(n - 1 - x).min(n - 1 - y);
                (-(edge * 8 + rng.below(12) as i32), i)
            })
            .collect();
        order.sort();
        let mut placed = 0;
        for &(_, i) in &order {
            if placed == want {
                break;
            }
            let (x, y) = ((i % n as usize) as i32, (i / n as usize) as i32);
            // the longest clear way out, mostly
            let mut best = (0, -1);
            for d in 1..=4u8 {
                b.cell[i] = d;
                if b.blocked(x, y).is_none() {
                    let score = b.exit(x, y) * 4 + rng.below(9) as i32;
                    if score > best.1 {
                        best = (d, score);
                    }
                }
            }
            b.cell[i] = best.0;
            if best.0 != 0 {
                placed += 1;
            }
        }
        if placed + 2 >= want {
            return b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_level_deals() {
        for l in 0..LEVELS {
            let b = deal(l);
            assert!(b.valid() && b.clearable());
            let free = (0..b.n).flat_map(|y| (0..b.n).map(move |x| (x, y))).filter(|&(x, y)| b.at(x, y) != 0 && b.blocked(x, y).is_none()).count();
            println!("level {} side {} arrows {} free {}", l + 1, b.n, b.count(), free);
        }
    }
}
