//! The pipes: each square's openings, turning, which squares the water
//! reaches from the tap, and the level deals.
#[cfg(test)]
use wg::Rng;

pub const MAXN: usize = 8;
pub const LEVELS: i32 = 30;

/// openings as bits: up, right, down, left
#[cfg(test)]
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
        self.flow()[(self.drain * self.n + x) as usize] >= 0 && self.open(x, self.drain) & RIGHT != 0 && self.leaks().is_empty()
    }
    /// Unsealed openings on the live network. Unconnected decorative
    /// pipes don't leak; the inlet and garden outlet are intentional ports.
    pub fn leaks(&self) -> Vec<(i32, i32, usize)> {
        let flow = self.flow();
        let mut leaks = Vec::new();
        for y in 0..self.n {
            for x in 0..self.n {
                if flow[(y * self.n + x) as usize] < 0 {
                    continue;
                }
                for k in 0..4 {
                    if self.open(x, y) & (1 << k) == 0 {
                        continue;
                    }
                    if (x == 0 && y == self.tap && k == 3) || (x == self.n - 1 && y == self.drain && k == 1) {
                        continue;
                    }
                    let (nx, ny) = (x + DX[k], y + DY[k]);
                    if !self.inside(nx, ny) || self.open(nx, ny) & (1 << ((k + 2) % 4)) == 0 {
                        leaks.push((x, y, k));
                    }
                }
            }
        }
        leaks
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

pub fn side(level: i32) -> i32 {
    [3, 4, 5, 6, 7, 8][(level / 5) as usize]
}

#[cfg(test)]
fn generate(level: i32) -> Grid {
    let n = side(level);
    let lesson = level % 5;
    let mut path = Vec::new();
    if level == 0 {
        path = vec![(0, 1), (1, 1), (2, 1)];
    } else if level == 1 {
        path = vec![(0, 0), (1, 0), (1, 1), (2, 1)];
    } else if level == 2 {
        path = vec![(0, 2), (0, 1), (1, 1), (1, 0), (2, 0)];
    } else if level == 3 {
        path = vec![(0, 0), (0, 1), (0, 2), (1, 2), (1, 1), (1, 0), (2, 0)];
    } else {
        // A different corridor per lesson: straight, low bend, high bend,
        // double-back, and a full serpentine chapter finale.
        let rows = match lesson {
            0 => 1,
            1 | 2 => 2,
            3 => 3,
            _ => n,
        };
        let mut x = 0;
        for y in 0..rows {
            let yy = if lesson == 2 { n - 1 - y } else { y };
            let end = if y == rows - 1 {
                n - 1
            } else if y % 2 == 0 {
                n - 2
            } else {
                1
            };
            if !path.is_empty() {
                path.push((x, yy));
            } else {
                path.push((0, yy));
            }
            while x != end {
                x += if end > x { 1 } else { -1 };
                path.push((x, yy));
            }
        }
    }
    let tap = path[0].1;
    let drain = path.last().unwrap().1;
    let mut g = Grid { n, tap, drain, pipe: [0; MAXN * MAXN], rot: [0; MAXN * MAXN] };
    let dir = |a: (i32, i32), b: (i32, i32)| (0..4).find(|&k| a.0 + DX[k] == b.0 && a.1 + DY[k] == b.1).unwrap();
    for (j, &p) in path.iter().enumerate() {
        let inn = if j == 0 { LEFT } else { 1 << dir(p, path[j - 1]) };
        let out = if j + 1 == path.len() { RIGHT } else { 1 << dir(p, path[j + 1]) };
        g.pipe[(p.1 * n + p.0) as usize] = inn | out;
    }
    let mut rng = Rng::new(0xF10A_2026 + level as u32 * 7919);
    for i in 0..(n * n) as usize {
        if g.pipe[i] == 0 {
            g.pipe[i] = turned(if rng.below(3) == 0 { UP | DOWN } else { UP | RIGHT }, rng.below(4) as u8);
        }
    }
    // One turn, two bends, then increasing repair work. Decorative pipes
    // are already oriented: the opening lesson has one obvious broken joint.
    for (j, &(x, y)) in path.iter().enumerate() {
        if j == 0 || (level > 0 && (j + lesson as usize) % 3 != 0) {
            g.rot[(y * n + x) as usize] = if level < 5 { 3 } else { 1 + rng.below(3) as u8 };
        }
    }
    let inlet = (tap * n) as usize;
    while g.open(0, tap) & LEFT != 0 {
        g.rot[inlet] = (g.rot[inlet] + 1) % 4;
    }
    g
}

pub fn deal(level: i32) -> Grid {
    campaign(level)
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
    fn connected_outlet_with_an_open_branch_is_not_a_win() {
        let mut g = Grid { n: 4, tap: 0, drain: 0, pipe: [0; MAXN * MAXN], rot: [0; MAXN * MAXN] };
        g.pipe[..4].fill(LEFT | RIGHT);
        assert!(g.through());
        g.pipe[1] |= UP;
        assert!(!g.through());
        assert_eq!(g.leaks(), vec![(1, 0, 0)]);
        // A neighbouring pipe with its opening facing away cannot carry water.
        g.pipe[1] = UP | DOWN;
        assert_eq!(g.flow()[1], -1);
    }

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

#[cfg(test)]
#[path = "curation.rs"]
mod curation;

include!("campaign.rs");

#[cfg(test)]
mod campaign_checks {
    use super::*;
    #[test]
    fn distinct_levels_teach_then_combine() {
        let mut seen = std::collections::HashSet::new();
        for l in 0..LEVELS {
            let g = deal(l);
            assert!(seen.insert((g.n, g.tap, g.drain, g.pipe, g.rot)), "duplicate level {}", l + 1);
            assert_eq!(g.n, side(l));
            assert!(g.flow().iter().all(|&d| d < 0), "level {} starts wet", l + 1);
            let s = solve(&g).unwrap();
            assert!(s.through());
            let turns = par(&g, &s);
            if l == 0 {
                assert_eq!(turns, 1, "opening should teach a single turn");
            }
            // Even at three seconds per clockwise turn the route fits
            // the timer. Reading time before the first turn is unlimited.
            assert!(turns * 3 < 40 + g.n * g.n * 2 - l / 2, "level {} is a speed test", l + 1);
        }
    }
}
