//! The Z-machine, version 3, as the Z-Machine Standards Document 1.1
//! describes it: Infocom's virtual machine for its story files.
//!
//! Everything that changes as a story plays (dynamic memory, the stack, the
//! program counter, the random numbers, the game saved with SAVE) is in
//! [`Machine`], a plain struct the wg save copies byte for byte. The story
//! file itself never changes and stays outside it.

use wg::Rng;

/// Dynamic memory kept in a save: the largest of Zork I, II and III is
/// 0x3104 bytes.
pub const DYN: usize = 0x3400;
/// The stack, in words: call frames and the evaluation stack together.
pub const STACK: usize = 2048;
/// Instructions one call to [`Vm::run`] may run before it yields.
pub const BUDGET: u32 = 2_000_000;

/// What the machine is doing ([`Machine::mode`]).
pub const RUN: u32 = 0;
/// Waiting for a line of input (sread).
pub const READ: u32 = 1;
/// The story quit.
pub const QUIT: u32 = 2;
/// The story did something the machine can't (a bad opcode, a stack overflow).
pub const FAIL: u32 = 3;

/// Words above each call frame's locals: return address (2), the caller's
/// frame, the store variable and the number of locals.
const FRAME: usize = 5;
/// Store variable of a call whose result is thrown away.
const NO_STORE: u16 = 0x100;

/// The running state, everything a save keeps.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Core {
    pub pc: u32,
    /// stack[..sp] is in use
    pub sp: u32,
    /// where the current routine's locals start (0 outside any routine)
    pub fp: u32,
    /// random numbers
    pub rng: Rng,
    /// predictable random numbers (random with a small negative range): the
    /// next one and the range, 0 when off
    pub seq: u32,
    pub seq_max: u32,
    /// the window being printed to (text for window 1 is dropped: the stories
    /// this plays never split the screen)
    pub window: u32,
    /// output stream 1 (the screen) off
    pub screen_off: u32,
    /// output stream 3: how many tables are open, and each table's address
    pub s3_depth: u32,
    pub s3: [u32; 16],
    pub mem: [u8; DYN],
    pub stack: [u16; STACK],
}

/// The machine: its state, the line it waits for, and the game SAVE made.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Machine {
    pub mode: u32,
    /// sread's text and parse buffers, while mode is READ
    pub text_buf: u32,
    pub parse_buf: u32,
    /// 1 when `saved` holds a game saved with SAVE
    pub has_saved: u32,
    pub core: Core,
    /// the game SAVE made, its pc at the save instruction's branch
    pub saved: Core,
}
unsafe impl wg::Plain for Machine {}

/// The story's header word at `a`.
pub fn header(story: &[u8], a: usize) -> u16 {
    (story[a] as u16) << 8 | story[a + 1] as u16
}

/// Release and checksum, to tell a save made for another story file.
pub fn story_id(story: &[u8]) -> u32 {
    (header(story, 2) as u32) << 16 | header(story, 0x1C) as u32
}

/// Where global variable i (variable 16 + i) is kept in memory.
fn global_at(story: &[u8], i: u32) -> u32 {
    header(story, 0x0C) as u32 + 2 * i
}

/// The score the status line shows (global 1), 0 for a story whose status
/// line shows the time instead, or below zero.
pub fn score(story: &[u8], mem: &[u8; DYN]) -> u32 {
    let base = global_at(story, 1) as usize;
    if base + 1 >= DYN || story[1] & 2 != 0 {
        return 0;
    }
    (((mem[base] as u16) << 8 | mem[base + 1] as u16) as i16).max(0) as u32
}

/// Keeps the higher of a score already kept and another.
pub fn keep_best(kept: &mut Option<u32>, score: u32) {
    *kept = Some(kept.map_or(score, |k| k.max(score)));
}

/// A story this machine can play: version 3, dynamic memory that fits.
pub fn playable(story: &[u8]) -> bool {
    story.len() >= 64 && story[0] == 3 && (header(story, 0x0E) as usize) <= DYN && (header(story, 0x0E) as usize) >= 64
}

const A2: &[u8; 26] = b" \n0123456789.,!?_#'\"/\\-:()";

