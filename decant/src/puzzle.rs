//! The tubes and their layers: moves, the level deals and a solver that
//! proves each deal can be finished.
#[cfg(test)]
use std::collections::HashSet;
#[cfg(test)]
use wg::Rng;

/// layers a tube holds
pub const CAP: usize = 4;
pub const MAXP: usize = 12;
pub const LEVELS: i32 = 100;

/// Tubes, bottom layer first; 0 is empty, colours are 1 and up.
#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct Stand {
    pub n: i32,
    pub tube: [[u8; CAP]; MAXP],
}
// SAFETY: #[repr(C)], integers only
unsafe impl wg::Plain for Stand {}

impl Stand {
    pub fn empty(n: i32) -> Stand {
        Stand { n, tube: [[0; CAP]; MAXP] }
    }
    pub fn len(&self, p: usize) -> usize {
        self.tube[p].iter().take_while(|&&c| c != 0).count()
    }
    pub fn top(&self, p: usize) -> u8 {
        let l = self.len(p);
        if l == 0 {
            0
        } else {
            self.tube[p][l - 1]
        }
    }
    /// how many layers of the top colour lie together at the top of tube p
    pub fn run(&self, p: usize) -> usize {
        let l = self.len(p);
        let mut k = 0;
        while k < l && self.tube[p][l - 1 - k] == self.tube[p][l - 1] {
            k += 1;
        }
        k
    }
    /// layers that would pour from a to b, 0 when the pour isn't allowed
    pub fn can(&self, a: usize, b: usize) -> usize {
        if a == b || a >= self.n as usize || b >= self.n as usize {
            return 0;
        }
        let (la, lb) = (self.len(a), self.len(b));
        if la == 0 || lb == CAP {
            return 0;
        }
        if lb > 0 && self.top(b) != self.top(a) {
            return 0;
        }
        self.run(a).min(CAP - lb)
    }
    /// pours k layers from a to b, unchecked
    pub fn shift(&mut self, a: usize, b: usize, k: usize) {
        for _ in 0..k {
            let la = self.len(a);
            let c = self.tube[a][la - 1];
            self.tube[a][la - 1] = 0;
            let lb = self.len(b);
            self.tube[b][lb] = c;
        }
    }
    pub fn done(&self, p: usize) -> bool {
        self.len(p) == CAP && self.run(p) == CAP
    }
    pub fn solved(&self) -> bool {
        (0..self.n as usize).all(|p| self.len(p) == 0 || self.done(p))
    }
    pub fn any_move(&self) -> bool {
        let n = self.n as usize;
        (0..n).any(|a| (0..n).any(|b| self.can(a, b) > 0))
    }
    /// tubes and colours in range, CAP layers of each colour, no gaps
    pub fn valid(&self, colours: usize) -> bool {
        if self.n < 1 || self.n as usize > MAXP {
            return false;
        }
        let mut count = [0usize; 16];
        for (p, tube) in self.tube.iter().enumerate() {
            let l = self.len(p);
            if p >= self.n as usize && l > 0 {
                return false;
            }
            for (i, &c) in tube.iter().enumerate() {
                if (i < l) != (c != 0) || c as usize > colours {
                    return false;
                }
                count[c as usize] += 1;
            }
        }
        (1..=colours).all(|c| count[c] == CAP)
    }
    /// the same position whatever order the tubes stand in
    #[cfg(test)]
    fn key(&self) -> [u64; 3] {
        let mut tubes: Vec<u16> = (0..self.n as usize).map(|p| self.tube[p].iter().fold(0u16, |k, &c| k << 4 | c as u16)).collect();
        tubes.sort_unstable();
        let mut k = [0u64; 3];
        for (i, v) in tubes.iter().enumerate() {
            k[i / 4] |= (*v as u64) << (16 * (i % 4));
        }
        k
    }
}

/// Palette chapters: learn runs with two colours, then add one at a time.
pub fn colours(level: i32) -> usize {
    if level < 30 {
        [2, 3, 4, 5, 6, 7][(level.max(0) / 5) as usize]
    } else {
        6 + ((level - 30) / 25) as usize
    }
}

