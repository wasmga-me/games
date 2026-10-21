//! Sudoku - an original wasmga.me game. MIT licensed.
//! Fill the grid so every row, column and box holds 1 to 9 once. Every
//! puzzle is made here and kept only if it can be finished by reasoning
//! alone (logic.rs), so each has one solution and needs no guessing. Notes,
//! undo and hints that say why; the page draws setup, pause and game over.
mod logic;
mod glyphs;

use logic::{Why, ALL, PEERS, UNITS, UNIT_OF};
use wg::{ease_out, hex, mix, Game, Input, KeyRepeat, Pointer, Rng, Screen};

const VERSION: u32 = 1;
const END_MS: i32 = 1600; // the finished grid stays in view this long
const UNDO: usize = 600;

/// options, in the catalog `setup` order
const O_LEVEL: usize = 0;
const O_THEME: usize = 1;
const THEMES: usize = 4;

/// tools, after the digits 1-9 in the button list
const T_UNDO: u8 = 10;
const T_ERASE: u8 = 11;
const T_NOTES: u8 = 12;
const T_HINT: u8 = 13;
const TOOLS: [u8; 4] = [T_UNDO, T_ERASE, T_NOTES, T_HINT];

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 2],
    rng: Rng,
    rep: KeyRepeat,
    /// the difficulty this grid was made for (the option may change since)
    level: i32,
    given: [u8; 81],
    solution: [u8; 81],
    /// what the grid shows, givens included
    val: [u8; 81],
    notes: [u16; 81],
    sel: i32,
    notes_mode: i32,
    time_ms: i32,
    won: i32,
    end_ms: i32,
    hints: i32,
    placed: i32,
    wrong: i32,
    notes_made: i32,
    undos: i32,
    /// each entry: cell | old value << 7 | old notes << 11 | first of a move << 31
    undo: [u32; UNDO],
    ulen: i32,
}
unsafe impl wg::Plain for Save {}

/// What a hint said, shown for a while; not saved.
#[derive(Clone, Copy, PartialEq)]
enum Msg {
    None,
    Only(u8),
    Last(u8, u8),
    Wrong(u8),
}

struct Sudoku {
    s: Save,
    msg: Msg,
    msg_ms: i32,
    /// per cell: time left of its pop (a digit going in) and of its flash
    /// (a finished row, column or box, a hint, the win)
    pop: [i32; 81],
    flash: [i32; 81],
    hover: i32,
    press: i32,
    ev: u32,
    lay: Layout,
}