/// The machine with its story, for one call; output collects in `out`.
pub struct Vm<'a> {
    pub story: &'static [u8],
    pub m: &'a mut Machine,
    pub out: &'a mut Vec<u8>,
    /// The score of the run a RESTART during this call ended.
    pub restarted: Option<u32>,
    dyn_len: usize,
}

impl<'a> Vm<'a> {
    pub fn new(story: &'static [u8], m: &'a mut Machine, out: &'a mut Vec<u8>) -> Self {
        let dyn_len = (header(story, 0x0E) as usize).min(DYN);
        Vm { story, m, out, restarted: None, dyn_len }
    }

    /// Starts the story from the beginning (also RESTART).
    pub fn boot(&mut self, seed: u32) {
        let c = &mut self.m.core;
        // flags 2 (transcript, fixed pitch) live through a restart
        let flags2 = [c.mem[0x10], c.mem[0x11]];
        let restart = c.pc != 0;
        c.mem = [0; DYN];
        c.mem[..self.dyn_len].copy_from_slice(&self.story[..self.dyn_len]);
        // no status line split, no screen splitting, a variable-pitch font
        c.mem[1] = (c.mem[1] & !0x30) | 0x40;
        if restart {
            c.mem[0x10] = flags2[0] & 3;
            c.mem[0x11] = flags2[1];
        }
        c.pc = header(self.story, 6) as u32;
        c.sp = 0;
        c.fp = 0;
        if !restart || c.rng.0 == 0 {
            c.rng = Rng::new(seed);
        }
        c.seq = 0;
        c.seq_max = 0;
        c.window = 0;
        c.screen_off = 0;
        c.s3_depth = 0;
        c.stack = [0; STACK];
        self.m.mode = RUN;
    }

    // ---------- memory ----------

    fn rb(&self, a: u32) -> u8 {
        let a = a as usize;
        if a < self.dyn_len {
            self.m.core.mem[a]
        } else {
            self.story.get(a).copied().unwrap_or(0)
        }
    }
    fn rw(&self, a: u32) -> u16 {
        (self.rb(a) as u16) << 8 | self.rb(a + 1) as u16
    }
    fn wb(&mut self, a: u32, v: u8) {
        if (a as usize) < self.dyn_len {
            self.m.core.mem[a as usize] = v;
        }
    }
    fn ww(&mut self, a: u32, v: u16) {
        self.wb(a, (v >> 8) as u8);
        self.wb(a + 1, v as u8);
    }
    fn fetch(&mut self) -> u8 {
        let b = self.rb(self.m.core.pc);
        self.m.core.pc += 1;
        b
    }
    fn fetch_w(&mut self) -> u16 {
        let w = self.rw(self.m.core.pc);
        self.m.core.pc += 2;
        w
    }

    // ---------- the stack and variables ----------

    fn push(&mut self, v: u16) {
        if self.m.core.sp as usize >= STACK {
            self.fail("stack overflow");
            return;
        }
        let c = &mut self.m.core;
        c.stack[c.sp as usize] = v;
        c.sp += 1;
    }
    fn pop(&mut self) -> u16 {
        // never below the current frame
        let floor = if self.m.core.fp == 0 { 0 } else { self.m.core.fp + self.locals() };
        let c = &mut self.m.core;
        if c.sp <= floor {
            return 0;
        }
        c.sp -= 1;
        c.stack[c.sp as usize]
    }
    fn locals(&self) -> u32 {
        let fp = self.m.core.fp as usize;
        if fp < FRAME {
            0
        } else {
            self.m.core.stack[fp - 1] as u32
        }
    }
    fn global_addr(&self, v: u8) -> u32 {
        global_at(self.story, v as u32 - 16)
    }
    fn read_var(&mut self, v: u8) -> u16 {
        match v {
            0 => self.pop(),
            1..=15 => {
                if v as u32 > self.locals() {
                    return 0;
                }
                self.m.core.stack[self.m.core.fp as usize + v as usize - 1]
            }
            _ => self.rw(self.global_addr(v)),
        }
    }
    fn write_var(&mut self, v: u8, x: u16) {
        match v {
            0 => self.push(x),
            1..=15 => {
                if v as u32 <= self.locals() {
                    let i = self.m.core.fp as usize + v as usize - 1;
                    self.m.core.stack[i] = x;
                }
            }
            _ => self.ww(self.global_addr(v), x),
        }
    }
    /// Variables named by an operand (inc, dec, load, store, pull): the
    /// stack's top is read and written where it is, not pushed or popped.
    fn peek_var(&mut self, v: u8) -> u16 {
        if v == 0 {
            let x = self.pop();
            self.push(x);
            return x;
        }
        self.read_var(v)
    }
    fn poke_var(&mut self, v: u8, x: u16) {
        if v == 0 {
            self.pop();
        }
        self.write_var(v, x);
    }

