//! The rules of Facets: an 8 by 8 board of gems; swap two neighbours to line
//! up three or more of a colour. Everything lives in one plain struct, so a
//! save is a byte copy of it (see lib.rs).

pub const N: usize = 8;
pub const COLOURS: u8 = 7;
/// a gem with no colour of its own: swapped with a gem, it takes that colour
pub const PRISM: u8 = 8;

pub const NORMAL: u8 = 0;
/// four in a line: clears the 3 by 3 square round it
pub const BOMB: u8 = 1;
/// lines crossing (an L or a T): clears its row and column
pub const CROSS: u8 = 2;

pub const SWAP_MS: i32 = 150;
pub const CLEAR_MS: i32 = 230;
pub const SHUFFLE_MS: i32 = 700;
pub const END_MS: i32 = 1400;
pub const TIMED_MS: i32 = 120_000;
pub const MOVES: i32 = 30;
pub const HINT_MS: i32 = 5000;
/// Classic: reshuffles for when no swap is left
pub const SHUFFLES: i32 = 3;
/// fall speed: cells per second gained per second, and the cap, both x1000
const GRAVITY: i32 = 42_000;
const MAX_VEL: i32 = 16_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(i32)]
pub enum Phase {
    Idle = 0,
    Swap = 1,
    Back = 2,
    Clear = 3,
    Fall = 4,
    Shuffle = 5,
    Over = 6,
}

pub const MODE_CLASSIC: i32 = 0;
pub const MODE_TIMED: i32 = 1;
pub const MODE_MOVES: i32 = 2;

pub const DIFF_EASY: i32 = 0;
pub const DIFF_NORMAL: i32 = 1;
pub const DIFF_HARD: i32 = 2;
/// per difficulty: how often (in 100ths) a new gem takes the colour of a
/// gem next to it, which makes lines of four and more, and so special gems
const KIN: [u32; 3] = [30, 18, 0];
/// per difficulty: how often (in 100ths) new gems that would leave no swap
/// are dealt again before they land, at level 1, and how much less each
/// level after; it reaches 0, so a Classic run still ends
const RESCUE: [i32; 3] = [100, 80, 0];
const RESCUE_FADE: [i32; 3] = [4, 5, 0];

/// why a run ended
pub const END_NONE: i32 = 0;
pub const END_STUCK: i32 = 1;
pub const END_TIME: i32 = 2;
pub const END_MOVES: i32 = 3;

pub const EV_BLIP: i32 = 1;
pub const EV_HIT: i32 = 2;
pub const EV_LOSE: i32 = 4;
pub const EV_GAMEOVER: i32 = 8;
pub const EV_LEVEL: i32 = 16;
pub const EV_WIN: i32 = 32;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Board {
    pub rng: u32,
    pub mode: i32,
    pub phase: i32,
    pub t: i32,
    /// the two squares of the swap in progress
    pub ax: i32,
    pub ay: i32,
    pub bx: i32,
    pub by: i32,
    pub score: i32,
    pub level: i32,
    pub progress: i32,
    pub time_left: i32,
    pub moves_left: i32,
    pub chain: i32,
    /// Classic: reshuffles left for when no swap is left
    pub shuffles: i32,
    pub idle_ms: i32,
    pub end: i32,
    pub end_ms: i32,
    /// stats
    pub time_ms: i32,
    pub moves: i32,
    pub cleared: i32,
    pub best_chain: i32,
    pub specials: i32,
    pub biggest: i32,
    pub diff: i32,
    /// times the board was shuffled
    pub shuffled: i32,
    /// gem colour (0 empty, 1..=7, PRISM) and kind of every square
    pub gem: [[u8; N]; N],
    pub kind: [[u8; N]; N],
    /// 1 while a square's gem is being cleared
    pub dying: [[u8; N]; N],
    /// how far above its square a falling gem is, and how fast it falls,
    /// in thousandths of a square (per second)
    pub off: [[i32; N]; N],
    pub vel: [[i32; N]; N],
}

/// what a step of the rules did, for the pictures: a gem cleared at a
/// square, points scored at a place
pub trait Sink {
    fn cleared(&mut self, x: usize, y: usize, colour: u8, kind: u8);
    fn points(&mut self, x: f32, y: f32, pts: i32, chain: i32);
    fn blast(&mut self, x: usize, y: usize, kind: u8);
}
pub struct NoSink;
impl Sink for NoSink {
    fn cleared(&mut self, _: usize, _: usize, _: u8, _: u8) {}
    fn points(&mut self, _: f32, _: f32, _: i32, _: i32) {}
    fn blast(&mut self, _: usize, _: usize, _: u8) {}
}