#[derive(Clone, Copy, Default)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl Rect {
    fn has(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// Where everything sits, for a plan of pw x ph.
#[derive(Clone, Copy, Default)]
struct Layout {
    board: Rect,
    /// buttons: digits 1-9 at 1..=9, tools at T_*
    btn: [Rect; 14],
    /// the clock and the line under it
    head: Rect,
    tall: bool,
    /// tool buttons show their names beside the icon (wide ones)
    wide_tools: bool,
}

const M: f32 = 10.0;

fn layout(pw: f32, ph: f32) -> Layout {
    let mut l = Layout::default();
    if pw < ph {
        // phone: the board on top, a row of digits and a row of tools below
        l.tall = true;
        l.wide_tools = true;
        let b = pw - 2.0 * M;
        let top = ((ph - (b + 154.0)) / 2.0).max(M);
        l.head = Rect { x: M, y: top, w: b, h: 30.0 };
        l.board = Rect { x: M, y: top + 36.0, w: b, h: b };
        let dy = l.board.y + b + 12.0;
        let dw = (b - 8.0 * 4.0) / 9.0;
        for d in 1..=9 {
            l.btn[d] = Rect { x: M + (d - 1) as f32 * (dw + 4.0), y: dy, w: dw, h: 50.0 };
        }
        let tw = (b - 3.0 * 8.0) / 4.0;
        for (i, &t) in TOOLS.iter().enumerate() {
            l.btn[t as usize] = Rect { x: M + i as f32 * (tw + 8.0), y: dy + 58.0, w: tw, h: 40.0 };
        }
        return l;
    }
    // wide: the board on the left, a column of buttons on its right
    let gap = 14.0;
    let three = pw - 340.0 - 2.0 * M - gap >= 150.0;
    let side = if three { (pw - 340.0 - 2.0 * M - gap).min(186.0) } else { 94.0f32.max(pw - 340.0 - 2.0 * M - gap).min(120.0) };
    let b = (ph - 2.0 * M).min(pw - 2.0 * M - gap - side);
    let x0 = (pw - (b + gap + side)) / 2.0;
    let y0 = (ph - b) / 2.0;
    l.board = Rect { x: x0, y: y0, w: b, h: b };
    let sx = x0 + b + gap;
    l.head = Rect { x: sx, y: y0, w: side, h: 46.0 };
    let cols = if three { 3 } else { 2 };
    let g = 6.0;
    let bw = (side - g * (cols - 1) as f32) / cols as f32;
    let rows = (9 + cols - 1) / cols;
    let tool_rows = if three { 1 } else { 2 };
    let room = b - l.head.h - 10.0 - g * (rows + tool_rows) as f32;
    let tool_h = if three { 40.0f32 } else { (room / (rows + tool_rows) as f32).min(44.0) };
    let bh = ((room - tool_h * tool_rows as f32) / rows as f32).min(bw);
    let mut y = y0 + l.head.h + 10.0;
    for d in 1..=9usize {
        let (r, c) = ((d - 1) / cols, (d - 1) % cols);
        // a last row with fewer digits sits in the middle
        let in_row = (9 - r * cols).min(cols);
        let left = sx + (side - (in_row as f32 * bw + (in_row - 1) as f32 * g)) / 2.0;
        l.btn[d] = Rect { x: left + c as f32 * (bw + g), y: y + r as f32 * (bh + g), w: bw, h: bh };
    }
    y += rows as f32 * (bh + g);
    let per = if three { 4 } else { 2 };
    let tw = (side - g * (per - 1) as f32) / per as f32;
    for (i, &t) in TOOLS.iter().enumerate() {
        let (r, c) = (i / per, i % per);
        l.btn[t as usize] = Rect { x: sx + c as f32 * (tw + g), y: y + r as f32 * (tool_h + g), w: tw, h: tool_h };
    }
    l.wide_tools = tw >= 80.0;
    l
}

struct Theme {
    bg: u32,
    panel: u32,
    cell: u32,
    thin: u32,
    thick: u32,
    given: u32,
    ink: u32,
    note: u32,
    sel: u32,
    peer: u32,
    same: u32,
    bad: u32,
    btn: u32,
    btn_ink: u32,
    dim: u32,
    text: u32,
}

const THEME: [Theme; THEMES] = [
    // Paper
    Theme {
        bg: hex(0xefe6d4), panel: hex(0x3b3226), cell: hex(0xfffaf0), thin: hex(0xd9cdb6), thick: hex(0x3b3226),
        given: hex(0x2a231b), ink: hex(0x2f5fd0), note: hex(0x7a6c58), sel: hex(0x9db7f5), peer: hex(0xe9dfca),
        same: hex(0xc9d6f7), bad: hex(0xd8432f), btn: hex(0xfffaf0), btn_ink: hex(0x2a231b), dim: hex(0x9a8c76), text: hex(0x2a231b),
    },
    // Night
    Theme {
        bg: hex(0x141a2b), panel: hex(0x0b0f1a), cell: hex(0x1f2740), thin: hex(0x323c5c), thick: hex(0x0b0f1a),
        given: hex(0xeef1fa), ink: hex(0x7fc8ff), note: hex(0x98a3c4), sel: hex(0x2f5f9a), peer: hex(0x253050),
        same: hex(0x2c4a78), bad: hex(0xff6b5a), btn: hex(0x232c48), btn_ink: hex(0xeef1fa), dim: hex(0x6b7699), text: hex(0xeef1fa),
    },
    // Sage
    Theme {
        bg: hex(0xdfe7d8), panel: hex(0x2f4a35), cell: hex(0xf7faf2), thin: hex(0xcbd9c3), thick: hex(0x2f4a35),
        given: hex(0x1f2d22), ink: hex(0x2f7d4f), note: hex(0x6b7f6c), sel: hex(0xa9d6a0), peer: hex(0xe6eedf),
        same: hex(0xc3e3bb), bad: hex(0xc8432e), btn: hex(0xf7faf2), btn_ink: hex(0x1f2d22), dim: hex(0x8a9c8a), text: hex(0x1f2d22),
    },
    // Dusk
    Theme {
        bg: hex(0x2a1d33), panel: hex(0x170f1d), cell: hex(0x3a2945), thin: hex(0x4f3a5d), thick: hex(0x170f1d),
        given: hex(0xfbeee6), ink: hex(0xffb36b), note: hex(0xc4a8c9), sel: hex(0x7a4a8a), peer: hex(0x45324f),
        same: hex(0x6a3f6e), bad: hex(0xff5f6d), btn: hex(0x3f2d4a), btn_ink: hex(0xfbeee6), dim: hex(0x8f7898), text: hex(0xfbeee6),
    },
];

/// Each level's name, in wg::LANGS order.
const LEVEL_NAMES: [[&str; 10]; 4] = [
    ["Easy", "简单", "かんたん", "쉬움", "Fácil", "Fácil", "Facile", "Leicht", "Лёгкий", "簡單"],
    ["Medium", "中等", "ふつう", "보통", "Medio", "Médio", "Moyen", "Mittel", "Средний", "中等"],
    ["Hard", "困难", "むずかしい", "어려움", "Difícil", "Difícil", "Difficile", "Schwer", "Сложный", "困難"],
    ["Expert", "专家", "エキスパート", "전문가", "Experto", "Especialista", "Expert", "Experte", "Эксперт", "專家"],
];
/// "Last place for {} in this row", then column and box, in wg::LANGS order.
const LAST_PLACE: [[&str; 10]; 3] = [
    [
        "Last place for {} in this row",
        "这一行里 {} 只剩这个位置",
        "この行で {} が入るのはここだけ",
        "이 행에서 {}의 마지막 자리",
        "Último lugar del {} en esta fila",
        "Último lugar do {} nesta linha",
        "Dernière case du {} sur cette ligne",
        "Letztes Feld für die {} in dieser Zeile",
        "Последнее место для {} в этой строке",
        "這一行裡 {} 只剩這個位置",
    ],
    [
        "Last place for {} in this column",
        "这一列里 {} 只剩这个位置",
        "この列で {} が入るのはここだけ",
        "이 열에서 {}의 마지막 자리",
        "Último lugar del {} en esta columna",
        "Último lugar do {} nesta coluna",
        "Dernière case du {} dans cette colonne",
        "Letztes Feld für die {} in dieser Spalte",
        "Последнее место для {} в этом столбце",
        "這一列裡 {} 只剩這個位置",
    ],
    [
        "Last place for {} in this box",
        "这一宫里 {} 只剩这个位置",
        "このブロックで {} が入るのはここだけ",
        "이 박스에서 {}의 마지막 자리",
        "Último lugar del {} en este bloque",
        "Último lugar do {} neste bloco",
        "Dernière case du {} dans ce bloc",
        "Letztes Feld für die {} in diesem Block",
        "Последнее место для {} в этом блоке",
        "這一宮裡 {} 只剩這個位置",
    ],
];

const POP_MS: i32 = 220;
const FLASH_MS: i32 = 650;

impl Sudoku {
    fn t(&self) -> &'static Theme {
        &THEME[self.s.opt[O_THEME].clamp(0, THEMES as i32 - 1) as usize]
    }

    fn push_undo(&mut self, c: usize, first: bool) {
        let e = c as u32 | (self.s.val[c] as u32) << 7 | ((self.s.notes[c] >> 1) as u32) << 11 | (first as u32) << 31;
        if self.s.ulen as usize == UNDO {
            // full: the oldest move goes, whole
            let mut cut = 1;
            while cut < UNDO && self.s.undo[cut] >> 31 == 0 {
                cut += 1;
            }
            self.s.undo.copy_within(cut.., 0);
            self.s.ulen -= cut as i32;
        }
        self.s.undo[self.s.ulen as usize] = e;
        self.s.ulen += 1;
    }

    fn undo(&mut self) {
        let mut any = false;
        while self.s.ulen > 0 {
            self.s.ulen -= 1;
            let e = self.s.undo[self.s.ulen as usize];
            let c = (e & 127) as usize;
            if c < 81 && self.s.given[c] == 0 {
                self.s.val[c] = (e >> 7 & 15) as u8;
                self.s.notes[c] = ((e >> 11 & 0x1FF) << 1) as u16;
                if !any {
                    self.s.sel = c as i32;
                }
                any = true;
            }
            if e >> 31 != 0 {
                break;
            }
        }
        if any {
            self.s.undos += 1;
            self.ev |= wg::EV_BLIP;
        }
    }

    fn editable(&self) -> Option<usize> {
        let c = self.s.sel;
        (0..81).contains(&c).then_some(c as usize).filter(|&c| self.s.given[c] == 0)
    }

    /// puts d in cell c (0 clears it), clearing the notes it rules out
    fn set(&mut self, c: usize, d: u8, hint: bool) {
        self.push_undo(c, true);
        self.s.val[c] = d;
        self.s.notes[c] = 0;
        if d == 0 {
            return;
        }
        for &p in PEERS[c].iter() {
            let p = p as usize;
            if self.s.notes[p] & (1 << d) != 0 {
                self.push_undo(p, false);
                self.s.notes[p] &= !(1 << d);
            }
        }
        self.pop[c] = POP_MS;
        if !hint {
            self.s.placed += 1;
            if d != self.s.solution[c] {
                self.s.wrong += 1;
            }
        }
        self.ev |= wg::EV_BLIP;
        // a row, column or box just filled without a clash flashes
        for &u in UNIT_OF[c].iter() {
            let cells = &UNITS[u as usize];
            let mut seen = 0u16;
            if cells.iter().all(|&k| {
                let v = self.s.val[k as usize];
                let fresh = v != 0 && seen & (1 << v) == 0;
                seen |= 1 << v;
                fresh
            }) {
                self.flash_unit(u as usize, c);
                self.ev |= wg::EV_HIT;
            }
        }
        if self.s.val == self.s.solution {
            self.s.won = 1;
            self.s.end_ms = END_MS;
            self.s.sel = -1;
            for k in 0..81 {
                let (dr, dc) = ((k / 9) as i32 - (c / 9) as i32, (k % 9) as i32 - (c % 9) as i32);
                self.flash[k] = FLASH_MS + 45 * (dr.abs() + dc.abs());
            }
            self.ev |= wg::EV_WIN;
        }
    }

    /// a flash along a unit, outwards from cell `from`
    fn flash_unit(&mut self, u: usize, from: usize) {
        let at = UNITS[u].iter().position(|&k| k as usize == from).unwrap_or(0) as i32;
        for (i, &k) in UNITS[u].iter().enumerate() {
            self.flash[k as usize] = FLASH_MS + 40 * (i as i32 - at).abs();
        }
    }

    fn digit(&mut self, d: u8, as_note: bool) {
        if self.s.won != 0 {
            return;
        }
        let Some(c) = self.editable() else { return };
        if as_note {
            if self.s.val[c] != 0 {
                return;
            }
            self.push_undo(c, true);
            self.s.notes[c] ^= 1 << d;
            if self.s.notes[c] & (1 << d) != 0 {
                self.s.notes_made += 1;
            }
            self.ev |= wg::EV_BLIP;
        } else if self.s.val[c] == d {
            self.set(c, 0, false);
        } else {
            self.set(c, d, false);
        }
    }

    fn erase(&mut self) {
        let Some(c) = self.editable() else { return };
        if self.s.val[c] != 0 || self.s.notes[c] != 0 {
            self.set(c, 0, false);
            self.ev |= wg::EV_BLIP;
        }
    }

    /// A wrong digit is taken out first (the one selected, else the first);
    /// otherwise the next digit reasoning finds goes in, with why.
    fn hint(&mut self) {
        if self.s.won != 0 {
            return;
        }
        let wrong = |c: usize| self.s.val[c] != 0 && self.s.val[c] != self.s.solution[c];
        let pick = self.editable().filter(|&c| wrong(c)).or_else(|| (0..81).find(|&c| wrong(c)));
        self.s.hints += 1;
        self.msg_ms = 4000;
        if let Some(c) = pick {
            self.msg = Msg::Wrong(self.s.val[c]);
            self.set(c, 0, true);
            self.s.sel = c as i32;
            self.flash[c] = FLASH_MS;
            return;
        }
        let mut known = [0u8; 81];
        for c in 0..81 {
            known[c] = self.s.val[c];
        }
        match logic::hint(&known) {
            Some(h) => {
                let c = h.cell as usize;
                self.s.sel = c as i32;
                self.msg = match h.why {
                    Why::Only => {
                        self.flash[c] = FLASH_MS;
                        Msg::Only(h.digit)
                    }
                    Why::Last(u) => {
                        self.flash_unit(u as usize, c);
                        Msg::Last(h.digit, u / 9)
                    }
                };
                self.set(c, h.digit, true);
            }
            None => {
                // reasoning ran out (it never should): the selected cell's digit
                let c = self.editable().filter(|&c| self.s.val[c] == 0).or_else(|| (0..81).find(|&c| self.s.val[c] == 0));
                if let Some(c) = c {
                    self.s.sel = c as i32;
                    self.msg = Msg::Only(self.s.solution[c]);
                    self.set(c, self.s.solution[c], true);
                }
            }
        }
    }

    fn tool(&mut self, t: u8) {
        if self.s.won != 0 {
            return;
        }
        match t {
            T_UNDO => self.undo(),
            T_ERASE => self.erase(),
            T_NOTES => {
                self.s.notes_mode ^= 1;
                self.ev |= wg::EV_BLIP;
            }
            T_HINT => self.hint(),
            _ => {}
        }
    }

    fn cell_at(&self, x: f32, y: f32) -> i32 {
        let b = self.lay.board;
        if !b.has(x, y) {
            return -1;
        }
        let cs = b.w / 9.0;
        let (c, r) = (((x - b.x) / cs) as i32, ((y - b.y) / cs) as i32);
        r.clamp(0, 8) * 9 + c.clamp(0, 8)
    }

    fn button_at(&self, x: f32, y: f32) -> i32 {
        (1..14).find(|&i| self.lay.btn[i].has(x, y)).map_or(-1, |i| i as i32)
    }

    /// how many times each digit is on the board
    fn counts(&self) -> [u8; 10] {
        let mut n = [0u8; 10];
        for &v in self.s.val.iter() {
            n[v as usize] += 1;
        }
        n
    }

    fn clashes(&self) -> [bool; 81] {
        let mut bad = [false; 81];
        for c in 0..81 {
            let v = self.s.val[c];
            bad[c] = v != 0 && PEERS[c].iter().any(|&p| self.s.val[p as usize] == v);
        }
        bad
    }

    fn score_now(&self) -> u32 {
        if self.s.won == 0 {
            return 0;
        }
        let secs = self.s.time_ms / 1000 + 60 * self.s.hints;
        ((self.s.level + 1) as u32 * 200_000 / (secs as u32 + 120)).max(1)
    }

    // ---------- drawing ----------

    fn draw_board(&self, s: &mut Screen) {
        let t = self.t();
        let b = self.lay.board;
        let cs = b.w / 9.0;
        let sel = self.s.sel;
        let sel_v = if (0..81).contains(&sel) { self.s.val[sel as usize] } else { 0 };
        let bad = self.clashes();
        for c in 0..81usize {
            let (r, k) = (c / 9, c % 9);
            let (x, y) = (b.x + k as f32 * cs, b.y + r as f32 * cs);
            let mut fill = None;
            if sel >= 0 {
                let su = UNIT_OF[sel as usize];
                if UNIT_OF[c].iter().any(|u| su.contains(u)) {
                    fill = Some(t.peer);
                }
                if sel_v != 0 && self.s.val[c] == sel_v {
                    fill = Some(t.same);
                }
                if c as i32 == sel {
                    fill = Some(t.sel);
                }
            }
            if bad[c] && self.s.given[c] == 0 {
                fill = Some(mix(t.bad, fill.unwrap_or(t.cell), 0.22));
            }
            if let Some(f) = fill {
                s.rect(x, y, cs, cs, 0.0, f, 1.0);
            }
            if self.flash[c] > 0 && self.flash[c] <= FLASH_MS {
                let p = self.flash[c] as f32 / FLASH_MS as f32;
                s.rect(x, y, cs, cs, 0.0, t.ink, 0.35 * (1.0 - (2.0 * p - 1.0).abs()));
            }
        }
        // lines over the fills: thin ones, then the box lines
        let thin = (0.9f32).max(s.hair());
        for i in 1..9 {
            if i % 3 == 0 {
                continue;
            }
            let o = i as f32 * cs;
            s.rect(b.x + o - thin / 2.0, b.y, thin, b.h, 0.0, t.thin, 1.0);
            s.rect(b.x, b.y + o - thin / 2.0, b.w, thin, 0.0, t.thin, 1.0);
        }
        let thick = 2.4;
        for i in [3, 6] {
            let o = i as f32 * cs;
            s.rect(b.x + o - thick / 2.0, b.y, thick, b.h, 0.0, t.thick, 1.0);
            s.rect(b.x, b.y + o - thick / 2.0, b.w, thick, 0.0, t.thick, 1.0);
        }
        for c in 0..81usize {
            let (r, k) = (c / 9, c % 9);
            let (x, y) = (b.x + k as f32 * cs, b.y + r as f32 * cs);
            let v = self.s.val[c];
            if v != 0 {
                let p = if self.pop[c] > 0 { ease_out(1.0 - self.pop[c] as f32 / POP_MS as f32) } else { 1.0 };
                let col = if self.s.given[c] != 0 {
                    t.given
                } else if bad[c] {
                    t.bad
                } else {
                    t.ink
                };
                let size = cs * 0.64 * (0.6 + 0.4 * p);
                s.text_c(DIGITS[v as usize], x + cs / 2.0, y + cs / 2.0, size, col, 0.4 + 0.6 * p);
            } else if self.s.notes[c] != 0 {
                let n = self.s.notes[c];
                for d in 1..=9u8 {
                    if n & (1 << d) == 0 {
                        continue;
                    }
                    let (nx, ny) = (x + cs * (0.2 + 0.3 * ((d - 1) % 3) as f32), y + cs * (0.2 + 0.3 * ((d - 1) / 3) as f32));
                    let hot = d == sel_v;
                    if hot {
                        s.disc(nx, ny, cs * 0.14, t.same, 1.0);
                    }
                    s.text_c(DIGITS[d as usize], nx, ny, cs * 0.25, if hot { t.given } else { t.note }, 1.0);
                }
            }
        }
        // the selection's outline
        if (0..81).contains(&sel) {
            let (r, k) = (sel as usize / 9, sel as usize % 9);
            let (x, y) = (b.x + k as f32 * cs, b.y + r as f32 * cs);
            let w = 2.0;
            let col = mix(t.ink, t.sel, 0.75);
            s.rect(x, y, cs, w, 0.0, col, 1.0);
            s.rect(x, y + cs - w, cs, w, 0.0, col, 1.0);
            s.rect(x, y, w, cs, 0.0, col, 1.0);
            s.rect(x + cs - w, y, w, cs, 0.0, col, 1.0);
        }
    }

    fn draw_head(&self, s: &mut Screen) {
        let t = self.t();
        let h = self.lay.head;
        let clock = wg::clock(self.s.time_ms);
        let msg = match self.msg {
            _ if self.msg_ms <= 0 => None,
            Msg::None => None,
            Msg::Only(d) => Some(
                wg::tr(&[
                    "Only {} fits here",
                    "这里只能填 {}",
                    "ここに入るのは {} だけ",
                    "여기에는 {}만 들어갑니다",
                    "Aquí solo cabe el {}",
                    "Aqui só cabe o {}",
                    "Seul le {} va ici",
                    "Hier passt nur die {}",
                    "Сюда подходит только {}",
                    "這裡只能填 {}",
                ])
                .replace("{}", &d.to_string()),
            ),
            Msg::Last(d, u) => Some(wg::tr(&LAST_PLACE[u as usize]).replace("{}", &d.to_string())),
            Msg::Wrong(d) => Some(
                wg::tr(&[
                    "That {} was wrong",
                    "这个 {} 填错了",
                    "この {} は間違いです",
                    "잘못 넣은 {}입니다",
                    "Ese {} estaba mal",
                    "Esse {} estava errado",
                    "Ce {} était faux",
                    "Die {} war falsch",
                    "Цифра {} была неверной",
                    "這個 {} 填錯了",
                ])
                .replace("{}", &d.to_string()),
            ),
        };
        if self.lay.tall {
            let cy = h.y + h.h / 2.0;
            match &msg {
                Some(m) => {
                    let size = s.fit_size(m, 15.0, h.w - 70.0);
                    s.text(m, h.x + 2.0, cy, size, t.ink, 1.0);
                }
                None => {
                    s.text(self.level_name(), h.x + 2.0, cy, 15.0, t.dim, 1.0);
                }
            }
            s.text_r(&clock, h.x + h.w - 2.0, cy, 17.0, t.text, 1.0);
        } else {
            s.text_c(&clock, h.x + h.w / 2.0, h.y + 14.0, 22.0, t.text, 1.0);
            match &msg {
                Some(m) => {
                    let size = s.fit_size(m, 11.0, h.w);
                    s.text_c(m, h.x + h.w / 2.0, h.y + 36.0, size, t.ink, 1.0);
                }
                None => s.text_c(self.level_name(), h.x + h.w / 2.0, h.y + 36.0, 11.0, t.dim, 1.0),
            }
        }
    }

    fn level_name(&self) -> &'static str {
        let i = self.s.level as usize & 3;
        wg::tr(&LEVEL_NAMES[i])
    }

    fn draw_buttons(&self, s: &mut Screen) {
        let t = self.t();
        let n = self.counts();
        let can = self.s.won == 0;
        for i in 1..14usize {
            let r = self.lay.btn[i];
            let lit = i as u8 == T_NOTES && self.s.notes_mode != 0;
            let hot = can && (self.hover == i as i32 || self.press == i as i32);
            let face = if lit {
                t.ink
            } else if hot {
                mix(t.sel, t.btn, 0.45)
            } else {
                t.btn
            };
            s.rect(r.x, r.y, r.w, r.h, 7.0, face, 1.0);
            let ink = if lit { t.btn } else { t.btn_ink };
            if i <= 9 {
                let done = n[i] >= 9;
                let size = (r.h * 0.56).min(r.w * 0.7);
                let col = if self.s.notes_mode != 0 { t.note } else { ink };
                let cy = if self.lay.tall || r.h < 34.0 { r.y + r.h / 2.0 } else { r.y + r.h * 0.45 };
                s.text_c(DIGITS[i], r.x + r.w / 2.0, cy, size, col, if done { 0.3 } else { 1.0 });
                if !self.lay.tall && r.h >= 34.0 && !done {
                    // how many are still to place
                    s.text_c(DIGITS[9 - n[i] as usize], r.x + r.w / 2.0, r.y + r.h * 0.82, (r.h * 0.17).min(10.0), t.dim, 1.0);
                }
            } else {
                let name = match i as u8 {
                    T_UNDO => wg::tr(&["Undo", "撤销", "元に戻す", "되돌리기", "Deshacer", "Desfazer", "Annuler", "Rückgängig", "Отмена", "復原"]),
                    T_ERASE => wg::tr(&["Erase", "擦除", "消す", "지우기", "Borrar", "Apagar", "Effacer", "Löschen", "Стереть"]),
                    T_NOTES => wg::tr(&["Notes", "笔记", "メモ", "메모", "Notas", "Notas", "Notes", "Notizen", "Заметки", "筆記"]),
                    _ => wg::tr(&["Hint", "提示", "ヒント", "힌트", "Pista", "Dica", "Indice", "Tipp", "Подсказка"]),
                };
                let icon = (r.h * 0.5).min(r.w * 0.5).min(20.0);
                if self.lay.wide_tools {
                    let label = s.fit_size(name, (r.h * 0.34).min(14.0), r.w - icon - 14.0);
                    let tw = s.text_width(name, label);
                    let x0 = r.x + (r.w - (icon + 6.0 + tw)) / 2.0;
                    draw_icon(s, i as u8, x0 + icon / 2.0, r.y + r.h / 2.0, icon, ink);
                    s.text(name, x0 + icon + 6.0, r.y + r.h / 2.0, label, ink, 1.0);
                } else if r.h >= 36.0 {
                    draw_icon(s, i as u8, r.x + r.w / 2.0, r.y + r.h * 0.38, icon * 0.9, ink);
                    s.text_c(name, r.x + r.w / 2.0, r.y + r.h * 0.78, s.fit_size(name, (r.h * 0.2).min(10.0), r.w - 4.0), ink, 0.85);
                } else {
                    draw_icon(s, i as u8, r.x + r.w / 2.0, r.y + r.h / 2.0, icon, ink);
                }
            }
        }
    }
}