    fn store(&mut self, x: u16) {
        let v = self.fetch();
        self.write_var(v, x);
    }

    fn branch(&mut self, cond: bool) {
        let b = self.fetch();
        let off: i32 = if b & 0x40 != 0 {
            (b & 0x3F) as i32
        } else {
            let lo = self.fetch();
            let o = ((b as i32 & 0x3F) << 8) | lo as i32;
            if o & 0x2000 != 0 {
                o - 0x4000
            } else {
                o
            }
        };
        if (b & 0x80 != 0) != cond {
            return;
        }
        match off {
            0 => self.ret(0),
            1 => self.ret(1),
            _ => self.m.core.pc = (self.m.core.pc as i32 + off - 2) as u32,
        }
    }

    fn call(&mut self, routine: u16, args: &[u16], store: u16) {
        if routine == 0 {
            if store != NO_STORE {
                self.write_var(store as u8, 0);
            }
            return;
        }
        let addr = 2 * routine as u32;
        let n = (self.rb(addr) as usize).min(15);
        if self.m.core.sp as usize + FRAME + n >= STACK {
            self.fail("stack overflow");
            return;
        }
        let ret = self.m.core.pc;
        self.push((ret >> 16) as u16);
        self.push(ret as u16);
        self.push(self.m.core.fp as u16);
        self.push(store);
        self.push(n as u16);
        self.m.core.fp = self.m.core.sp;
        for i in 0..n {
            let init = self.rw(addr + 1 + 2 * i as u32);
            self.push(args.get(i).copied().unwrap_or(init));
        }
        self.m.core.pc = addr + 1 + 2 * n as u32;
    }

    fn ret(&mut self, x: u16) {
        let fp = self.m.core.fp as usize;
        if fp < FRAME {
            self.fail("return from the main routine");
            return;
        }
        let s = &self.m.core.stack;
        let pc = (s[fp - 5] as u32) << 16 | s[fp - 4] as u32;
        let old_fp = s[fp - 3] as u32;
        let store = s[fp - 2];
        self.m.core.sp = (fp - FRAME) as u32;
        self.m.core.fp = old_fp;
        self.m.core.pc = pc;
        if store != NO_STORE {
            self.write_var(store as u8, x);
        }
    }

    fn fail(&mut self, why: &str) {
        if self.m.mode == FAIL {
            return;
        }
        self.m.mode = FAIL;
        let msg = format!("\n\nThe story stopped: {} (at {:05x}).\n", why, self.m.core.pc);
        self.out.extend_from_slice(msg.as_bytes());
    }

    // ---------- text ----------

    /// Prints a character (ZSCII) to the output streams.
    fn put(&mut self, z: u16) {
        let c = &self.m.core;
        if c.s3_depth > 0 {
            let t = c.s3[(c.s3_depth - 1) as usize];
            let n = self.rw(t);
            self.wb(t + 2 + n as u32, z as u8);
            self.ww(t, n.wrapping_add(1));
            return;
        }
        if c.screen_off != 0 || c.window != 0 {
            return;
        }
        match z {
            13 => self.out.push(b'\n'),
            32..=126 => self.out.push(z as u8),
            9 => self.out.push(b' '),
            _ => {}
        }
    }
    fn put_str(&mut self, s: &str) {
        for b in s.bytes() {
            self.put(if b == b'\n' { 13 } else { b as u16 });
        }
    }

    /// Decodes the string at `a` (abbreviations expanded) and returns the
    /// address after it.
    fn print_at(&mut self, a: u32) -> u32 {
        let mut text = Vec::new();
        let end = self.decode(a, &mut text, true);
        for z in text {
            self.put(z);
        }
        end
    }