fn xorshift(s: &mut u32) -> u32 {
    let mut x = if *s == 0 { 0x9E37_79B9 } else { *s };
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *s = x;
    x
}

pub fn level_need(level: i32) -> i32 {
    1000 + 250 * (level - 1)
}

impl Board {
    pub const fn zeroed() -> Board {
        Board {
            rng: 0, mode: 0, phase: 0, t: 0, ax: 0, ay: 0, bx: 0, by: 0, score: 0, level: 1, progress: 0,
            time_left: 0, moves_left: 0, chain: 0, idle_ms: 0, end: 0, end_ms: 0, time_ms: 0, moves: 0,
            shuffles: 0, cleared: 0, best_chain: 0, specials: 0, biggest: 0, diff: 0, shuffled: 0, gem: [[0; N]; N], kind: [[0; N]; N],
            dying: [[0; N]; N], off: [[0; N]; N], vel: [[0; N]; N],
        }
    }

    pub fn phase(&self) -> Phase {
        match self.phase {
            1 => Phase::Swap,
            2 => Phase::Back,
            3 => Phase::Clear,
            4 => Phase::Fall,
            5 => Phase::Shuffle,
            6 => Phase::Over,
            _ => Phase::Idle,
        }
    }
    fn set(&mut self, p: Phase) {
        self.phase = p as i32;
        self.t = 0;
    }

    fn rand(&mut self, n: u32) -> u32 {
        xorshift(&mut self.rng) % n
    }
    fn colour(&mut self) -> u8 {
        1 + self.rand(COLOURS as u32) as u8
    }

    pub fn start(&mut self, seed: u32, mode: i32, diff: i32) {
        *self = Board::zeroed();
        self.rng = seed | 1;
        self.mode = mode;
        self.diff = diff;
        self.level = 1;
        self.time_left = if mode == MODE_TIMED { TIMED_MS } else { 0 };
        self.moves_left = if mode == MODE_MOVES { MOVES } else { 0 };
        self.shuffles = if mode == MODE_CLASSIC { SHUFFLES } else { 0 };
        self.fill();
        self.set(Phase::Idle);
    }

    /// a fresh board: no line of three, at least one move
    fn fill(&mut self) {
        loop {
            for y in 0..N {
                for x in 0..N {
                    self.kind[y][x] = NORMAL;
                    loop {
                        let c = self.colour();
                        let h = x >= 2 && self.gem[y][x - 1] == c && self.gem[y][x - 2] == c;
                        let v = y >= 2 && self.gem[y - 1][x] == c && self.gem[y - 2][x] == c;
                        if !h && !v {
                            self.gem[y][x] = c;
                            break;
                        }
                    }
                }
            }
            if self.find_move().is_some() {
                return;
            }
        }
    }

    pub fn over(&self) -> bool {
        self.end != END_NONE
    }

    pub fn can_play(&self) -> bool {
        self.phase() == Phase::Idle && !self.over()
    }

    fn matchable(&self, x: usize, y: usize) -> u8 {
        let g = self.gem[y][x];
        if g >= 1 && g <= COLOURS && self.dying[y][x] == 0 { g } else { 0 }
    }

    /// lengths of the horizontal and vertical runs through each square
    fn runs(&self, h: &mut [[u8; N]; N], v: &mut [[u8; N]; N]) {
        for y in 0..N {
            let mut x = 0;
            while x < N {
                let c = self.matchable(x, y);
                let mut e = x + 1;
                while c != 0 && e < N && self.matchable(e, y) == c {
                    e += 1;
                }
                for i in x..e {
                    h[y][i] = if c != 0 { (e - x) as u8 } else { 0 };
                }
                x = e;
            }
        }
        for x in 0..N {
            let mut y = 0;
            while y < N {
                let c = self.matchable(x, y);
                let mut e = y + 1;
                while c != 0 && e < N && self.matchable(x, e) == c {
                    e += 1;
                }
                for j in y..e {
                    v[j][x] = if c != 0 { (e - y) as u8 } else { 0 };
                }
                y = e;
            }
        }
    }

