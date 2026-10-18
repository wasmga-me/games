//! Branchwise: learn git by typing it. Each level is a small repository and
//! a goal; the commit graph redraws as each command runs, and a level is
//! cleared when the repository matches the goal. Lessons run from the first
//! commit to rebasing stacks, remotes, bisect and worktrees.
mod git;
mod glyphs;
mod graph;
mod levels;
mod play;

use git::{Snap, World, CMD, ERR, FAIL, GOOD, HINT};
use graph::{Area, Motion, Spot, Theme, THEMES};
use levels::LEVELS;
use wg::{hex, tr, Game, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 1;
const N: usize = LEVELS.len();
/// the commands run on this level, one per line
const CMDS: usize = 4096;
/// characters a command needs beyond the font (the glyph tool reads this)
#[allow(dead_code)]
const TYPED: &str = "~@{}\"#$&*=_;[]|`\\";

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    /// options, in the catalog `setup` order: level to start at, theme
    opt: [i32; 2],
    level: i32,
    /// 0 playing, 1 level cleared (waiting to go on), 2 every level done
    phase: i32,
    moves: i32,
    hints: i32,
    level_ms: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
    done: i32,
    commands: i32,
    mistakes: i32,
    hints_total: i32,
    time_ms: i32,
    cmds_len: i32,
    cmds: [u8; CMDS],
}
unsafe impl wg::Plain for Save {}

#[derive(Clone, Debug, PartialEq)]
enum Btn {
    Run,
    Back,
    Hint,
    Undo,
    Goal,
    Next,
    Act(u8),
    Up,
    Down,
    Start,
    Abort,
}

#[derive(Clone, Debug, PartialEq)]
enum Hit {
    Spot(Spot),
    Chip(String),
    Btn(Btn),
    Todo(usize),
}

struct Branchwise {
    s: Save,
    world: World,
    start: World,
    goal: World,
    goal_snap: Snap,
    built_for: i32,
    input: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    hist: usize,
    log: Vec<(u8, String)>,
    ms: u32,
    dt: f32,
    pending: u32,
    dirty: u32,
    hits: Vec<(f32, f32, f32, f32, Hit)>,
    hover: Option<Hit>,
    tip: Option<(String, f32, f32)>,
    tip_ms: i32,
    motion: [Motion; 4],
    animating: bool,
    show_goal: bool,
    rep: KeyRepeat,
}

fn fresh() -> Save {
    Save {
        version: VERSION,
        opt: [0, 0],
        level: 0,
        phase: 0,
        moves: 0,
        hints: 0,
        level_ms: 0,
        score: 0,
        cleared: -1,
        cleared_score: 0,
        done: 0,
        commands: 0,
        mistakes: 0,
        hints_total: 0,
        time_ms: 0,
        cmds_len: 0,
        cmds: [0; CMDS],
    }
}

/// Commands a chip starts (it replaces what is typed) rather than adds to.
fn starts_command(c: &str) -> bool {
    c.starts_with("git ")
        || c.starts_with("cd ")
        || c.starts_with("edit ")
        || c == "make test"
        || c == "hint"
        || c == "undo"
}

fn level_of(n: usize, of: usize) -> String {
    match wg::lang() {
        wg::ZH => format!("第 {n} 关，共 {of} 关"),
        wg::JA => format!("レベル {n} / {of}"),
        wg::KO => format!("레벨 {n} / {of}"),
        wg::ES => format!("Nivel {n} de {of}"),
        wg::PT => format!("Nível {n} de {of}"),
        wg::FR => format!("Niveau {n} sur {of}"),
        wg::DE => format!("Level {n} von {of}"),
        wg::RU => format!("Уровень {n} из {of}"),
        _ => format!("Level {n} of {of}"),
    }
}

fn t_goal() -> &'static str {
    tr(&[
        "Goal",
        "目标",
        "ゴール",
        "목표",
        "Objetivo",
        "Objetivo",
        "Objectif",
        "Ziel",
        "Цель",
    ])
}

fn t_score() -> &'static str {
    tr(&[
        "Score",
        "得分",
        "スコア",
        "점수",
        "Puntos",
        "Pontos",
        "Score",
        "Punkte",
        "Очки",
    ])
}

/// Words of a lesson: (text, is code, sticks to the word before). Code
/// (between backticks) stays in one piece, and punctuation after it sticks.
fn words(text: &str) -> Vec<(String, bool, bool)> {
    let mut out = vec![];
    for (k, part) in text.split('`').enumerate() {
        if k % 2 == 1 {
            let glue = !out.is_empty() && !text.split('`').nth(k - 1).unwrap_or(" ").ends_with(' ');
            out.push((part.to_string(), true, glue));
            continue;
        }
        for (j, w) in part.split(' ').enumerate().filter(|(_, w)| !w.is_empty()) {
            out.push((w.to_string(), false, j == 0 && k > 0));
        }
    }
    out
}

/// Greedy line breaks; each line is its words with their x offsets.
fn wrap(
    s: &Screen,
    ws: &[(String, bool, bool)],
    size: f32,
    max_w: f32,
) -> Vec<Vec<(f32, String, bool)>> {
    let space = s.text_width(" ", size);
    let mut lines: Vec<Vec<(f32, String, bool)>> = vec![vec![]];
    let mut x = 0.0;
    for (w, code, glue) in ws {
        let ww = s.text_width(w, size);
        if *glue && x > 0.0 {
            x -= space;
        } else if x > 0.0 && x + ww > max_w {
            lines.push(vec![]);
            x = 0.0;
        }
        lines.last_mut().unwrap().push((x, w.clone(), *code));
        x += ww + space;
    }
    lines
}