    /// Decodes Z-characters from `a` into ZSCII in `text`.
    fn decode(&self, mut a: u32, text: &mut Vec<u16>, abbrevs: bool) -> u32 {
        let mut alpha = 0u8;
        // 1-3 an abbreviation's bank waiting for its index; 4 the high five
        // bits of a ten-bit character next; 5 the low five
        let mut pending = 0u8;
        let mut high = 0u16;
        let mut guard = 0;
        loop {
            let w = self.rw(a);
            a += 2;
            for zc in [(w >> 10) & 31, (w >> 5) & 31, w & 31] {
                let zc = zc as u8;
                match pending {
                    1..=3 => {
                        if abbrevs {
                            let entry = header(self.story, 0x18) as u32 + 2 * (32 * (pending as u32 - 1) + zc as u32);
                            let at = 2 * self.rw(entry) as u32;
                            self.decode(at, text, false);
                        }
                        pending = 0;
                        continue;
                    }
                    4 => {
                        high = zc as u16;
                        pending = 5;
                        continue;
                    }
                    5 => {
                        text.push(high << 5 | zc as u16);
                        pending = 0;
                        continue;
                    }
                    _ => {}
                }
                match zc {
                    0 => text.push(32),
                    1..=3 => pending = zc,
                    4 => {
                        alpha = 1;
                        continue;
                    }
                    5 => {
                        alpha = 2;
                        continue;
                    }
                    _ => match alpha {
                        0 => text.push((b'a' + zc - 6) as u16),
                        1 => text.push((b'A' + zc - 6) as u16),
                        _ => match zc {
                            6 => pending = 4,
                            7 => text.push(13),
                            _ => text.push(A2[zc as usize - 6] as u16),
                        },
                    },
                }
                alpha = 0;
            }
            guard += 1;
            if w & 0x8000 != 0 || guard > 4096 {
                return a;
            }
        }
    }

    /// The 4 bytes of a word as the dictionary keeps it (6 Z-characters).
    fn encode(word: &[u8]) -> [u8; 4] {
        let mut z: Vec<u8> = Vec::with_capacity(12);
        for &c in word {
            match c {
                b'a'..=b'z' => z.push(c - b'a' + 6),
                _ => match A2.iter().position(|&x| x == c).filter(|&i| i >= 2) {
                    Some(i) => z.extend_from_slice(&[5, i as u8 + 6]),
                    None => z.extend_from_slice(&[5, 6, c >> 5, c & 31]),
                },
            }
            if z.len() >= 6 {
                break;
            }
        }
        z.resize(6, 5);
        let w1 = (z[0] as u16) << 10 | (z[1] as u16) << 5 | z[2] as u16;
        let w2 = 0x8000 | (z[3] as u16) << 10 | (z[4] as u16) << 5 | z[5] as u16;
        [(w1 >> 8) as u8, w1 as u8, (w2 >> 8) as u8, w2 as u8]
    }

    // ---------- objects ----------