    pub fn has_match(&self) -> bool {
        let (mut h, mut v) = ([[0u8; N]; N], [[0u8; N]; N]);
        self.runs(&mut h, &mut v);
        (0..N).any(|y| (0..N).any(|x| h[y][x] >= 3 || v[y][x] >= 3))
    }

    /// a swap that makes a line: the square and its neighbour to the right
    /// (dir 0) or below (dir 1)
    pub fn find_move(&self) -> Option<(usize, usize, usize, usize)> {
        let mut b = *self;
        for y in 0..N {
            for x in 0..N {
                for d in 0..2 {
                    let (x2, y2) = if d == 0 { (x + 1, y) } else { (x, y + 1) };
                    if x2 >= N || y2 >= N {
                        continue;
                    }
                    if self.gem[y][x] == PRISM || self.gem[y2][x2] == PRISM {
                        return Some((x, y, x2, y2));
                    }
                    b.swap_gems(x, y, x2, y2);
                    let ok = b.line_through(x, y) || b.line_through(x2, y2);
                    b.swap_gems(x, y, x2, y2);
                    if ok {
                        return Some((x, y, x2, y2));
                    }
                }
            }
        }
        None
    }

    fn line_through(&self, x: usize, y: usize) -> bool {
        let c = self.matchable(x, y);
        if c == 0 {
            return false;
        }
        let count = |dx: i32, dy: i32| {
            let mut n = 0;
            let (mut i, mut j) = (x as i32 + dx, y as i32 + dy);
            while i >= 0 && j >= 0 && i < N as i32 && j < N as i32 && self.matchable(i as usize, j as usize) == c {
                n += 1;
                i += dx;
                j += dy;
            }
            n
        };
        count(-1, 0) + count(1, 0) >= 2 || count(0, -1) + count(0, 1) >= 2
    }

    fn swap_gems(&mut self, ax: usize, ay: usize, bx: usize, by: usize) {
        let g = self.gem[ay][ax];
        self.gem[ay][ax] = self.gem[by][bx];
        self.gem[by][bx] = g;
        let k = self.kind[ay][ax];
        self.kind[ay][ax] = self.kind[by][bx];
        self.kind[by][bx] = k;
    }

    /// the player asks to swap two squares; true if it starts
    pub fn try_swap(&mut self, ax: i32, ay: i32, bx: i32, by: i32) -> bool {
        if !self.can_play() || !inb(ax, ay) || !inb(bx, by) || (ax - bx).abs() + (ay - by).abs() != 1 {
            return false;
        }
        if self.gem[ay as usize][ax as usize] == 0 || self.gem[by as usize][bx as usize] == 0 {
            return false;
        }
        self.ax = ax;
        self.ay = ay;
        self.bx = bx;
        self.by = by;
        self.idle_ms = 0;
        self.set(Phase::Swap);
        true
    }

    /// advances the rules by dt ms; returns EV_* bits
    pub fn step(&mut self, dt: i32, sink: &mut dyn Sink) -> i32 {
        let mut ev = 0;
        if self.phase() != Phase::Over {
            self.time_ms += dt;
        }
        if self.mode == MODE_TIMED && self.time_left > 0 && self.phase() != Phase::Over {
            self.time_left = (self.time_left - dt).max(0);
        }
        if self.phase() == Phase::Idle {
            self.idle_ms += dt;
            if self.mode == MODE_TIMED && self.time_left == 0 {
                ev |= self.finish(END_TIME);
            }
        }
        self.t += dt;
        match self.phase() {
            Phase::Idle => {}
            Phase::Over => {
                if self.end_ms > 0 {
                    self.end_ms -= dt;
                    if self.end_ms <= 0 {
                        ev |= EV_GAMEOVER;
                    }
                }
            }
            Phase::Swap => {
                if self.t >= SWAP_MS {
                    let (ax, ay, bx, by) = (self.ax as usize, self.ay as usize, self.bx as usize, self.by as usize);
                    self.swap_gems(ax, ay, bx, by);
                    self.chain = 1;
                    if self.gem[ay][ax] == PRISM || self.gem[by][bx] == PRISM {
                        self.moves += 1;
                        if self.mode == MODE_MOVES {
                            self.moves_left -= 1;
                        }
                        self.prism_swap(ax, ay, bx, by, sink);
                        ev |= EV_HIT;
                    } else if self.has_match() {
                        self.moves += 1;
                        if self.mode == MODE_MOVES {
                            self.moves_left -= 1;
                        }
                        self.clear_matches(true, sink);
                        ev |= EV_HIT;
                    } else {
                        self.set(Phase::Back);
                        ev |= EV_BLIP;
                    }
                }
            }
            Phase::Back => {
                if self.t >= SWAP_MS {
                    let (ax, ay, bx, by) = (self.ax as usize, self.ay as usize, self.bx as usize, self.by as usize);
                    self.swap_gems(ax, ay, bx, by);
                    self.set(Phase::Idle);
                }
            }
            Phase::Clear => {
                if self.t >= CLEAR_MS {
                    self.collapse();
                    self.set(Phase::Fall);
                }
            }
            Phase::Fall => {
                if !self.fall(dt) {
                    if self.has_match() {
                        self.chain += 1;
                        self.clear_matches(false, sink);
                        ev |= EV_HIT;
                    } else {
                        ev |= self.settle();
                    }
                }
            }
            Phase::Shuffle => {
                if self.t >= SHUFFLE_MS / 2 && self.t - dt < SHUFFLE_MS / 2 {
                    self.reshuffle();
                }
                if self.t >= SHUFFLE_MS {
                    self.set(Phase::Idle);
                }
            }
        }
        while self.mode == MODE_CLASSIC && self.progress >= level_need(self.level) {
            self.progress -= level_need(self.level);
            self.level += 1;
            ev |= EV_LEVEL;
        }
        ev
    }

