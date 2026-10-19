//! Pantry Solitaire - an original wasmga.me game. MIT licensed.
//! Two patience games on one table. FreeCell: every card dealt face up into
//! eight columns, four free cells to park cards in, and the suits built up
//! from ace to king; deals are numbered as in Microsoft's FreeCell. Spider:
//! two decks in ten columns, runs built down from king to ace in one suit
//! and cleared away, a row dealt from the stock when stuck; with one, two
//! or four suits. The page shows the setup, pause and game over screens.
mod glyphs;
mod rules;

use rules::{Play, Step, Table, COL, FOUND, FREECELL, MAXP, MODES, NPILES, STOCK};
use wg::{hex, mix, rgb, Game, Input, KeyRepeat, Pointer, Rng, Screen, Sprite};

const VERSION: u32 = 1;
/// the solved table stays in view this long before game over
const END_MS: i32 = 1600;
/// steps the undo list keeps
const UNDO: usize = 1200;
/// time between cards going home on their own
const AUTO_MS: i32 = 110;
const HINT_MS: i32 = 2600;
const MSG_MS: i32 = 2400;
const TURN_MS: i32 = 220;

/// options, in the catalog `setup` order
const O_GAME: usize = 0;
const O_THEME: usize = 1;
const THEMES: usize = 4;

const PLAY: i32 = 0;
const WON: i32 = 1;

/// messages in the bar
const MSG_STUCK: i32 = 1;
const MSG_FILL: i32 = 2;

/// a card, in plan units
const CW: f32 = 60.0;
const CH: f32 = 84.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 2],
    rng: Rng,
    rep: KeyRepeat,
    /// the game this table was dealt for (the option may change since)
    mode: i32,
    /// the FreeCell deal number
    deal: i32,
    phase: i32,
    end_ms: i32,
    time_ms: i32,
    moves: i32,
    undos: i32,
    hints: i32,
    started: i32,
    /// cards may go home (or runs clear) on their own after the last move
    auto: i32,
    auto_ms: i32,
    score: i32,
    move_bonus: i32,
    time_bonus: i32,
    /// the keyboard cursor: shown, row (0 the top piles), column, card
    keyboard: i32,
    krow: i32,
    kx: i32,
    kidx: i32,
    /// cards picked up with the keyboard
    holding: i32,
    hold_pile: i32,
    hold_idx: i32,
    t: Table,
    ulen: i32,
    undo: [u32; UNDO],
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}

struct Theme {
    table: u32,
    table_lo: u32,
    slot: u32,
    text: u32,
    dim: u32,
    accent: u32,
    back: u32,
    back_hi: u32,
    panel: u32,
}

const THEME: [Theme; THEMES] = [
    // Basil: green baize, blue backs
    Theme { table: hex(0x2f6b4f), table_lo: hex(0x24553f), slot: hex(0x3f8062), text: hex(0xf0faf2), dim: hex(0xa6ceb8), accent: hex(0xffd166), back: hex(0x2c5a8a), back_hi: hex(0x5b8cc0), panel: hex(0x1b4733) },
    // Rye: a wooden board, red backs
    Theme { table: hex(0x6e4c30), table_lo: hex(0x583b24), slot: hex(0x82603f), text: hex(0xfff4e6), dim: hex(0xd8bc9c), accent: hex(0xffc857), back: hex(0x9a2f3b), back_hi: hex(0xc9586a), panel: hex(0x432c1a) },
    // Plum: purple cloth, teal backs
    Theme { table: hex(0x4b2a52), table_lo: hex(0x3a1f41), slot: hex(0x613a69), text: hex(0xfbefff), dim: hex(0xc6a6cf), accent: hex(0x7fe0c8), back: hex(0x23786b), back_hi: hex(0x4fa597), panel: hex(0x2c1632) },
    // Ink: dark navy, coral backs
    Theme { table: hex(0x1d2436), table_lo: hex(0x141a29), slot: hex(0x2c3650), text: hex(0xeef2ff), dim: hex(0x8f9bb8), accent: hex(0xff9f5a), back: hex(0xc9573d), back_hi: hex(0xec8a68), panel: hex(0x0f1420) },
];

const FACE: u32 = rgb(252, 249, 242);
const EDGE: u32 = rgb(120, 112, 104);
const RED: u32 = rgb(200, 38, 52);
const BLACK: u32 = rgb(28, 28, 40);
const RANKS: [&str; 13] = ["A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K"];

/// Where everything sits, for a plan of pw x ph.
#[derive(Clone, Copy)]
struct Lay {
    pw: f32,
    ph: f32,
    tall: bool,
    cols: usize,
    gap: f32,
    left: f32,
    bar: f32,
    top_y: f32,
    tab_y: f32,
    /// the bar's text size
    font: f32,
    btn_w: f32,
}

fn metrics(mode: i32, tall: bool) -> (usize, f32, f32) {
    let cols = if mode == FREECELL { 8 } else { 10 };
    if tall {
        (cols, 3.0, 3.0)
    } else {
        (cols, 8.0, 6.0)
    }
}

fn plan_for(mode: i32, w: i32, h: i32) -> (f32, f32) {
    let (w, h) = (w.max(1) as f32, h.max(1) as f32);
    let tall = h > w * 1.1;
    let (cols, gap, left) = metrics(mode, tall);
    let pw = 2.0 * left + cols as f32 * CW + (cols - 1) as f32 * gap;
    let ph = if tall { (pw * h / w).clamp(pw * 1.15, pw * 2.4) } else { (pw * h / w).clamp(pw * 0.6, pw * 1.1) };
    (pw, ph)
}

fn layout(mode: i32, pw: f32, ph: f32) -> Lay {
    let tall = ph > pw * 1.12;
    let (cols, gap, left) = metrics(mode, tall);
    let bar = if tall { 36.0 } else { 24.0 };
    let top_y = bar + if tall { 8.0 } else { 6.0 };
    Lay {
        pw,
        ph,
        tall,
        cols,
        gap,
        left,
        bar,
        top_y,
        tab_y: top_y + CH + 12.0,
        font: if tall { 16.0 } else { 12.0 },
        btn_w: if tall { 84.0 } else { 58.0 },
    }
}

impl Lay {
    fn col_x(&self, k: usize) -> f32 {
        self.left + k as f32 * (CW + self.gap)
    }
    /// the bar's buttons: hint, undo
    fn button(&self, b: i32) -> (f32, f32, f32, f32) {
        let (y, h) = (2.0, self.bar - 5.0);
        let undo = self.pw - self.left - self.btn_w;
        let x = if b == 2 { undo } else { undo - 6.0 - self.btn_w };
        (x, y, self.btn_w, h)
    }
    fn button_at(&self, x: f32, y: f32) -> i32 {
        for b in 1..=2 {
            let (bx, by, bw, bh) = self.button(b);
            if x >= bx && x < bx + bw && y >= by && y < by + bh {
                return b;
            }
        }
        0
    }
}

