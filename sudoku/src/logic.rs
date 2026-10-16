//! Sudoku logic: a solver that works the way a person does (singles, then
//! eliminations of growing difficulty), a generator that only keeps puzzles
//! this solver can finish, and the next step for a hint. A puzzle the solver
//! finishes by logic alone has exactly one solution, so every puzzle made
//! here is solvable and unique without guessing.

pub const ALL: u16 = 0x3FE; // digits 1-9 as bits 1-9

/// The 27 units: rows 0-8, columns 9-17, boxes 18-26.
pub static UNITS: [[u8; 9]; 27] = units();
/// The units each cell is in: row, column, box.
pub static UNIT_OF: [[u8; 3]; 81] = unit_of();
/// The 20 other cells sharing a unit with each cell.
pub static PEERS: [[u8; 20]; 81] = peers();

const fn units() -> [[u8; 9]; 27] {
    let mut u = [[0u8; 9]; 27];
    let mut i = 0;
    while i < 9 {
        let mut j = 0;
        while j < 9 {
            u[i][j] = (i * 9 + j) as u8;
            u[9 + i][j] = (j * 9 + i) as u8;
            let (br, bc) = (i / 3 * 3, i % 3 * 3);
            u[18 + i][j] = ((br + j / 3) * 9 + bc + j % 3) as u8;
            j += 1;
        }
        i += 1;
    }
    u
}

const fn unit_of() -> [[u8; 3]; 81] {
    let mut u = [[0u8; 3]; 81];
    let mut c = 0;
    while c < 81 {
        let (r, k) = (c / 9, c % 9);
        u[c] = [r as u8, (9 + k) as u8, (18 + r / 3 * 3 + k / 3) as u8];
        c += 1;
    }
    u
}

const fn peers() -> [[u8; 20]; 81] {
    let mut p = [[0u8; 20]; 81];
    let mut c = 0;
    while c < 81 {
        let (r, k) = (c / 9, c % 9);
        let mut n = 0;
        let mut o = 0;
        while o < 81 {
            let (orow, ok) = (o / 9, o % 9);
            let same = orow == r || ok == k || (orow / 3 == r / 3 && ok / 3 == k / 3);
            if o != c && same {
                p[c][n] = o as u8;
                n += 1;
            }
            o += 1;
        }
        c += 1;
    }
    p
}

#[inline]
pub fn count(m: u16) -> u32 {
    m.count_ones()
}
/// the lowest digit in a mask
#[inline]
pub fn first(m: u16) -> u8 {
    m.trailing_zeros() as u8
}

pub struct Rng(pub u32);
impl Rng {
    pub fn next(&mut self) -> u32 {
        let mut x = if self.0 == 0 { 0x9E37_79B9 } else { self.0 };
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    pub fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    pub fn shuffle(&mut self, a: &mut [u8]) {
        for i in (1..a.len()).rev() {
            let j = self.below(i as u32 + 1) as usize;
            a.swap(i, j);
        }
    }
}

/// How a hint's digit was found.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Why {
    /// the only digit left for the cell
    Only,
    /// the only place left for the digit in this unit
    Last(u8),
}

/// One digit found by logic.
#[derive(Clone, Copy, Debug)]
pub struct Step {
    pub cell: u8,
    pub digit: u8,
    pub why: Why,
    /// the hardest kind of reasoning needed before it, 1 to 3
    pub level: u8,
}

pub struct Solver {
    pub val: [u8; 81],
    pub cand: [u16; 81],
    pub hardest: u8,
}

impl Solver {
    /// None when two givens clash
    pub fn new(grid: &[u8; 81]) -> Option<Solver> {
        let mut s = Solver { val: [0; 81], cand: [ALL; 81], hardest: 0 };
        for c in 0..81 {
            if grid[c] != 0 {
                if s.cand[c] & (1 << grid[c]) == 0 {
                    return None;
                }
                s.place(c, grid[c]);
            }
        }
        Some(s)
    }