    fn finish(&mut self, why: i32) -> i32 {
        self.end = why;
        self.end_ms = END_MS;
        self.set(Phase::Over);
        if why == END_STUCK { EV_LOSE } else { EV_WIN }
    }

    /// the board came to rest with no line on it
    fn settle(&mut self) -> i32 {
        self.best_chain = self.best_chain.max(self.chain);
        self.chain = 0;
        self.idle_ms = 0;
        if self.mode == MODE_TIMED && self.time_left == 0 {
            return self.finish(END_TIME);
        }
        if self.mode == MODE_MOVES && self.moves_left <= 0 {
            return self.finish(END_MOVES);
        }
        if self.find_move().is_none() {
            if self.mode == MODE_CLASSIC {
                if self.shuffles == 0 {
                    return self.finish(END_STUCK);
                }
                self.shuffles -= 1;
            }
            self.shuffled += 1;
            self.set(Phase::Shuffle);
            return EV_BLIP;
        }
        self.set(Phase::Idle);
        0
    }

    fn add_points(&mut self, raw: i32, x: f32, y: f32, sink: &mut dyn Sink) {
        let mult = self.chain.max(1) * if self.mode == MODE_CLASSIC { self.level } else { 1 };
        let pts = raw * mult;
        self.score = self.score.saturating_add(pts);
        self.progress += raw;
        sink.points(x, y, pts, self.chain);
    }

