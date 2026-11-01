//! Bus Jam: clear a path through the crowd, then match the current bus.
//! Every deal includes a replayable solution; five waiting passengers lose.
#[cfg(test)]
use wg::Rng;
pub const LEVELS: i32 = 100;
pub const MAXN: usize = 6;
pub const CELLS: usize = MAXN * MAXN;
pub const BUSES: usize = CELLS / 3;
pub const BENCH: usize = 5;
pub const SEATS: u8 = 3;
pub const COLOURS: u8 = 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Yard {
    pub w: i32,
    pub h: i32,
    /// 0 = empty; passengers and buses use colours 1..=6.
    pub cell: [u8; CELLS],
    pub queue: [u8; BUSES],
    pub buses: i32,
    pub bus: i32,
    pub filled: i32,
    pub bench: [u8; 8],
    pub boarded: i32,
}
unsafe impl wg::Plain for Yard {}

impl Yard {
    pub fn empty(w: i32, h: i32) -> Self {
        Self { w, h, cell: [0; CELLS], queue: [0; BUSES], buses: 0, bus: 0, filled: 0, bench: [0; 8], boarded: 0 }
    }
    pub fn current(&self) -> u8 {
        if self.bus < self.buses {
            self.queue[self.bus as usize]
        } else {
            0
        }
    }
    pub fn waiting(&self) -> usize {
        self.bench[..BENCH].iter().filter(|&&c| c != 0).count()
    }
    pub fn left(&self) -> usize {
        self.cell.iter().filter(|&&c| c != 0).count()
    }
    pub fn won(&self) -> bool {
        self.bus == self.buses && self.left() == 0 && self.waiting() == 0
    }
    pub fn lost(&self) -> bool {
        !self.won() && self.waiting() == BENCH
    }
    /// A real route to the front edge, through empty cells only. Side and
    /// back edges are fenced: tapping a buried passenger cannot teleport it.
    pub fn path(&self, i: usize) -> Option<Vec<usize>> {
        let n = (self.w * self.h) as usize;
        if i >= n || self.cell[i] == 0 {
            return None;
        }
        let mut prev = [usize::MAX; CELLS];
        let mut todo = vec![i];
        prev[i] = i;
        let mut at = 0;
        while at < todo.len() {
            let j = todo[at];
            at += 1;
            let (x, y) = (j as i32 % self.w, j as i32 / self.w);
            if y == 0 {
                let mut path = vec![j];
                while *path.last().unwrap() != i {
                    path.push(prev[*path.last().unwrap()]);
                }
                path.reverse();
                return Some(path);
            }
            for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= self.w || ny >= self.h {
                    continue;
                }
                let k = (ny * self.w + nx) as usize;
                if self.cell[k] == 0 && prev[k] == usize::MAX {
                    prev[k] = j;
                    todo.push(k);
                }
            }
        }
        None
    }
    pub fn dispatch(&mut self, i: usize) -> bool {
        if self.lost() || self.won() || self.path(i).is_none() {
            return false;
        }
        let c = self.cell[i];
        self.cell[i] = 0;
        if c == self.current() {
            self.board();
        } else {
            let k = self.bench[..BENCH].iter().position(|&v| v == 0).unwrap();
            self.bench[k] = c;
        }
        // A departing bus reveals the next colour; waiting passengers
        // board immediately, including chains of several departing buses.
        while self.current() != 0 {
            let Some(k) = self.bench[..BENCH].iter().position(|&c| c == self.current()) else { break };
            self.bench[k] = 0;
            self.board();
        }
        true
    }
    fn board(&mut self) {
        self.filled += 1;
        self.boarded += 1;
        if self.filled == SEATS as i32 {
            self.filled = 0;
            self.bus += 1;
        }
    }
    pub fn valid(&self) -> bool {
        if !(3..=MAXN as i32).contains(&self.w)
            || !(3..=MAXN as i32).contains(&self.h)
            || !(1..=BUSES as i32).contains(&self.buses)
            || !(0..=self.buses).contains(&self.bus)
            || !(0..SEATS as i32).contains(&self.filled)
            || self.boarded != self.bus * 3 + self.filled
        {
            return false;
        }
        let mut counts = [0i32; COLOURS as usize + 1];
        for (i, &c) in self.cell.iter().enumerate() {
            if c > COLOURS || (i >= (self.w * self.h) as usize && c != 0) {
                return false;
            }
            counts[c as usize] += (c != 0) as i32;
        }
        for (i, &c) in self.bench.iter().enumerate() {
            if c > COLOURS || (i >= BENCH && c != 0) {
                return false;
            }
            counts[c as usize] += (c != 0) as i32;
        }
        for (i, &c) in self.queue.iter().enumerate() {
            if i < self.buses as usize {
                if !(1..=COLOURS).contains(&c) {
                    return false;
                }
                if i >= self.bus as usize {
                    counts[c as usize] -= 3;
                }
            } else if c != 0 {
                return false;
            }
        }
        if self.current() != 0 {
            counts[self.current() as usize] += self.filled;
        }
        counts.iter().all(|&c| c == 0)
    }
}