#[derive(Clone, Copy)]
struct Press {
    pile: usize,
    idx: i32,
    x: f32,
    y: f32,
}

struct Pantry {
    s: Save,
    lay: Lay,
    /// where each card is drawn, easing toward where it belongs
    pos: [(f32, f32); MAXP],
    /// time left before each card starts moving (the deal)
    delay: [i32; MAXP],
    /// time left in each column's top card turning over
    turn: [i32; 10],
    /// time to animate by at the next draw
    anim_dt: i32,
    animating: bool,
    /// on the next draw: put every card in place, or deal them out
    snap: bool,
    dealing: bool,
    mx: f32,
    my: f32,
    press: Option<Press>,
    dragging: bool,
    /// the bar button under the mouse: 1 hint, 2 undo
    hover: i32,
    hint: Option<Play>,
    hint_ms: i32,
    msg: i32,
    msg_ms: i32,
    ev: u32,
    faces: Vec<Sprite>,
    lit_faces: Vec<Sprite>,
    back: Sprite,
    /// buffers kept from one draw to the next: targets(), and cards drawn out of their piles
    targets: Vec<(f32, f32, usize, usize)>,
    loose: Vec<usize>,
}

fn mode_of(opt: &[i32; 2]) -> i32 {
    opt[O_GAME].clamp(0, MODES - 1)
}