    /// clears the lines on the board; a line of four or more leaves a
    /// special gem behind, on the square the player moved when it is in it
    fn clear_matches(&mut self, by_player: bool, sink: &mut dyn Sink) {
        let (mut h, mut v) = ([[0u8; N]; N], [[0u8; N]; N]);
        self.runs(&mut h, &mut v);
        let mut group = [[0u8; N]; N];
        let mut make: [(usize, usize, u8, u8); N * N / 3] = [(0, 0, 0, 0); N * N / 3];
        let mut nmake = 0;
        let mut ngroups = 0u8;
        for sy in 0..N {
            for sx in 0..N {
                if group[sy][sx] != 0 || (h[sy][sx] < 3 && v[sy][sx] < 3) {
                    continue;
                }
                // one group: the matched squares of a colour joined to this one
                ngroups += 1;
                let c = self.gem[sy][sx];
                let mut stack = [(0usize, 0usize); N * N];
                let mut sp = 0;
                stack[sp] = (sx, sy);
                sp += 1;
                group[sy][sx] = ngroups;
                let (mut n, mut maxh, mut maxv, mut sumx, mut sumy) = (0, 0u8, 0u8, 0f32, 0f32);
                let mut at: Option<(usize, usize)> = None;
                let mut cells = [(0usize, 0usize); N * N];
                while sp > 0 {
                    sp -= 1;
                    let (x, y) = stack[sp];
                    cells[n] = (x, y);
                    n += 1;
                    sumx += x as f32;
                    sumy += y as f32;
                    if h[y][x] >= 3 {
                        maxh = maxh.max(h[y][x]);
                    }
                    if v[y][x] >= 3 {
                        maxv = maxv.max(v[y][x]);
                    }
                    if by_player
                        && ((x as i32 == self.ax && y as i32 == self.ay) || (x as i32 == self.bx && y as i32 == self.by))
                        && at.is_none()
                    {
                        at = Some((x, y));
                    }
                    let nb = [(x.wrapping_sub(1), y), (x + 1, y), (x, y.wrapping_sub(1)), (x, y + 1)];
                    for (nx, ny) in nb {
                        if nx < N && ny < N && group[ny][nx] == 0 && self.matchable(nx, ny) == c && (h[ny][nx] >= 3 || v[ny][nx] >= 3)
                        {
                            group[ny][nx] = ngroups;
                            stack[sp] = (nx, ny);
                            sp += 1;
                        }
                    }
                }
                let special = if maxh >= 5 || maxv >= 5 {
                    Some((PRISM, NORMAL))
                } else if maxh >= 3 && maxv >= 3 {
                    Some((c, CROSS))
                } else if maxh == 4 || maxv == 4 {
                    Some((c, BOMB))
                } else {
                    None
                };
                for &(x, y) in &cells[..n] {
                    self.dying[y][x] = 1;
                }
                if let Some((g, k)) = special {
                    // where: the moved square, else the square nearest the middle
                    let (cx, cy) = (sumx / n as f32, sumy / n as f32);
                    let spot = at.unwrap_or_else(|| {
                        let mut best = cells[0];
                        let mut bd = 1e9f32;
                        for &(x, y) in &cells[..n] {
                            let d = (x as f32 - cx) * (x as f32 - cx) + (y as f32 - cy) * (y as f32 - cy);
                            if d < bd {
                                bd = d;
                                best = (x, y);
                            }
                        }
                        best
                    });
                    if nmake < make.len() {
                        make[nmake] = (spot.0, spot.1, g, k);
                        nmake += 1;
                    }
                }
                let raw = match n {
                    0..=3 => 50,
                    4 => 100,
                    _ => 150 + 50 * (n as i32 - 5),
                };
                self.biggest = self.biggest.max(n as i32);
                self.add_points(raw, sumx / n as f32, sumy / n as f32, sink);
            }
        }
        self.detonate_all(sink);
        for &(x, y, g, k) in &make[..nmake] {
            // the old gem there is gone (and went off if it was special)
            self.gem[y][x] = g;
            self.kind[y][x] = k;
            self.dying[y][x] = 0;
            self.specials += 1;
        }
        self.set(Phase::Clear);
    }

    /// a prism swapped with a gem clears every gem of that gem's colour;
    /// two prisms clear the board
    fn prism_swap(&mut self, ax: usize, ay: usize, bx: usize, by: usize, sink: &mut dyn Sink) {
        let (a, b) = (self.gem[ay][ax], self.gem[by][bx]);
        let target = if a == PRISM && b == PRISM { 0 } else if a == PRISM { b } else { a };
        let mut n = 0;
        for y in 0..N {
            for x in 0..N {
                if target == 0 || self.gem[y][x] == target {
                    self.dying[y][x] = 1;
                    n += 1;
                }
            }
        }
        self.dying[ay][ax] = 1;
        self.dying[by][bx] = 1;
        let (px, py) = if a == PRISM { (ax, ay) } else { (bx, by) };
        sink.blast(px, py, PRISM);
        self.add_points(40 * n, px as f32, py as f32, sink);
        self.detonate_all(sink);
        self.set(Phase::Clear);
    }

