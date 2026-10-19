//! A Z-machine (version 3) on the wg engine, for Infocom's story files:
//! a text terminal with the status line on top, the story's transcript
//! scrolling under it and the line being typed at the end. MIT licensed.
//!
//! A game crate names its story file once:
//!
//! ```ignore
//! zmachine::story!(Zork1, include_bytes!("../zork1.z3"));
//! ```
//!
//! Besides the wg exports, a story module exports `wg_wheel` (the mouse
//! wheel, in hundredths of a line) and `wg_dpr` (device pixels per CSS
//! pixel, in hundredths), which the play page sends when a module has them.
pub mod vm;

pub use wg;

use core::marker::PhantomData;
use vm::{Machine, Vm, BUDGET, FAIL, QUIT, READ, RUN};
use wg::{hex, Game, Input, Pointer, Screen};

/// A story file, built into the module.
pub trait Story: 'static {
    const STORY: &'static [u8];
}

/// Exports a story's game: `story!(Zork1, include_bytes!("../zork1.z3"));`
#[macro_export]
macro_rules! story {
    ($name:ident, $bytes:expr) => {
        pub struct $name;
        impl $crate::Story for $name {
            const STORY: &'static [u8] = $bytes;
        }
        $crate::wg::game!($crate::Term<$name>);
        /// The mouse wheel, in hundredths of a line (down is positive).
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_wheel(d: i32) {
            WG_GAME.get().game.wheel(d)
        }
        /// Device pixels per CSS pixel, in hundredths, so text keeps its size.
        #[unsafe(no_mangle)]
        pub extern "C" fn wg_dpr(d: i32) {
            WG_GAME.get().game.set_dpr(d)
        }
    };
}

const VERSION: u32 = 1;
/// The transcript kept (and saved): the oldest text goes first.
pub const TEXT: usize = 24 * 1024;
/// The longest line the player can type.
pub const LINE: usize = 100;
const HISTORY: usize = 40;
const BLINK_MS: i32 = 530;

/// Keys beyond characters, as the play page sends them to wg_key (the
/// function key code points macOS uses).
pub const K_ENTER: i32 = 13;
pub const K_BACKSPACE: i32 = 8;
pub const K_UP: i32 = 0xF700;
pub const K_DOWN: i32 = 0xF701;
pub const K_HOME: i32 = 0xF729;
pub const K_END: i32 = 0xF72B;
pub const K_PAGE_UP: i32 = 0xF72C;
pub const K_PAGE_DOWN: i32 = 0xF72D;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Save {
    version: u32,
    /// the story's release and checksum
    story: u32,
    text_len: u32,
    /// counts cuts at the front of the transcript, so wrapping starts over
    text_gen: u32,
    line_len: u32,
    /// commands typed this run
    commands: u32,
    m: Machine,
    line: [u8; LINE],
    text: [u8; TEXT],
}
unsafe impl wg::Plain for Save {}

/// One row on screen: bytes start..end of the transcript, or of the tail
/// (the last paragraph with the line being typed), and whether it is part of
/// a command (a paragraph starting with the prompt).
#[derive(Clone, Copy)]
struct Row {
    start: u32,
    end: u32,
    cmd: bool,
}

pub struct Term<S: Story> {
    s: Box<Save>,
    _story: PhantomData<S>,
    history: Vec<Vec<u8>>,
    /// where Up and Down are in the history (its length: the line typed now)
    hist_at: usize,
    /// the line being typed before Up went into the history
    draft: Vec<u8>,
    /// rows scrolled back from the newest, and the most there are
    scroll: i32,
    max_scroll: i32,
    page_rows: i32,
    wheel: i32,
    drag: Option<(f32, f32)>,
    blink: i32,
    dpr: f32,
    /// the plan's width when last drawn
    pw: f32,
    rev: u32,
    ended: bool,
    /// the score of a run the story restarted, reported as game over on the
    /// next frame, and that score while it is being reported
    restarted: Option<u32>,
    over_score: Option<u32>,
    /// width of each ASCII character at size 1, filled when first drawn
    adv: [f32; 128],
    /// rows of the transcript's whole paragraphs, for `wrap_key`
    rows: Vec<Row>,
    wrap_key: (u32, u32, u32, u32),
}