impl Pantry {
    fn th(&self) -> &'static Theme {
        &THEME[self.s.opt[O_THEME].clamp(0, THEMES as i32 - 1) as usize]
    }
    fn t(&self) -> &Table {
        &self.s.t
    }

    // ---------- layout ----------

    /// a top pile's place, or a column's first card's
    fn slot(&self, p: usize) -> (f32, f32) {
        let l = &self.lay;
        let spider = self.t().spider();
        match p {
            STOCK => (l.col_x(0), l.top_y),
            p if p < FOUND => (l.col_x(p - 1), l.top_y),
            p if p < COL => (l.col_x(p - FOUND + if spider { 2 } else { 4 }), l.top_y),
            p => (l.col_x(p - COL), l.tab_y),
        }
    }

    /// the spacing of column p: face-down cards, face-up cards, squeezed so
    /// the column fits
    fn steps(&self, p: usize) -> (f32, f32) {
        let l = &self.lay;
        let t = self.t();
        let (down, up) = (t.down_of(p) as f32, (t.len(p) - t.down_of(p)) as f32);
        let (mut d, mut u) = if l.tall { (8.0, 25.0) } else { (7.0, 20.0) };
        let room = l.ph - 4.0 - l.tab_y - CH;
        if down * d + (up - 1.0).max(0.0) * u > room {
            d = if l.tall { 5.0 } else { 4.0 };
            if up > 1.0 {
                u = ((room - down * d) / (up - 1.0)).clamp(4.0, u);
            }
        }
        (d, u)
    }

    fn card_xy(&self, p: usize, i: usize) -> (f32, f32) {
        let (x, y) = self.slot(p);
        if p == STOCK {
            return (x + (i / 10) as f32 * 3.0, y - (i / 10) as f32 * 1.5);
        }
        if p < COL {
            return (x, y);
        }
        let (d, u) = self.steps(p);
        let down = self.t().down_of(p);
        let y = if i < down { y + i as f32 * d } else { y + down as f32 * d + (i - down) as f32 * u };
        (x, y)
    }

    /// the pile and card under a point; idx -1 for an empty pile's place
    fn hit(&self, x: f32, y: f32) -> Option<(usize, i32)> {
        let t = self.t();
        for p in t.piles() {
            let n = t.len(p);
            let (sx, sy) = self.slot(p);
            if x < sx || x >= sx + CW + if p == STOCK { 12.0 } else { 0.0 } {
                continue;
            }
            if t.is_col(p) {
                for i in (0..n).rev() {
                    let (_, cy) = self.card_xy(p, i);
                    if y >= cy && y < cy + CH {
                        return Some((p, i as i32));
                    }
                }
                if y >= sy && y < sy + CH {
                    return Some((p, -1));
                }
            } else if y >= sy - 6.0 && y < sy + CH {
                return Some((p, n as i32 - 1));
            }
        }
        None
    }

    /// where cards dropped with their top left at x, y land
    fn drop_at(&self, x: f32, y: f32, card: u8) -> Option<usize> {
        let l = &self.lay;
        let t = self.t();
        let cx = x + CW / 2.0;
        let k = (((cx - l.left + l.gap / 2.0) / (CW + l.gap)).floor() as i32).clamp(0, l.cols as i32 - 1) as usize;
        if y + 20.0 < l.tab_y - 6.0 {
            if t.spider() {
                return None;
            }
            if k < 4 {
                let c = rules::CELL + k;
                return Some(if t.len(c) == 0 { c } else { t.empty_cell().unwrap_or(c) });
            }
            return Some(t.home_for(card).unwrap_or(FOUND + k - 4));
        }
        Some(COL + k)
    }

    // ---------- moves ----------

    fn record(&mut self, mut st: Step) {
        if self.s.ulen as usize == UNDO {
            // forget the oldest move
            let j = (1..UNDO).find(|&j| Step::unpack(self.s.undo[j]).first).unwrap_or(UNDO);
            self.s.undo.copy_within(j..UNDO, 0);
            self.s.ulen = (UNDO - j) as i32;
        }
        // the first step kept starts a move
        st.first |= self.s.ulen == 0;
        self.s.undo[self.s.ulen as usize] = st.pack();
        self.s.ulen += 1;
    }

    /// a step, kept for undo; `first` when it starts a new move
    fn step(&mut self, from: usize, to: usize, count: usize, first: bool) {
        let st = self.s.t.apply(from, to, count);
        if st.flip {
            self.turn[from - COL] = TURN_MS;
        }
        self.record(Step { first, ..st });
    }

    /// the player moves cards idx.. of p onto d, if the rules allow
    fn play(&mut self, p: usize, idx: usize, d: usize) -> bool {
        if self.s.phase != PLAY || !self.s.t.can_move(p, idx, d) {
            return false;
        }
        let count = self.s.t.len(p) - idx;
        self.step(p, d, count, true);
        self.moved();
        self.ev |= if self.s.t.is_found(d) { wg::EV_HIT } else { wg::EV_BLIP };
        self.check_win();
        true
    }

    fn moved(&mut self) {
        self.s.moves += 1;
        self.s.started = 1;
        self.s.auto = 1;
        self.s.auto_ms = AUTO_MS;
        self.s.holding = 0;
        self.hint = None;
        self.hint_ms = 0;
        self.msg_ms = 0;
    }

    /// a new row from the stock (Spider)
    fn deal_row(&mut self) -> bool {
        if self.s.phase != PLAY || !self.s.t.spider() || self.s.t.n[STOCK] == 0 {
            return false;
        }
        if !self.s.t.can_deal() {
            self.msg = MSG_FILL;
            self.msg_ms = MSG_MS;
            return false;
        }
        for k in 0..10 {
            let card = self.s.t.top(STOCK).unwrap();
            self.step(STOCK, COL + k, 1, k == 0);
            self.delay[card as usize] = k as i32 * 30;
        }
        self.moved();
        self.ev |= wg::EV_BLIP;
        true
    }

    /// a tap: the cards go to the best place for them
    fn send(&mut self, p: usize, idx: i32) -> bool {
        if p == STOCK {
            return self.deal_row();
        }
        if idx < 0 {
            return false;
        }
        match self.s.t.best_target(p, idx as usize) {
            Some(d) => self.play(p, idx as usize, d),
            None => false,
        }
    }

    fn undo(&mut self) {
        if self.s.phase != PLAY || self.s.ulen == 0 {
            return;
        }
        while self.s.ulen > 0 {
            self.s.ulen -= 1;
            let st = Step::unpack(self.s.undo[self.s.ulen as usize]);
            self.s.t.unapply(st);
            if st.first {
                break;
            }
        }
        self.s.undos += 1;
        self.s.auto = 0;
        self.s.holding = 0;
        self.hint = None;
        self.hint_ms = 0;
        self.msg_ms = 0;
        self.turn = [0; 10];
        self.ev |= wg::EV_BLIP;
    }

    fn give_hint(&mut self) {
        if self.s.phase != PLAY {
            return;
        }
        self.s.holding = 0;
        // tables already played, back along the undo list, so a hint
        // never leads round in a circle
        let mut seen = vec![self.s.t.sig()];
        let mut t = self.s.t;
        for k in (0..self.s.ulen as usize).rev().take(400) {
            let st = Step::unpack(self.s.undo[k]);
            t.unapply(st);
            if st.first {
                seen.push(t.sig());
            }
        }
        match self.s.t.hint(|h| seen.contains(&h)) {
            Some(h) => {
                self.hint = Some(h);
                self.hint_ms = HINT_MS;
                self.s.hints += 1;
                self.msg_ms = 0;
            }
            None => {
                self.hint = None;
                self.msg = MSG_STUCK;
                self.msg_ms = MSG_MS;
            }
        }
    }

    fn check_win(&mut self) {
        if !self.s.t.won() || self.s.phase == WON {
            return;
        }
        let s = &mut self.s;
        s.phase = WON;
        s.end_ms = END_MS;
        s.holding = 0;
        let secs = s.time_ms / 1000;
        if s.mode == FREECELL {
            s.move_bonus = (4000 - 25 * s.moves).max(0);
            s.time_bonus = 300_000 / (secs + 30);
        } else {
            s.move_bonus = (6000 - 20 * s.moves).max(0);
            s.time_bonus = 600_000 / (secs + 60);
        }
        s.score = (2000 + s.move_bonus + s.time_bonus - 50 * s.undos - 100 * s.hints).max(100);
        self.ev |= wg::EV_WIN;
        self.hint = None;
    }

    /// cards going home, or a finished run clearing, on their own
    fn auto_step(&mut self) -> bool {
        let t = &self.s.t;
        if t.spider() {
            let Some(p) = (COL..COL + 10).find(|&p| t.full_run(p)) else { return false };
            let Some(f) = t.free_found() else { return false };
            let n = t.len(p);
            for i in 0..13 {
                // the run goes up from its ace, one card after another
                let card = t.card(p, n - 1 - i);
                self.delay[card as usize] = i as i32 * 22;
            }
            self.step(p, f, 13, false);
        } else {
            let Some((p, f)) = t.auto_home() else { return false };
            self.step(p, f, 1, false);
        }
        self.ev |= wg::EV_HIT;
        self.check_win();
        true
    }

    // ---------- keyboard ----------

    fn key_pile(&self) -> usize {
        let t = self.t();
        if self.s.krow == 0 {
            if t.spider() {
                return STOCK;
            }
            let k = self.s.kx as usize;
            return if k < 4 { rules::CELL + k } else { FOUND + k - 4 };
        }
        COL + self.s.kx as usize
    }

    /// the card the cursor is on (-1 for an empty pile), kept in range
    fn key_index(&mut self) -> i32 {
        let p = self.key_pile();
        let t = &self.s.t;
        let n = t.len(p) as i32;
        if !t.is_col(p) {
            return n - 1;
        }
        if n == 0 {
            return -1;
        }
        let lo = t.down_of(p) as i32;
        self.s.kidx = self.s.kidx.clamp(lo, n - 1);
        self.s.kidx
    }

    /// the cursor onto column k, on the run that can be picked up
    fn key_to_col(&mut self, k: i32) {
        self.s.krow = 1;
        self.s.kx = k;
        let p = COL + k as usize;
        self.s.kidx = self.s.t.run_start(p) as i32;
    }

    fn keys(&mut self, input: &Input) {
        let dirs = self.s.rep.dirs(input);
        if input.pressed != 0 {
            self.s.keyboard = 1;
        }
        if self.s.phase != PLAY {
            return;
        }
        let cols = self.s.t.cols() as i32;
        let spider = self.s.t.spider();
        if dirs & (wg::LEFT | wg::RIGHT) != 0 {
            let dx = if dirs & wg::LEFT != 0 { -1 } else { 1 };
            if self.s.krow == 1 {
                let k = (self.s.kx + dx + cols) % cols;
                self.key_to_col(k);
            } else if !spider {
                self.s.kx = (self.s.kx + dx + 8) % 8;
            }
        }
        if dirs & wg::DOWN != 0 {
            if self.s.krow == 0 {
                let k = if spider { 0 } else { self.s.kx };
                self.key_to_col(k);
            } else {
                let n = self.s.t.len(COL + self.s.kx as usize) as i32;
                self.s.kidx = (self.s.kidx + 1).min(n - 1).max(0);
            }
        }
        if dirs & wg::UP != 0 && self.s.krow == 1 {
            let p = COL + self.s.kx as usize;
            let lo = self.s.t.down_of(p) as i32;
            let i = self.key_index();
            if i > lo {
                self.s.kidx = i - 1;
            } else {
                self.s.krow = 0;
                if spider {
                    self.s.kx = 0;
                }
            }
        }
        if input.pressed & wg::A != 0 {
            self.key_act();
        }
        if input.pressed & wg::B != 0 {
            if self.s.holding != 0 {
                self.s.holding = 0;
            } else {
                self.undo();
            }
        }
    }

    /// Space: pick cards up, put them down, or deal
    fn key_act(&mut self) {
        let p = self.key_pile();
        let idx = self.key_index();
        if self.s.holding != 0 {
            let (hp, hi) = (self.s.hold_pile as usize, self.s.hold_idx as usize);
            self.s.holding = 0;
            if hp == p && hi as i32 == idx {
                self.send(p, idx);
                return;
            }
            if hp == p {
                return;
            }
            // put down: the part of the held cards that fits there
            let n = self.s.t.len(hp);
            let mut d = p;
            if self.s.t.is_cell(p) && self.s.t.len(p) > 0 {
                d = self.s.t.empty_cell().unwrap_or(p);
            }
            if self.s.t.is_found(p) {
                d = self.s.t.top(hp).and_then(|c| self.s.t.home_for(c)).unwrap_or(p);
            }
            if let Some(i) = (hi..n).find(|&i| self.s.t.can_move(hp, i, d)) {
                self.play(hp, i, d);
                self.follow(d);
            }
            return;
        }
        if p == STOCK {
            self.deal_row();
        } else if idx >= 0 && self.s.t.movable(p, idx as usize) {
            self.s.holding = 1;
            self.s.hold_pile = p as i32;
            self.s.hold_idx = idx;
        }
    }

    /// the cursor stays on the pile the cards went to
    fn follow(&mut self, d: usize) {
        if self.s.t.is_col(d) {
            self.s.kidx = self.s.t.run_start(d) as i32;
        }
    }

    // ---------- drawing ----------

    fn lifted(&self, p: usize, i: usize) -> bool {
        self.dragging && self.press.is_some_and(|pr| pr.pile == p && pr.idx >= 0 && i >= pr.idx as usize)
    }
    fn lit(&self, p: usize, i: usize) -> bool {
        if self.s.holding != 0 && self.s.hold_pile as usize == p && i as i32 >= self.s.hold_idx {
            return true;
        }
        matches!(self.hint, Some(Play::Move { from, idx, .. }) if self.hint_ms > 0 && from == p && i >= idx)
    }
}

