//! The table and its rules: FreeCell and Spider on one set of piles.
//!
//! A card is an id: 0..52 in FreeCell, 0..104 in Spider (two decks). Its
//! rank is id % 13 (0 the ace, 12 the king); its suit depends on the game:
//! 0 spades, 1 hearts, 2 diamonds, 3 clubs.

use wg::Rng;

/// Piles: the Spider stock, four free cells, eight foundations (FreeCell
/// uses four) and ten columns (FreeCell uses eight).
pub const NPILES: usize = 23;
pub const STOCK: usize = 0;
pub const CELL: usize = 1;
pub const FOUND: usize = 5;
pub const COL: usize = 13;
/// most cards a pile can hold
pub const MAXP: usize = 104;

/// Games, in the catalog's `game` order.
pub const FREECELL: i32 = 0;
pub const MODES: i32 = 4;

/// The FreeCell deal Microsoft's numbering has no answer for.
const UNSOLVABLE: u32 = 11982;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Table {
    pub c: [[u8; MAXP]; NPILES],
    pub n: [u8; NPILES],
    /// face-down cards at the bottom of each column (Spider)
    pub down: [u8; 10],
    pub mode: i32,
}
// SAFETY: #[repr(C)], integers only
unsafe impl wg::Plain for Table {}

/// One step of a move, as the undo list keeps it: `count` cards from the
/// top of `from` onto `to`; `flip` when the card left on top of `from`
/// turned face up; `first` on the first step of what one undo takes back.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Step {
    pub from: usize,
    pub to: usize,
    pub count: usize,
    pub flip: bool,
    pub first: bool,
}

impl Step {
    pub fn pack(self) -> u32 {
        self.from as u32 | (self.to as u32) << 5 | (self.count as u32) << 10 | (self.flip as u32) << 17 | (self.first as u32) << 18
    }
    pub fn unpack(v: u32) -> Step {
        Step {
            from: (v & 31) as usize,
            to: (v >> 5 & 31) as usize,
            count: (v >> 10 & 127) as usize,
            flip: v >> 17 & 1 != 0,
            first: v >> 18 & 1 != 0,
        }
    }
}

/// Where a hint or a tap sends cards: cards `idx..` of `from` onto `to`, or
/// a new row from the stock.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Play {
    Move { from: usize, idx: usize, to: usize },
    Deal,
}

/// A FreeCell deal by its number, 1 to 32000, dealt as Microsoft's FreeCell
/// deals it (its well-known random number generator), so deal numbers match.
pub fn deal_freecell(num: u32) -> Table {
    let mut t = Table::empty(FREECELL);
    // the deck in that game's order: ace of clubs, diamonds, hearts,
    // spades, then the twos...; our suits are spades, hearts, diamonds, clubs
    const SUIT: [u8; 4] = [3, 2, 1, 0];
    let mut deck = [0u8; 52];
    for (i, d) in deck.iter_mut().enumerate() {
        *d = SUIT[i % 4] * 13 + (i / 4) as u8;
    }
    let mut seed = num;
    for i in 0..52 {
        seed = (seed.wrapping_mul(214_013).wrapping_add(2_531_011)) & 0x7fff_ffff;
        let left = 52 - i;
        let j = (seed >> 16) as usize % left;
        let card = deck[j];
        deck[j] = deck[left - 1];
        t.push(COL + i % 8, card);
    }
    t
}

/// A deal number from a run's seed, never the one deal that can't be won.
pub fn deal_number(seed: u32) -> u32 {
    let n = 1 + seed % 32000;
    if n == UNSOLVABLE {
        n + 1
    } else {
        n
    }
}