/// Colours: a dark screen with light text, the status line inverted, the
/// player's commands warm.
const BG: u32 = hex(0x12161c);
const INK: u32 = hex(0xd9dee6);
const CMD: u32 = hex(0xf0c674);
const BAR: u32 = hex(0xd9dee6);
const BAR_INK: u32 = hex(0x12161c);
const DIM: u32 = hex(0x59626f);

impl<S: Story> Term<S> {
    fn vm<'a>(m: &'a mut Machine, out: &'a mut Vec<u8>) -> Vm<'a> {
        Vm::new(S::STORY, m, out)
    }

    /// Runs the story until it waits for input (or a budget runs out) and
    /// adds what it printed to the transcript.
    fn run(&mut self) {
        let mut out = Vec::new();
        let mut vm = Self::vm(&mut self.s.m, &mut out);
        vm.run(BUDGET);
        if let Some(s) = vm.restarted {
            // the story started over (RESTART, as after dying): the run ends here
            vm::keep_best(&mut self.restarted, s);
        }
        self.print(&out);
    }

    fn print(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let s = &mut *self.s;
        let mut len = s.text_len as usize;
        if len + bytes.len() > TEXT {
            // drop the oldest quarter (or more), up to the end of a paragraph
            let want = (len + bytes.len() - TEXT).max(TEXT / 4).min(len);
            let cut = s.text[want..len].iter().position(|&c| c == b'\n').map_or(len, |p| want + p + 1);
            s.text.copy_within(cut..len, 0);
            len -= cut;
            s.text_gen = s.text_gen.wrapping_add(1);
        }
        let n = bytes.len().min(TEXT - len);
        s.text[len..len + n].copy_from_slice(&bytes[bytes.len() - n..]);
        s.text_len = (len + n) as u32;
        self.scroll = 0;
        self.rev = self.rev.wrapping_add(1);
    }

    fn reading(&self) -> bool {
        self.s.m.mode == READ
    }

    /// The most characters the story's text buffer takes.
    fn line_max(&self) -> usize {
        if !self.reading() {
            return 0;
        }
        let t = self.s.m.text_buf as usize;
        let max = if t < vm::DYN { self.s.m.core.mem[t] as usize } else { 0 };
        max.saturating_sub(1).min(LINE)
    }

    fn line(&self) -> &[u8] {
        &self.s.line[..self.s.line_len as usize]
    }

    fn set_line(&mut self, text: &[u8]) {
        let n = text.len().min(LINE);
        self.s.line[..n].copy_from_slice(&text[..n]);
        self.s.line_len = n as u32;
        self.touch();
    }

    /// Something on screen changed: back to the newest text, caret shown.
    fn touch(&mut self) {
        self.scroll = 0;
        self.blink = 0;
        self.rev = self.rev.wrapping_add(1);
    }

    fn type_char(&mut self, c: u8) {
        if !self.reading() || (self.s.line_len as usize) >= self.line_max() {
            return;
        }
        let n = self.s.line_len as usize;
        self.s.line[n] = c;
        self.s.line_len += 1;
        self.touch();
    }

    fn submit(&mut self) {
        if !self.reading() {
            return;
        }
        let line = self.line().to_vec();
        let mut echo = line.clone();
        echo.push(b'\n');
        self.print(&echo);
        if !line.iter().all(|&c| c == b' ') && self.history.last() != Some(&line) {
            self.history.push(line.clone());
            if self.history.len() > HISTORY {
                self.history.remove(0);
            }
        }
        self.hist_at = self.history.len();
        self.draft.clear();
        self.s.line_len = 0;
        self.s.commands += 1;
        let mut out = Vec::new();
        Self::vm(&mut self.s.m, &mut out).input(&line);
        self.print(&out);
        self.run();
        self.touch();
    }