/// Plain text broken into lines that fit.
fn wrap_plain(s: &Screen, text: &str, size: f32, max_w: f32) -> Vec<String> {
    let ws: Vec<(String, bool, bool)> = text
        .split(' ')
        .map(|w| (w.to_string(), false, false))
        .collect();
    let mut out = vec![];
    for line in wrap(s, &ws, size, max_w) {
        out.push(
            line.iter()
                .map(|w| w.1.as_str())
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    out
}

impl Branchwise {
    fn lvl(&self) -> usize {
        (self.s.level.max(0) as usize).min(N - 1)
    }

    fn build(&mut self) {
        let l = self.lvl();
        self.start = play::start(l);
        self.goal = play::goal(l);
        self.goal_snap = self.goal.snap();
        self.built_for = l as i32;
        self.motion = Default::default();
        self.input.clear();
        self.cursor = 0;
        self.show_goal = false;
        self.hover = None;
        self.tip = None;
    }

    fn lines(&self) -> Vec<String> {
        let bytes = &self.s.cmds[..self.s.cmds_len.clamp(0, CMDS as i32) as usize];
        String::from_utf8_lossy(bytes)
            .split('\n')
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect()
    }

    /// The level's repository after every command kept so far.
    fn replay(&mut self) {
        self.world = self.start.clone();
        for line in self.lines() {
            self.world.run(&line);
        }
        self.world.out.clear();
        self.world.todo = None;
    }

    fn record(&mut self, line: &str) {
        let b = line.as_bytes();
        let at = self.s.cmds_len as usize;
        if at + b.len() + 1 <= CMDS {
            self.s.cmds[at..at + b.len()].copy_from_slice(b);
            self.s.cmds[at + b.len()] = b'\n';
            self.s.cmds_len += b.len() as i32 + 1;
        }
    }

    fn begin_level(&mut self, level: i32) {
        self.s.level = level;
        self.s.phase = 0;
        self.s.moves = 0;
        self.s.hints = 0;
        self.s.level_ms = 0;
        self.s.cmds_len = 0;
        self.s.cmds = [0; CMDS];
        self.build();
        self.world = self.start.clone();
        self.log.clear();
    }

    fn say(&mut self, kind: u8, s: impl Into<String>) {
        self.log.push((kind, s.into()));
        if self.log.len() > 200 {
            self.log.drain(..50);
        }
    }

    fn par(&self) -> i32 {
        LEVELS[self.lvl()]
            .solution
            .iter()
            .map(|l| git::split(l).len() as i32)
            .sum()
    }

    fn check(&mut self) {
        if self.s.phase != 0 || self.world.snap() != self.goal_snap {
            return;
        }
        let extra = (self.s.moves - self.par()).max(0);
        let score = (1000 - 100 * extra - 250 * self.s.hints).max(200);
        self.s.score += score;
        self.s.cleared = self.s.level;
        self.s.cleared_score = score;
        self.s.done += 1;
        self.pending |= wg::EV_LEVEL;
        if self.s.level as usize + 1 >= N {
            self.s.phase = 2;
            self.pending |= wg::EV_WIN | wg::EV_GAMEOVER;
        } else {
            self.s.phase = 1;
        }
    }

    fn next(&mut self) {
        if self.s.phase == 1 {
            self.begin_level(self.s.level + 1);
            self.dirty += 1;
        }
    }

    fn submit(&mut self) {
        let line: String = self.input.iter().collect::<String>().trim().to_string();
        self.input.clear();
        self.cursor = 0;
        if line.is_empty() {
            return;
        }
        if self.history.last() != Some(&line) {
            self.history.push(line.clone());
        }
        self.hist = self.history.len();
        self.say(CMD, line.clone());
        self.s.commands += 1;
        match line.as_str() {
            "hint" => {
                self.s.hints += 1;
                self.s.hints_total += 1;
                self.say(HINT, LEVELS[self.lvl()].hint);
                return;
            }
            "solution" => {
                self.s.hints += 2;
                self.s.hints_total += 2;
                for l in LEVELS[self.lvl()].solution {
                    let shown = match l.split_once(" #todo ") {
                        Some((cmd, todo)) => {
                            let acts: Vec<&str> = todo
                                .split(' ')
                                .map(|e| match e.as_bytes()[0] {
                                    b's' => "squash",
                                    b'f' => "fixup",
                                    b'd' => "drop",
                                    _ => "pick",
                                })
                                .collect();
                            format!("{cmd}, then: {}", acts.join(", "))
                        }
                        None => l.to_string(),
                    };
                    self.say(HINT, shown);
                }
                return;
            }
            "undo" => {
                self.undo();
                return;
            }
            "clear" => {
                self.log.clear();
                return;
            }
            "help" | "?" => {
                self.say(OUT_HELP, "Type git commands, such as git status or git log. Also: hint, solution, undo, clear.");
                return;
            }
            _ => {}
        }
        self.world.out.clear();
        let changed = self.world.run(&line);
        let out = std::mem::take(&mut self.world.out);
        if out.iter().any(|o| o.0 == ERR) {
            self.s.mistakes += 1;
        }
        for o in out {
            self.say(o.0, o.1);
        }
        if changed {
            self.record(&line);
            self.s.moves += git::split(&line).len() as i32;
            self.check();
        }
    }

    fn undo(&mut self) {
        let mut lines = self.lines();
        let Some(last) = lines.pop() else {
            self.say(HINT, "Nothing to undo.");
            return;
        };
        self.s.cmds_len = 0;
        self.s.cmds = [0; CMDS];
        for l in &lines {
            self.record(l);
        }
        self.replay();
        let shown = last.split(" #todo ").next().unwrap_or(&last).to_string();
        self.say(HINT, format!("Undid: {shown}"));
    }

    fn insert_str(&mut self, w: &str) {
        for c in w.chars() {
            self.input.insert(self.cursor, c);
            self.cursor += 1;
        }
    }

    fn chip(&mut self, c: &str) {
        if starts_command(c) {
            self.input = c.chars().collect();
            self.input.push(' ');
            self.cursor = self.input.len();
        } else {
            self.word(c);
        }
    }

    /// Adds a word at the caret, with spaces around it.
    fn word(&mut self, w: &str) {
        if self.cursor > 0 && self.input[self.cursor - 1] != ' ' {
            self.insert_str(" ");
        }
        self.insert_str(w);
        self.insert_str(" ");
    }

    fn todo_key(&mut self, code: i32) -> bool {
        let Some(t) = self.world.todo.as_mut() else {
            return false;
        };
        let n = t.lines.len();
        match code {
            13 => self.finish_todo(),
            8 | 27 | 0x71 => {
                self.world.todo = None;
                self.say(HINT, "Rebase stopped; nothing changed.");
            }
            0x70 | 0x73 | 0x66 | 0x64 | 0x72 => {
                let act = if code == 0x72 { b'p' } else { code as u8 };
                t.lines[t.sel].1 = act;
            }
            0x6b if t.sel > 0 => {
                t.lines.swap(t.sel, t.sel - 1);
                t.sel -= 1;
            }
            0x6a if t.sel + 1 < n => {
                t.lines.swap(t.sel, t.sel + 1);
                t.sel += 1;
            }
            _ => {}
        }
        true
    }

    fn finish_todo(&mut self) {
        let Some(line) = self.world.todo_line() else {
            return;
        };
        self.world.todo = None;
        self.world.out.clear();
        let changed = self.world.run(&line);
        let out = std::mem::take(&mut self.world.out);
        if out.iter().any(|o| o.0 == ERR) {
            self.s.mistakes += 1;
        }
        for o in out {
            self.say(o.0, o.1);
        }
        if changed {
            self.record(&line);
            self.s.moves += 1;
            self.check();
        }
    }

    fn press(&mut self, h: Hit) {
        match h {
            Hit::Spot(Spot::Commit(id, msg)) => {
                if self.world.todo.is_none() && self.s.phase == 0 {
                    self.word(&id);
                }
                self.tip = Some((format!("{id} {msg}"), 0.0, 0.0));
                self.tip_ms = 2500;
            }
            Hit::Spot(Spot::Name(n)) => {
                if self.world.todo.is_none() && self.s.phase == 0 {
                    self.word(&n);
                }
            }
            Hit::Chip(c) => self.chip(&c),
            Hit::Todo(i) => {
                if let Some(t) = self.world.todo.as_mut() {
                    t.sel = i.min(t.lines.len().saturating_sub(1));
                }
            }
            Hit::Btn(b) => match b {
                Btn::Run => self.submit(),
                Btn::Back => {
                    if self.cursor > 0 {
                        self.cursor -= 1;
                        self.input.remove(self.cursor);
                    }
                }
                Btn::Hint => {
                    self.input = "hint".chars().collect();
                    self.submit();
                }
                Btn::Undo => {
                    self.input = "undo".chars().collect();
                    self.submit();
                }
                Btn::Goal => self.show_goal = !self.show_goal,
                Btn::Next => self.next(),
                Btn::Act(a) => {
                    self.todo_key(a as i32);
                }
                Btn::Up => {
                    self.todo_key(0x6b);
                }
                Btn::Down => {
                    self.todo_key(0x6a);
                }
                Btn::Start => self.finish_todo(),
                Btn::Abort => {
                    self.todo_key(27);
                }
            },
        }
        self.dirty += 1;
    }
}

const OUT_HELP: u8 = git::OUT;

impl Game for Branchwise {
    type Save = Save;
    const PLAN: (f32, f32) = (1200.0, 750.0);
    const CHOICES: &'static [i32] = &[N as i32, 2];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let start = play::start(0);
        let goal = play::goal(0);
        Branchwise {
            s: fresh(),
            world: start.clone(),
            goal_snap: goal.snap(),
            start,
            goal,
            built_for: 0,
            input: vec![],
            cursor: 0,
            history: vec![],
            hist: 0,
            log: vec![],
            ms: 0,
            dt: 16.0,
            pending: 0,
            dirty: 0,
            hits: vec![],
            hover: None,
            tip: None,
            tip_ms: 0,
            motion: Default::default(),
            animating: false,
            show_goal: false,
            rep: KeyRepeat(0),
        }
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, _seed: u32) {
        let opt = self.s.opt;
        self.s = fresh();
        self.s.opt = opt;
        self.begin_level(opt[0].clamp(0, N as i32 - 1));
        self.history.clear();
        self.hist = 0;
        self.dirty += 1;
    }
    fn update(&mut self, input: &Input) -> u32 {
        self.dt = input.dt as f32;
        self.ms = self.ms.wrapping_add(input.dt as u32);
        if self.tip_ms > 0 {
            self.tip_ms -= input.dt;
            if self.tip_ms <= 0 {
                self.tip = None;
                self.dirty += 1;
            }
        }
        if self.s.phase == 0 {
            self.s.level_ms += input.dt;
            self.s.time_ms += input.dt;
        }
        let dirs = self.rep.dirs(input);
        if dirs != 0 {
            self.dirty += 1;
            if self.world.todo.is_some() {
                if let Some(t) = self.world.todo.as_mut() {
                    if dirs & wg::UP != 0 && t.sel > 0 {
                        t.sel -= 1;
                    }
                    if dirs & wg::DOWN != 0 && t.sel + 1 < t.lines.len() {
                        t.sel += 1;
                    }
                }
            } else {
                if dirs & wg::LEFT != 0 && self.cursor > 0 {
                    self.cursor -= 1;
                }
                if dirs & wg::RIGHT != 0 && self.cursor < self.input.len() {
                    self.cursor += 1;
                }
                if dirs & wg::UP != 0 && self.hist > 0 {
                    self.hist -= 1;
                    self.input = self.history[self.hist].chars().collect();
                    self.cursor = self.input.len();
                }
                if dirs & wg::DOWN != 0 && self.hist < self.history.len() {
                    self.hist += 1;
                    self.input = self
                        .history
                        .get(self.hist)
                        .map(|h| h.chars().collect())
                        .unwrap_or_default();
                    self.cursor = self.input.len();
                }
            }
        }
        if input.pressed & wg::A != 0 && self.s.phase == 1 {
            self.next();
        }
        std::mem::take(&mut self.pending)
    }
    fn key(&mut self, code: i32) -> bool {
        self.dirty += 1;
        if self.s.phase == 2 {
            return false;
        }
        if self.s.phase == 1 {
            if code == 13 || code == 32 {
                self.next();
                return true;
            }
            return false;
        }
        if self.world.todo.is_some() {
            return self.todo_key(code);
        }
        match code {
            13 => {
                if self.input.iter().all(|c| *c == ' ') {
                    return false;
                }
                self.submit();
            }
            8 => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.input.remove(self.cursor);
                }
            }
            127 => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
            }
            c if c >= 32 && c != 127 && self.input.len() < 160 => {
                if let Some(ch) = char::from_u32(c as u32) {
                    self.input.insert(self.cursor, ch);
                    self.cursor += 1;
                }
            }
            _ => return false,
        }
        true
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        let at = |x: f32, y: f32, hits: &[(f32, f32, f32, f32, Hit)]| {
            hits.iter()
                .rev()
                .find(|h| x >= h.0 && y >= h.1 && x <= h.0 + h.2 && y <= h.1 + h.3)
                .map(|h| h.4.clone())
        };
        let over = if p.inside {
            at(p.x, p.y, &self.hits)
        } else {
            None
        };
        if !p.touch && over != self.hover {
            self.hover = over.clone();
            self.tip = match &over {
                Some(Hit::Spot(Spot::Commit(id, msg))) => Some((format!("{id} {msg}"), p.x, p.y)),
                _ => None,
            };
            self.dirty += 1;
        }
        if p.pressed & wg::M_LEFT != 0 {
            if let Some(h) = over {
                self.press(h);
                if let Some(t) = self.tip.as_mut() {
                    t.1 = p.x;
                    t.2 = p.y;
                }
            }
        }
        0
    }
    fn picture(&mut self) -> u32 {
        let caret = (self.ms / 530) % 2;
        let extra = [
            self.dirty,
            caret,
            self.input.len() as u32,
            self.cursor as u32,
        ];
        wg::hash(wg::hash(0, wg::bytes_of(&self.s)), wg::bytes_of(&extra))
    }
    fn moving(&self) -> bool {
        self.animating
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        if w >= h * 1.15 {
            (1040.0, (1040.0 * h / w).max(540.0))
        } else {
            (440.0, (440.0 * h / w).max(560.0))
        }
    }
    fn draw(&mut self, s: &mut Screen) {
        self.hits.clear();
        let t = &THEMES[self.s.opt[1].clamp(0, 1) as usize];
        s.clear(hex(t.bg));
        let wide = s.pw > 900.0;
        if wide {
            self.draw_wide(s, t);
        } else {
            self.draw_tall(s, t);
        }
        if let Some((tip, x, y)) = &self.tip {
            let w = s.text_width(tip, 14.0) + 20.0;
            let x = x.min(s.pw - w - 8.0).max(8.0);
            let y = if *y > 60.0 { y - 44.0 } else { y + 24.0 };
            s.shadow(x, y, w, 28.0, 8.0, 6.0, 0, 0.25);
            s.rect(x, y, w, 28.0, 8.0, hex(t.ink), 1.0);
            s.text(tip, x + 10.0, y + 14.0, 14.0, hex(t.bg), 1.0);
        }
    }
    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.phase == 2
    }
    fn reached(&self) -> i32 {
        self.s.level
    }
    fn cleared(&self) -> Option<(i32, u32)> {
        (self.s.cleared >= 0).then(|| (self.s.cleared, self.s.cleared_score as u32))
    }
    fn stat(&self, i: usize) -> i32 {
        match i {
            0 => self.s.done,
            1 => self.s.commands,
            2 => self.s.mistakes,
            3 => self.s.hints_total,
            4 => self.s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION
            && (0..N as i32).contains(&s.level)
            && (0..N as i32).contains(&s.opt[0])
            && (0..2).contains(&s.opt[1])
            && (0..=2).contains(&s.phase)
            && (0..=CMDS as i32).contains(&s.cmds_len)
            && (-1..N as i32).contains(&s.cleared)
            && s.score >= 0
            && s.time_ms >= 0
            && s.level_ms >= 0;
        if !ok {
            return false;
        }
        self.build();
        self.replay();
        self.log.clear();
        self.history.clear();
        self.hist = 0;
        self.dirty += 1;
        true
    }
}