    pub fn place(&mut self, c: usize, d: u8) {
        self.val[c] = d;
        self.cand[c] = 0;
        for &p in PEERS[c].iter() {
            self.cand[p as usize] &= !(1 << d);
        }
    }

    pub fn solved(&self) -> bool {
        self.val.iter().all(|&v| v != 0)
    }

    fn stuck(&self) -> bool {
        (0..81).any(|c| self.val[c] == 0 && self.cand[c] == 0)
    }

    /// a hidden single first (in a box, then a row, then a column), as people
    /// usually spot those, then a naked single
    fn single(&self) -> Option<(usize, u8, Why)> {
        for &u in [18usize, 0, 9].iter() {
            for unit in u..u + 9 {
                for d in 1..=9u8 {
                    let mut at = 99;
                    let mut n = 0;
                    for &c in UNITS[unit].iter() {
                        if self.cand[c as usize] & (1 << d) != 0 {
                            at = c as usize;
                            n += 1;
                        }
                    }
                    if n == 1 {
                        return Some((at, d, Why::Last(unit as u8)));
                    }
                }
            }
        }
        for c in 0..81 {
            if self.val[c] == 0 && count(self.cand[c]) == 1 {
                return Some((c, first(self.cand[c]), Why::Only));
            }
        }
        None
    }

    /// one round of eliminations no harder than `max`; returns the level
    /// used, or 0 when nothing at that level helps
    fn eliminate(&mut self, max: u8) -> u8 {
        if max >= 2 && (self.locked() || self.naked_set(2) || self.hidden_set(2)) {
            return 2;
        }
        if max >= 3 && (self.naked_set(3) || self.hidden_set(3) || self.fish(2) || self.xy_wing() || self.fish(3)) {
            return 3;
        }
        0
    }

    /// The next digit logic finds, with eliminations no harder than `max`;
    /// those eliminations are kept.
    pub fn step(&mut self, max: u8) -> Option<Step> {
        let mut level = 1;
        loop {
            if self.stuck() {
                return None;
            }
            if let Some((c, d, why)) = self.single() {
                return Some(Step { cell: c as u8, digit: d, why, level });
            }
            let l = self.eliminate(max);
            if l == 0 {
                return None;
            }
            level = level.max(l);
        }
    }

    /// Solves as far as logic no harder than `max` goes; true if finished.
    pub fn run(&mut self, max: u8) -> bool {
        while !self.solved() {
            match self.step(max) {
                Some(s) => {
                    self.hardest = self.hardest.max(s.level);
                    self.place(s.cell as usize, s.digit);
                }
                None => return false,
            }
        }
        true
    }

    /// pointing and claiming: a digit kept to one line inside a box, or to
    /// one box along a line, leaves the rest of that box or line
    fn locked(&mut self) -> bool {
        let mut changed = false;
        for b in 18..27 {
            for line in 0..18 {
                let inter: [u8; 9] = UNITS[b];
                let lu = UNITS[line];
                for d in 1..=9u8 {
                    let bit = 1u16 << d;
                    let in_both = inter.iter().filter(|&&c| lu.contains(&c) && self.cand[c as usize] & bit != 0).count();
                    if in_both == 0 {
                        continue;
                    }
                    let box_rest = inter.iter().any(|&c| !lu.contains(&c) && self.cand[c as usize] & bit != 0);
                    let line_rest = lu.iter().any(|&c| !inter.contains(&c) && self.cand[c as usize] & bit != 0);
                    if !box_rest && line_rest {
                        for &c in lu.iter() {
                            if !inter.contains(&c) {
                                self.cand[c as usize] &= !bit;
                            }
                        }
                        changed = true;
                    } else if box_rest && !line_rest {
                        for &c in inter.iter() {
                            if !lu.contains(&c) {
                                self.cand[c as usize] &= !bit;
                            }
                        }
                        changed = true;
                    }
                }
            }
        }
        changed
    }