/// A Spider deal: 54 cards in ten columns (six in the first four, five in
/// the rest), only the top one of each face up; 50 in the stock.
pub fn deal_spider(mode: i32, rng: &mut Rng) -> Table {
    let mut t = Table::empty(mode);
    let mut deck = [0u8; 104];
    for (i, d) in deck.iter_mut().enumerate() {
        *d = i as u8;
    }
    rng.shuffle(&mut deck);
    let mut k = 0;
    for row in 0..6 {
        for col in 0..10 {
            if row == 5 && col >= 4 {
                continue;
            }
            t.push(COL + col, deck[k]);
            k += 1;
        }
    }
    for col in 0..10 {
        t.down[col] = t.n[COL + col] - 1;
    }
    while k < 104 {
        t.push(STOCK, deck[k]);
        k += 1;
    }
    t
}

impl Table {
    pub fn empty(mode: i32) -> Table {
        Table { c: [[0; MAXP]; NPILES], n: [0; NPILES], down: [0; 10], mode }
    }
    pub fn spider(&self) -> bool {
        self.mode != FREECELL
    }
    pub fn cols(&self) -> usize {
        if self.spider() {
            10
        } else {
            8
        }
    }
    pub fn founds(&self) -> usize {
        if self.spider() {
            8
        } else {
            4
        }
    }
    pub fn cards(&self) -> usize {
        if self.spider() {
            104
        } else {
            52
        }
    }
    /// every pile this game uses
    pub fn piles(&self) -> impl Iterator<Item = usize> {
        let spider = self.spider();
        let (cols, founds) = (self.cols(), self.founds());
        (0..NPILES).filter(move |&p| match p {
            STOCK => spider,
            p if p < FOUND => !spider,
            p if p < COL => p - FOUND < founds,
            p => p - COL < cols,
        })
    }
    pub fn is_col(&self, p: usize) -> bool {
        p >= COL && p - COL < self.cols()
    }
    pub fn is_cell(&self, p: usize) -> bool {
        !self.spider() && (CELL..FOUND).contains(&p)
    }
    pub fn is_found(&self, p: usize) -> bool {
        (FOUND..COL).contains(&p) && p - FOUND < self.founds()
    }

    pub fn rank(&self, id: u8) -> i32 {
        (id % 13) as i32
    }
    pub fn suit(&self, id: u8) -> i32 {
        let deck = (id / 13) as i32;
        match self.mode {
            FREECELL => deck,
            1 => 0,
            2 => deck % 2,
            _ => deck % 4,
        }
    }
    pub fn red(&self, id: u8) -> bool {
        matches!(self.suit(id), 1 | 2)
    }
    /// the card's face, 0..52, for drawing
    pub fn face(&self, id: u8) -> usize {
        (self.suit(id) * 13 + self.rank(id)) as usize
    }

    pub fn len(&self, p: usize) -> usize {
        self.n[p] as usize
    }
    pub fn card(&self, p: usize, i: usize) -> u8 {
        self.c[p][i]
    }
    pub fn top(&self, p: usize) -> Option<u8> {
        self.len(p).checked_sub(1).map(|i| self.c[p][i])
    }
    fn push(&mut self, p: usize, card: u8) {
        let n = self.len(p);
        self.c[p][n] = card;
        self.n[p] += 1;
    }
    /// face-down cards under column p (0 for other piles)
    pub fn down_of(&self, p: usize) -> usize {
        if self.is_col(p) {
            self.down[p - COL] as usize
        } else {
            0
        }
    }
    pub fn face_up(&self, p: usize, i: usize) -> bool {
        p != STOCK && i >= self.down_of(p)
    }