// ---------- drawing ----------

/// A rounded box's edge, `bw` thick, over whatever is inside it.
#[allow(clippy::too_many_arguments)]
fn outline(s: &mut Screen, x: f32, y: f32, w: f32, h: f32, r: f32, bw: f32, c: u32, alpha: f32) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let hw = bw / 2.0;
    s.line(x + r, y, x + w - r, y, hw, c, alpha);
    s.line(x + r, y + h, x + w - r, y + h, hw, c, alpha);
    s.line(x, y + r, x, y + h - r, hw, c, alpha);
    s.line(x + w, y + r, x + w, y + h - r, hw, c, alpha);
    for (cx, cy, a0) in [
        (x + w - r, y + r, -90.0f32),
        (x + w - r, y + h - r, 0.0),
        (x + r, y + h - r, 90.0),
        (x + r, y + r, 180.0),
    ] {
        let mut prev = None;
        for k in 0..=6 {
            let a = (a0 + 15.0 * k as f32).to_radians();
            let p = (cx + r * a.cos(), cy + r * a.sin());
            if let Some((px, py)) = prev {
                s.line(px, py, p.0, p.1, hw, c, alpha);
            }
            prev = Some(p);
        }
    }
}

impl Branchwise {
    fn button(
        &mut self,
        s: &mut Screen,
        t: &Theme,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        hit: Hit,
        strong: bool,
    ) {
        let hov = self.hover.as_ref() == Some(&hit);
        let (fill, ink) = if strong {
            (t.accent, 0xffffff)
        } else {
            (t.panel, t.ink)
        };
        let fill = if hov {
            wg::mix(hex(fill), hex(t.ink), 0.12)
        } else {
            hex(fill)
        };
        s.rect(x, y + 2.0, w, h, h * 0.3, hex(t.line), 1.0);
        s.rect(x, y, w, h, h * 0.3, fill, 1.0);
        if !strong {
            outline(s, x, y, w, h, h * 0.3, 1.2, hex(t.line), 1.0);
        }
        let size = s.fit_size(label, (h * 0.42).min(16.0), w - 14.0);
        s.text_c(label, x + w / 2.0, y + h / 2.0, size, hex(ink), 1.0);
        self.hits.push((x, y, w, h, hit));
    }