#[cfg(test)]
pub fn shape(level: i32) -> (i32, i32, u8) {
    if level >= 30 {
        return [(4, 6, 5), (5, 6, 5), (6, 6, 6), (6, 5, 6), (5, 6, 6)][(level % 5) as usize];
    }
    if level == 29 {
        return (6, 6, 6);
    }
    [(3, 3, 3), (3, 4, 3), (4, 4, 4), (4, 5, 4), (5, 5, 5), (5, 6, 6)][(level / 5) as usize]
}

pub fn deal(level: i32) -> Yard {
    campaign(level).0
}

#[cfg(test)]
fn random_play(mut y: Yard, rng: &mut Rng) -> bool {
    while !y.won() && !y.lost() {
        let open: Vec<_> = (0..(y.w * y.h) as usize).filter(|&i| y.path(i).is_some()).collect();
        if open.is_empty() {
            return false;
        }
        y.dispatch(open[rng.below(open.len() as u32) as usize]);
    }
    y.won()
}

#[cfg(test)]
fn generate_order(level: i32) -> (Yard, Vec<usize>) {
    if level < 3 {
        let boards = [[1, 1, 1, 2, 3, 2, 3, 2, 3], [2, 1, 1, 1, 2, 3, 3, 3, 2], [2, 3, 1, 1, 2, 3, 3, 1, 2]];
        let orders = [[0, 1, 2, 3, 5, 4, 7, 6, 8], [1, 2, 4, 3, 0, 5, 8, 6, 7], [2, 0, 1, 3, 4, 7, 8, 5, 6]];
        let mut y = Yard::empty(3, 3);
        y.cell[..9].copy_from_slice(&boards[level as usize]);
        y.queue[..3].copy_from_slice(&[1, 2, 3]);
        y.buses = 3;
        return (y, orders[level as usize].to_vec());
    }
    let (w, h, colours) = shape(level);
    let mut rng = Rng::new(0xb05a_2026 ^ (level as u32 + 1).wrapping_mul(7919));
    let difficulty = level.min(29);
    let target = 0.20 + (difficulty / 5) as f32 * 0.10 + (difficulty % 5) as f32 * 0.025;
    let mut best: Option<(f32, Yard, Vec<usize>)> = None;
    for _ in 0..80 {
        let mut y = Yard::empty(w, h);
        let total = (w * h / 3 * 3) as usize;
        y.buses = total as i32 / 3;
        y.cell[..total].fill(1);
        let mut trial = y;
        let mut order = Vec::new();
        while trial.left() > 0 {
            let open: Vec<_> = (0..total).filter(|&i| trial.path(i).is_some()).collect();
            let i = open[rng.below(open.len() as u32) as usize];
            order.push(i);
            trial.cell[i] = 0;
        }
        for b in 0..y.buses as usize {
            let mut c = 1 + rng.below(colours as u32) as u8;
            while b > 0 && y.queue[b - 1] == c {
                c = 1 + rng.below(colours as u32) as u8;
            }
            y.queue[b] = c;
        }
        let base: Vec<_> = y.queue[..y.buses as usize].iter().flat_map(|&c| [c; 3]).collect();
        // Mix a small lookahead window, retaining only routes that respect
        // the actual bench and automatic-boarding rules.
        for _ in 0..24 {
            let mut seq = base.clone();
            let window = 2 + (difficulty / 5) as u32 + (level % 5 >= 3) as u32;
            for k in 0..total {
                let j = (k + rng.below(window) as usize).min(total - 1);
                seq.swap(k, j);
            }
            for (&i, &c) in order.iter().zip(&seq) {
                y.cell[i] = c;
            }
            let mut check = y;
            if !order.iter().all(|&i| check.dispatch(i)) || !check.won() {
                continue;
            }
            let lost = (0..48).filter(|_| !random_play(y, &mut rng)).count();
            if lost == 0 {
                continue;
            }
            let off = (lost as f32 / 48.0 - target).abs();
            if best.as_ref().is_none_or(|b| off < b.0) {
                best = Some((off, y, order.clone()));
            }
            break;
        }
        if best.as_ref().is_some_and(|b| b.0 < 0.035) {
            break;
        }
    }
    let (_, y, order) = best.expect("validated passenger deal");
    (y, order)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_level_has_a_replayable_win_and_a_loss() {
        for level in 0..LEVELS {
            let (y, order) = deal_order(level);
            assert!(y.valid());
            assert_eq!(y, deal(level));
            let mut play = y;
            for i in order {
                assert!(play.dispatch(i));
                assert!(play.valid());
            }
            assert!(play.won());
            let mut rng = Rng::new(73);
            let losses = (0..200).filter(|_| !random_play(y, &mut rng)).count();
            assert!(losses > 0, "level {} cannot lose", level + 1);
            println!("level {}: {}x{}, {} passengers, random losses {}/200", level + 1, y.w, y.h, y.left(), losses);
        }
    }
    #[test]
    fn buried_passengers_need_an_open_route() {
        let mut y = Yard::empty(3, 3);
        y.cell[..9].fill(1);
        assert!(y.path(4).is_none());
        y.cell[1] = 0;
        assert_eq!(y.path(4), Some(vec![4, 1]));
    }
    #[test]
    fn waiting_passengers_board_successive_buses() {
        let mut y = Yard::empty(3, 3);
        y.buses = 2;
        y.queue[..2].copy_from_slice(&[1, 2]);
        y.filled = 2;
        y.boarded = 2;
        y.cell[0] = 1;
        y.bench[..3].fill(2);
        assert!(y.valid());
        assert!(y.dispatch(0));
        assert!(y.won());
        assert!(y.valid());
    }
}

#[cfg(test)]
#[path = "curation.rs"]
mod curation;

#[cfg(test)]
pub fn deal_order(level: i32) -> (Yard, Vec<usize>) {
    let (y, o) = campaign(level);
    (y, o.to_vec())
}

include!("campaign.rs");

#[cfg(test)]
mod campaign_checks {
    use super::*;
    #[test]
    fn distinct_levels_teach_then_combine() {
        let mut seen = std::collections::HashSet::new();
        for l in 0..LEVELS {
            let (mut y, order) = deal_order(l);
            assert!(seen.insert((y.w, y.h, y.cell, y.queue)), "duplicate yard {}", l + 1);
            assert_eq!((y.w, y.h), (shape(l).0, shape(l).1));
            let mut peak = 0;
            for i in order {
                assert!(y.dispatch(i));
                peak = peak.max(y.bench[..BENCH].iter().filter(|&&c| c != 0).count());
            }
            if l < 3 {
                assert!(peak <= l as usize + 1, "tutorial fills the bench");
            }
            assert!(y.won());
        }
    }
}