const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// a tool's picture, centred on cx, cy, about `z` across
fn draw_icon(s: &mut Screen, tool: u8, cx: f32, cy: f32, z: f32, c: u32) {
    let w = (z * 0.09).max(0.8);
    match tool {
        T_UNDO => {
            // an arrow curling back to the left
            let r = z * 0.32;
            let (ox, oy) = (cx + z * 0.06, cy + z * 0.05);
            let mut prev = (ox + r, oy);
            for i in 1..=10 {
                let a = -core::f32::consts::PI * i as f32 / 10.0;
                let p = (ox + r * a.cos(), oy + r * a.sin());
                s.line(prev.0, prev.1, p.0, p.1, w, c, 1.0);
                prev = p;
            }
            let (tx, ty) = (ox - r, oy);
            s.tri((tx - z * 0.2, ty - z * 0.04), (tx + z * 0.2, ty - z * 0.04), (tx, ty + z * 0.24), c, 1.0);
        }
        T_ERASE => {
            // a tilted eraser over a line
            let (dx, dy) = (z * 0.24, -z * 0.24);
            let (nx, ny) = (z * 0.14, z * 0.14);
            let a = (cx - dx * 0.9, cy - dy * 0.9 - z * 0.06);
            let b = (cx + dx, cy + dy - z * 0.06);
            s.poly(&[(a.0 - nx, a.1 - ny), (b.0 - nx, b.1 - ny), (b.0 + nx, b.1 + ny), (a.0 + nx, a.1 + ny)], c, 1.0);
            s.line(cx - z * 0.4, cy + z * 0.38, cx + z * 0.4, cy + z * 0.38, w, c, 0.6);
        }
        T_NOTES => {
            // a pencil
            let (dx, dy) = (z * 0.3, -z * 0.3);
            let (nx, ny) = (z * 0.1, z * 0.1);
            let a = (cx - dx * 0.55, cy - dy * 0.55);
            let b = (cx + dx, cy + dy);
            s.poly(&[(a.0 - nx, a.1 - ny), (b.0 - nx, b.1 - ny), (b.0 + nx, b.1 + ny), (a.0 + nx, a.1 + ny)], c, 1.0);
            let tip = (cx - dx * 1.05, cy - dy * 1.05);
            s.tri((a.0 - nx, a.1 - ny), (a.0 + nx, a.1 + ny), tip, c, 1.0);
        }
        _ => {
            // a light bulb
            s.disc(cx, cy - z * 0.1, z * 0.28, c, 1.0);
            s.rect(cx - z * 0.14, cy + z * 0.08, z * 0.28, z * 0.2, z * 0.04, c, 1.0);
            s.rect(cx - z * 0.11, cy + z * 0.32, z * 0.22, z * 0.07, z * 0.03, c, 1.0);
        }
    }
}