    fn obj_addr(&self, o: u16) -> u32 {
        header(self.story, 0x0A) as u32 + 31 * 2 + 9 * (o as u32 - 1)
    }
    fn parent(&self, o: u16) -> u16 {
        if o == 0 {
            0
        } else {
            self.rb(self.obj_addr(o) + 4) as u16
        }
    }
    fn sibling(&self, o: u16) -> u16 {
        if o == 0 {
            0
        } else {
            self.rb(self.obj_addr(o) + 5) as u16
        }
    }
    fn child(&self, o: u16) -> u16 {
        if o == 0 {
            0
        } else {
            self.rb(self.obj_addr(o) + 6) as u16
        }
    }
    fn set_parent(&mut self, o: u16, v: u16) {
        self.wb(self.obj_addr(o) + 4, v as u8)
    }
    fn set_sibling(&mut self, o: u16, v: u16) {
        self.wb(self.obj_addr(o) + 5, v as u8)
    }
    fn set_child(&mut self, o: u16, v: u16) {
        self.wb(self.obj_addr(o) + 6, v as u8)
    }
    fn props(&self, o: u16) -> u32 {
        self.rw(self.obj_addr(o) + 7) as u32
    }
    fn attr(&self, o: u16, n: u16) -> bool {
        o != 0 && n < 32 && self.rb(self.obj_addr(o) + n as u32 / 8) & (0x80 >> (n % 8)) != 0
    }
    fn set_attr(&mut self, o: u16, n: u16, on: bool) {
        if o == 0 || n >= 32 {
            return;
        }
        let a = self.obj_addr(o) + n as u32 / 8;
        let b = self.rb(a);
        let bit = 0x80 >> (n % 8);
        self.wb(a, if on { b | bit } else { b & !bit });
    }
    fn remove(&mut self, o: u16) {
        let p = self.parent(o);
        if o == 0 || p == 0 {
            return;
        }
        let next = self.sibling(o);
        if self.child(p) == o {
            self.set_child(p, next);
        } else {
            let mut k = self.child(p);
            let mut guard = 0;
            while k != 0 && guard < 256 {
                if self.sibling(k) == o {
                    self.set_sibling(k, next);
                    break;
                }
                k = self.sibling(k);
                guard += 1;
            }
        }
        self.set_parent(o, 0);
        self.set_sibling(o, 0);
    }
    fn insert(&mut self, o: u16, d: u16) {
        if o == 0 || d == 0 {
            return;
        }
        self.remove(o);
        let first = self.child(d);
        self.set_sibling(o, first);
        self.set_child(d, o);
        self.set_parent(o, d);
    }
    /// The first property entry (its size byte) of object o.
    fn first_prop(&self, o: u16) -> u32 {
        let p = self.props(o);
        p + 1 + 2 * self.rb(p) as u32
    }
    /// The size byte of property `n` of object o, or None.
    fn find_prop(&self, o: u16, n: u16) -> Option<u32> {
        if o == 0 {
            return None;
        }
        let mut a = self.first_prop(o);
        loop {
            let b = self.rb(a);
            let num = (b & 31) as u16;
            if b == 0 || num < n {
                return None;
            }
            if num == n {
                return Some(a);
            }
            a += 2 + (b >> 5) as u32;
        }
    }
    fn get_prop(&self, o: u16, n: u16) -> u16 {
        match self.find_prop(o, n) {
            Some(a) => {
                if self.rb(a) >> 5 == 0 {
                    self.rb(a + 1) as u16
                } else {
                    self.rw(a + 1)
                }
            }
            None => self.rw(header(self.story, 0x0A) as u32 + 2 * (n as u32).saturating_sub(1)),
        }
    }

    /// An object's short name.
    pub fn name(&self, o: u16) -> String {
        if o == 0 || o > 255 {
            return String::new();
        }
        let p = self.props(o);
        if self.rb(p) == 0 {
            return String::new();
        }
        let mut text = Vec::new();
        self.decode(p + 1, &mut text, true);
        text.iter().filter(|&&z| (32..127).contains(&z)).map(|&z| z as u8 as char).collect()
    }

    // ---------- the status line ----------

    /// The global holding variable 16 + i.
    pub fn global(&self, i: u8) -> u16 {
        self.rw(global_at(self.story, i as u32))
    }
    /// Room, then score and moves (or hours and minutes for a time game).
    pub fn status(&self) -> (String, i16, i16, bool) {
        (self.name(self.global(0)), self.global(1) as i16, self.global(2) as i16, self.story[1] & 2 != 0)
    }

    // ---------- input ----------

    /// Gives the waiting sread its line, lower case, and runs on.
    pub fn input(&mut self, line: &[u8]) {
        if self.m.mode != READ {
            return;
        }
        let (t, p) = (self.m.text_buf, self.m.parse_buf);
        let max = (self.rb(t) as usize).saturating_sub(1);
        let text: Vec<u8> =
            line.iter().map(|c| c.to_ascii_lowercase()).filter(|c| (32..127).contains(c)).take(max).collect();
        for (i, &c) in text.iter().enumerate() {
            self.wb(t + 1 + i as u32, c);
        }
        self.wb(t + 1 + text.len() as u32, 0);
        self.tokenise(p, &text);
        self.m.mode = RUN;
    }