// ---------- cards ----------

fn suit_icon(s: &mut Screen, cx: f32, cy: f32, z: f32, suit: i32, c: u32, a: f32) {
    let r = z * 0.27;
    match suit {
        1 => {
            // heart
            s.disc(cx - z * 0.235, cy - z * 0.14, r, c, a);
            s.disc(cx + z * 0.235, cy - z * 0.14, r, c, a);
            s.poly(&[(cx - z * 0.49, cy - z * 0.06), (cx, cy - z * 0.02), (cx + z * 0.49, cy - z * 0.06), (cx, cy + z * 0.5)], c, a);
        }
        2 => {
            // diamond
            s.poly(&[(cx, cy - z * 0.52), (cx + z * 0.38, cy), (cx, cy + z * 0.52), (cx - z * 0.38, cy)], c, a);
        }
        0 => {
            // spade
            s.poly(&[(cx, cy - z * 0.52), (cx + z * 0.48, cy + z * 0.06), (cx, cy + z * 0.06), (cx - z * 0.48, cy + z * 0.06)], c, a);
            s.disc(cx - z * 0.235, cy + z * 0.1, r, c, a);
            s.disc(cx + z * 0.235, cy + z * 0.1, r, c, a);
            s.tri((cx, cy + z * 0.1), (cx + z * 0.17, cy + z * 0.5), (cx - z * 0.17, cy + z * 0.5), c, a);
        }
        _ => {
            // club
            let (q, o) = (z * 0.22, z * 0.25);
            s.disc(cx, cy - o, q, c, a);
            s.disc(cx - o, cy + z * 0.07, q, c, a);
            s.disc(cx + o, cy + z * 0.07, q, c, a);
            s.disc(cx, cy, q * 0.6, c, a);
            s.tri((cx, cy - z * 0.05), (cx + z * 0.17, cy + z * 0.5), (cx - z * 0.17, cy + z * 0.5), c, a);
        }
    }
}

/// the card's outline and shadow; w < CW while it turns over
fn card_base(s: &mut Screen, x: f32, y: f32, w: f32, edge: u32) {
    let r = 5.5f32.min(w / 2.0);
    // only the bottom edge's shadow shows past the cards around it
    s.shadow(x + 1.0, y + CH - 8.0, w - 2.0, 9.5, r, 2.5, 0, 0.32);
    s.rect(x, y, w, CH, r, edge, 1.0);
    s.rect(x + 0.7, y + 0.7, w - 1.4, CH - 1.4, r - 0.7, FACE, 1.0);
}

fn card_back(s: &mut Screen, x: f32, y: f32, w: f32, th: &Theme) {
    card_base(s, x, y, w, EDGE);
    if w < 10.0 {
        return;
    }
    let (ix, iy, iw, ih) = (x + 4.0, y + 4.0, w - 8.0, CH - 8.0);
    s.rect_grad(ix, iy, iw, ih, 3.5, th.back_hi, th.back, 1.0);
    if w < CW {
        return;
    }
    // a lattice of small diamonds
    for j in 0..9 {
        for i in 0..7 {
            let cx = ix + 4.0 + i as f32 * 8.0 + (j & 1) as f32 * 4.0;
            let cy = iy + 4.5 + j as f32 * 8.5;
            if cx > ix + iw - 3.0 {
                continue;
            }
            s.poly(&[(cx, cy - 2.4), (cx + 2.0, cy), (cx, cy + 2.4), (cx - 2.0, cy)], th.back_hi, 0.55);
        }
    }
}

/// a card's face; `tall` draws a bigger corner, for phones
fn card_face(s: &mut Screen, x: f32, y: f32, face: usize, w: f32, lit: Option<u32>, tall: bool) {
    let (r, suit) = (face % 13, (face / 13) as i32);
    let c = if suit == 1 || suit == 2 { RED } else { BLACK };
    card_base(s, x, y, w, lit.unwrap_or(EDGE));
    if let Some(a) = lit {
        s.rect(x + 0.7, y + 0.7, w - 1.4, CH - 1.4, 4.8, a, 0.16);
    }
    if w < CW - 4.0 {
        // turning over: just the middle
        suit_icon(s, x + w / 2.0, y + CH / 2.0, 24.0f32.min(w * 0.5), suit, c, 1.0);
        return;
    }
    if tall {
        s.text(RANKS[r], x + 4.0, y + 13.5, if r == 9 { 17.0 } else { 21.0 }, c, 1.0);
        suit_icon(s, x + CW - 12.0, y + 13.5, 14.0, suit, c, 1.0);
    } else {
        s.text(RANKS[r], x + 5.0, y + 11.0, if r == 9 { 14.0 } else { 16.0 }, c, 1.0);
        suit_icon(s, x + CW - 11.0, y + 11.0, 11.0, suit, c, 1.0);
    }
    if r >= 10 {
        // court cards: the letter, framed, with the suit under it
        let t = 0.8;
        let top = if tall { 28.0 } else { 24.0 };
        s.rect(x + 12.0, y + top, CW - 24.0, CH - top - 7.0, 3.0, c, 1.0);
        s.rect(x + 12.0 + t, y + top + t, CW - 24.0 - 2.0 * t, CH - top - 7.0 - 2.0 * t, 2.4, FACE, 1.0);
        s.text_c(RANKS[r], x + CW / 2.0, y + top + 19.0, 22.0, c, 1.0);
        suit_icon(s, x + CW / 2.0, y + top + 40.0 - if tall { 3.0 } else { 0.0 }, 11.0, suit, c, 1.0);
    } else {
        suit_icon(s, x + CW / 2.0, y + if tall { 55.0 } else { 52.0 }, 26.0, suit, c, 1.0);
    }
}