    /// The lesson: chapter, title, text and goal, shrunk to fit the box.
    fn draw_lesson(
        &mut self,
        s: &mut Screen,
        t: &Theme,
        a: &Area,
        title_size: f32,
        text_size: f32,
    ) -> f32 {
        let l = &LEVELS[self.lvl()];
        s.text(
            &level_of(self.lvl() + 1, N),
            a.x,
            a.y + 8.0,
            title_size * 0.5,
            hex(t.muted),
            1.0,
        );
        s.text_r(
            l.chapter,
            a.x + a.w,
            a.y + 8.0,
            title_size * 0.5,
            hex(t.accent),
            1.0,
        );
        let ts = s.fit_size(l.title, title_size, a.w);
        s.text(l.title, a.x, a.y + 12.0 + title_size, ts, hex(t.ink), 1.0);
        let body_y = a.y + 22.0 + title_size * 1.6;
        let goal = format!("{}: {}", t_goal(), l.goal);
        let mut size = text_size;
        loop {
            let lh = size * 1.48;
            let body = wrap(s, &words(l.text), size, a.w);
            let gl = wrap_plain(s, &goal, size, a.w);
            let h = (body.len() + gl.len()) as f32 * lh + size * 0.8;
            if h <= a.y + a.h - body_y || size <= 10.5 {
                let mut y = body_y + lh * 0.5;
                for line in &body {
                    for (x, w, code) in line {
                        if *code {
                            let ww = s.text_width(w, size);
                            s.rect(
                                a.x + x - 2.0,
                                y - size * 0.62,
                                ww + 4.0,
                                size * 1.24,
                                4.0,
                                hex(t.code),
                                0.1,
                            );
                        }
                        s.text(
                            w,
                            a.x + x,
                            y,
                            size,
                            hex(if *code { t.code } else { t.ink }),
                            if *code { 1.0 } else { 0.86 },
                        );
                    }
                    y += lh;
                }
                y += size * 0.8;
                for g in gl {
                    s.text(&g, a.x, y, size, hex(t.accent), 1.0);
                    y += lh;
                }
                return y;
            }
            size -= 0.5;
        }
    }

    /// Working tree and stash, for the lessons that have files.
    fn draw_files(
        s: &mut Screen,
        t: &Theme,
        w: &World,
        x: f32,
        y: f32,
        width: f32,
        small: bool,
    ) -> f32 {
        if !w.uses_files {
            return y;
        }
        let fs = if small { 11.5 } else { 13.5 };
        let lh = fs * 1.6;
        let rows = w.files.len() + if w.stash.is_empty() { 0 } else { 1 };
        let h = rows as f32 * lh + 12.0;
        s.rect(x, y, width, h, 8.0, hex(t.panel), 0.92);
        outline(s, x, y, width, h, 8.0, 1.0, hex(t.line), 1.0);
        let mut yy = y + 6.0 + lh / 2.0;
        for f in &w.files {
            let (label, c) = if f.staged && f.modified {
                ("staged, edited", t.warn)
            } else if f.staged {
                ("staged", t.good)
            } else if f.modified && !f.tracked {
                ("untracked", t.bad)
            } else if f.modified {
                ("modified", t.warn)
            } else if f.tracked {
                ("committed", t.muted)
            } else {
                ("", t.muted)
            };
            s.disc(x + 12.0, yy, fs * 0.3, hex(c), 1.0);
            s.text(&f.name, x + 22.0, yy, fs, hex(t.ink), 1.0);
            s.text_r(label, x + width - 10.0, yy, fs * 0.9, hex(c), 1.0);
            yy += lh;
        }
        if !w.stash.is_empty() {
            s.text(
                &format!("stash: {}", w.stash.len()),
                x + 22.0,
                yy,
                fs,
                hex(t.muted),
                1.0,
            );
        }
        y + h
    }