    /// special gems among those being cleared go off, which may set off more
    fn detonate_all(&mut self, sink: &mut dyn Sink) {
        let mut fired = [[0u8; N]; N];
        loop {
            let mut any = false;
            for y in 0..N {
                for x in 0..N {
                    if self.dying[y][x] == 0 || fired[y][x] != 0 {
                        continue;
                    }
                    let (g, k) = (self.gem[y][x], self.kind[y][x]);
                    if k == NORMAL && g != PRISM {
                        continue;
                    }
                    fired[y][x] = 1;
                    any = true;
                    let mut n = 0;
                    let mut hit = |b: &mut Board, i: usize, j: usize| {
                        if b.dying[j][i] == 0 && b.gem[j][i] != 0 {
                            b.dying[j][i] = 1;
                            n += 1;
                        }
                    };
                    if g == PRISM {
                        // set off by a blast: the commonest colour goes
                        let mut count = [0i32; 9];
                        for j in 0..N {
                            for i in 0..N {
                                if self.dying[j][i] == 0 && self.gem[j][i] >= 1 && self.gem[j][i] <= COLOURS {
                                    count[self.gem[j][i] as usize] += 1;
                                }
                            }
                        }
                        let mut best = 1;
                        for c in 1..=COLOURS as usize {
                            if count[c] > count[best] {
                                best = c;
                            }
                        }
                        for j in 0..N {
                            for i in 0..N {
                                if self.gem[j][i] == best as u8 {
                                    hit(self, i, j);
                                }
                            }
                        }
                        sink.blast(x, y, PRISM);
                    } else if k == BOMB {
                        for j in y.saturating_sub(1)..(y + 2).min(N) {
                            for i in x.saturating_sub(1)..(x + 2).min(N) {
                                hit(self, i, j);
                            }
                        }
                        sink.blast(x, y, BOMB);
                    } else {
                        for i in 0..N {
                            hit(self, i, y);
                        }
                        for j in 0..N {
                            hit(self, x, j);
                        }
                        sink.blast(x, y, CROSS);
                    }
                    if n > 0 {
                        self.add_points(20 * n, x as f32, y as f32, sink);
                    }
                }
            }
            if !any {
                break;
            }
        }
        for y in 0..N {
            for x in 0..N {
                if self.dying[y][x] != 0 {
                    sink.cleared(x, y, self.gem[y][x], self.kind[y][x]);
                }
            }
        }
    }

    /// cleared squares empty; gems above drop, and new ones come from the top
    fn collapse(&mut self) {
        let mut fresh = [[false; N]; N];
        for x in 0..N {
            let mut dst = N as i32 - 1;
            for y in (0..N).rev() {
                if self.dying[y][x] != 0 {
                    self.cleared += 1;
                    continue;
                }
                let d = dst as usize;
                if d != y {
                    self.gem[d][x] = self.gem[y][x];
                    self.kind[d][x] = self.kind[y][x];
                    self.off[d][x] = self.off[y][x] - (d - y) as i32 * 1000;
                    self.vel[d][x] = self.vel[y][x];
                }
                dst -= 1;
            }
            let count = dst + 1;
            for d in (0..=dst).rev() {
                let d = d as usize;
                self.gem[d][x] = self.new_colour(x, d);
                self.kind[d][x] = NORMAL;
                self.off[d][x] = -count * 1000 - 300;
                self.vel[d][x] = 0;
                fresh[d][x] = true;
            }
            for y in 0..N {
                self.dying[y][x] = 0;
            }
        }
        self.rescue_fresh(&fresh);
    }

    /// how often new gems that leave no swap are dealt again, in 100ths
    pub fn rescue(&self) -> i32 {
        let d = self.diff.clamp(0, 2) as usize;
        let fade = if self.mode == MODE_CLASSIC { RESCUE_FADE[d] * (self.level - 1) } else { 0 };
        (RESCUE[d] - fade).max(0)
    }

    /// a colour for a new gem at a square: now and then that of a gem
    /// beside or below it
    fn new_colour(&mut self, x: usize, y: usize) -> u8 {
        let kin = KIN[self.diff.clamp(0, 2) as usize];
        if kin > 0 && self.rand(100) < kin {
            let mut near = [0u8; 3];
            let mut n = 0;
            for (nx, ny) in [(x as i32 - 1, y as i32), (x as i32 + 1, y as i32), (x as i32, y as i32 + 1)] {
                if inb(nx, ny) {
                    let g = self.gem[ny as usize][nx as usize];
                    if g >= 1 && g <= COLOURS {
                        near[n] = g;
                        n += 1;
                    }
                }
            }
            if n > 0 {
                return near[self.rand(n as u32) as usize];
            }
        }
        self.colour()
    }

    /// new gems still above the board that leave nothing to clear and no
    /// swap are dealt again, now and then, so fewer boards need a shuffle
    fn rescue_fresh(&mut self, fresh: &[[bool; N]; N]) {
        if self.has_match() || self.find_move().is_some() {
            return;
        }
        let chance = self.rescue();
        if chance <= 0 || self.rand(100) as i32 >= chance {
            return;
        }
        for _ in 0..12 {
            for y in (0..N).rev() {
                for x in 0..N {
                    if fresh[y][x] {
                        self.gem[y][x] = self.new_colour(x, y);
                    }
                }
            }
            if self.has_match() || self.find_move().is_some() {
                return;
            }
        }
    }