    /// b may lie on a as part of a run that moves together
    pub fn follows(&self, a: u8, b: u8) -> bool {
        self.rank(a) == self.rank(b) + 1
            && if self.spider() { self.suit(a) == self.suit(b) } else { self.red(a) != self.red(b) }
    }
    /// cards i.. of pile p can be picked up together
    pub fn movable(&self, p: usize, i: usize) -> bool {
        let n = self.len(p);
        if i >= n {
            return false;
        }
        if self.is_cell(p) {
            return true;
        }
        if !self.is_col(p) || i < self.down_of(p) {
            return false;
        }
        (i + 1..n).all(|k| self.follows(self.c[p][k - 1], self.c[p][k]))
    }
    /// where the run that can be picked up from column p starts
    pub fn run_start(&self, p: usize) -> usize {
        let n = self.len(p);
        let mut i = n.saturating_sub(1);
        while i > self.down_of(p) && self.follows(self.c[p][i - 1], self.c[p][i]) {
            i -= 1;
        }
        i
    }
    pub fn free_cells(&self) -> usize {
        if self.spider() {
            return 0;
        }
        (CELL..FOUND).filter(|&p| self.n[p] == 0).count()
    }
    pub fn empty_cols(&self, except: usize) -> usize {
        (COL..COL + self.cols()).filter(|&p| p != except && self.n[p] == 0).count()
    }
    /// how many cards FreeCell lets move at once onto column d: one more
    /// than the free cells, doubled for each empty column besides d
    pub fn capacity(&self, d: usize) -> usize {
        (1 + self.free_cells()) << self.empty_cols(d).min(6)
    }
    pub fn empty_cell(&self) -> Option<usize> {
        if self.spider() {
            return None;
        }
        (CELL..FOUND).find(|&p| self.n[p] == 0)
    }
    /// FreeCell foundation f takes this card next
    pub fn takes(&self, f: usize, card: u8) -> bool {
        match self.top(f) {
            None => self.rank(card) == 0,
            Some(t) => self.suit(t) == self.suit(card) && self.rank(card) == self.rank(t) + 1,
        }
    }
    /// a foundation that takes this card next (FreeCell)
    pub fn home_for(&self, card: u8) -> Option<usize> {
        if self.spider() {
            return None;
        }
        (FOUND..FOUND + 4).find(|&f| self.takes(f, card))
    }
    /// cards of each suit on the foundations (FreeCell)
    fn home_counts(&self) -> [i32; 4] {
        let mut h = [0; 4];
        for f in FOUND..FOUND + self.founds() {
            if let Some(t) = self.top(f) {
                h[self.suit(t) as usize] = self.len(f) as i32;
            }
        }
        h
    }
    /// cards on the foundations
    pub fn home(&self) -> usize {
        (FOUND..FOUND + self.founds()).map(|f| self.len(f)).sum()
    }
    pub fn won(&self) -> bool {
        self.home() == self.cards()
    }

    /// cards i.. of p may go onto d
    pub fn can_move(&self, p: usize, i: usize, d: usize) -> bool {
        if p == d || !self.movable(p, i) {
            return false;
        }
        let card = self.c[p][i];
        let count = self.len(p) - i;
        if self.spider() {
            return self.is_col(d) && self.top(d).is_none_or(|t| self.rank(t) == self.rank(card) + 1);
        }
        if self.is_found(d) {
            return count == 1 && self.takes(d, card);
        }
        if self.is_cell(d) {
            return count == 1 && self.n[d] == 0;
        }
        if !self.is_col(d) {
            return false;
        }
        count <= self.capacity(d)
            && self.top(d).is_none_or(|t| self.red(t) != self.red(card) && self.rank(t) == self.rank(card) + 1)
    }

    /// does a step; returns it with `flip` set when a face-down card turned up
    pub fn apply(&mut self, from: usize, to: usize, count: usize) -> Step {
        let n = self.len(from);
        for k in n - count..n {
            let card = self.c[from][k];
            self.push(to, card);
        }
        self.n[from] = (n - count) as u8;
        let mut flip = false;
        if self.is_col(from) {
            let (k, left) = (from - COL, self.len(from));
            if left > 0 && self.down[k] as usize >= left {
                self.down[k] = (left - 1) as u8;
                flip = true;
            }
        }
        Step { from, to, count, flip, first: false }
    }
    /// takes a step back
    pub fn unapply(&mut self, s: Step) {
        if s.flip && self.is_col(s.from) {
            self.down[s.from - COL] += 1;
        }
        let n = self.len(s.to);
        for k in n - s.count..n {
            let card = self.c[s.to][k];
            self.push(s.from, card);
        }
        self.n[s.to] = (n - s.count) as u8;
    }