    /// The repository (and origin when there is one) in an area.
    #[allow(clippy::too_many_arguments)]
    fn draw_world(
        &mut self,
        s: &mut Screen,
        t: &Theme,
        goal: bool,
        a: &Area,
        vertical: bool,
        small: bool,
    ) {
        let w = if goal { &self.goal } else { &self.world };
        let has_origin = w.origin.is_some();
        let label = |s: &mut Screen, text: &str, x: f32, y: f32| {
            s.text(
                text,
                x,
                y,
                if small { 11.0 } else { 13.0 },
                hex(t.muted),
                1.0,
            );
        };
        let (la, oa) = if has_origin {
            if vertical {
                let lw = a.w * 0.56;
                (
                    Area {
                        x: a.x,
                        y: a.y + 18.0,
                        w: lw - 6.0,
                        h: a.h - 18.0,
                    },
                    Some(Area {
                        x: a.x + lw + 6.0,
                        y: a.y + 18.0,
                        w: a.w - lw - 6.0,
                        h: a.h - 18.0,
                    }),
                )
            } else {
                let lh = a.h * 0.56;
                (
                    Area {
                        x: a.x,
                        y: a.y + 16.0,
                        w: a.w,
                        h: lh - 22.0,
                    },
                    Some(Area {
                        x: a.x,
                        y: a.y + lh + 16.0,
                        w: a.w,
                        h: a.h - lh - 16.0,
                    }),
                )
            }
        } else {
            (
                Area {
                    x: a.x,
                    y: a.y,
                    w: a.w,
                    h: a.h,
                },
                None,
            )
        };
        let dt = self.dt;
        let mut spots = vec![];
        let hover = match &self.hover {
            Some(Hit::Spot(Spot::Commit(id, _))) => Some(id.clone()),
            _ => None,
        };
        let base = if goal { 2 } else { 0 };
        let w = if goal { &self.goal } else { &self.world };
        if has_origin {
            let path = w
                .repo
                .wts
                .get(w.repo.cur)
                .map(|x| x.path.clone())
                .unwrap_or_default();
            label(s, &format!("~/{path}"), la.x + 4.0, la.y - 8.0);
            s.line(
                oa.as_ref().unwrap().x - if vertical { 6.0 } else { 0.0 },
                oa.as_ref().unwrap().y - if vertical { 12.0 } else { 18.0 },
                if vertical {
                    oa.as_ref().unwrap().x - 6.0
                } else {
                    a.x + a.w
                },
                if vertical {
                    a.y + a.h
                } else {
                    oa.as_ref().unwrap().y - 18.0
                },
                0.8,
                hex(t.line),
                1.0,
            );
            let oa2 = oa.as_ref().unwrap();
            label(s, "origin", oa2.x + 4.0, oa2.y - 8.0);
        }
        let look = |r: &git::Repo, bare: bool| graph::Look {
            vertical,
            small,
            bare,
            // the goal leaves out commits nothing reaches
            shown: if goal {
                graph::reachable(r)
            } else {
                vec![true; r.commits.len()]
            },
        };
        let lk = look(&w.repo, false);
        let lay = graph::layout(&w.repo, &lk.shown);
        let moving_l = self.motion[base].step(&w.repo, &lay, dt);
        let m = &self.motion[base];
        let ids: Vec<String> = w.repo.commits.iter().map(|c| c.id.clone()).collect();
        let pos = |i: usize| m.at.get(&ids[i]).copied().unwrap_or(lay[i]);
        graph::draw(
            s,
            t,
            w,
            &w.repo,
            &pos,
            &la,
            &lk,
            hover.as_deref(),
            &mut spots,
        );
        let mut moving = moving_l;
        if let (Some(o), Some(oa)) = (&w.origin, &oa) {
            let lk = look(o, true);
            let lay = graph::layout(o, &lk.shown);
            moving |= self.motion[base + 1].step(o, &lay, dt);
            let m = &self.motion[base + 1];
            let ids: Vec<String> = o.commits.iter().map(|c| c.id.clone()).collect();
            let pos = |i: usize| m.at.get(&ids[i]).copied().unwrap_or(lay[i]);
            let mut ospots = vec![];
            graph::draw(s, t, w, o, &pos, oa, &lk, hover.as_deref(), &mut ospots);
            if !goal {
                spots.extend(
                    ospots
                        .into_iter()
                        .filter(|x| matches!(x.4, Spot::Commit(..))),
                );
            }
        }
        if moving {
            self.animating = true;
        }
        if !goal {
            for (x, y, w, h, sp) in spots {
                self.hits.push((x, y, w, h, Hit::Spot(sp)));
            }
        }
    }

    fn draw_todo(&mut self, s: &mut Screen, t: &Theme, a: &Area) {
        let Some(todo) = self.world.todo.clone() else {
            return;
        };
        s.rect(a.x, a.y, a.w, a.h, 12.0, hex(t.panel), 1.0);
        outline(s, a.x, a.y, a.w, a.h, 12.0, 1.2, hex(t.line), 1.0);
        let fs = if a.w > 500.0 { 16.0 } else { 14.0 };
        s.text(
            "git-rebase-todo",
            a.x + 16.0,
            a.y + 22.0,
            fs * 0.85,
            hex(t.muted),
            1.0,
        );
        let lh = fs * 2.2;
        let mut y = a.y + 44.0;
        for (i, (c, act, _)) in todo.lines.iter().enumerate() {
            let sel = i == todo.sel;
            if sel {
                s.rect(a.x + 8.0, y, a.w - 16.0, lh - 4.0, 8.0, hex(t.accent), 0.16);
            }
            let word = match act {
                b's' => "squash",
                b'f' => "fixup",
                b'd' => "drop",
                _ => "pick",
            };
            let col = match act {
                b'd' => t.bad,
                b's' | b'f' => t.warn,
                _ => t.code,
            };
            let k = &self.world.repo.commits[*c];
            let alpha = if *act == b'd' { 0.5 } else { 1.0 };
            s.text(word, a.x + 20.0, y + lh / 2.0 - 2.0, fs, hex(col), 1.0);
            s.text(
                &k.id,
                a.x + 20.0 + fs * 4.4,
                y + lh / 2.0 - 2.0,
                fs,
                hex(t.muted),
                alpha,
            );
            s.text(
                &k.msg,
                a.x + 20.0 + fs * 7.6,
                y + lh / 2.0 - 2.0,
                fs,
                hex(t.ink),
                alpha,
            );
            self.hits
                .push((a.x + 8.0, y, a.w - 16.0, lh - 4.0, Hit::Todo(i)));
            y += lh;
        }
        let help = "Up, Down: pick a line.  P S F D: pick, squash, fixup, drop.  K, J: move it.  Enter: start.";
        let mut hy = a.y + a.h - 14.0;
        for l in wrap_plain(s, help, fs * 0.8, a.w - 32.0).iter().rev() {
            s.text(l, a.x + 16.0, hy, fs * 0.8, hex(t.muted), 1.0);
            hy -= fs * 1.2;
        }
    }