    fn tokenise(&mut self, p: u32, text: &[u8]) {
        let dict = header(self.story, 8) as u32;
        let nsep = self.rb(dict) as u32;
        let seps: Vec<u8> = (0..nsep).map(|i| self.rb(dict + 1 + i)).collect();
        let mut words: Vec<(usize, usize)> = Vec::new();
        let mut start = None;
        for (i, &c) in text.iter().enumerate() {
            if c == b' ' || seps.contains(&c) {
                if let Some(s) = start.take() {
                    words.push((s, i));
                }
                if c != b' ' {
                    words.push((i, i + 1));
                }
            } else if start.is_none() {
                start = Some(i);
            }
        }
        if let Some(s) = start {
            words.push((s, text.len()));
        }
        let max = self.rb(p) as usize;
        let n = words.len().min(max);
        self.wb(p + 1, n as u8);
        for (k, &(s, e)) in words.iter().take(n).enumerate() {
            let at = self.lookup(&text[s..e]);
            let entry = p + 2 + 4 * k as u32;
            self.ww(entry, at);
            self.wb(entry + 2, (e - s) as u8);
            self.wb(entry + 3, (s + 1) as u8);
        }
    }

    /// A word's dictionary entry, 0 when the dictionary lacks it.
    fn lookup(&self, word: &[u8]) -> u16 {
        let key = Self::encode(word);
        let dict = header(self.story, 8) as u32;
        let nsep = self.rb(dict) as u32;
        let len = self.rb(dict + 1 + nsep) as u32;
        let count = self.rw(dict + 2 + nsep) as i16;
        let first = dict + 4 + nsep;
        let n = count.unsigned_abs() as u32;
        if count > 0 {
            // sorted: binary search
            let (mut lo, mut hi) = (0u32, n);
            while lo < hi {
                let mid = (lo + hi) / 2;
                let a = first + mid * len;
                let e = [self.rb(a), self.rb(a + 1), self.rb(a + 2), self.rb(a + 3)];
                match e.cmp(&key) {
                    core::cmp::Ordering::Equal => return a as u16,
                    core::cmp::Ordering::Less => lo = mid + 1,
                    core::cmp::Ordering::Greater => hi = mid,
                }
            }
            return 0;
        }
        for i in 0..n {
            let a = first + i * len;
            if [self.rb(a), self.rb(a + 1), self.rb(a + 2), self.rb(a + 3)] == key {
                return a as u16;
            }
        }
        0
    }

    // ---------- running ----------

    fn random(&mut self, range: i16) -> u16 {
        let c = &mut self.m.core;
        if range <= 0 {
            if range == 0 {
                c.seq = 0;
                c.seq_max = 0;
                c.rng = Rng::new(c.rng.0.wrapping_mul(2_654_435_761));
            } else if range > -1000 {
                c.seq = 1;
                c.seq_max = (-range) as u32;
            } else {
                c.seq = 0;
                c.seq_max = 0;
                c.rng = Rng::new((-(range as i32)) as u32);
            }
            return 0;
        }
        if c.seq_max > 0 {
            let v = c.seq;
            c.seq = if c.seq >= c.seq_max { 1 } else { c.seq + 1 };
            return ((v - 1) % range as u32 + 1) as u16;
        }
        (c.rng.next() % range as u32 + 1) as u16
    }

    /// Runs until the story waits for input, quits or fails, or the budget
    /// is spent (then the caller runs it again later).
    pub fn run(&mut self, budget: u32) {
        let mut n = 0;
        while self.m.mode == RUN && n < budget {
            self.step();
            n += 1;
        }
    }

    fn operand(&mut self, kind: u8) -> Option<u16> {
        match kind {
            0 => Some(self.fetch_w()),
            1 => Some(self.fetch() as u16),
            2 => {
                let v = self.fetch();
                Some(self.read_var(v))
            }
            _ => None,
        }
    }

