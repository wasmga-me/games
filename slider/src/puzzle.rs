//! The sliding puzzle itself: an n x n board of tiles 1..n²-1 and one gap.
//! Nothing here draws; lib.rs keeps it in the saved state.

pub const MAXN: usize = 10;
pub const CELLS: usize = MAXN * MAXN;

/// Tile numbers by square, row by row; 0 is the gap. Only the first n x n
/// squares (as n rows of n) are used.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Board {
    pub n: i32,
    pub gap: i32,
    pub tile: [u8; CELLS],
}

impl Board {
    pub fn solved(n: i32) -> Board {
        let mut b = Board { n, gap: n * n - 1, tile: [0; CELLS] };
        for i in 0..n * n - 1 {
            b.tile[i as usize] = (i + 1) as u8;
        }
        b
    }

    pub fn size(&self) -> i32 {
        self.n * self.n
    }

    /// The square tile t belongs on.
    pub fn home(t: u8) -> i32 {
        t as i32 - 1
    }

    pub fn is_solved(&self) -> bool {
        (0..self.size() - 1).all(|i| self.tile[i as usize] as i32 == i + 1)
    }

    pub fn in_place(&self) -> i32 {
        (0..self.size() - 1).filter(|&i| self.tile[i as usize] as i32 == i + 1).count() as i32
    }

    /// Sum over the tiles of how far each is from its square, in moves.
    pub fn distance(&self) -> i32 {
        let mut d = 0;
        for i in 0..self.size() {
            let t = self.tile[i as usize];
            if t == 0 {
                continue;
            }
            let h = Board::home(t);
            d += (i % self.n - h % self.n).abs() + (i / self.n - h / self.n).abs();
        }
        d
    }

    /// A uniformly random board that can be solved and isn't already.
    /// Half of all orders can't be reached by sliding: the order's parity
    /// (counting the gap as tile n²) must match the parity of the gap's
    /// distance from its own square, so a wrong one swaps two tiles.
    pub fn shuffled(n: i32, rng: &mut wg::Rng) -> Board {
        loop {
            let mut b = Board::solved(n);
            let size = n * n;
            for i in (1..size).rev() {
                let j = rng.below(i as u32 + 1) as i32;
                b.tile.swap(i as usize, j as usize);
            }
            b.gap = (0..size).find(|&i| b.tile[i as usize] == 0).unwrap_or(0);
            let mut perm = [0u8; CELLS];
            for i in 0..size as usize {
                perm[i] = if b.tile[i] == 0 { size as u8 } else { b.tile[i] };
            }
            let mut inv = 0;
            let mut seen = [false; CELLS];
            for i in 0..size as usize {
                if seen[i] {
                    continue;
                }
                let mut len = 0;
                let mut j = i;
                while !seen[j] {
                    seen[j] = true;
                    j = perm[j] as usize - 1;
                    len += 1;
                }
                inv += len - 1;
            }
            let gap_d = (n - 1 - b.gap % n) + (n - 1 - b.gap / n);
            if (inv + gap_d) % 2 != 0 {
                // swap the first two tiles that aren't the gap
                let (mut a, mut c) = (0usize, 1usize);
                if b.tile[a] == 0 {
                    a = 2;
                }
                if b.tile[c] == 0 {
                    c = 2;
                }
                b.tile.swap(a, c);
            }
            // a board this close to done is no puzzle
            if b.distance() >= n * n / 2 + 2 {
                return b;
            }
        }
    }

    /// Slides every tile between the gap and square sq toward the gap, if sq
    /// shares its row or column. Fills `moved` with the squares the tiles
    /// moved from (nearest the gap first) and returns how many.
    pub fn slide_to(&mut self, sq: i32, moved: &mut [i32; MAXN]) -> i32 {
        let n = self.n;
        if sq < 0 || sq >= self.size() || sq == self.gap {
            return 0;
        }
        let (gx, gy, x, y) = (self.gap % n, self.gap / n, sq % n, sq / n);
        let step = if gy == y {
            if x > gx { 1 } else { -1 }
        } else if gx == x {
            if y > gy { n } else { -n }
        } else {
            return 0;
        };
        let mut k = 0;
        while self.gap != sq {
            let from = self.gap + step;
            self.tile[self.gap as usize] = self.tile[from as usize];
            self.tile[from as usize] = 0;
            moved[k as usize] = from;
            self.gap = from;
            k += 1;
        }
        k
    }

    /// The square next to the gap from which a tile moving (dx, dy) comes.
    pub fn toward_gap(&self, dx: i32, dy: i32) -> i32 {
        let (x, y) = (self.gap % self.n - dx, self.gap / self.n - dy);
        if x < 0 || y < 0 || x >= self.n || y >= self.n {
            -1
        } else {
            y * self.n + x
        }
    }

    /// Checks a board read from a save: n in range, each tile once.
    pub fn valid(&self, n: i32) -> bool {
        if self.n != n || !(3..=MAXN as i32).contains(&n) {
            return false;
        }
        let mut seen = [false; CELLS];
        for i in 0..CELLS {
            let t = self.tile[i] as usize;
            if i >= (n * n) as usize {
                if t != 0 {
                    return false;
                }
                continue;
            }
            if t >= (n * n) as usize || seen[t] {
                return false;
            }
            seen[t] = true;
            if t == 0 && i as i32 != self.gap {
                return false;
            }
        }
        true
    }
}

// SAFETY: integers and a byte array, #[repr(C)]
unsafe impl wg::Plain for Board {}