    /// a step that can be taken back is one the table allows (for saves)
    pub fn can_unapply(&self, s: Step) -> bool {
        s.from < NPILES
            && s.to < NPILES
            && s.from != s.to
            && (1..=13).contains(&s.count)
            && self.piles().any(|p| p == s.from)
            && self.piles().any(|p| p == s.to)
            && s.count <= self.len(s.to)
            && self.len(s.from) + s.count <= MAXP
            && (!s.flip || (self.is_col(s.from) && self.len(s.from) > 0 && self.down[s.from - COL] as usize + 1 == self.len(s.from)))
    }

    /// a finished king-to-ace run of one suit on top of column p (Spider)
    pub fn full_run(&self, p: usize) -> bool {
        let n = self.len(p);
        if !self.spider() || n < 13 || n - 13 < self.down_of(p) {
            return false;
        }
        let k = self.c[p][n - 13];
        self.rank(k) == 12 && self.movable(p, n - 13)
    }
    pub fn free_found(&self) -> Option<usize> {
        (FOUND..FOUND + self.founds()).find(|&f| self.n[f] == 0)
    }
    /// a row can be dealt: cards in the stock and no empty column
    pub fn can_deal(&self) -> bool {
        self.spider() && self.n[STOCK] > 0 && self.empty_cols(NPILES) == 0
    }

    /// a FreeCell card that can go home now without being missed later: an
    /// ace or a two, or a card whose two lower cards of the other colour
    /// are home; any card once the table can be finished in order
    pub fn auto_home(&self) -> Option<(usize, usize)> {
        if self.spider() {
            return None;
        }
        let finish = self.finishable();
        let h = self.home_counts();
        let mut best: Option<(usize, usize, i32)> = None;
        for p in (CELL..FOUND).chain(COL..COL + 8) {
            let Some(card) = self.top(p) else { continue };
            let Some(f) = self.home_for(card) else { continue };
            let r = self.rank(card);
            let other = if self.red(card) { [h[0], h[3]] } else { [h[1], h[2]] };
            if (r <= 1 || other.iter().all(|&o| o >= r) || finish) && best.is_none_or(|b| r < b.2) {
                best = Some((p, f, r));
            }
        }
        best.map(|(p, f, _)| (p, f))
    }
    /// every column runs down in rank to its top, so every card can go home
    fn finishable(&self) -> bool {
        (COL..COL + 8).all(|p| (1..self.len(p)).all(|k| self.rank(self.c[p][k - 1]) > self.rank(self.c[p][k])))
    }

    /// moving cards idx.. off column p uncovers something new: the column
    /// empties, a face-down card turns up, or they don't lie on a card they
    /// belong on
    fn frees(&self, p: usize, idx: usize, strict: bool) -> bool {
        if !self.is_col(p) || idx == 0 {
            return true;
        }
        let (below, card) = (self.c[p][idx - 1], self.c[p][idx]);
        if idx == self.down_of(p) {
            return true;
        }
        if strict {
            !self.follows(below, card)
        } else {
            self.rank(below) != self.rank(card) + 1
        }
    }

    /// the best place for cards idx.. of p (a tap), or None
    pub fn best_target(&self, p: usize, idx: usize) -> Option<usize> {
        if !self.movable(p, idx) {
            return None;
        }
        let count = self.len(p) - idx;
        let cols = self.cols();
        // columns from the one after p round to the one before it
        let order = (0..cols).map(|k| COL + (p.saturating_sub(COL) + 1 + k) % cols).filter(|&d| d != p);
        let ok = |d: usize| self.can_move(p, idx, d);
        if self.spider() {
            let card = self.c[p][idx];
            let same = order.clone().find(|&d| self.top(d).is_some_and(|t| self.suit(t) == self.suit(card)) && ok(d));
            let any = || order.clone().find(|&d| self.n[d] > 0 && ok(d));
            let empty = || order.clone().find(|&d| self.n[d] == 0 && idx > 0 && ok(d));
            return same.or_else(any).or_else(empty);
        }
        if count == 1 {
            if let Some(f) = self.home_for(self.c[p][idx]) {
                return Some(f);
            }
        }
        let filled = order.clone().find(|&d| self.n[d] > 0 && ok(d));
        let empty = || order.clone().find(|&d| self.n[d] == 0 && (self.is_cell(p) || idx > 0) && ok(d));
        let cell = || if count == 1 && self.is_col(p) { self.empty_cell() } else { None };
        filled.or_else(empty).or_else(cell)
    }