    /// n cells of a unit holding only n digits between them
    fn naked_set(&mut self, n: u32) -> bool {
        let mut changed = false;
        for u in 0..27 {
            let cells: [u8; 9] = UNITS[u];
            let open: heapless_cells::Cells = cells.iter().copied().filter(|&c| self.val[c as usize] == 0).collect();
            let k = open.len();
            if k <= n as usize {
                continue;
            }
            let mut pick = [0usize; 3];
            if combos(k, n as usize, &mut pick, &mut |p: &[usize]| {
                let mask = p.iter().fold(0u16, |m, &i| m | self.cand[open.get(i)]);
                if count(mask) != n {
                    return false;
                }
                let mut did = false;
                for i in 0..k {
                    if !p.contains(&i) && self.cand[open.get(i)] & mask != 0 {
                        self.cand[open.get(i)] &= !mask;
                        did = true;
                    }
                }
                did
            }) {
                changed = true;
            }
        }
        changed
    }

    /// n digits of a unit that fit only in the same n cells
    fn hidden_set(&mut self, n: u32) -> bool {
        let mut changed = false;
        for u in 0..27 {
            let cells: [u8; 9] = UNITS[u];
            let mut digits = [0usize; 9];
            let mut k = 0;
            for d in 1..=9u8 {
                let places = cells.iter().filter(|&&c| self.cand[c as usize] & (1 << d) != 0).count();
                if places >= 2 {
                    digits[k] = d as usize;
                    k += 1;
                }
            }
            if k <= n as usize {
                continue;
            }
            let mut pick = [0usize; 3];
            if combos(k, n as usize, &mut pick, &mut |p: &[usize]| {
                let dmask = p.iter().fold(0u16, |m, &i| m | 1 << digits[i]);
                let spots: u16 = cells
                    .iter()
                    .enumerate()
                    .filter(|(_, &c)| self.cand[c as usize] & dmask != 0)
                    .fold(0, |m, (i, _)| m | 1 << i);
                if count(spots) != n {
                    return false;
                }
                let mut did = false;
                for (i, &c) in cells.iter().enumerate() {
                    if spots & (1 << i) != 0 && self.cand[c as usize] & !dmask != 0 {
                        self.cand[c as usize] &= dmask;
                        did = true;
                    }
                }
                did
            }) {
                changed = true;
            }
        }
        changed
    }

    /// X-wing (n = 2) and swordfish (n = 3): a digit kept to the same n
    /// columns in n rows leaves the rest of those columns, and the other way
    fn fish(&mut self, n: usize) -> bool {
        let mut changed = false;
        for d in 1..=9u8 {
            let bit = 1u16 << d;
            for by_row in [true, false] {
                // where the digit can go in each line, as a mask of positions
                let mut lines = [0usize; 9];
                let mut pos = [0u16; 9];
                let mut k = 0;
                for l in 0..9 {
                    let mut m = 0u16;
                    for i in 0..9 {
                        let c = if by_row { l * 9 + i } else { i * 9 + l };
                        if self.cand[c] & bit != 0 {
                            m |= 1 << i;
                        }
                    }
                    if count(m) >= 2 && count(m) as usize <= n {
                        lines[k] = l;
                        pos[k] = m;
                        k += 1;
                    }
                }
                if k < n {
                    continue;
                }
                let mut pick = [0usize; 3];
                if combos(k, n, &mut pick, &mut |p: &[usize]| {
                    let cover = p.iter().fold(0u16, |m, &i| m | pos[i]);
                    if count(cover) as usize != n {
                        return false;
                    }
                    let mut did = false;
                    for l in 0..9 {
                        if p.iter().any(|&i| lines[i] == l) {
                            continue;
                        }
                        for i in 0..9 {
                            if cover & (1 << i) == 0 {
                                continue;
                            }
                            let c = if by_row { l * 9 + i } else { i * 9 + l };
                            if self.cand[c] & bit != 0 {
                                self.cand[c] &= !bit;
                                did = true;
                            }
                        }
                    }
                    did
                }) {
                    changed = true;
                }
            }
        }
        changed
    }