/// a box's outline
fn outline(s: &mut Screen, x: f32, y: f32, w: f32, h: f32, c: u32) {
    let t = 1.1;
    s.line(x, y, x + w, y, t, c, 1.0);
    s.line(x + w, y, x + w, y + h, t, c, 1.0);
    s.line(x + w, y + h, x, y + h, t, c, 1.0);
    s.line(x, y + h, x, y, t, c, 1.0);
}

impl Pantry {
    /// where each card belongs: (x, y, pile, index)
    fn targets(&self, out: &mut Vec<(f32, f32, usize, usize)>) {
        let t = self.t();
        out.clear();
        out.resize(t.cards(), (0.0, 0.0, 0, 0));
        for p in t.piles() {
            for i in 0..t.len(p) {
                let (x, y) = self.card_xy(p, i);
                out[t.card(p, i) as usize] = (x, y, p, i);
            }
        }
    }

    /// moves every card on toward its place
    fn animate(&mut self, tg: &[(f32, f32, usize, usize)]) {
        let dt = self.anim_dt.min(60);
        self.anim_dt = 0;
        let n = tg.len();
        if self.snap {
            self.snap = false;
            self.dealing = false;
            for c in 0..n {
                self.pos[c] = (tg[c].0, tg[c].1);
                self.delay[c] = 0;
            }
        }
        if self.dealing {
            // every card leaves the deck, one after another in dealing order
            self.dealing = false;
            let l = &self.lay;
            let from = if self.t().spider() { self.slot(STOCK) } else { (l.pw / 2.0 - CW / 2.0, l.ph + 20.0) };
            let mut order: Vec<usize> = (0..n).filter(|&c| tg[c].2 != STOCK).collect();
            order.sort_by_key(|&c| (tg[c].3, tg[c].2));
            for c in 0..n {
                self.pos[c] = if tg[c].2 == STOCK { (tg[c].0, tg[c].1) } else { from };
                self.delay[c] = 0;
            }
            for (k, &c) in order.iter().enumerate() {
                self.delay[c] = k as i32 * 14;
            }
        }
        let k = 1.0 - 0.5f32.powf(dt as f32 / 32.0);
        let mut busy = false;
        for c in 0..n {
            if self.delay[c] > 0 {
                self.delay[c] -= dt;
                busy = true;
                continue;
            }
            let (x, y) = self.pos[c];
            let (dx, dy) = (tg[c].0 - x, tg[c].1 - y);
            if dx.abs() < 0.3 && dy.abs() < 0.3 {
                self.pos[c] = (tg[c].0, tg[c].1);
            } else {
                self.pos[c] = (x + dx * k, y + dy * k);
                busy = true;
            }
        }
        if self.dragging {
            if let Some(pr) = self.press {
                let (ox, oy) = (self.mx - pr.x, self.my - pr.y);
                for c in 0..n {
                    if self.lifted(tg[c].2, tg[c].3) {
                        self.pos[c] = (tg[c].0 + ox, tg[c].1 + oy);
                    }
                }
            }
        }
        self.animating = busy || self.turn.iter().any(|&v| v > 0);
    }

    /// one card at its place on screen, face up or down
    fn draw_card(&mut self, s: &mut Screen, card: u8, p: usize, i: usize, x: f32, y: f32) {
        let t = &self.s.t;
        let th = self.th();
        let tall = self.lay.tall;
        if !t.face_up(p, i) {
            let key = self.s.opt[O_THEME] as u32 + 1;
            s.sprite_make_alpha(&mut self.back, key, -1.0, -1.0, CW + 2.0, CH + 6.0, |s| card_back(s, 0.0, 0.0, CW, th));
            s.sprite(&self.back, x - 1.0, y - 1.0);
            return;
        }
        let face = t.face(card);
        if t.is_col(p) && i == t.len(p) - 1 && i == t.down_of(p) && self.turn[p - COL] > 0 {
            // turning over
            let k = wg::ease_out(1.0 - self.turn[p - COL] as f32 / TURN_MS as f32);
            let w = (CW * (k * 2.0 - 1.0).abs()).max(1.0);
            if k < 0.5 {
                card_back(s, x + (CW - w) / 2.0, y, w, th);
            } else {
                card_face(s, x + (CW - w) / 2.0, y, face, w, None, tall);
            }
            return;
        }
        let lit = self.lit(p, i);
        let accent = th.accent;
        let key = wg::hash(face as u32 + 1, &[tall as u8, self.s.opt[O_THEME] as u8]);
        let sp = if lit { &mut self.lit_faces[face] } else { &mut self.faces[face] };
        s.sprite_make_alpha(sp, key, -1.0, -1.0, CW + 2.0, CH + 6.0, |s| {
            card_face(s, 0.0, 0.0, face, CW, if lit { Some(accent) } else { None }, tall)
        });
        s.sprite(sp, x - 1.0, y - 1.0);
    }

    fn draw_table(&mut self, s: &mut Screen, tg: &[(f32, f32, usize, usize)]) {
        let n = tg.len();
        let settled = |me: &Self, c: usize| me.delay[c] <= 0 && me.pos[c] == (tg[c].0, tg[c].1) && !me.lifted(tg[c].2, tg[c].3);
        let raise = |me: &Self, p: usize, i: usize| if me.lit(p, i) { -4.0 } else { 0.0 };
        let piles: Vec<usize> = self.t().piles().collect();
        // cards at rest, pile by pile; in a column only the strip a card
        // shows past the next one is drawn
        for &p in &piles {
            let len = self.t().len(p);
            if len == 0 {
                continue;
            }
            if self.t().is_col(p) {
                for i in 0..len {
                    let c = self.t().card(p, i) as usize;
                    if !settled(self, c) {
                        continue;
                    }
                    let (x, y) = (tg[c].0, tg[c].1 + raise(self, p, i));
                    let next = (i + 1 < len).then(|| self.t().card(p, i + 1) as usize).filter(|&d| settled(self, d));
                    match next {
                        Some(d) => {
                            let ny = tg[d].1 + raise(self, p, i + 1);
                            s.set_clip(x - 2.0, y - 2.0, CW + 4.0, ny - y + 8.0);
                        }
                        None => s.no_clip(),
                    }
                    self.draw_card(s, c as u8, p, i, x, y);
                }
                s.no_clip();
            } else if p == STOCK {
                // the top card of each layer of ten, bottom layer first
                for i in (9..len).step_by(10) {
                    let c = self.t().card(p, i) as usize;
                    if settled(self, c) {
                        self.draw_card(s, c as u8, p, i, tg[c].0, tg[c].1);
                    }
                }
            } else if let Some(i) = (0..len).rev().find(|&i| settled(self, self.t().card(p, i) as usize)) {
                // only the top card at rest shows
                let c = self.t().card(p, i) as usize;
                self.draw_card(s, c as u8, p, i, tg[c].0, tg[c].1 + raise(self, p, i));
            }
        }
        let mut loose = core::mem::take(&mut self.loose);
        // cards on their way, over the rest, in pile order
        loose.clear();
        loose.extend((0..n).filter(|&c| !settled(self, c) && !self.lifted(tg[c].2, tg[c].3)));
        // those still waiting to be dealt lie under those already moving
        loose.sort_by_key(|&c| (self.delay[c] <= 0, -self.delay[c], tg[c].2, tg[c].3));
        for &c in &loose {
            let (x, y) = self.pos[c];
            self.draw_card(s, c as u8, tg[c].2, tg[c].3, x, y);
        }
        // the dragged cards, last
        if self.dragging {
            loose.clear();
            loose.extend((0..n).filter(|&c| self.lifted(tg[c].2, tg[c].3)));
            loose.sort_by_key(|&c| tg[c].3);
            for &c in &loose {
                let (x, y) = self.pos[c];
                s.shadow(x + 2.0, y + 5.0, CW, CH, 6.0, 6.0, 0, 0.22);
                self.draw_card(s, c as u8, tg[c].2, tg[c].3, x, y);
            }
        }
        self.loose = loose;
    }