    /// moves falling gems on; returns whether any is still falling
    fn fall(&mut self, dt: i32) -> bool {
        let mut moving = false;
        for y in 0..N {
            for x in 0..N {
                if self.off[y][x] >= 0 {
                    self.off[y][x] = 0;
                    self.vel[y][x] = 0;
                    continue;
                }
                self.vel[y][x] = (self.vel[y][x] + GRAVITY * dt / 1000).min(MAX_VEL);
                self.off[y][x] += self.vel[y][x] * dt / 1000;
                if self.off[y][x] >= 0 {
                    self.off[y][x] = 0;
                    self.vel[y][x] = 0;
                } else {
                    moving = true;
                }
            }
        }
        moving
    }

    /// new colours for the plain gems, keeping specials where they are
    fn reshuffle(&mut self) {
        for _ in 0..200 {
            for y in 0..N {
                for x in 0..N {
                    if self.kind[y][x] == NORMAL && self.gem[y][x] != PRISM {
                        self.gem[y][x] = self.colour();
                    }
                }
            }
            if !self.has_match() && self.find_move().is_some() {
                return;
            }
        }
        self.fill();
    }

    /// whether a loaded board is one the rules could have made
    pub fn valid(&self) -> bool {
        if self.mode < 0 || self.mode > 2 || self.diff < 0 || self.diff > 2 || self.shuffled < 0 || self.phase < 0 || self.phase > 6 || self.level < 1 || self.level > 100_000 {
            return false;
        }
        if self.score < 0 || self.progress < 0 || self.time_left < 0 || self.time_left > TIMED_MS || self.moves_left < 0 || self.moves_left > MOVES || self.shuffles < 0 || self.shuffles > SHUFFLES {
            return false;
        }
        if (self.phase() == Phase::Over) != (self.end != END_NONE) || self.end < 0 || self.end > 3 {
            return false;
        }
        if !inb(self.ax, self.ay) || !inb(self.bx, self.by) || self.t < 0 || self.chain < 0 {
            return false;
        }
        for y in 0..N {
            for x in 0..N {
                let (g, k) = (self.gem[y][x], self.kind[y][x]);
                if g == 0 || g > PRISM || k > CROSS || (g == PRISM && k != NORMAL) || self.dying[y][x] > 1 {
                    return false;
                }
                if self.off[y][x] > 0 || self.off[y][x] < -20_000 || self.vel[y][x] < 0 || self.vel[y][x] > MAX_VEL {
                    return false;
                }
            }
        }
        true
    }
}

// SAFETY: integers and arrays of them only, so any bytes are a Board
unsafe impl wg::Plain for Board {}