    /// XY-wing: a pivot with digits xy sees a cell xz and a cell yz; any
    /// cell seeing both wings can't hold z
    fn xy_wing(&mut self) -> bool {
        for pivot in 0..81 {
            let pm = self.cand[pivot];
            if count(pm) != 2 {
                continue;
            }
            for &a in PEERS[pivot].iter() {
                let am = self.cand[a as usize];
                if count(am) != 2 || am == pm || count(am & pm) != 1 {
                    continue;
                }
                for &b in PEERS[pivot].iter() {
                    let bm = self.cand[b as usize];
                    if b <= a || count(bm) != 2 || bm == pm || bm == am || count(bm & pm) != 1 {
                        continue;
                    }
                    let z = am & bm;
                    if count(z) != 1 || z & pm != 0 || (am | bm | pm) != (pm | z) {
                        continue;
                    }
                    let mut did = false;
                    for &c in PEERS[a as usize].iter() {
                        let c = c as usize;
                        if c != pivot && c != b as usize && PEERS[b as usize].contains(&(c as u8)) && self.cand[c] & z != 0 {
                            self.cand[c] &= !z;
                            did = true;
                        }
                    }
                    if did {
                        return true;
                    }
                }
            }
        }
        false
    }
}

/// Calls f with every choice of n of 0..k (n <= 3) until it returns true.
fn combos(k: usize, n: usize, pick: &mut [usize; 3], f: &mut dyn FnMut(&[usize]) -> bool) -> bool {
    fn go(start: usize, depth: usize, k: usize, n: usize, pick: &mut [usize; 3], f: &mut dyn FnMut(&[usize]) -> bool) -> bool {
        if depth == n {
            return f(&pick[..n]);
        }
        for i in start..k {
            pick[depth] = i;
            if go(i + 1, depth + 1, k, n, pick, f) {
                return true;
            }
        }
        false
    }
    go(0, 0, k, n, pick, f)
}

/// A tiny fixed list of up to 9 cells, so the solver needs no allocator.
mod heapless_cells {
    pub struct Cells {
        c: [u8; 9],
        n: usize,
    }
    impl Cells {
        pub fn len(&self) -> usize {
            self.n
        }
        pub fn get(&self, i: usize) -> usize {
            self.c[i] as usize
        }
    }
    impl core::iter::FromIterator<u8> for Cells {
        fn from_iter<I: IntoIterator<Item = u8>>(it: I) -> Self {
            let mut s = Cells { c: [0; 9], n: 0 };
            for v in it {
                s.c[s.n] = v;
                s.n += 1;
            }
            s
        }
    }
}

/// A random full grid.
fn full_grid(rng: &mut Rng) -> [u8; 81] {
    fn fill(g: &mut [u8; 81], rng: &mut Rng) -> bool {
        // the empty cell with fewest options
        let mut best = 99;
        let mut best_mask = 0;
        let mut best_n = 10;
        for c in 0..81 {
            if g[c] != 0 {
                continue;
            }
            let mut m = ALL;
            for &p in PEERS[c].iter() {
                m &= !(1 << g[p as usize]);
            }
            let n = count(m);
            if n < best_n {
                best = c;
                best_mask = m;
                best_n = n;
                if n == 0 {
                    return false;
                }
            }
        }
        if best == 99 {
            return true;
        }
        let mut ds = [1u8, 2, 3, 4, 5, 6, 7, 8, 9];
        rng.shuffle(&mut ds);
        for &d in ds.iter() {
            if best_mask & (1 << d) != 0 {
                g[best] = d;
                if fill(g, rng) {
                    return true;
                }
            }
        }
        g[best] = 0;
        false
    }
    let mut g = [0u8; 81];
    fill(&mut g, rng);
    g
}

fn solves(grid: &[u8; 81], max: u8) -> Option<u8> {
    let mut s = Solver::new(grid)?;
    if s.run(max) {
        Some(s.hardest)
    } else {
        None
    }
}