    /// a move worth making, for a hint: the best one that doesn't lead
    /// back to a table `seen` says was already played
    pub fn hint(&self, seen: impl Fn(u32) -> bool) -> Option<Play> {
        let mut all = if self.spider() { self.hints_spider() } else { self.hints_freecell() };
        if self.can_deal() {
            all.push((Play::Deal, 0));
        }
        // the best first; ties keep the order they were found in
        all.sort_by_key(|&(_, worth)| -worth);
        all.into_iter().map(|(play, _)| play).find(|&play| {
            let mut t = *self;
            t.play(play);
            t.settle();
            !seen(t.sig())
        })
    }

    /// does a play, without keeping it
    pub fn play(&mut self, play: Play) {
        match play {
            Play::Move { from, idx, to } => {
                let n = self.len(from);
                self.apply(from, to, n - idx);
            }
            Play::Deal => {
                for k in 0..10 {
                    self.apply(STOCK, COL + k, 1);
                }
            }
        }
    }

    /// what follows on its own: cards going home, finished runs clearing
    pub fn settle(&mut self) {
        loop {
            if let Some((p, f)) = self.auto_home() {
                self.apply(p, f, 1);
            } else if let (Some(p), Some(f)) = ((COL..COL + 10).find(|&p| self.full_run(p)), self.free_found()) {
                self.apply(p, f, 13);
            } else {
                return;
            }
        }
    }

    /// the table's cards (not what is left behind past the tops)
    pub fn sig(&self) -> u32 {
        let mut h = wg::hash(0, &self.n);
        h = wg::hash(h, &self.down);
        for p in 0..NPILES {
            h = wg::hash(h, &self.c[p][..self.len(p)]);
        }
        h
    }

    fn hints_freecell(&self) -> Vec<(Play, i32)> {
        let cols = COL..COL + 8;
        let mut all = Vec::new();
        let mut offer = |play: Play, worth: i32| all.push((play, worth));
        // a card that can go home
        for p in (CELL..FOUND).chain(cols.clone()) {
            if let Some(f) = self.top(p).and_then(|c| self.home_for(c)) {
                offer(Play::Move { from: p, idx: self.len(p) - 1, to: f }, 1000 - self.rank(self.top(p).unwrap()));
            }
        }
        // a run onto a column, uncovering the lowest card
        for p in cols.clone() {
            let n = self.len(p);
            for idx in self.run_start(p)..n {
                if !self.frees(p, idx, true) {
                    continue;
                }
                let worth = if idx == 0 { 30 } else { 40 - self.rank(self.c[p][idx - 1]) };
                for d in cols.clone() {
                    if self.n[d] > 0 && self.can_move(p, idx, d) {
                        offer(Play::Move { from: p, idx, to: d }, 100 + worth);
                    }
                }
            }
        }
        // a free cell's card back onto a column
        for p in CELL..FOUND {
            for d in cols.clone() {
                if self.n[p] > 0 && self.n[d] > 0 && self.can_move(p, 0, d) {
                    offer(Play::Move { from: p, idx: 0, to: d }, 90);
                }
            }
        }
        // into an empty column or a free cell, to reach a lower card
        let empty = cols.clone().find(|&d| self.n[d] == 0);
        for p in cols.clone() {
            let n = self.len(p);
            if n < 2 {
                continue;
            }
            let start = self.run_start(p);
            if let Some(d) = empty {
                if start > 0 && self.can_move(p, start, d) && self.frees(p, start, true) {
                    offer(Play::Move { from: p, idx: start, to: d }, 60 - self.rank(self.c[p][start - 1]));
                }
            }
            if let Some(c) = self.empty_cell() {
                if self.frees(p, n - 1, true) {
                    offer(Play::Move { from: p, idx: n - 1, to: c }, 30 - self.rank(self.c[p][n - 2]));
                }
            }
        }
        all
    }