#[cfg(test)]
fn generate(level: i32) -> (Stand, i32) {
    if level < 5 {
        let rows = [[[1, 1, 2, 2], [2, 2, 1, 1]], [[1, 2, 2, 2], [2, 1, 1, 1]], [[1, 2, 1, 2], [2, 1, 2, 1]], [[1, 2, 2, 1], [2, 1, 1, 2]], [[1, 1, 2, 1], [2, 2, 1, 2]]];
        let mut st = Stand::empty(3);
        st.tube[..2].copy_from_slice(&rows[level as usize]);
        let par = solve(&st, 100_000).unwrap();
        return (st, par);
    }
    let c = colours(level);
    let mut rng = Rng::new(0xC011_2026 ^ (level as u32 + 1).wrapping_mul(2_654_435_761));
    // New-colour lessons start with longer runs, then split into singles.
    let target = c as i32 + 3 + (level % 5) * 2;
    let mut best: Option<(i32, Stand, i32)> = None;
    for _ in 0..240 {
        let mut bag: Vec<u8> = (1..=c as u8).flat_map(|k| [k; CAP]).collect();
        rng.shuffle(&mut bag);
        let mut st = Stand::empty(c as i32 + 1);
        for (i, &k) in bag.iter().enumerate() {
            st.tube[i / CAP][i % CAP] = k;
        }
        if (0..c).any(|p| st.done(p)) {
            continue;
        }
        let splits = st.tube[..c].iter().map(|t| t.windows(2).filter(|w| w[0] != w[1]).count() as i32).sum::<i32>();
        if let Some(par) = solve(&st, 100_000) {
            let score = (splits - target).abs() * 10 + (par - splits - c as i32).abs();
            if best.as_ref().is_none_or(|b| score < b.0) {
                best = Some((score, st, par));
            }
        }
    }
    let (_, st, par) = best.expect("solvable water-sort deal");
    (st, par)
}

pub fn deal(level: i32) -> (Stand, i32) {
    campaign(level)
}

/// Depth first, the most useful moves first, skipping positions seen
/// before. Some(moves) when it finds a way to finish within `limit` positions.
#[cfg(test)]
pub fn solve(st: &Stand, limit: usize) -> Option<i32> {
    solution(st, limit).map(|p| p.len() as i32)
}

/// the pours of a way to finish, from and to
#[cfg(test)]
pub fn solution(st: &Stand, limit: usize) -> Option<Vec<(usize, usize)>> {
    let mut seen = HashSet::new();
    let mut path = Vec::new();
    let mut cur = *st;
    dfs(&mut cur, &mut seen, limit, &mut path).then_some(path)
}

#[cfg(test)]
fn dfs(st: &mut Stand, seen: &mut HashSet<[u64; 3]>, limit: usize, path: &mut Vec<(usize, usize)>) -> bool {
    if st.solved() {
        return true;
    }
    if seen.len() >= limit || !seen.insert(st.key()) {
        return false;
    }
    let n = st.n as usize;
    // onto the same colour first, then into an empty tube
    let mut moves: Vec<(usize, usize, usize, i32)> = Vec::new();
    for a in 0..n {
        for b in 0..n {
            let k = st.can(a, b);
            if k == 0 {
                continue;
            }
            let lb = st.len(b);
            // a tube of one colour gains nothing by pouring into an empty one
            if lb == 0 && st.run(a) == st.len(a) {
                continue;
            }
            let rank = (lb == 0) as i32 * 10 - k as i32;
            moves.push((a, b, k, rank));
        }
    }
    moves.sort_by_key(|m| m.3);
    for (a, b, k, _) in moves {
        let before = *st;
        st.shift(a, b, k);
        path.push((a, b));
        if dfs(st, seen, limit, path) {
            return true;
        }
        path.pop();
        *st = before;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pours_conserve_volume_and_dead_ends_are_reachable() {
        let mut rng = Rng::new(91);
        let mut losing_levels = 0;
        for l in 0..LEVELS {
            let (st, _) = deal(l);
            let mut lost = false;
            for _ in 0..120 {
                let mut trial = st;
                for _ in 0..100 {
                    let n = trial.n as usize;
                    let moves: Vec<_> =
                        (0..n).flat_map(|a| (0..n).map(move |b| (a, b))).filter(|&(a, b)| trial.can(a, b) > 0 && !(trial.len(b) == 0 && trial.run(a) == trial.len(a))).collect();
                    if moves.is_empty() {
                        lost |= !trial.solved();
                        break;
                    }
                    let (a, b) = moves[rng.below(moves.len() as u32) as usize];
                    trial.shift(a, b, trial.can(a, b));
                    assert!(trial.valid(colours(l)), "volume changed on level {}", l + 1);
                }
                if lost {
                    break;
                }
            }
            losing_levels += lost as usize;
        }
        // Small opening lessons may allow every sensible move. Most of
        // the campaign must require planning, not just repeated pouring.
        assert!(losing_levels >= 20, "only {losing_levels} levels have a dead end");
    }

    #[test]
    fn every_level_deals() {
        for l in 0..LEVELS {
            let (st, par) = deal(l);
            assert!(st.valid(colours(l)));
            assert!(!st.solved());
            assert!(par > 0);
            println!("level {} colours {} par {}", l + 1, colours(l), par);
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
            let (s, par) = deal(l);
            assert!(seen.insert(s.key()), "duplicate tube arrangement at {}", l + 1);
            assert_eq!(s.n as usize, colours(l) + 1);
            if l < 5 {
                assert_eq!(colours(l), 2);
                assert!(par <= 8);
            }
            let solution = solution(&s, 100_000).unwrap();
            assert_eq!(solution.len() as i32, par, "stale target at {}", l + 1);
        }
    }
}