    fn draw_cleared(&mut self, s: &mut Screen, t: &Theme, a: &Area) {
        let w = a.w.min(420.0);
        let h = 170.0;
        let x = a.x + (a.w - w) / 2.0;
        let y = a.y + (a.h - h) / 2.0;
        s.shadow(x, y, w, h, 16.0, 18.0, 0, 0.25);
        s.rect(x, y, w, h, 16.0, hex(t.panel), 1.0);
        outline(s, x, y, w, h, 16.0, 2.0, hex(t.accent), 1.0);
        let title = if self.s.phase == 2 {
            tr(&[
                "All lessons done",
                "全部完成",
                "全レッスン完了",
                "모든 레슨 완료",
                "Todas las lecciones hechas",
                "Todas as lições feitas",
                "Toutes les leçons faites",
                "Alle Lektionen geschafft",
                "Все уроки пройдены",
            ])
        } else {
            tr(&[
                "Level cleared",
                "关卡完成",
                "レベルクリア",
                "레벨 클리어",
                "Nivel superado",
                "Nível concluído",
                "Niveau réussi",
                "Level geschafft",
                "Уровень пройден",
            ])
        };
        s.text_c(
            title,
            x + w / 2.0,
            y + 38.0,
            s.fit_size(title, 26.0, w - 30.0),
            hex(t.ink),
            1.0,
        );
        let score_l = t_score();
        let cmd_l = tr(&[
            "Commands",
            "命令",
            "コマンド",
            "명령",
            "Comandos",
            "Comandos",
            "Commandes",
            "Befehle",
            "Команды",
        ]);
        let line = format!(
            "{score_l} +{}    {cmd_l} {} / {}",
            self.s.cleared_score,
            self.s.moves,
            self.par()
        );
        s.text_c(&line, x + w / 2.0, y + 76.0, 16.0, hex(t.muted), 1.0);
        if self.s.phase == 1 {
            let label = tr(&[
                "Next level",
                "下一关",
                "次のレベル",
                "다음 레벨",
                "Siguiente nivel",
                "Próximo nível",
                "Niveau suivant",
                "Nächstes Level",
                "Следующий уровень",
            ]);
            self.button(
                s,
                t,
                x + w / 2.0 - 100.0,
                y + 106.0,
                200.0,
                44.0,
                label,
                Hit::Btn(Btn::Next),
                true,
            );
        }
    }