pub fn inb(x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < N as i32 && y < N as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(b: &mut Board) {
        for _ in 0..2000 {
            b.step(16, &mut NoSink);
            if b.phase() == Phase::Idle || b.phase() == Phase::Over {
                return;
            }
        }
        panic!("never settled: {:?}", b.phase());
    }

    #[test]
    fn fresh_board_has_no_line_and_a_move() {
        for seed in 1..200 {
            let mut b = Board::zeroed();
            b.start(seed, MODE_CLASSIC, DIFF_NORMAL);
            assert!(!b.has_match());
            assert!(b.find_move().is_some());
            settle(&mut b);
            assert!(b.valid());
        }
    }

    #[test]
    fn bad_swap_goes_back() {
        let mut b = Board::zeroed();
        b.start(7, MODE_MOVES, DIFF_NORMAL);
        settle(&mut b);
        // find a swap that makes nothing
        'o: for y in 0..N {
            for x in 0..N - 1 {
                let mut c = b;
                c.swap_gems(x, y, x + 1, y);
                if !c.has_match() && b.gem[y][x] != PRISM && b.gem[y][x + 1] != PRISM {
                    let before = b.gem;
                    assert!(b.try_swap(x as i32, y as i32, x as i32 + 1, y as i32));
                    settle(&mut b);
                    assert_eq!(before, b.gem);
                    assert_eq!(b.moves_left, MOVES);
                    break 'o;
                }
            }
        }
    }

    #[test]
    fn bot_plays_every_mode_to_the_end() {
        for mode in 0..3 {
            for seed in 1..20 {
                let mut b = Board::zeroed();
                b.start(seed * 77, mode, seed as i32 % 3);
                settle(&mut b);
                let mut turns = 0;
                while !b.over() && turns < 20000 {
                    let (x, y, x2, y2) = b.find_move().expect("idle board without a move");
                    assert!(b.try_swap(x as i32, y as i32, x2 as i32, y2 as i32));
                    settle(&mut b);
                    assert!(b.valid());
                    turns += 1;
                    if mode == MODE_TIMED {
                        b.step(1500, &mut NoSink);
                        settle(&mut b);
                    }
                }
                assert!(b.over(), "mode {} seed {} never ended", mode, seed);
                assert!(b.score > 0);
                if mode == MODE_MOVES {
                    assert_eq!(b.moves, MOVES);
                }
            }
        }
    }

    #[test]
    fn four_in_a_row_makes_a_bomb() {
        let mut b = Board::zeroed();
        b.start(3, MODE_CLASSIC, DIFF_HARD);
        settle(&mut b);
        // a hand-made board: row 7 is 1 1 _ 1 1 with a 1 above the gap
        let pal = [2u8, 3, 4, 5, 6, 7];
        for y in 0..N {
            for x in 0..N {
                b.gem[y][x] = pal[(x + y * 2) % 6];
                b.kind[y][x] = NORMAL;
            }
        }
        for x in [0, 1, 3] {
            b.gem[7][x] = 1;
        }
        b.gem[6][2] = 1;
        assert!(b.try_swap(2, 6, 2, 7));
        let mut made = false;
        for _ in 0..200 {
            b.step(16, &mut NoSink);
            if b.specials > 0 {
                made = true;
            }
        }
        assert!(made);
    }

    /// every swap that does something, for the bots
    fn all_moves(b: &Board) -> Vec<(usize, usize, usize, usize)> {
        let mut out = Vec::new();
        let mut c = *b;
        for y in 0..N {
            for x in 0..N {
                for (x2, y2) in [(x + 1, y), (x, y + 1)] {
                    if x2 >= N || y2 >= N {
                        continue;
                    }
                    c.swap_gems(x, y, x2, y2);
                    if b.gem[y][x] == PRISM || b.gem[y2][x2] == PRISM || c.line_through(x, y) || c.line_through(x2, y2) {
                        out.push((x, y, x2, y2));
                    }
                    c.swap_gems(x, y, x2, y2);
                }
            }
        }
        out
    }

    /// plays many runs per mode and difficulty with a bot that takes a
    /// random swap that does something (the first one with FIRST=1); prints averages (cargo test --release lengths -- --nocapture)
    #[test]
    fn lengths() {
        let first = std::env::var("FIRST").is_ok();
        for mode in 0..3 {
            for diff in 0..3 {
                let runs = 40u32;
                let (mut moves, mut specials, mut shuffled, mut level, mut score, mut min, mut max) = (0i64, 0i64, 0i64, 0i64, 0i64, i32::MAX, 0);
                for seed in 1..=runs {
                    let mut b = Board::zeroed();
                    b.start(seed * 31, mode, diff);
                    settle(&mut b);
                    let mut r = seed * 977 + 5;
                    while !b.over() {
                        let all = all_moves(&b);
                        let (x, y, x2, y2) = if first { b.find_move().unwrap() } else { all[xorshift(&mut r) as usize % all.len()] };
                        b.try_swap(x as i32, y as i32, x2 as i32, y2 as i32);
                        settle(&mut b);
                        if mode == MODE_TIMED {
                            b.step(800, &mut NoSink);
                            settle(&mut b);
                        }
                    }
                    moves += b.moves as i64;
                    specials += b.specials as i64;
                    shuffled += b.shuffled as i64;
                    level += b.level as i64;
                    score += b.score as i64;
                    min = min.min(b.moves);
                    max = max.max(b.moves);
                }
                let n = runs as i64;
                println!(
                    "mode {} diff {}: moves {} ({}..{}) specials/100 swaps {:.1} shuffles/100 swaps {:.2} level {:.1} score {}",
                    mode, diff, moves / n, min, max, specials as f64 * 100.0 / moves as f64, shuffled as f64 * 100.0 / moves as f64, level as f64 / n as f64, score / n
                );
            }
        }
    }
}