    fn step(&mut self) {
        let at = self.m.core.pc;
        let op = self.fetch();
        let mut ops: [u16; 4] = [0; 4];
        let mut n = 0usize;
        match op {
            0x00..=0x7F => {
                // long form: 2OP, small constants or variables
                for (i, bit) in [0x40, 0x20].into_iter().enumerate() {
                    ops[i] = self.operand(if op & bit != 0 { 2 } else { 1 }).unwrap_or(0);
                }
                self.op2(op & 31, &ops[..2], at);
            }
            0x80..=0xBF => {
                let kind = (op >> 4) & 3;
                match self.operand(kind) {
                    Some(a) => self.op1(op & 15, a, at),
                    None => self.op0(op & 15, at),
                }
            }
            _ => {
                let types = self.fetch();
                for i in 0..4 {
                    match self.operand((types >> (6 - 2 * i)) & 3) {
                        Some(v) => {
                            ops[n] = v;
                            n += 1;
                        }
                        None => break,
                    }
                }
                if op < 0xE0 {
                    self.op2(op & 31, &ops[..n], at);
                } else {
                    self.opv(op & 31, &ops[..n], at);
                }
            }
        }
    }

    fn bad(&mut self, at: u32) {
        let op = self.rb(at);
        self.m.core.pc = at;
        self.fail(&format!("opcode {:02x} isn't in version 3", op));
    }

    fn op2(&mut self, op: u8, o: &[u16], at: u32) {
        let a = o.first().copied().unwrap_or(0);
        let b = o.get(1).copied().unwrap_or(0);
        match op {
            1 => {
                let c = o.get(1..).is_some_and(|rest| rest.contains(&a));
                self.branch(c)
            }
            2 => self.branch((a as i16) < (b as i16)),
            3 => self.branch((a as i16) > (b as i16)),
            4 => {
                let v = (self.peek_var(a as u8) as i16).wrapping_sub(1);
                self.poke_var(a as u8, v as u16);
                self.branch(v < b as i16)
            }
            5 => {
                let v = (self.peek_var(a as u8) as i16).wrapping_add(1);
                self.poke_var(a as u8, v as u16);
                self.branch(v > b as i16)
            }
            6 => {
                let c = self.parent(a) == b;
                self.branch(c)
            }
            7 => self.branch(a & b == b),
            8 => self.store(a | b),
            9 => self.store(a & b),
            10 => {
                let c = self.attr(a, b);
                self.branch(c)
            }
            11 => self.set_attr(a, b, true),
            12 => self.set_attr(a, b, false),
            13 => self.poke_var(a as u8, b),
            14 => self.insert(a, b),
            15 => {
                let v = self.rw(a.wrapping_add(b.wrapping_mul(2)) as u32);
                self.store(v)
            }
            16 => {
                let v = self.rb(a.wrapping_add(b) as u32) as u16;
                self.store(v)
            }
            17 => {
                let v = self.get_prop(a, b);
                self.store(v)
            }
            18 => {
                let v = self.find_prop(a, b).map_or(0, |p| p + 1) as u16;
                self.store(v)
            }
            19 => {
                let v = if a == 0 {
                    0
                } else if b == 0 {
                    (self.rb(self.first_prop(a)) & 31) as u16
                } else {
                    match self.find_prop(a, b) {
                        Some(p) => (self.rb(p + 2 + (self.rb(p) >> 5) as u32) & 31) as u16,
                        None => {
                            self.fail("get_next_prop of a missing property");
                            return;
                        }
                    }
                };
                self.store(v)
            }
            20 => self.store((a as i16).wrapping_add(b as i16) as u16),
            21 => self.store((a as i16).wrapping_sub(b as i16) as u16),
            22 => self.store((a as i16).wrapping_mul(b as i16) as u16),
            23 | 24 => {
                if b == 0 {
                    self.fail("division by zero");
                    return;
                }
                let v = if op == 23 { (a as i16).wrapping_div(b as i16) } else { (a as i16).wrapping_rem(b as i16) };
                self.store(v as u16)
            }
            _ => self.bad(at),
        }
    }

    fn op1(&mut self, op: u8, a: u16, at: u32) {
        match op {
            0 => self.branch(a == 0),
            1 => {
                let v = self.sibling(a);
                self.store(v);
                self.branch(v != 0)
            }
            2 => {
                let v = self.child(a);
                self.store(v);
                self.branch(v != 0)
            }
            3 => {
                let v = self.parent(a);
                self.store(v)
            }
            4 => {
                let v = if a == 0 { 0 } else { (self.rb(a as u32 - 1) >> 5) as u16 + 1 };
                self.store(v)
            }
            5 => {
                let v = self.peek_var(a as u8).wrapping_add(1);
                self.poke_var(a as u8, v)
            }
            6 => {
                let v = self.peek_var(a as u8).wrapping_sub(1);
                self.poke_var(a as u8, v)
            }
            7 => {
                self.print_at(a as u32);
            }
            9 => self.remove(a),
            10 => {
                let s = self.name(a);
                self.put_str(&s)
            }
            11 => self.ret(a),
            12 => self.m.core.pc = (self.m.core.pc as i32 + a as i16 as i32 - 2) as u32,
            13 => {
                self.print_at(2 * a as u32);
            }
            14 => {
                let v = self.peek_var(a as u8);
                self.store(v)
            }
            15 => self.store(!a),
            _ => self.bad(at),
        }
    }