    /// the hint's goal and the keyboard cursor
    fn draw_marks(&mut self, s: &mut Screen) {
        let th = self.th();
        if self.hint_ms > 0 {
            match self.hint {
                Some(Play::Move { to, .. }) => {
                    let (x, y) = self.pile_top(to);
                    outline(s, x - 3.5, y - 3.5, CW + 7.0, CH + 7.0, th.accent);
                }
                Some(Play::Deal) => {
                    let n = self.t().len(STOCK);
                    let (x, y) = self.card_xy(STOCK, n.saturating_sub(1));
                    outline(s, x - 3.5, y - 3.5, CW + 7.0, CH + 7.0, th.accent);
                }
                None => {}
            }
        }
        if self.s.keyboard != 0 && self.s.phase == PLAY {
            let p = self.key_pile();
            let idx = self.key_index();
            let n = self.t().len(p);
            let (x, y, h) = if idx < 0 {
                let (x, y) = self.slot(p);
                (x, y, CH)
            } else {
                let (x, y) = self.card_xy(p, idx as usize);
                let (_, by) = self.card_xy(p, n - 1);
                let lift = if self.lit(p, idx as usize) { 4.0 } else { 0.0 };
                (x, y - lift, by + CH - y + lift)
            };
            outline(s, x - 3.5, y - 3.5, CW + 7.0, h + 7.0, th.accent);
        }
    }

    /// where the next card on pile p would lie
    fn pile_top(&self, p: usize) -> (f32, f32) {
        let n = self.t().len(p);
        if n == 0 || !self.t().is_col(p) {
            return self.slot(p);
        }
        self.card_xy(p, n - 1)
    }

    fn draw_bar(&self, s: &mut Screen) {
        let th = self.th();
        let l = &self.lay;
        let f = l.font;
        let mid = (l.bar - 2.0) / 2.0 + 0.5;
        let mut x = l.left + 4.0;
        if self.s.mode == FREECELL {
            let deal = format!("#{}", self.s.deal);
            x = s.text(&deal, x, mid, f, th.text, 1.0) + f * 1.3;
        } else {
            let suits = [1, 1, 2, 4][self.s.mode as usize];
            for k in 0..suits {
                suit_icon(s, x + f * 0.45, mid, f * 0.9, [0, 1, 2, 3][k], th.text, 1.0);
                x += f * 1.0;
            }
            x += f * 0.8;
        }
        let x = s.text(&wg::clock(self.s.time_ms), x, mid, f, th.text, 1.0) + f * 1.3;
        let (hx, _, _, _) = l.button(1);
        let room = hx - x - 8.0;
        if self.msg_ms > 0 {
            let m = match self.msg {
                MSG_FILL => wg::tr(&["Fill every column first", "先把每一列都放上牌", "先に空いた列を埋めてください", "빈 열을 먼저 채우세요", "Primero llena todas las columnas", "Preencha todas as colunas primeiro", "Remplissez d'abord chaque colonne", "Erst jede Spalte füllen", "Сначала заполните все столбцы"]),
                _ => wg::tr(&["No useful moves left", "没有有用的走法了", "有効な手がありません", "쓸 만한 수가 없습니다", "No quedan movimientos útiles", "Não há mais jogadas úteis", "Plus aucun coup utile", "Keine sinnvollen Züge mehr", "Полезных ходов не осталось"]),
            };
            let size = s.fit_size(m, f, room);
            s.text(m, x, mid, size, th.accent, 1.0);
        } else {
            let label = wg::tr(&["Moves ", "步数 ", "手数 ", "이동 ", "Movimientos ", "Movimentos ", "Coups ", "Züge ", "Ходы "]);
            let text = format!("{}{}", label, self.s.moves);
            let size = s.fit_size(&text, f, room);
            s.text(&text, x, mid, size, th.dim, 1.0);
        }
        for b in 1..=2 {
            let (bx, by, bw, bh) = l.button(b);
            let can = self.s.phase == PLAY && (b == 1 || self.s.ulen > 0);
            let hot = can && self.hover == b && self.s.keyboard == 0;
            let a = if can { 1.0 } else { 0.4 };
            s.rect(bx, by, bw, bh, 6.0, if hot { th.accent } else { th.panel }, a);
            let name = if b == 1 {
                wg::tr(&["Hint", "提示", "ヒント", "힌트", "Pista", "Dica", "Indice", "Tipp", "Подсказка"])
            } else {
                wg::tr(&["Undo", "撤销", "元に戻す", "되돌리기", "Deshacer", "Desfazer", "Annuler", "Zurück", "Отмена"])
            };
            let size = s.fit_size(name, f, bw - 8.0);
            s.text_c(name, bx + bw / 2.0, by + bh / 2.0, size, if hot { th.panel } else { th.text }, a);
        }
    }

    fn draw_won(&self, s: &mut Screen) {
        let th = self.th();
        let l = &self.lay;
        let z = if l.tall { 1.3 } else { 1.0 };
        let (pw, ph) = (240.0 * z, 100.0 * z);
        let (px, py) = ((l.pw - pw) / 2.0, l.tab_y + 30.0);
        s.shadow(px, py + 4.0, pw, ph, 14.0, 12.0, 0, 0.4);
        s.rect(px, py, pw, ph, 14.0, th.panel, 0.96);
        let solved = wg::tr(&["Solved", "完成", "完成", "완성", "Resuelto", "Resolvido", "Résolu", "Gelöst", "Решено"]);
        s.text_c(solved, l.pw / 2.0, py + 30.0 * z, 26.0 * z, th.accent, 1.0);
        let score = format!("{}{}", wg::tr(&["Score ", "得分 ", "スコア ", "점수 ", "Puntos ", "Pontos ", "Score ", "Punkte ", "Очки "]), self.s.score);
        s.text_c(&score, l.pw / 2.0, py + 70.0 * z, 18.0 * z, th.text, 1.0);
    }
}