pub const LEVELS: usize = 4;
/// per difficulty: hardest reasoning allowed, hardest it must need, fewest
/// clues to stop digging at
const SPEC: [(u8, u8, usize); LEVELS] = [(1, 1, 38), (1, 1, 29), (2, 2, 0), (3, 3, 0)];

/// A puzzle of the difficulty (0 easy .. 3 expert) and its solution: clues
/// come out in pairs mirrored through the centre, while the solver can still
/// finish the grid with reasoning no harder than the difficulty allows.
pub fn generate(level: usize, rng: &mut Rng) -> ([u8; 81], [u8; 81]) {
    let (max, need, floor) = SPEC[level];
    let mut fallback: Option<([u8; 81], [u8; 81])> = None;
    for attempt in 0..60 {
        let solution = full_grid(rng);
        let mut puzzle = solution;
        let mut order: [u8; 41] = [0; 41];
        for (i, o) in order.iter_mut().enumerate() {
            *o = i as u8;
        }
        rng.shuffle(&mut order);
        let mut clues = 81;
        for &c in order.iter() {
            let (a, b) = (c as usize, 80 - c as usize);
            let gone = if a == b { 1 } else { 2 };
            if clues - gone < floor {
                continue;
            }
            let keep = (puzzle[a], puzzle[b]);
            puzzle[a] = 0;
            puzzle[b] = 0;
            if solves(&puzzle, max).is_some() {
                clues -= gone;
            } else {
                puzzle[a] = keep.0;
                puzzle[b] = keep.1;
            }
        }
        let hardest = solves(&puzzle, max).unwrap_or(0);
        if hardest >= need && (floor == 0 || clues <= floor + 3) {
            return (puzzle, solution);
        }
        // keep the hardest near miss, in case nothing better turns up
        if fallback.is_none() || attempt % 8 == 0 && hardest + 1 >= need {
            fallback = Some((puzzle, solution));
        }
    }
    fallback.unwrap()
}

/// The next digit to tell the player, from the givens and the digits they
/// placed right (wrong ones are left out); None only if the logic runs out.
pub fn hint(known: &[u8; 81]) -> Option<Step> {
    let mut s = Solver::new(known)?;
    s.step(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique(g: &[u8; 81]) -> usize {
        fn go(g: &mut [u8; 81], n: &mut usize) {
            if *n > 1 {
                return;
            }
            let Some(c) = (0..81).find(|&c| g[c] == 0) else {
                *n += 1;
                return;
            };
            for d in 1..=9u8 {
                if PEERS[c].iter().all(|&p| g[p as usize] != d) {
                    g[c] = d;
                    go(g, n);
                    g[c] = 0;
                }
            }
        }
        let mut n = 0;
        go(&mut g.clone(), &mut n);
        n
    }

    #[test]
    fn every_level_is_unique_and_graded() {
        let mut rng = Rng(12345);
        for level in 0..LEVELS {
            for _ in 0..6 {
                let t = std::time::Instant::now();
                let (p, s) = generate(level, &mut rng);
                let clues = p.iter().filter(|&&v| v != 0).count();
                let hardest = solves(&p, 3).unwrap();
                println!("level {level}: {clues} clues, hardest {hardest}, {:?}", t.elapsed());
                assert_eq!(unique(&p), 1);
                for c in 0..81 {
                    assert!(p[c] == 0 || p[c] == s[c]);
                }
                let mut full = Solver::new(&p).unwrap();
                assert!(full.run(3));
                assert_eq!(full.val, s);
            }
        }
    }

    #[test]
    fn hints_walk_to_the_end() {
        let mut rng = Rng(99);
        let (mut p, s) = generate(3, &mut rng);
        while p.iter().any(|&v| v == 0) {
            let h = hint(&p).expect("a hint");
            assert_eq!(s[h.cell as usize], h.digit);
            p[h.cell as usize] = h.digit;
        }
    }
}