impl Game for Sudoku {
    type Save = Save;
    const PLAN: (f32, f32) = (480.0, 360.0);
    const CHOICES: &'static [i32] = &[4, THEMES as i32];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        // SAFETY: Save is Plain, so all zeroes is a valid value
        let mut s: Save = unsafe { core::mem::zeroed() };
        s.version = VERSION;
        s.opt = [1, 0];
        s.sel = -1;
        Sudoku { s, msg: Msg::None, msg_ms: 0, pop: [0; 81], flash: [0; 81], hover: -1, press: -1, ev: 0, lay: layout(480.0, 360.0) }
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        let opt = self.s.opt;
        *self = Sudoku::new();
        self.s.opt = opt;
        self.s.rng = Rng::new(seed);
        self.s.level = opt[O_LEVEL].clamp(0, 3);
        let mut r = logic::Rng(self.s.rng.next() | 1);
        let (p, sol) = logic::generate(self.s.level as usize, &mut r);
        self.s.given = p;
        self.s.val = p;
        self.s.solution = sol;
        self.s.sel = 40;
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt;
        for c in 0..81 {
            self.pop[c] = (self.pop[c] - dt).max(0);
            self.flash[c] = (self.flash[c] - dt).max(0);
        }
        self.msg_ms = (self.msg_ms - dt).max(0);
        let mut ev = core::mem::take(&mut self.ev);
        if self.s.won != 0 {
            if self.s.end_ms > 0 {
                self.s.end_ms -= dt;
                if self.s.end_ms <= 0 {
                    ev |= wg::EV_GAMEOVER;
                }
            }
            return ev;
        }
        self.s.time_ms += dt;
        let dirs = self.s.rep.dirs(input);
        if dirs != 0 {
            let mut c = if self.s.sel < 0 { 40 } else { self.s.sel };
            let (mut r, mut k) = (c / 9, c % 9);
            if dirs & wg::LEFT != 0 { k = (k + 8) % 9; }
            if dirs & wg::RIGHT != 0 { k = (k + 1) % 9; }
            if dirs & wg::UP != 0 { r = (r + 8) % 9; }
            if dirs & wg::DOWN != 0 { r = (r + 1) % 9; }
            c = r * 9 + k;
            self.s.sel = c;
            self.hover = -1;
        }
        // a pad without digits: A runs the cell through 1-9, B clears it
        if input.pressed & wg::A != 0 {
            if let Some(c) = self.editable() {
                let next = (self.s.val[c] % 9) + 1;
                self.set(c, next, false);
            }
        }
        if input.pressed & wg::B != 0 {
            self.erase();
        }
        ev | core::mem::take(&mut self.ev)
    }

    fn key(&mut self, code: i32) -> bool {
        if self.s.won != 0 || self.s.solution[0] == 0 {
            return false;
        }
        match code {
            49..=57 => self.digit((code - 48) as u8, self.s.notes_mode != 0),
            // Shift with a digit key writes the other kind
            0x131..=0x139 => self.digit((code - 0x130) as u8, self.s.notes_mode == 0),
            48 | 8 | 127 => self.erase(),
            0x6E => self.tool(T_NOTES), // n
            0x68 => self.tool(T_HINT),  // h
            0x75 => self.tool(T_UNDO),  // u
            _ => return false,
        }
        true
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        self.hover = if p.inside && !p.touch { self.button_at(p.x, p.y) } else { -1 };
        if !p.inside || self.s.won != 0 {
            self.press = -1;
            return 0;
        }
        let cell = self.cell_at(p.x, p.y);
        if p.pressed & wg::M_LEFT != 0 {
            self.press = self.button_at(p.x, p.y);
            if cell >= 0 {
                self.s.sel = cell;
            }
        } else if p.held & wg::M_LEFT != 0 && cell >= 0 && self.press < 0 {
            self.s.sel = cell; // dragging moves the selection
        }
        if p.pressed & wg::M_RIGHT != 0 && cell >= 0 {
            self.s.sel = cell;
        }
        if p.released & wg::M_LEFT != 0 {
            let b = self.button_at(p.x, p.y);
            if b > 0 && b == self.press {
                if b <= 9 {
                    self.digit(b as u8, self.s.notes_mode != 0);
                } else {
                    self.tool(b as u8);
                }
            }
            self.press = -1;
        }
        core::mem::take(&mut self.ev)
    }

    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        let a = w.max(1) as f32 / h.max(1) as f32;
        // whichever shows the board bigger: buttons beside it, or below it
        let wide = (410.0f32.max(360.0 * a), 360.0);
        let tall = (360.0, 520.0f32.max(360.0 / a));
        let board = |(pw, ph): (f32, f32)| {
            let l = layout(pw, ph);
            l.board.w * (w as f32 / pw).min(h as f32 / ph)
        };
        if board(tall) > board(wide) * 1.04 {
            tall
        } else {
            wide
        }
    }

    fn picture(&mut self) -> u32 {
        let extra = [self.hover, self.press, self.s.time_ms / 1000, self.msg_ms.min(1), match self.msg { Msg::None => 0, Msg::Only(d) => 10 + d as i32, Msg::Last(d, u) => 100 + 10 * u as i32 + d as i32, Msg::Wrong(d) => 200 + d as i32 }];
        let mut h = wg::hash(0, wg::bytes_of(&self.s.val));
        h = wg::hash(h, wg::bytes_of(&self.s.notes));
        let fields = [self.s.sel, self.s.notes_mode, self.s.opt[O_THEME], self.s.won];
        h = wg::hash(h, wg::bytes_of(&fields));
        wg::hash(h, wg::bytes_of(&extra))
    }

    fn moving(&self) -> bool {
        self.pop.iter().any(|&v| v > 0) || self.flash.iter().any(|&v| v > 0)
    }

    fn draw(&mut self, s: &mut Screen) {
        self.lay = layout(s.pw, s.ph);
        let t = self.t();
        let lay = self.lay;
        let b = lay.board;
        let theme = self.s.opt[O_THEME] as u32;
        let key = wg::hash(theme + 1, wg::bytes_of(&[b.x, b.y, b.w, s.pw, s.ph]));
        s.background(key, |s| {
            let (x0, y0, x1, y1) = s.visible();
            s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, t.bg, 1.0);
            s.shadow(b.x - 2.0, b.y + 1.0, b.w + 4.0, b.h + 6.0, 8.0, 9.0, 0, 0.28);
            s.rect(b.x - 3.0, b.y - 3.0, b.w + 6.0, b.h + 6.0, 8.0, t.panel, 1.0);
            s.rect(b.x, b.y, b.w, b.h, 4.0, t.cell, 1.0);
            for r in lay.btn[1..].iter() {
                s.shadow(r.x, r.y + 1.5, r.w, r.h, 7.0, 3.0, 0, 0.18);
            }
        });
        self.draw_board(s);
        self.draw_head(s);
        self.draw_buttons(s);
    }

    fn score(&self) -> u32 {
        self.score_now()
    }
    fn over(&self) -> bool {
        self.s.won != 0
    }
    fn stat(&self, i: usize) -> i32 {
        match i {
            0 => self.s.time_ms,
            1 => self.s.given.iter().filter(|&&v| v != 0).count() as i32,
            2 => self.s.hints,
            3 => self.s.placed,
            4 => self.s.wrong,
            5 => self.s.notes_made,
            6 => self.s.undos,
            _ => -1,
        }
    }

    fn loaded(&mut self) -> bool {
        let s = &mut self.s;
        if s.version != VERSION || !(0..4).contains(&s.level) || s.time_ms < 0 || s.won & !1 != 0 || !(-1..81).contains(&s.sel) {
            return false;
        }
        if !(0..=UNDO as i32).contains(&s.ulen) || s.notes_mode & !1 != 0 {
            return false;
        }
        // the solution is a full, valid grid, and the givens and digits fit it
        for c in 0..81 {
            let v = s.solution[c];
            if !(1..=9).contains(&v) || PEERS[c].iter().any(|&p| s.solution[p as usize] == v) {
                return false;
            }
            if (s.given[c] != 0 && (s.given[c] != v || s.val[c] != v)) || s.val[c] > 9 || s.notes[c] & !ALL != 0 {
                return false;
            }
        }
        if (s.won != 0) != (s.val == s.solution) {
            return false;
        }
        self.pop = [0; 81];
        self.flash = [0; 81];
        self.msg = Msg::None;
        self.msg_ms = 0;
        self.hover = -1;
        self.press = -1;
        true
    }
}

wg::game!(Sudoku);