impl Game for Pantry {
    type Save = Save;
    const CHOICES: &'static [i32] = &[MODES, THEMES as i32];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        // SAFETY: Save is Plain, so all zeroes is a valid value
        let mut s: Save = unsafe { core::mem::zeroed() };
        s.version = VERSION;
        s.t = Table::empty(FREECELL);
        let (pw, ph) = plan_for(FREECELL, 640, 480);
        Pantry {
            s,
            lay: layout(FREECELL, pw, ph),
            pos: [(0.0, 0.0); MAXP],
            delay: [0; MAXP],
            turn: [0; 10],
            anim_dt: 0,
            animating: false,
            snap: true,
            dealing: false,
            mx: -1.0,
            my: -1.0,
            press: None,
            dragging: false,
            hover: 0,
            hint: None,
            hint_ms: 0,
            msg: 0,
            msg_ms: 0,
            ev: 0,
            faces: (0..52).map(|_| Sprite::default()).collect(),
            lit_faces: (0..52).map(|_| Sprite::default()).collect(),
            back: Sprite::default(),
            targets: Vec::new(),
            loose: Vec::new(),
        }
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        let opt = self.s.opt;
        let faces = core::mem::take(&mut self.faces);
        let lit_faces = core::mem::take(&mut self.lit_faces);
        let back = core::mem::take(&mut self.back);
        *self = Pantry::new();
        (self.faces, self.lit_faces, self.back) = (faces, lit_faces, back);
        let s = &mut self.s;
        s.opt = opt;
        s.mode = mode_of(&opt);
        s.rng = Rng::new(seed);
        if s.mode == FREECELL {
            s.deal = rules::deal_number(seed) as i32;
            s.t = rules::deal_freecell(s.deal as u32);
        } else {
            s.t = rules::deal_spider(s.mode, &mut s.rng);
        }
        s.auto = 1;
        s.auto_ms = 900;
        s.krow = 1;
        s.kidx = s.t.run_start(COL) as i32;
        self.snap = false;
        self.dealing = true;
    }

    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        plan_for(self.s.mode, w, h)
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt;
        if !self.animating {
            self.anim_dt = 0;
        }
        self.anim_dt += dt;
        for v in self.turn.iter_mut() {
            *v = (*v - dt).max(0);
        }
        self.hint_ms = (self.hint_ms - dt).max(0);
        self.msg_ms = (self.msg_ms - dt).max(0);
        if self.s.phase == WON {
            if self.s.end_ms > 0 {
                self.s.end_ms -= dt;
                if self.s.end_ms <= 0 {
                    self.s.end_ms = 0;
                    self.ev |= wg::EV_GAMEOVER;
                }
            }
            return core::mem::take(&mut self.ev);
        }
        if self.s.started != 0 {
            self.s.time_ms = self.s.time_ms.saturating_add(dt);
        }
        self.keys(input);
        if self.s.auto != 0 && !self.dragging && dt > 0 {
            self.s.auto_ms -= dt;
            if self.s.auto_ms <= 0 {
                self.s.auto_ms = AUTO_MS;
                if !self.auto_step() {
                    self.s.auto = 0;
                }
            }
        }
        core::mem::take(&mut self.ev)
    }

    fn key(&mut self, code: i32) -> bool {
        if self.s.phase != PLAY {
            return false;
        }
        match code {
            0x75 => self.undo(),      // u
            0x68 => self.give_hint(), // h
            _ => return false,
        }
        true
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.hover = 0;
            self.press = None;
            self.dragging = false;
            return core::mem::take(&mut self.ev);
        }
        // the keyboard's cursor goes once the pointer is used
        if p.touch || p.x != self.mx || p.y != self.my {
            self.s.keyboard = 0;
        }
        self.mx = p.x;
        self.my = p.y;
        self.hover = if p.touch { 0 } else { self.lay.button_at(p.x, p.y) };
        if self.s.phase != PLAY {
            return core::mem::take(&mut self.ev);
        }
        if p.pressed & wg::M_LEFT != 0 {
            self.s.holding = 0;
            match self.lay.button_at(p.x, p.y) {
                1 => self.give_hint(),
                2 => self.undo(),
                _ => {
                    self.press = self.hit(p.x, p.y).map(|(pile, idx)| Press { pile, idx, x: p.x, y: p.y });
                    self.dragging = false;
                }
            }
        }
        if let Some(pr) = self.press {
            if p.held & wg::M_LEFT != 0 && !self.dragging {
                let far = if p.touch { 8.0 } else { 5.0 };
                let (dx, dy) = (p.x - pr.x, p.y - pr.y);
                if dx * dx + dy * dy > far * far && pr.idx >= 0 && self.s.t.movable(pr.pile, pr.idx as usize) {
                    self.dragging = true;
                    self.hint_ms = 0;
                }
            }
        }
        if p.released & wg::M_LEFT != 0 {
            if let Some(pr) = self.press.take() {
                if self.dragging {
                    self.dragging = false;
                    let idx = pr.idx as usize;
                    let (x, y) = self.card_xy(pr.pile, idx);
                    let (x, y) = (x + p.x - pr.x, y + p.y - pr.y);
                    let card = self.s.t.card(pr.pile, idx);
                    if let Some(d) = self.drop_at(x, y, card) {
                        if !self.play(pr.pile, idx, d) && self.s.t.is_cell(d) {
                            if let Some(c) = self.s.t.empty_cell() {
                                self.play(pr.pile, idx, c);
                            }
                        }
                    }
                } else {
                    self.send(pr.pile, pr.idx);
                }
            }
        }
        core::mem::take(&mut self.ev)
    }

    fn picture(&mut self) -> u32 {
        let s = &self.s;
        let hint = match self.hint {
            Some(Play::Move { from, idx, to }) => (from * 10000 + idx * 100 + to) as i32,
            Some(Play::Deal) => 1,
            None => 0,
        };
        let drag = if self.dragging { ((self.mx * 8.0) as i32, (self.my * 8.0) as i32) } else { (0, 0) };
        let v = [
            s.phase, s.holding, s.hold_pile, s.hold_idx, s.keyboard, s.krow, s.kx, s.kidx, self.dragging as i32,
            drag.0, drag.1, self.hover, s.ulen, s.time_ms / 1000, s.moves, s.opt[O_THEME], (self.hint_ms > 0) as i32,
            hint, (self.msg_ms > 0) as i32, self.msg, s.score,
        ];
        wg::hash(wg::hash(0, wg::bytes_of(&s.t)), wg::bytes_of(&v))
    }
    fn moving(&self) -> bool {
        self.animating || self.turn.iter().any(|&v| v > 0)
    }

    fn draw(&mut self, s: &mut Screen) {
        self.lay = layout(self.s.mode, s.pw, s.ph);
        let th = self.th();
        let l = self.lay;
        let spider = self.t().spider();
        let key = wg::hash(self.s.opt[O_THEME] as u32 + 1, wg::bytes_of(&[l.pw, l.ph, self.s.mode as f32]));
        let me = &*self;
        s.background(key, |s| {
            let (x0, y0, x1, y1) = s.visible();
            s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, th.table_lo, 1.0);
            s.rect_grad(x0, l.bar - 2.0, x1 - x0, y1 - l.bar + 2.0, 0.0, mix(hex(0xffffff), th.table, 0.05), th.table_lo, 1.0);
            for p in me.t().piles() {
                let (x, y) = me.slot(p);
                s.rect(x, y, CW, CH, 6.0, th.slot, 0.9);
                s.rect(x + 1.6, y + 1.6, CW - 3.2, CH - 3.2, 4.6, th.table_lo, 1.0);
                if !spider && me.t().is_found(p) {
                    s.text_c("A", x + CW / 2.0, y + CH / 2.0, 22.0, th.slot, 1.0);
                }
            }
        });
        let mut tg = core::mem::take(&mut self.targets);
        self.targets(&mut tg);
        self.animate(&tg);
        self.draw_table(s, &tg);
        self.targets = tg;
        self.draw_marks(s);
        self.draw_bar(s);
        if self.s.phase == WON {
            self.draw_won(s);
        }
    }

    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.phase == WON
    }
    /// time, moves, cards home, undos, hints, deal, move bonus, time bonus
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        match i {
            0 => s.time_ms,
            1 => s.moves,
            2 => s.t.home() as i32,
            3 => s.undos,
            4 => s.hints,
            5 if s.mode == FREECELL => s.deal,
            6 if s.phase == WON => s.move_bonus,
            7 if s.phase == WON => s.time_bonus,
            _ => -1,
        }
    }

    fn loaded(&mut self) -> bool {
        let s = &self.s;
        if s.version != VERSION || !(0..MODES).contains(&s.mode) || s.t.mode != s.mode || !(s.phase == PLAY || s.phase == WON) {
            return false;
        }
        if s.time_ms < 0 || s.moves < 0 || s.undos < 0 || s.hints < 0 || !(0..=UNDO as i32).contains(&s.ulen) {
            return false;
        }
        if s.mode == FREECELL && !(1..=32000).contains(&s.deal) {
            return false;
        }
        if !valid_table(&s.t) || s.t.won() != (s.phase == WON) {
            return false;
        }
        // every step of the undo list goes back to a table that holds
        let mut t = s.t;
        for k in (0..s.ulen as usize).rev() {
            let st = Step::unpack(s.undo[k]);
            if !t.can_unapply(st) {
                return false;
            }
            t.unapply(st);
            if !valid_table(&t) {
                return false;
            }
        }
        let s = &mut self.s;
        s.kx = s.kx.clamp(0, s.t.cols() as i32 - 1);
        s.krow = s.krow.clamp(0, 1);
        s.holding = 0;
        s.auto_ms = s.auto_ms.clamp(0, 1000);
        self.snap = true;
        self.dealing = false;
        self.press = None;
        self.dragging = false;
        self.hint = None;
        self.hint_ms = 0;
        self.msg_ms = 0;
        self.turn = [0; 10];
        self.hover = 0;
        true
    }
}