    fn hints_spider(&self) -> Vec<(Play, i32)> {
        let cols = COL..COL + 10;
        let mut all = Vec::new();
        let mut offer = |play: Play, worth: i32| all.push((play, worth));
        for p in cols.clone() {
            let n = self.len(p);
            if n == 0 {
                continue;
            }
            let start = self.run_start(p);
            let reveals = start > 0 && start == self.down_of(p);
            for d in cols.clone() {
                if d == p || !self.can_move(p, start, d) {
                    continue;
                }
                let card = self.c[p][start];
                match self.top(d) {
                    Some(t) => {
                        let same = self.suit(t) == self.suit(card);
                        // onto its own suit: always worth it unless it
                        // already lies on its own suit
                        if self.frees(p, start, same) {
                            let base = if same { 300 } else { 200 };
                            offer(Play::Move { from: p, idx: start, to: d }, base + reveals as i32 * 50 + (n - start) as i32);
                        }
                    }
                    None => {
                        if start > 0 && (reveals || self.frees(p, start, false)) {
                            offer(Play::Move { from: p, idx: start, to: d }, 100 + reveals as i32 * 50);
                        }
                    }
                }
            }
            // part of a run onto another card of its suit
            for idx in start + 1..n {
                for d in cols.clone() {
                    if d != p && self.top(d).is_some_and(|t| self.suit(t) == self.suit(self.c[p][idx])) && self.can_move(p, idx, d) {
                        // only when what it leaves can then go somewhere of its own suit
                        let left = self.c[p][idx - 1];
                        let fits = cols.clone().any(|e| e != p && e != d && self.top(e).is_some_and(|t| self.follows(t, left)));
                        if fits {
                            offer(Play::Move { from: p, idx, to: d }, 150);
                        }
                    }
                }
            }
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(t: &Table, id: u8) -> String {
        let r = ["A", "2", "3", "4", "5", "6", "7", "8", "9", "T", "J", "Q", "K"][t.rank(id) as usize];
        let s = ["S", "H", "D", "C"][t.suit(id) as usize];
        format!("{}{}", r, s)
    }

    #[test]
    fn deal_one_matches() {
        // deal #1's first row, as every FreeCell deal list prints it
        let t = deal_freecell(1);
        let row: Vec<String> = (0..8).map(|k| name(&t, t.c[COL + k][0])).collect();
        assert_eq!(row.join(" "), "JD 2D 9H JC 5D 7H 7C 5H");
        let n: Vec<u8> = (0..8).map(|k| t.n[COL + k]).collect();
        assert_eq!(n, [7, 7, 7, 7, 6, 6, 6, 6]);
    }

    #[test]
    fn spider_deal() {
        let mut rng = Rng::new(5);
        let t = deal_spider(3, &mut rng);
        assert_eq!(t.n[STOCK], 50);
        assert_eq!((COL..COL + 10).map(|p| t.n[p] as usize).sum::<usize>(), 54);
        assert!(t.can_deal());
    }

    #[test]
    fn steps_round_trip() {
        let s = Step { from: 20, to: 7, count: 13, flip: true, first: true };
        assert_eq!(Step::unpack(s.pack()), s);
    }

    #[test]
    fn hints_play_freecell_deals() {
        // following hints alone wins many deals; it must never loop forever
        let mut wins = 0;
        for num in 1..=40 {
            let mut t = deal_freecell(num);
            for _ in 0..400 {
                while let Some((p, f)) = t.auto_home() {
                    t.apply(p, f, 1);
                }
                match t.hint(|_| false) {
                    Some(Play::Move { from, idx, to }) => {
                        let n = t.len(from);
                        assert!(t.can_move(from, idx, to));
                        t.apply(from, to, n - idx);
                    }
                    _ => break,
                }
            }
            wins += t.won() as i32;
        }
        assert!(wins > 0, "hints won no deal");
    }
}
