//! The pipes: each square's openings, turning, which squares the water
//! reaches from the tap, and the level deals.
use wg::Rng;

pub const MAXN: usize = 8;
pub const LEVELS: i32 = 30;

/// openings as bits: up, right, down, left
pub const UP: u8 = 1;
pub const RIGHT: u8 = 2;
pub const DOWN: u8 = 4;
pub const LEFT: u8 = 8;
pub const DX: [i32; 4] = [0, 1, 0, -1];
pub const DY: [i32; 4] = [-1, 0, 1, 0];

/// a pipe's openings turned a quarter clockwise `r` times
pub fn turned(mask: u8, r: u8) -> u8 {
    let mut m = mask & 15;
    for _ in 0..r % 4 {
        m = ((m << 1) | (m >> 3)) & 15;
    }
    m
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct Grid {
    pub n: i32,
    /// the row the tap feeds, on the left, and the row the drain takes, on the right
    pub tap: i32,
    pub drain: i32,
    /// each pipe's openings as dealt, and its quarter turns since
    pub pipe: [u8; MAXN * MAXN],
    pub rot: [u8; MAXN * MAXN],
}
// SAFETY: #[repr(C)], integers only
unsafe impl wg::Plain for Grid {}

impl Grid {
    pub fn open(&self, x: i32, y: i32) -> u8 {
        let i = (y * self.n + x) as usize;
        turned(self.pipe[i], self.rot[i])
    }
    pub fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.n && y < self.n
    }
    /// how far the water is from the tap on each square (pipes it passed),
    /// -1 where it doesn't reach
    pub fn flow(&self) -> [i32; MAXN * MAXN] {
        let mut dist = [-1i32; MAXN * MAXN];
        if self.open(0, self.tap) & LEFT == 0 {
            return dist;
        }
        let mut queue = vec![(0, self.tap)];
        dist[(self.tap * self.n) as usize] = 0;
        let mut at = 0;
        while at < queue.len() {
            let (x, y) = queue[at];
            at += 1;
            let here = self.open(x, y);
            let d = dist[(y * self.n + x) as usize];
            for k in 0..4 {
                if here & (1 << k) == 0 {
                    continue;
                }
                let (nx, ny) = (x + DX[k], y + DY[k]);
                if !self.inside(nx, ny) {
                    continue;
                }
                let back = 1 << ((k + 2) % 4);
                let j = (ny * self.n + nx) as usize;
                if self.open(nx, ny) & back != 0 && dist[j] < 0 {
                    dist[j] = d + 1;
                    queue.push((nx, ny));
                }
            }
        }
        dist
    }
    /// whether the water reaches the drain
    pub fn through(&self) -> bool {
        let x = self.n - 1;
        self.flow()[(self.drain * self.n + x) as usize] >= 0 && self.open(x, self.drain) & RIGHT != 0
    }
    pub fn valid(&self) -> bool {
        let cells = (self.n * self.n) as usize;
        (3..=MAXN as i32).contains(&self.n)
            && (0..self.n).contains(&self.tap)
            && (0..self.n).contains(&self.drain)
            && self.pipe.iter().enumerate().all(|(i, &p)| if i < cells { p > 0 && p < 16 } else { p == 0 })
            && self.rot.iter().all(|&r| r < 4)
    }
}

/// the board's side in level l (0 for the first): 4 up to 8
pub fn side(level: i32) -> i32 {
    4 + (level * 4 + 14) / (LEVELS - 1)
}

/// A random walk from the tap to the drain that doesn't cross itself,
/// as squares in order.
fn walk(n: i32, tap: i32, drain: i32, rng: &mut Rng, min_len: usize) -> Option<Vec<(i32, i32)>> {
    for _ in 0..400 {
        let mut path = vec![(0, tap)];
        let mut seen = [false; MAXN * MAXN];
        seen[(tap * n) as usize] = true;
        while let Some(&(x, y)) = path.last() {
            if (x, y) == (n - 1, drain) {
                if path.len() >= min_len {
                    return Some(path);
                }
                break;
            }
            let mut dirs = [0usize, 1, 2, 3];
            rng.shuffle(&mut dirs);
            // lean toward the right side, so walks end
            if rng.below(3) == 0 {
                dirs.sort_by_key(|&k| if k == 1 { 0 } else { 1 });
            }
            let next = dirs.iter().map(|&k| (x + DX[k], y + DY[k])).find(|&(nx, ny)| {
                nx >= 0 && ny >= 0 && nx < n && ny < n && !seen[(ny * n + nx) as usize]
            });
            match next {
                Some((nx, ny)) => {
                    seen[(ny * n + nx) as usize] = true;
                    path.push((nx, ny));
                }
                None => break,
            }
        }
    }
    None
}