    fn op0(&mut self, op: u8, at: u32) {
        match op {
            0 => self.ret(1),
            1 => self.ret(0),
            2 => {
                let pc = self.m.core.pc;
                self.m.core.pc = self.print_at(pc);
            }
            3 => {
                let pc = self.m.core.pc;
                self.m.core.pc = self.print_at(pc);
                self.put(13);
                self.ret(1)
            }
            4 => {}
            5 => {
                // save: the whole machine, to come back to this branch
                self.m.saved = self.m.core;
                self.m.has_saved = 1;
                self.branch(true)
            }
            6 => {
                if self.m.has_saved == 0 {
                    return self.branch(false);
                }
                self.m.core = self.m.saved;
                self.m.core.s3_depth = 0;
                self.m.core.window = 0;
                self.m.core.screen_off = 0;
                self.branch(true)
            }
            7 => {
                let ended = score(self.story, &self.m.core.mem);
                keep_best(&mut self.restarted, ended);
                let seed = self.m.core.rng.0;
                self.boot(seed);
            }
            8 => {
                let v = self.pop();
                self.ret(v)
            }
            9 => {
                self.pop();
            }
            10 => self.m.mode = QUIT,
            11 => self.put(13),
            12 => {} // the status line is drawn from the globals every frame
            13 => {
                let len = (2 * header(self.story, 0x1A) as usize).min(self.story.len());
                let sum = self.story[0x40.min(len)..len].iter().fold(0u16, |s, &b| s.wrapping_add(b as u16));
                self.branch(sum == header(self.story, 0x1C))
            }
            15 => self.branch(true),
            _ => self.bad(at),
        }
    }

    fn opv(&mut self, op: u8, o: &[u16], at: u32) {
        let a = o.first().copied().unwrap_or(0);
        let b = o.get(1).copied().unwrap_or(0);
        let c = o.get(2).copied().unwrap_or(0);
        match op {
            0 => {
                let store = self.fetch() as u16;
                self.call(a, &o[o.len().min(1)..], store)
            }
            1 => self.ww(a.wrapping_add(b.wrapping_mul(2)) as u32, c),
            2 => self.wb(a.wrapping_add(b) as u32, c as u8),
            3 => match self.find_prop(a, b) {
                Some(p) => {
                    if self.rb(p) >> 5 == 0 {
                        self.wb(p + 1, c as u8)
                    } else {
                        self.ww(p + 1, c)
                    }
                }
                None => self.fail("put_prop of a missing property"),
            },
            4 => {
                self.m.text_buf = a as u32;
                self.m.parse_buf = b as u32;
                self.m.mode = READ;
            }
            5 => self.put(a),
            6 => {
                let s = (a as i16).to_string();
                self.put_str(&s)
            }
            7 => {
                let v = self.random(a as i16);
                self.store(v)
            }
            8 => self.push(a),
            9 => {
                let v = self.pop();
                self.poke_var(a as u8, v)
            }
            10 => {}
            11 => self.m.core.window = a as u32,
            19 => match a as i16 {
                1 => self.m.core.screen_off = 0,
                -1 => self.m.core.screen_off = 1,
                3 => {
                    let d = self.m.core.s3_depth as usize;
                    if d < 16 {
                        self.m.core.s3[d] = b as u32;
                        self.m.core.s3_depth += 1;
                        self.ww(b as u32, 0);
                    }
                }
                -3 => self.m.core.s3_depth = self.m.core.s3_depth.saturating_sub(1),
                _ => {}
            },
            20 | 21 => {}
            _ => self.bad(at),
        }
    }
}
