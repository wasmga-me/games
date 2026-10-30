//! The tubes and their layers: moves, the level deals and a solver that
//! proves each deal can be finished.
use std::collections::HashSet;
use wg::Rng;

/// layers a tube holds
pub const CAP: usize = 4;
pub const MAXP: usize = 12;
pub const LEVELS: i32 = 30;

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
        if l == 0 { 0 } else { self.tube[p][l - 1] }
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
    fn key(&self) -> [u64; 3] {
        let mut tubes: Vec<u16> = (0..self.n as usize)
            .map(|p| self.tube[p].iter().fold(0u16, |k, &c| k << 4 | c as u16))
            .collect();
        tubes.sort_unstable();
        let mut k = [0u64; 3];
        for (i, v) in tubes.iter().enumerate() {
            k[i / 4] |= (*v as u64) << (16 * (i % 4));
        }
        k
    }
}

/// colours in level l (0 for the first): 3 up to 10
pub fn colours(level: i32) -> usize {
    3 + ((level * 7 + 14) / (LEVELS - 1)) as usize
}

/// The deal for a level, the same for everyone, and the length of a
/// solution the solver found (the par).
pub fn deal(level: i32) -> (Stand, i32) {
    let c = colours(level);
    let mut rng = Rng::new(0x5249_4e47 ^ (level as u32 + 1).wrapping_mul(2_654_435_761));
    loop {
        let mut bag: Vec<u8> = (1..=c as u8).flat_map(|k| [k; CAP]).collect();
        rng.shuffle(&mut bag);
        let mut st = Stand::empty(c as i32 + 2);
        for (i, &k) in bag.iter().enumerate() {
            st.tube[i / CAP][i % CAP] = k;
        }
        // no layer already on one of its own colour, so nothing comes for free
        let together = (0..c).any(|p| (1..CAP).any(|i| st.tube[p][i] == st.tube[p][i - 1]));
        // the first levels may keep a pair together, later ones never
        if together && level >= 6 {
            continue;
        }
        if let Some(par) = solve(&st, 400_000) {
            return (st, par);
        }
    }
}

/// Depth first, the most useful moves first, skipping positions seen
/// before. Some(moves) when it finds a way to finish within `limit` positions.
pub fn solve(st: &Stand, limit: usize) -> Option<i32> {
    solution(st, limit).map(|p| p.len() as i32)
}

/// the pours of a way to finish, from and to
pub fn solution(st: &Stand, limit: usize) -> Option<Vec<(usize, usize)>> {
    let mut seen = HashSet::new();
    let mut path = Vec::new();
    let mut cur = *st;
    dfs(&mut cur, &mut seen, limit, &mut path).then_some(path)
}

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