/// The deal for a level, the same for everyone: a pipe line from the tap to
/// the drain, other pipes around it, and every pipe turned at random.
pub fn deal(level: i32) -> Grid {
    let n = side(level);
    let mut rng = Rng::new(0x464c_4f57 ^ (level as u32 + 1).wrapping_mul(2_654_435_761));
    let min_len = (n * n) as usize * (35 + level as usize) / 100;
    loop {
        let tap = rng.below(n as u32) as i32;
        let drain = rng.below(n as u32) as i32;
        let Some(path) = walk(n, tap, drain, &mut rng, min_len.max(n as usize)) else {
            continue;
        };
        let mut g = Grid { n, tap, drain, pipe: [0; MAXN * MAXN], rot: [0; MAXN * MAXN] };
        let dir_to = |a: (i32, i32), b: (i32, i32)| (0..4).find(|&k| a.0 + DX[k] == b.0 && a.1 + DY[k] == b.1).unwrap();
        for (i, &here) in path.iter().enumerate() {
            let inn = if i == 0 { LEFT } else { 1 << ((dir_to(here, path[i - 1])) as u8) };
            let out = if i + 1 == path.len() { RIGHT } else { 1 << (dir_to(here, path[i + 1]) as u8) };
            let mut m = inn | out;
            // now and then a branch off the line, to mislead
            if level > 4 && rng.below(6) == 0 {
                m |= [UP, RIGHT, DOWN, LEFT][rng.below(4) as usize];
            }
            g.pipe[(here.1 * n + here.0) as usize] = m;
        }
        // the rest: mostly bends and straights, a few three-ways
        for i in 0..(n * n) as usize {
            if g.pipe[i] == 0 {
                let m = match rng.below(10) {
                    4..=6 => UP | DOWN,
                    7..=8 => UP | RIGHT | DOWN,
                    _ => UP | RIGHT,
                };
                g.pipe[i] = turned(m, rng.below(4) as u8);
            }
        }
        for r in g.rot[..(n * n) as usize].iter_mut() {
            *r = rng.below(4) as u8;
        }
        if !g.through() {
            return g;
        }
    }
}

/// The par: the clockwise quarter turns from the deal to the pipes of a
/// solution. (Another way through may take fewer.)
pub fn par(g: &Grid, solved: &Grid) -> i32 {
    (0..(g.n * g.n) as usize)
        .map(|i| {
            let want = turned(solved.pipe[i], solved.rot[i]);
            (0..4).find(|&r| turned(g.pipe[i], (g.rot[i] + r) % 4) == want).unwrap_or(0) as i32
        })
        .sum()
}

/// Turns the pipes until the water gets through: depth first over the
/// squares it reaches, trying each turn of the next pipe. For tests and the par.
pub fn solve(g: &Grid) -> Option<Grid> {
    fn go(g: &mut Grid, x: i32, y: i32, from: u8, seen: &mut [bool; MAXN * MAXN]) -> bool {
        let i = (y * g.n + x) as usize;
        if seen[i] {
            return false;
        }
        seen[i] = true;
        let keep = g.rot[i];
        for r in 0..4 {
            g.rot[i] = r;
            let m = g.open(x, y);
            if m & from == 0 {
                continue;
            }
            if (x, y) == (g.n - 1, g.drain) && m & RIGHT != 0 {
                return true;
            }
            for k in 0..4 {
                let bit = 1u8 << k;
                if bit == from || m & bit == 0 {
                    continue;
                }
                let (nx, ny) = (x + DX[k], y + DY[k]);
                if g.inside(nx, ny) && go(g, nx, ny, 1 << ((k + 2) % 4), seen) {
                    return true;
                }
            }
        }
        g.rot[i] = keep;
        seen[i] = false;
        false
    }
    let mut s = *g;
    let mut seen = [false; MAXN * MAXN];
    go(&mut s, 0, g.tap, LEFT, &mut seen).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_level_deals() {
        for l in 0..LEVELS {
            let g = deal(l);
            assert!(g.valid() && !g.through());
            let s = solve(&g).expect("solvable");
            assert!(s.through());
            println!("level {} side {} par {}", l + 1, g.n, par(&g, &s));
        }
    }
}