    fn recall(&mut self, up: bool) {
        if !self.reading() {
            return;
        }
        if up && self.hist_at > 0 {
            if self.hist_at == self.history.len() {
                self.draft = self.line().to_vec();
            }
            self.hist_at -= 1;
            let l = self.history[self.hist_at].clone();
            self.set_line(&l);
        } else if !up && self.hist_at < self.history.len() {
            self.hist_at += 1;
            let l = if self.hist_at == self.history.len() {
                self.draft.clone()
            } else {
                self.history[self.hist_at].clone()
            };
            self.set_line(&l);
        }
    }

    fn scroll_by(&mut self, rows: i32) {
        let to = (self.scroll + rows).clamp(0, self.max_scroll.max(0));
        if to != self.scroll {
            self.scroll = to;
            self.rev = self.rev.wrapping_add(1);
        }
    }

    /// The mouse wheel, in hundredths of a row (down, towards the newest
    /// text, is positive).
    pub fn wheel(&mut self, d: i32) {
        self.wheel += d;
        let rows = self.wheel / 100;
        self.wheel -= rows * 100;
        self.scroll_by(-rows);
    }

    pub fn set_dpr(&mut self, hundredths: i32) {
        self.dpr = (hundredths as f32 / 100.0).clamp(0.5, 8.0);
    }

    /// The transcript so far, with the line being typed after it.
    pub fn transcript(&self) -> String {
        let mut t = String::from_utf8_lossy(&self.s.text[..self.s.text_len as usize]).into_owned();
        t.push_str(&String::from_utf8_lossy(self.line()));
        t
    }

    /// Room, score and moves as the status line shows them.
    pub fn status(&mut self) -> (String, i16, i16) {
        let mut out = Vec::new();
        let st = Self::vm(&mut self.s.m, &mut out).status();
        (st.0, st.1, st.2)
    }

    /// The machine's mode (vm::RUN, READ, QUIT or FAIL).
    pub fn mode(&self) -> u32 {
        self.s.m.mode
    }

    // ---------- layout ----------

    /// Text size, row height and margin for a plan this wide.
    fn metrics(pw: f32) -> (f32, f32, f32) {
        let f = (pw / 44.0).clamp(13.0, 17.0);
        (f, (f * 1.45).round(), (f * 0.9).round())
    }

    fn wrap(&self, bytes: &[u8], tail: bool, width: f32, f: f32, out: &mut Vec<Row>) {
        let w = |c: u8| self.adv[(c & 127) as usize] * f;
        let mut ps = 0usize;
        while ps <= bytes.len() {
            let pe = bytes[ps..].iter().position(|&c| c == b'\n').map_or(bytes.len(), |p| ps + p);
            if pe == bytes.len() && ps == pe && !tail {
                break; // nothing after the last newline
            }
            let cmd = bytes.get(ps) == Some(&b'>');
            let row = |a: usize, b: usize| Row { start: a as u32, end: b as u32, cmd };
            let mut ls = ps;
            let mut x = 0.0;
            let mut i = ps;
            while i < pe {
                let c = bytes[i];
                let cw = w(c);
                if x + cw > width && i > ls && c != b' ' {
                    // back to the last space on this row, or break the word
                    let sp = bytes[ls..i].iter().rposition(|&c| c == b' ').map(|p| ls + p).filter(|&p| p > ls);
                    let brk = sp.unwrap_or(i);
                    out.push(row(ls, brk));
                    ls = if sp.is_some() { brk + 1 } else { brk };
                    x = bytes[ls..i].iter().map(|&c| w(c)).sum();
                    continue;
                }
                x += cw;
                i += 1;
            }
            out.push(row(ls, pe));
            ps = pe + 1;
        }
    }
}