/// every card once, piles no taller than they can grow, and each column's
/// face-down cards under at least one face-up card
fn valid_table(t: &Table) -> bool {
    let mut seen = [false; MAXP];
    let mut total = 0;
    let used: Vec<usize> = t.piles().collect();
    for p in 0..NPILES {
        let n = t.len(p);
        if n > MAXP || (n > 0 && !used.contains(&p)) {
            return false;
        }
        let most = if t.is_cell(p) {
            1
        } else if t.is_found(p) {
            13
        } else if p == STOCK {
            50
        } else {
            MAXP
        };
        if n > most {
            return false;
        }
        for i in 0..n {
            let c = t.card(p, i) as usize;
            if c >= t.cards() || seen[c] {
                return false;
            }
            seen[c] = true;
            total += 1;
        }
    }
    for k in 0..10 {
        let p = COL + k;
        let (n, d) = (t.len(p), t.down[k] as usize);
        if if n > 0 { d >= n } else { d > 0 } {
            return false;
        }
    }
    total == t.cards()
}

wg::game!(Pantry);

#[cfg(test)]
mod tests {
    use super::*;

    /// plays a run by following hints, frame by frame; returns the events seen
    fn play_out(g: &mut Pantry) -> u32 {
        let mut ev = 0;
        let mut seen: Vec<u32> = Vec::new();
        for _ in 0..20000 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
            if ev & wg::EV_GAMEOVER != 0 {
                break;
            }
            if g.s.phase != PLAY || g.s.auto != 0 {
                continue;
            }
            seen.push(g.s.t.sig());
            match g.s.t.hint(|h| seen.contains(&h)) {
                Some(Play::Move { from, idx, to }) => assert!(g.play(from, idx, to)),
                Some(Play::Deal) => assert!(g.deal_row()),
                None => break,
            }
        }
        ev
    }

    #[test]
    fn spider_win_ends_the_run() {
        let mut won = 0;
        for seed in 1..=12 {
            let mut g = Pantry::new();
            g.s.opt = [1, 0];
            g.start(seed);
            let ev = play_out(&mut g);
            if g.over() {
                won += 1;
                assert!(ev & wg::EV_WIN != 0 && ev & wg::EV_GAMEOVER != 0);
                assert!(g.score() >= 100 && g.score() <= 18000, "score {}", g.score());
                assert_eq!(g.stat(2), 104);
                assert_eq!(g.stat(5), -1);
                assert!(g.loaded());
            } else {
                // a run in progress saves and loads, and undo walks it all back
                assert!(g.loaded());
                while g.s.ulen > 0 {
                    g.undo();
                }
                assert_eq!(g.s.t.n[STOCK], 50);
            }
        }
        assert!(won > 0);
    }

    #[test]
    fn freecell_win_and_bad_saves() {
        let mut won = false;
        for seed in 0..40 {
            let mut g = Pantry::new();
            g.start(seed);
            assert_eq!(g.s.deal, rules::deal_number(seed) as i32);
            play_out(&mut g);
            if g.over() {
                won = true;
                assert_eq!(g.stat(2), 52);
                assert!(g.score() > 2000);
                break;
            }
        }
        assert!(won);
        let mut g = Pantry::new();
        g.start(7);
        g.s.t.c[COL][0] = g.s.t.c[COL + 1][0]; // a card twice
        assert!(!g.loaded());
    }
}