    /// The console: output, the command line, and chips; returns its top.
    fn draw_console(
        &mut self,
        s: &mut Screen,
        t: &Theme,
        a: &Area,
        fs: f32,
        log_lines: usize,
        tall: bool,
    ) {
        let mut chips: Vec<(String, Hit)> = vec![];
        if self.world.todo.is_some() {
            for (k, n) in [
                (b'p', "pick"),
                (b's', "squash"),
                (b'f', "fixup"),
                (b'd', "drop"),
            ] {
                chips.push((n.to_string(), Hit::Btn(Btn::Act(k))));
            }
            chips.push((
                tr(&[
                    "Move up",
                    "上移",
                    "上へ",
                    "위로",
                    "Subir",
                    "Subir",
                    "Monter",
                    "Hoch",
                    "Вверх",
                ])
                .into(),
                Hit::Btn(Btn::Up),
            ));
            chips.push((
                tr(&[
                    "Move down",
                    "下移",
                    "下へ",
                    "아래로",
                    "Bajar",
                    "Descer",
                    "Descendre",
                    "Runter",
                    "Вниз",
                ])
                .into(),
                Hit::Btn(Btn::Down),
            ));
            chips.push((
                tr(&[
                    "Abort",
                    "取消",
                    "中止",
                    "중단",
                    "Cancelar",
                    "Cancelar",
                    "Abandonner",
                    "Abbrechen",
                    "Отмена",
                ])
                .into(),
                Hit::Btn(Btn::Abort),
            ));
            chips.push((
                tr(&[
                    "Start",
                    "开始",
                    "開始",
                    "시작",
                    "Empezar",
                    "Começar",
                    "Lancer",
                    "Starten",
                    "Начать",
                ])
                .into(),
                Hit::Btn(Btn::Start),
            ));
        } else if self.s.phase == 0 {
            if tall {
                chips.push((
                    tr(&[
                        "Hint",
                        "提示",
                        "ヒント",
                        "힌트",
                        "Pista",
                        "Dica",
                        "Indice",
                        "Tipp",
                        "Подсказка",
                    ])
                    .into(),
                    Hit::Btn(Btn::Hint),
                ));
                chips.push((
                    tr(&[
                        "Undo",
                        "撤销",
                        "元に戻す",
                        "실행 취소",
                        "Deshacer",
                        "Desfazer",
                        "Annuler",
                        "Rückgängig",
                        "Отменить",
                    ])
                    .into(),
                    Hit::Btn(Btn::Undo),
                ));
            }
            for c in LEVELS[self.lvl()].chips {
                chips.push((c.to_string(), Hit::Chip(c.to_string())));
            }
        }
        // chips wrap from the bottom up
        let ch = if tall { 34.0 } else { 34.0 };
        let cfs = 14.0;
        let mut rows: Vec<Vec<(f32, String, Hit)>> = vec![vec![]];
        let mut x = 0.0;
        for (label, hit) in chips {
            let w = s.text_width(&label, cfs) + 22.0;
            if x > 0.0 && x + w > a.w {
                rows.push(vec![]);
                x = 0.0;
            }
            rows.last_mut().unwrap().push((x, label, hit));
            x += w + 8.0;
        }
        let rows_h = if rows[0].is_empty() {
            0.0
        } else {
            rows.len() as f32 * (ch + 8.0)
        };
        let mut y = a.y + a.h - rows_h;
        for row in rows {
            for (x, label, hit) in row {
                let w = s.text_width(&label, cfs) + 22.0;
                let strong = matches!(hit, Hit::Btn(Btn::Start));
                self.chip_draw(s, t, a.x + x, y, w, ch, &label, hit, strong);
            }
            y += ch + 8.0;
        }
        // the command line
        let ih = 46.0;
        let iy = a.y + a.h - rows_h - ih - 4.0;
        s.rect(a.x, iy, a.w, ih, 10.0, hex(t.panel), 1.0);
        outline(
            s,
            a.x,
            iy,
            a.w,
            ih,
            10.0,
            1.5,
            hex(if self.world.todo.is_none() && self.s.phase == 0 {
                t.accent
            } else {
                t.line
            }),
            1.0,
        );
        let run = tr(&[
            "Run",
            "运行",
            "実行",
            "실행",
            "Ejecutar",
            "Executar",
            "Exécuter",
            "Ausführen",
            "Выполнить",
        ]);
        let rw = (s.text_width(run, 15.0) + 28.0).max(70.0);
        let bw = 46.0;
        self.button(
            s,
            t,
            a.x + a.w - rw - 5.0,
            iy + 5.0,
            rw,
            ih - 10.0,
            run,
            Hit::Btn(Btn::Run),
            true,
        );
        let bx = a.x + a.w - rw - bw - 10.0;
        self.button(
            s,
            t,
            bx,
            iy + 5.0,
            bw,
            ih - 10.0,
            "",
            Hit::Btn(Btn::Back),
            false,
        );
        // a backspace key, drawn
        let (cx, cy) = (bx + bw / 2.0, iy + ih / 2.0);
        s.poly(
            &[
                (cx - 12.0, cy),
                (cx - 5.0, cy - 8.0),
                (cx + 11.0, cy - 8.0),
                (cx + 11.0, cy + 8.0),
                (cx - 5.0, cy + 8.0),
            ],
            hex(t.ink),
            0.85,
        );
        s.line(
            cx - 1.0,
            cy - 4.0,
            cx + 6.0,
            cy + 4.0,
            1.4,
            hex(t.panel),
            1.0,
        );
        s.line(
            cx + 6.0,
            cy - 4.0,
            cx - 1.0,
            cy + 4.0,
            1.4,
            hex(t.panel),
            1.0,
        );
        let r = &self.world.repo;
        let path = r
            .wts
            .get(r.cur)
            .map(|w| w.path.clone())
            .unwrap_or_else(|| git::REPO.to_string());
        let head = match r.wts.get(r.cur).map(|w| &w.head) {
            Some(git::Head::Branch(b)) => format!(" ({b})"),
            Some(git::Head::Detached(c)) => format!(" ({}...)", r.commits[*c].id),
            None => String::new(),
        };
        let prompt = format!("~/{path}{head} $ ");
        let ifs = fs + 1.0;
        let px = a.x + 14.0;
        let pw = s.text_width(&prompt, ifs);
        let text_max = bx - 10.0 - px;
        let show_prompt = pw < text_max * 0.45;
        let mut tx = px;
        if show_prompt {
            s.text(&prompt, px, iy + ih / 2.0, ifs, hex(t.muted), 1.0);
            tx += pw;
        } else {
            s.text("$ ", px, iy + ih / 2.0, ifs, hex(t.muted), 1.0);
            tx += s.text_width("$ ", ifs);
        }
        let room = bx - 10.0 - tx;
        let text: String = self.input.iter().collect();
        let before: String = self.input[..self.cursor.min(self.input.len())]
            .iter()
            .collect();
        let caret_w = s.text_width(&before, ifs);
        let shift = if caret_w > room - 8.0 {
            caret_w - room + 8.0
        } else {
            0.0
        };
        s.set_clip(tx, iy, room, ih);
        if text.is_empty() && self.world.todo.is_none() && self.s.phase == 0 {
            let ph = tr(&[
                "Type a git command",
                "输入 git 命令",
                "git コマンドを入力",
                "git 명령 입력",
                "Escribe un comando git",
                "Digite um comando git",
                "Tapez une commande git",
                "Git-Befehl eingeben",
                "Введите команду git",
            ]);
            s.text(ph, tx, iy + ih / 2.0, ifs, hex(t.muted), 0.55);
        }
        s.text(&text, tx - shift, iy + ih / 2.0, ifs, hex(t.ink), 1.0);
        if (self.ms / 530) % 2 == 0 && self.s.phase == 0 && self.world.todo.is_none() {
            s.rect(
                tx - shift + caret_w,
                iy + ih / 2.0 - ifs * 0.7,
                2.0,
                ifs * 1.4,
                1.0,
                hex(t.accent),
                1.0,
            );
        }
        s.no_clip();
        // the output above it
        let lh = fs * 1.42;
        let top = iy - 8.0 - log_lines as f32 * lh;
        let mut shown: Vec<(u8, String)> = vec![];
        for (kind, line) in self.log.iter().rev() {
            let text = if *kind == CMD {
                format!("$ {line}")
            } else {
                line.clone()
            };
            let ls = if text.is_empty() {
                vec![String::new()]
            } else {
                wrap_plain(s, &text.replace('\t', "    "), fs, a.w - 8.0)
            };
            for l in ls.into_iter().rev() {
                shown.push((*kind, l));
            }
            if shown.len() >= log_lines {
                break;
            }
        }
        shown.truncate(log_lines);
        let mut y = top + lh * (log_lines as f32 - 0.5);
        for (kind, line) in shown {
            let c = match kind {
                ERR | FAIL => t.bad,
                GOOD => t.good,
                HINT => t.warn,
                CMD => t.ink,
                _ => t.muted,
            };
            let x = if line.starts_with(' ') {
                a.x + 4.0
            } else {
                a.x + 4.0
            };
            s.text(
                &line,
                x,
                y,
                fs,
                hex(c),
                if kind == CMD { 1.0 } else { 0.95 },
            );
            y -= lh;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn chip_draw(
        &mut self,
        s: &mut Screen,
        t: &Theme,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        label: &str,
        hit: Hit,
        strong: bool,
    ) {
        let hov = self.hover.as_ref() == Some(&hit);
        let code = matches!(hit, Hit::Chip(_));
        let fill = if strong {
            hex(t.accent)
        } else if hov {
            wg::mix(hex(t.panel), hex(t.accent), 0.15)
        } else {
            hex(t.panel)
        };
        s.rect(x, y, w, h, h / 2.0, fill, 1.0);
        outline(
            s,
            x,
            y,
            w,
            h,
            h / 2.0,
            1.2,
            hex(if strong { t.accent } else { t.line }),
            1.0,
        );
        let ink = if strong {
            0xffffff
        } else if code {
            t.code
        } else {
            t.ink
        };
        s.text_c(label, x + w / 2.0, y + h / 2.0, 14.0, hex(ink), 1.0);
        self.hits.push((x, y, w, h, hit));
    }

    fn draw_wide(&mut self, s: &mut Screen, t: &Theme) {
        self.animating = false;
        let (pw, ph) = (s.pw, s.ph);
        let lw = 340.0;
        // left: the lesson, the goal and the help buttons
        s.rect(0.0, 0.0, lw + 8.0, ph, 0.0, hex(t.panel), 0.6);
        s.line(lw + 8.0, 0.0, lw + 8.0, ph, 0.8, hex(t.line), 1.0);
        let goal_h = (ph * 0.36).clamp(170.0, 300.0);
        let by = ph - 62.0;
        let gy = by - 12.0 - goal_h;
        self.draw_lesson(
            s,
            t,
            &Area {
                x: 24.0,
                y: 20.0,
                w: lw - 28.0,
                h: gy - 30.0,
            },
            27.0,
            16.5,
        );
        let ga = Area {
            x: 20.0,
            y: gy,
            w: lw - 20.0,
            h: goal_h,
        };
        s.rect(ga.x, ga.y, ga.w, ga.h, 12.0, hex(t.bg), 1.0);
        outline(s, ga.x, ga.y, ga.w, ga.h, 12.0, 1.2, hex(t.line), 1.0);
        s.text(t_goal(), ga.x + 12.0, ga.y + 16.0, 13.0, hex(t.accent), 1.0);
        let fy = Self::draw_files(
            s,
            t,
            &self.goal,
            ga.x + ga.w - 190.0,
            ga.y + 8.0,
            182.0,
            true,
        );
        let gtop = if self.goal.uses_files {
            fy + 4.0
        } else {
            ga.y + 26.0
        };
        self.draw_world(
            s,
            t,
            true,
            &Area {
                x: ga.x + 6.0,
                y: gtop,
                w: ga.w - 12.0,
                h: ga.y + ga.h - gtop - 6.0,
            },
            false,
            true,
        );
        let hint = tr(&[
            "Hint",
            "提示",
            "ヒント",
            "힌트",
            "Pista",
            "Dica",
            "Indice",
            "Tipp",
            "Подсказка",
        ]);
        let undo = tr(&[
            "Undo",
            "撤销",
            "元に戻す",
            "실행 취소",
            "Deshacer",
            "Desfazer",
            "Annuler",
            "Rückgängig",
            "Отменить",
        ]);
        let bw = (lw - 20.0 - 10.0) / 2.0;
        self.button(s, t, 20.0, by, bw, 42.0, hint, Hit::Btn(Btn::Hint), false);
        self.button(
            s,
            t,
            30.0 + bw,
            by,
            bw,
            42.0,
            undo,
            Hit::Btn(Btn::Undo),
            false,
        );
        // right: the graph, then the console
        let rx = lw + 30.0;
        let rw = pw - rx - 22.0;
        let console_h = (ph * 0.42).clamp(250.0, 330.0);
        let ca = Area {
            x: rx,
            y: ph - console_h - 14.0,
            w: rw,
            h: console_h,
        };
        let gra = Area {
            x: rx,
            y: 16.0,
            w: rw,
            h: ca.y - 24.0,
        };
        let score = format!("{} {}", t_score(), self.s.score);
        s.text_r(&score, pw - 24.0, 28.0, 18.0, hex(t.muted), 1.0);
        if self.world.todo.is_some() {
            self.draw_todo(s, t, &gra);
        } else {
            self.draw_world(s, t, false, &gra, false, false);
            Self::draw_files(s, t, &self.world, gra.x, gra.y + 4.0, 220.0, false);
            if self.s.phase > 0 {
                self.draw_cleared(s, t, &gra);
            }
        }
        let lines = ((console_h - 46.0 - 2.0 * 42.0 - 20.0) / (15.0 * 1.42))
            .floor()
            .clamp(3.0, 8.0) as usize;
        self.draw_console(s, t, &ca, 15.0, lines, false);
    }

    fn draw_tall(&mut self, s: &mut Screen, t: &Theme) {
        self.animating = false;
        let (pw, ph) = (s.pw, s.ph);
        let m = 16.0;
        let text_h = (ph * 0.27).clamp(150.0, 260.0);
        let end = self.draw_lesson(
            s,
            t,
            &Area {
                x: m,
                y: 12.0,
                w: pw - 2.0 * m,
                h: text_h,
            },
            22.0,
            15.0,
        );
        let console_h = (ph * 0.36).clamp(230.0, 320.0);
        let ca = Area {
            x: m,
            y: ph - console_h - 10.0,
            w: pw - 2.0 * m,
            h: console_h,
        };
        let gy = end.max(12.0 + text_h * 0.5);
        let gra = Area {
            x: m,
            y: gy,
            w: pw - 2.0 * m,
            h: ca.y - gy - 8.0,
        };
        s.rect(
            gra.x - 6.0,
            gra.y - 4.0,
            gra.w + 12.0,
            gra.h + 8.0,
            12.0,
            hex(t.panel),
            0.55,
        );
        if self.world.todo.is_some() {
            self.draw_todo(s, t, &gra);
        } else {
            let goal = self.show_goal;
            let inner = Area {
                x: gra.x,
                y: gra.y + 34.0,
                w: gra.w,
                h: gra.h - 34.0,
            };
            let w = if goal { &self.goal } else { &self.world };
            let fy = Self::draw_files(s, t, w, gra.x + 4.0, gra.y + 36.0, 230.0, true);
            let inner = if w.uses_files {
                Area {
                    x: inner.x,
                    y: fy + 4.0,
                    w: inner.w,
                    h: inner.y + inner.h - fy - 4.0,
                }
            } else {
                inner
            };
            self.draw_world(s, t, goal, &inner, true, false);
            let label = if goal {
                tr(&[
                    "Show yours",
                    "查看你的",
                    "自分のを見る",
                    "내 것 보기",
                    "Ver el tuyo",
                    "Ver o seu",
                    "Voir le vôtre",
                    "Deins zeigen",
                    "Показать свой",
                ])
            } else {
                tr(&[
                    "Show goal",
                    "查看目标",
                    "ゴールを見る",
                    "목표 보기",
                    "Ver objetivo",
                    "Ver objetivo",
                    "Voir l'objectif",
                    "Ziel zeigen",
                    "Показать цель",
                ])
            };
            let bw = s.text_width(label, 14.0) + 28.0;
            self.button(
                s,
                t,
                gra.x + gra.w - bw - 4.0,
                gra.y + 2.0,
                bw,
                30.0,
                label,
                Hit::Btn(Btn::Goal),
                goal,
            );
            if goal {
                s.text(
                    t_goal(),
                    gra.x + 6.0,
                    gra.y + 17.0,
                    14.0,
                    hex(t.accent),
                    1.0,
                );
            } else {
                s.text(
                    &format!("{} {}", t_score(), self.s.score),
                    gra.x + 6.0,
                    gra.y + 17.0,
                    14.0,
                    hex(t.muted),
                    1.0,
                );
            }
            if self.s.phase > 0 {
                self.draw_cleared(s, t, &gra);
            }
        }
        self.draw_console(s, t, &ca, 13.5, 4, true);
    }
}

wg::game!(Branchwise);