impl<S: Story> Game for Term<S> {
    type Save = Save;
    const PLAN: (f32, f32) = (640.0, 480.0);

    fn new() -> Self {
        // SAFETY: Save is Plain, so all zeroes is a valid value
        let s: Box<Save> = unsafe { Box::new_zeroed().assume_init() };
        let mut t = Term {
            s,
            _story: PhantomData,
            history: Vec::new(),
            hist_at: 0,
            draft: Vec::new(),
            scroll: 0,
            max_scroll: 0,
            page_rows: 10,
            wheel: 0,
            drag: None,
            blink: 0,
            dpr: 1.0,
            pw: 640.0,
            rev: 0,
            ended: false,
            restarted: None,
            over_score: None,
            adv: [0.0; 128],
            rows: Vec::new(),
            wrap_key: (u32::MAX, 0, 0, 0),
        };
        t.s.version = VERSION;
        t.s.story = vm::story_id(S::STORY);
        t
    }

    fn save(&mut self) -> &mut Save {
        &mut self.s
    }

    fn start(&mut self, seed: u32) {
        let keep_dpr = self.dpr;
        let history = core::mem::take(&mut self.history);
        *self = Term::new();
        self.dpr = keep_dpr;
        self.hist_at = history.len();
        self.history = history;
        if !vm::playable(S::STORY) {
            self.print(b"This story file needs a version 3 Z-machine.\n");
            self.s.m.mode = FAIL;
            return;
        }
        let mut out = Vec::new();
        Self::vm(&mut self.s.m, &mut out).boot(seed.max(1));
        self.run();
    }

    fn update(&mut self, input: &Input) -> u32 {
        let mut ev = 0;
        self.over_score = None;
        if self.s.m.mode == RUN {
            self.run();
        }
        // one game over a frame: a restarted run first, then a story that quit
        if let Some(s) = self.restarted.take() {
            self.over_score = Some(s);
            ev |= wg::EV_GAMEOVER;
        } else if self.over() && !self.ended {
            self.ended = true;
            ev |= wg::EV_GAMEOVER;
        }
        // the caret blinking changes picture()
        self.blink += input.dt;
        // a controller's d-pad goes through the commands typed before
        if input.pressed & wg::UP != 0 {
            self.recall(true);
        }
        if input.pressed & wg::DOWN != 0 {
            self.recall(false);
        }
        ev
    }

    fn key(&mut self, code: i32) -> bool {
        match code {
            K_ENTER => {
                if self.over() {
                    // after the story ends, Enter starts it again
                    let seed = self.s.m.core.rng.0 ^ 0x9e37_79b9;
                    self.start(seed);
                } else {
                    self.submit();
                }
            }
            K_BACKSPACE => {
                if self.s.line_len > 0 {
                    self.s.line_len -= 1;
                    self.touch();
                }
            }
            127 => {}
            K_UP => self.recall(true),
            K_DOWN => self.recall(false),
            K_PAGE_UP => self.scroll_by((self.page_rows - 1).max(1)),
            K_PAGE_DOWN => self.scroll_by(-(self.page_rows - 1).max(1)),
            K_HOME => self.scroll_by(i32::MAX / 2),
            K_END => self.scroll_by(-(i32::MAX / 2)),
            // Shift with a digit key: what it types on a US keyboard
            0x130..=0x139 => self.type_char(b")!@#$%^&*("[(code - 0x130) as usize]),
            32..=126 => self.type_char(code as u8),
            _ => return false,
        }
        true
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        // dragging scrolls the transcript
        if p.pressed & wg::M_LEFT != 0 {
            self.drag = Some((p.y, 0.0));
        } else if p.held & wg::M_LEFT != 0 && p.inside {
            if let Some((y, acc)) = self.drag {
                let (_, lh, _) = Self::metrics(self.pw);
                let acc = acc + (p.y - y) / lh;
                let rows = acc.trunc() as i32;
                self.drag = Some((p.y, acc - rows as f32));
                self.scroll_by(rows);
            }
        }
        if p.released & wg::M_LEFT != 0 || !p.inside {
            self.drag = None;
        }
        0
    }

    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        // one plan unit is a CSS pixel, so text keeps its size on any screen
        ((w.max(1) as f32 / self.dpr).round(), (h.max(1) as f32 / self.dpr).round())
    }

    fn picture(&mut self) -> u32 {
        let caret = self.reading() && self.scroll == 0 && (self.blink / BLINK_MS) % 2 == 0;
        let fields = [self.rev, self.scroll as u32, caret as u32, self.s.line_len, self.s.m.mode];
        wg::hash(0, wg::bytes_of(&fields))
    }

    fn draw(&mut self, s: &mut Screen) {
        if self.adv[b'a' as usize] == 0.0 {
            for c in 32..127u8 {
                self.adv[c as usize] = s.text_width(core::str::from_utf8(&[c]).unwrap_or(" "), 1.0);
            }
        }
        let (x0, y0, x1, y1) = s.visible();
        let (pw, ph) = (x1 - x0, y1 - y0);
        let (f, lh, m) = Self::metrics(pw);
        self.pw = pw;

        // the status line: room on the left, score and moves on the right
        let bar = (f * 2.0).round();
        let (room, score, moves, time) = {
            let mut out = Vec::new();
            Self::vm(&mut self.s.m, &mut out).status()
        };
        let right = if time {
            let (h, mi) = (score.rem_euclid(24), moves.rem_euclid(60));
            format!("{}:{:02} {}", if h % 12 == 0 { 12 } else { h % 12 }, mi, if h < 12 { "am" } else { "pm" })
        } else if pw < 300.0 {
            format!("{}/{}", score, moves)
        } else {
            format!("Score: {}   Moves: {}", score, moves)
        };

        // the transcript, wrapped to the column
        let colw = (pw - 2.0 * m).min(46.0 * f);
        let tx = x0 + (pw - colw) / 2.0;
        let len = self.s.text_len as usize;
        let whole = self.s.text[..len].iter().rposition(|&c| c == b'\n').map_or(0, |p| p + 1);
        let key = (self.s.text_gen, whole as u32, colw.to_bits(), f.to_bits());
        if key != self.wrap_key {
            let mut rows = Vec::new();
            self.wrap(&self.s.text[..whole], false, colw, f, &mut rows);
            self.rows = rows;
            self.wrap_key = key;
        }
        let mut tail: Vec<u8> = self.s.text[whole..len].to_vec();
        if self.reading() {
            tail.extend_from_slice(self.line());
        }
        let mut tail_rows = Vec::new();
        if !tail.is_empty() || self.reading() {
            self.wrap(&tail, true, colw, f, &mut tail_rows);
        }
        let total = self.rows.len() + tail_rows.len();
        let top = y0 + bar + (f * 0.6).round();
        let bottom = y1 - (f * 0.7).round();
        let fit = (((bottom - top) / lh).floor() as i32).max(1);
        self.page_rows = fit;
        self.max_scroll = (total as i32 - fit).max(0);
        self.scroll = self.scroll.clamp(0, self.max_scroll);
        let last = total as i32 - self.scroll; // rows before this one show
        let first = (last - fit).max(0);
        // with less than a screen of text, it starts at the top
        let y_first = if total as i32 <= fit { top + lh / 2.0 } else { bottom - (last - first) as f32 * lh + lh / 2.0 };
        // rows first..last: the transcript's up to `whole_rows`, the tail's after
        let whole_rows = self.rows.len() as i32;

        // Everything but the last paragraph (where the typing goes) is drawn
        // once into the background, so a key typed redraws only that.
        let shown = [
            self.wrap_key.0,
            self.wrap_key.1,
            first as u32,
            last as u32,
            tail_rows.len() as u32,
            score as u32,
            moves as u32,
        ];
        let bg_key = wg::hash(wg::hash(wg::hash(0, wg::bytes_of(&shown)), room.as_bytes()), wg::bytes_of(&[pw, ph, f]));
        let text = &self.s.text;
        let rows = &self.rows;
        s.background(bg_key, |s| {
            s.rect(x0, y0, pw, ph, 0.0, BG, 1.0);
            s.rect(x0, y0, pw, bar, 0.0, BAR, 1.0);
            let fs = f * 0.95;
            s.text_r(&right, x1 - m, y0 + bar / 2.0, fs, BAR_INK, 1.0);
            let room_w = pw - 3.0 * m - s.text_width(&right, fs);
            let rs = s.fit_size(&room, fs, room_w.max(10.0));
            s.text(&room, x0 + m, y0 + bar / 2.0, rs, BAR_INK, 1.0);
            for i in first..last.min(whole_rows) {
                let r = rows[i as usize];
                let line = core::str::from_utf8(&text[r.start as usize..r.end as usize]).unwrap_or("");
                s.text(line, tx, y_first + (i - first) as f32 * lh, f, if r.cmd { CMD } else { INK }, 1.0);
            }
        });
        for i in first.max(whole_rows)..last {
            let r = tail_rows[(i - whole_rows) as usize];
            let y = y_first + (i - first) as f32 * lh;
            let line = core::str::from_utf8(&tail[r.start as usize..r.end as usize]).unwrap_or("");
            let end = s.text(line, tx, y, f, if r.cmd { CMD } else { INK }, 1.0);
            if i as usize == total - 1 && self.reading() && self.scroll == 0 && (self.blink / BLINK_MS) % 2 == 0 {
                // the caret after the text being typed
                s.rect(end + 1.0, y - f * 0.62, (f * 0.09).max(1.5), f * 1.2, 0.0, CMD, 1.0);
            }
        }
        // where the view is in the transcript, while scrolled back
        if self.scroll > 0 && total as i32 > fit {
            let track = bottom - top;
            let h = (track * fit as f32 / total as f32).max(f * 1.5);
            let pos = first as f32 / (total as i32 - fit) as f32;
            s.rect(x1 - 5.0, top + (track - h) * pos, 3.0, h, 1.5, DIM, 1.0);
        }
    }

    fn score(&self) -> u32 {
        self.over_score.unwrap_or_else(|| vm::score(S::STORY, &self.s.m.core.mem))
    }

    fn over(&self) -> bool {
        self.s.m.mode == QUIT || self.s.m.mode == FAIL
    }

    fn stat(&self, i: usize) -> i32 {
        match i {
            0 => self.s.commands as i32,
            _ => -1,
        }
    }

    fn loaded(&mut self) -> bool {
        let s = &*self.s;
        let c = &s.m.core;
        let ok = s.version == VERSION
            && s.story == vm::story_id(S::STORY)
            && s.m.mode <= FAIL
            && (s.text_len as usize) <= TEXT
            && (s.line_len as usize) <= LINE
            && (c.pc as usize) < S::STORY.len()
            && (c.sp as usize) <= vm::STACK
            && (c.fp as usize) <= c.sp as usize
            && c.s3_depth <= 16
            && s.m.has_saved <= 1
            && (s.m.saved.sp as usize) <= vm::STACK
            && (s.m.saved.fp as usize) <= s.m.saved.sp as usize
            && (s.m.saved.pc as usize) < S::STORY.len();
        if !ok {
            return false;
        }
        self.ended = self.over();
        self.restarted = None;
        self.over_score = None;
        self.wrap_key = (u32::MAX, 0, 0, 0);
        self.hist_at = self.history.len();
        self.drag = None;
        self.touch();
        true
    }
}
