//! Hive Guard - an original wasmga.me game. MIT licensed; the dog, the
//! bees, the hive and the trees are cartoons drawn by art/draw.py with
//! games-src/tools/cartoon.
//! A dog sits near a beehive. Draw lines with the ink you have, then let
//! the bees out: the ink drops as solid bars, and the bees pour out, find
//! their way to the dog and shove whatever is in it. Keep every bee off
//! the dog until the time runs out. 30 levels.
mod art;
mod glyphs;
mod levels;
mod world;

use levels::{Level, Rect, LEVELS};
use wg::{hex, mix, tr, Game, Image, Input, Pointer, Screen};
use world::{Bar, Swarm};

/// the buttons in the top bar
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Undo,
    Restart,
    Go,
}

const VERSION: u32 = 1;

/// drawing the line
const DRAW: i32 = 0;
/// the bees are out
const SWARM: i32 = 1;
const CLEARED: i32 = 2;
const DONE: i32 = 3;
/// a bee got the dog
const STUNG: i32 = 4;

const PLAN: (f32, f32) = (300.0, 400.0);
/// a drawn point every this many plan units
const STEP: f32 = 4.5;
/// the keyboard pen's speed, plan units a second
const PEN_SPEED: f32 = 130.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 1],
    level: i32,
    phase: i32,
    retries: i32,
    level_ms: i32,
    time_ms: i32,
    all_stung: i32,
    lines: i32,
    done: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}

struct HiveGuard {
    s: Save,
    lv: Level,
    sprites: Vec<Image>,
    /// the lines drawn so far with the ink each took, and the line being
    /// drawn with its ink
    lines: Vec<(Vec<(f32, f32)>, f32)>,
    pts: Vec<(f32, f32)>,
    stroke: f32,
    drawing: bool,
    /// the ink once the bees are out, the lines that touch as one bar
    bars: Vec<Bar>,
    swarm: Swarm,
    /// ms since the bees were let out, and physics time owed
    clock: f32,
    owed: f32,
    /// the keyboard pen, when it was used last
    pen: Option<(f32, f32)>,
    hover_button: bool,
    hover_tool: Option<Tool>,
}

impl HiveGuard {
    fn card(&self) -> (f32, f32, f32, f32) {
        let (w, h) = PLAN;
        let (pw, ph) = (248.0, if self.s.phase == DONE { 96.0 } else { 158.0 });
        (w / 2.0 - pw / 2.0, h * 0.36 - ph / 2.0, pw, ph)
    }
    fn button(&self) -> (f32, f32, f32, f32) {
        let (px, py, pw, ph) = self.card();
        let (bw, bh) = (pw - 48.0, 42.0);
        (px + (pw - bw) / 2.0, py + ph - bh - 20.0, bw, bh)
    }
    fn on_button(&self, x: f32, y: f32) -> bool {
        let b = self.button();
        matches!(self.s.phase, CLEARED | STUNG) && x >= b.0 && x < b.0 + b.2 && y >= b.1 && y < b.1 + b.3
    }

    fn begin(&mut self, level: i32) {
        self.s.level = level;
        self.s.phase = DRAW;
        self.s.level_ms = 0;
        self.lv = levels::level(level);
        self.swarm = Swarm::new(&self.lv, level as u32);
        self.restart();
    }
    /// The top bar's buttons that are up now: what each does, its middle
    /// and radius, and whether it can be pressed.
    fn tools(&self) -> Vec<(Tool, f32, f32, f32, bool)> {
        let (w, y, r) = (PLAN.0, 21.0, 13.0);
        let ready = !self.drawing && !self.lines.is_empty();
        match self.s.phase {
            DRAW => vec![(Tool::Undo, w - 104.0, y, r, ready), (Tool::Restart, w - 72.0, y, r, ready), (Tool::Go, w - 32.0, y, 17.0, ready)],
            SWARM => vec![(Tool::Restart, w - 72.0, y, r, true)],
            _ => vec![],
        }
    }
    fn tool_at(&self, px: f32, py: f32) -> Option<Tool> {
        let pad = 4.0;
        self.tools().into_iter().find(|&(_, x, y, r, on)| on && (px - x).powi(2) + (py - y).powi(2) < (r + pad).powi(2)).map(|t| t.0)
    }
    fn press_tool(&mut self, t: Tool) -> u32 {
        match t {
            Tool::Undo => {
                self.lines.pop();
                wg::EV_BLIP
            }
            Tool::Restart => {
                self.retry();
                wg::EV_BLIP
            }
            Tool::Go => self.go(),
        }
    }

    /// The same level again, counted as a retry once the bees were out.
    fn retry(&mut self) {
        if self.s.phase != DRAW {
            self.s.retries += 1;
        }
        self.restart();
    }
    /// The level from its start: no line, no bees.
    fn restart(&mut self) {
        self.s.phase = DRAW;
        self.lines.clear();
        self.pts.clear();
        self.stroke = 0.0;
        self.drawing = false;
        self.bars.clear();
        self.swarm.reset(self.s.level as u32);
        self.clock = 0.0;
        self.owed = 0.0;
    }

    /// Whether the line may run from a to b: clear of the ground, the
    /// ledges, the hives and the dog, and inside the screen.
    fn clear(&self, a: (f32, f32), b: (f32, f32)) -> bool {
        let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let n = (len / 1.5).ceil().max(1.0) as i32;
        (0..=n).all(|k| {
            let t = k as f32 / n as f32;
            let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            p.0 > 2.0 && p.0 < PLAN.0 - 2.0 && p.1 > 46.0 && !self.lv.solid(p, world::LINE_R + 0.5)
        })
    }

    /// Extends the line toward p while ink lasts.
    fn draw_to(&mut self, p: (f32, f32)) {
        let Some(&last) = self.pts.last() else {
            if self.clear(p, p) {
                self.pts.push(p);
            }
            return;
        };
        let (dx, dy) = (p.0 - last.0, p.1 - last.1);
        let d = (dx * dx + dy * dy).sqrt();
        if d < STEP {
            return;
        }
        // a step at a time, so the line keeps an even spacing
        let steps = (d / STEP) as i32;
        let mut at = last;
        for _ in 0..steps {
            let left = self.lv.ink - self.ink();
            if left <= 0.0 {
                return;
            }
            let k = STEP.min(left) / d;
            let next = (at.0 + dx * k, at.1 + dy * k);
            if !self.clear(at, next) {
                return;
            }
            self.stroke += STEP.min(left);
            self.pts.push(next);
            at = next;
        }
    }

    /// All the ink used: the lines drawn and the one being drawn.
    fn ink(&self) -> f32 {
        self.lines.iter().map(|l| l.1).sum::<f32>() + self.stroke
    }

    /// Ends the line being drawn: kept if it is more than a dot.
    fn release(&mut self) {
        if self.s.phase == DRAW && self.pts.len() >= 2 {
            self.lines.push((std::mem::take(&mut self.pts), self.stroke));
        }
        self.drop_stroke();
    }
    /// Takes back the line being drawn.
    fn drop_stroke(&mut self) {
        self.pts.clear();
        self.stroke = 0.0;
        self.drawing = false;
    }

    /// Lets the ink drop and the bees out. Lines that touch drop as one.
    fn go(&mut self) -> u32 {
        if self.s.phase != DRAW || self.drawing || self.lines.is_empty() {
            return 0;
        }
        let n = self.lines.len();
        let mut group: Vec<usize> = (0..n).collect();
        let touch = 2.0 * world::LINE_R + 1.0;
        for i in 0..n {
            for j in 0..i {
                let (a, b) = (&self.lines[i].0, &self.lines[j].0);
                let near = a.iter().any(|&p| b.windows(2).any(|w| world::nearest(w[0], w[1], p).1 < touch))
                    || b.iter().any(|&p| a.windows(2).any(|w| world::nearest(w[0], w[1], p).1 < touch));
                if near {
                    // everything in i's group joins j's
                    let (from, to) = (group[i], group[j]);
                    group.iter_mut().filter(|g| **g == from).for_each(|g| *g = to);
                }
            }
        }
        self.bars = (0..n)
            .filter(|&g| group.contains(&g))
            .map(|g| Bar::new(&(0..n).filter(|&i| group[i] == g).map(|i| &self.lines[i].0[..]).collect::<Vec<_>>()))
            .collect();
        self.s.lines += n as i32;
        self.s.phase = SWARM;
        wg::EV_BLIP
    }

    fn clear_level(&mut self) -> u32 {
        // more points for ink left over and no retries
        let left = 1.0 - self.ink() / self.lv.ink;
        let s = &mut self.s;
        let score = ((100.0 + 100.0 * left) as i32 - 30 * s.retries).max(30);
        s.score += score;
        s.cleared = s.level;
        s.cleared_score = score;
        s.done += 1;
        if s.level + 1 >= LEVELS {
            s.phase = DONE;
            return wg::EV_LEVEL | wg::EV_WIN | wg::EV_GAMEOVER;
        }
        s.phase = CLEARED;
        wg::EV_LEVEL | wg::EV_WIN
    }

    fn press_button(&mut self) -> u32 {
        match self.s.phase {
            CLEARED => {
                self.s.retries = 0;
                self.begin(self.s.level + 1);
            }
            STUNG => self.retry(),
            _ => return 0,
        }
        wg::EV_BLIP
    }

    /// Runs the bar and the bees in fixed steps.
    fn simulate(&mut self, dt: f32) -> u32 {
        self.clock += dt;
        self.owed = (self.owed + dt).min(100.0);
        let lv = &self.lv;
        while self.owed >= world::STEP_MS {
            self.owed -= world::STEP_MS;
            world::step_bars(lv, &mut self.bars);
            if self.swarm.step(lv, &mut self.bars, self.clock) {
                self.s.phase = STUNG;
                self.s.all_stung += 1;
                return wg::EV_LOSE;
            }
        }
        if self.clock >= lv.secs * 1000.0 {
            return self.clear_level();
        }
        0
    }

    /// Draws sprite `i` with its model's origin at (x, y), `u` plan units
    /// to a model unit.
    fn put(&self, s: &mut Screen, i: usize, x: f32, y: f32, u: f32, flip: bool, alpha: f32) {
        let a = &art::ARTS[i];
        let k = u / a.ppu;
        let (w, h) = (a.w as f32 * k, a.h as f32 * k);
        if flip {
            s.image(&self.sprites[i], x - (a.w as f32 - a.ax) * k, y - a.ay * k, -w, h, alpha);
        } else {
            s.image(&self.sprites[i], x - a.ax * k, y - a.ay * k, w, h, alpha);
        }
    }
}

impl Game for HiveGuard {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let sheet = Image::png(art::SHEET);
        let sprites = art::ARTS.iter().map(|a| sheet.crop(a.x, a.y, a.w, a.h)).collect();
        let lv = levels::level(0);
        HiveGuard {
            s: blank(),
            swarm: Swarm::new(&lv, 0),
            lv,
            sprites,
            lines: Vec::new(),
            pts: Vec::new(),
            stroke: 0.0,
            drawing: false,
            bars: Vec::new(),
            clock: 0.0,
            owed: 0.0,
            pen: None,
            hover_button: false,
            hover_tool: None,
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
        self.s = Save { opt, ..blank() };
        self.begin(opt[0]);
        self.pen = None;
    }
    fn plan(&self, _w: i32, _h: i32) -> (f32, f32) {
        PLAN
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        if self.s.phase == SWARM {
            ev |= self.simulate(dt);
        }
        if self.s.phase == SWARM || (self.s.phase == DRAW && (self.drawing || !self.lines.is_empty())) {
            self.s.level_ms = self.s.level_ms.saturating_add(input.dt);
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        // the keyboard pen: arrows move it, holding A draws
        let dirs = input.held & (wg::LEFT | wg::RIGHT | wg::UP | wg::DOWN);
        if self.s.phase == DRAW && (dirs != 0 || input.pressed & wg::A != 0) {
            let pen = self.pen.get_or_insert((self.lv.dog.0 - 60.0, self.lv.dog.1 - 70.0));
            let k = PEN_SPEED * dt / 1000.0;
            let mv = |b: u32| if dirs & b != 0 { k } else { 0.0 };
            pen.0 = (pen.0 + mv(wg::RIGHT) - mv(wg::LEFT)).clamp(4.0, PLAN.0 - 4.0);
            pen.1 = (pen.1 + mv(wg::DOWN) - mv(wg::UP)).clamp(50.0, PLAN.1 - 4.0);
            let pen = *pen;
            if input.held & wg::A != 0 {
                self.drawing = true;
                self.draw_to(pen);
            }
        }
        if self.s.phase == DRAW && self.drawing && self.pen.is_some() && input.held & wg::A == 0 {
            self.release();
        }
        // B lets the bees out
        if input.pressed & wg::B != 0 {
            ev |= self.go();
        }
        if self.s.phase != DRAW && self.s.phase != SWARM && input.pressed & wg::A != 0 {
            ev |= self.press_button();
        }
        ev
    }

    /// Backspace or U takes back a line, R starts the level over.
    fn key(&mut self, code: i32) -> bool {
        let tool = match code {
            8 | 0x75 | 0x55 => Tool::Undo,
            0x72 | 0x52 => Tool::Restart,
            _ => return false,
        };
        if self.tools().iter().any(|t| t.0 == tool && t.4) {
            self.press_tool(tool);
        }
        true
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        self.hover_button = !p.touch && p.inside && self.on_button(p.x, p.y);
        self.hover_tool = if !p.touch && p.inside { self.tool_at(p.x, p.y) } else { None };
        if p.pressed & wg::M_LEFT != 0 {
            self.pen = None;
            if self.on_button(p.x, p.y) {
                return self.press_button();
            }
            if let Some(t) = self.tool_at(p.x, p.y) {
                return self.press_tool(t);
            }
            if self.s.phase == DRAW && !self.drawing {
                self.drawing = true;
                self.draw_to((p.x, p.y));
            }
        } else if self.drawing && self.pen.is_none() {
            if !p.inside {
                // a cancelled touch takes back the line
                self.drop_stroke();
            } else if p.held & wg::M_LEFT != 0 {
                self.draw_to((p.x, p.y));
            } else {
                self.release();
            }
        }
        0
    }

    fn picture(&mut self) -> u32 {
        let tool = self.hover_tool.map_or(-1, |t| t as i32);
        let shown = [self.hover_button as i32, tool, self.lines.len() as i32, self.pts.len() as i32, self.pen.map_or(-1, |p| (p.0 as i32) << 12 | p.1 as i32)];
        let mut c = self.s;
        c.level_ms = 0;
        c.time_ms = 0;
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        self.s.phase == SWARM
    }

    fn draw(&mut self, s: &mut Screen) {
        let (w, _) = PLAN;
        let key = wg::hash(0, wg::bytes_of(&[self.s.level]));
        s.background(key, |s| self.draw_scene(s));
        let lv = &self.lv;

        // the hives, the dog, the line and the bees
        for &(hx, hy) in &lv.hives {
            self.put(s, art::HIVE, hx, hy, levels::HIVE_U, false, 1.0);
        }
        let (dx, dy, flip) = lv.dog;
        let dog = match self.s.phase {
            STUNG => art::DOG_SAD,
            CLEARED | DONE => art::DOG_HAPPY,
            SWARM if (self.clock / 260.0) as i32 % 2 == 1 => art::DOG_BREATHE,
            _ => art::DOG_IDLE,
        };
        self.put(s, dog, dx, dy, levels::DOG_U, flip, 1.0);
        if self.bars.is_empty() {
            for pts in self.lines.iter().map(|l| &l.0[..]).chain([&self.pts[..]]) {
                line(s, pts);
            }
        }
        for pts in self.bars.iter().flat_map(Bar::strokes) {
            line(s, pts);
        }
        for b in &self.swarm.bees {
            let a = &art::ARTS[art::BEE_A];
            let k = levels::BEE_H / a.h as f32;
            let frame = if (b.age / 40.0) as i32 % 2 == 0 { art::BEE_A } else { art::BEE_B };
            let (bw, bh) = (a.w as f32 * k, a.h as f32 * k);
            let flip = b.v.0 < 0.0;
            s.image(&self.sprites[frame], b.p.0 - bw / 2.0, b.p.1 - bh / 2.0, if flip { -bw } else { bw }, bh, 1.0);
        }
        if let Some(p) = self.pen.filter(|_| self.s.phase == DRAW) {
            s.disc(p.0, p.1, 5.0, hex(0x1d2330), 0.8);
            s.disc(p.0, p.1, 3.0, hex(0xffe066), 1.0);
        }

        // the top bar: the level, then the ink while drawing or the time left
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let level_t = format!("{} {}", level_l, self.s.level + 1);
        for (dy, col, a) in [(1.2, 0x1d2a44, 0.45), (0.0, 0xffffff, 1.0)] {
            s.text(&level_t, 14.0, 20.0 + dy, 17.0, hex(col), a);
        }
        if self.s.phase == DRAW {
            // the ink gauge between the level and the buttons
            let bx = (s.text_width(&level_t, 17.0) + 24.0).max(92.0);
            let (by, bw, bh) = (14.0, w - 122.0 - bx, 14.0);
            let left = 1.0 - self.ink() / lv.ink;
            s.rect(bx, by, bw, bh, bh / 2.0, hex(0xffffff), 0.6);
            if left > 0.0 {
                s.rect(bx + 2.0, by + 2.0, (bw - 4.0) * left, bh - 4.0, (bh - 4.0) / 2.0, hex(0x2b2b33), 1.0);
                s.rect(bx + 2.0, by + 2.0, (bw - 4.0) * left, (bh - 4.0) / 2.0, (bh - 4.0) / 4.0, hex(0x6b6b78), 0.6);
            }
        } else {
            let secs = ((lv.secs * 1000.0 - self.clock) / 1000.0).ceil().max(0.0) as i32;
            for (dy, col, a) in [(1.5, 0x1d2a44, 0.45), (0.0, 0xffffff, 1.0)] {
                s.text_c(&secs.to_string(), w / 2.0, 66.0 + dy, 34.0, hex(col), a);
            }
        }
        for (t, x, y, r, on) in self.tools() {
            tool_button(s, t, x, y, r, on, on && self.hover_tool == Some(t));
        }
        if !matches!(self.s.phase, DRAW | SWARM) {
            self.draw_card(s, w);
        }
    }

    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.phase == DONE
    }
    fn reached(&self) -> i32 {
        self.s.level
    }
    fn cleared(&self) -> Option<(i32, u32)> {
        (self.s.cleared >= 0).then(|| (self.s.cleared, self.s.cleared_score as u32))
    }
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        match i {
            0 => s.done,
            1 => s.all_stung,
            2 => s.lines,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION && (0..LEVELS).contains(&s.opt[0]) && (0..LEVELS).contains(&s.level) && (DRAW..=STUNG).contains(&s.phase);
        if ok {
            // the line and the bees are not saved: a level in play starts over
            self.lv = levels::level(self.s.level);
            self.swarm = Swarm::new(&self.lv, self.s.level as u32);
            let phase = self.s.phase;
            if matches!(phase, CLEARED | DONE) {
                self.restart();
                self.s.phase = phase;
            } else {
                self.retry();
            }
        }
        self.pen = None;
        ok
    }
}

/// A drawn line: a dark bar with a lighter edge on top.
fn line(s: &mut Screen, pts: &[(f32, f32)]) {
    if pts.len() == 1 {
        s.disc(pts[0].0, pts[0].1, world::LINE_R, hex(0x2b2b33), 1.0);
    }
    // shadow, bar, edge
    for (dy, r, col, a) in [(1.5, world::LINE_R, 0x000000, 0.2), (0.0, world::LINE_R, 0x2b2b33, 1.0), (-1.0, world::LINE_R * 0.35, 0x6b6b78, 0.7)] {
        for w in pts.windows(2) {
            s.line(w[0].0, w[0].1 + dy, w[1].0, w[1].1 + dy, r, hex(col), a);
        }
    }
}

/// A round top-bar button with its icon: a hooked arrow back for undo, a
/// circling arrow for restart, a play mark for go.
fn tool_button(s: &mut Screen, t: Tool, x: f32, y: f32, r: f32, on: bool, hover: bool) {
    use std::f32::consts::PI;
    let a = if on { 1.0 } else { 0.45 };
    let (fill, ink) = match t {
        Tool::Go => (0xffc928, 0x2a1810),
        _ => (0xffffff, 0x2b3a55),
    };
    let fill = if hover { mix(hex(0xffffff), hex(fill), 0.25) } else { hex(fill) };
    s.disc(x, y + 1.5, r, hex(0x1d2a44), 0.3 * a);
    s.disc(x, y, r, fill, 0.92 * a);
    let k = r / 13.0;
    let ink = hex(ink);
    let arc = |s: &mut Screen, (cx, cy): (f32, f32), from: f32, to: f32, rad: f32| {
        let n = 10;
        for i in 0..n {
            let (a0, a1) = (from + (to - from) * i as f32 / n as f32, from + (to - from) * (i + 1) as f32 / n as f32);
            s.line(cx + rad * a0.cos(), cy + rad * a0.sin(), cx + rad * a1.cos(), cy + rad * a1.sin(), 1.4 * k, ink, a);
        }
    };
    match t {
        Tool::Go => {
            let h = 7.5 * k;
            s.tri((x - h * 0.6, y - h), (x - h * 0.6, y + h), (x + h, y), ink, a);
        }
        Tool::Restart => {
            let rad = 6.0 * k;
            arc(s, (x, y), -PI * 0.35, PI * 1.45, rad);
            // the arrowhead at the arc's start, pointing along it
            let (ex, ey) = (x + rad * (-PI * 0.35_f32).cos(), y + rad * (-PI * 0.35_f32).sin());
            s.tri((ex - 4.0 * k, ey - 3.6 * k), (ex + 3.0 * k, ey - 3.0 * k), (ex + 0.4 * k, ey + 3.6 * k), ink, a);
        }
        Tool::Undo => {
            // a hook from the right curving back over to the left, arrowhead there
            let (cx, cy, rad) = (x + 1.0 * k, y + 1.0 * k, 5.0 * k);
            arc(s, (cx, cy), -PI * 0.5, PI * 0.5, rad);
            s.line(cx, cy + rad, cx - 4.0 * k, cy + rad, 1.4 * k, ink, a);
            s.line(cx, cy - rad, cx - 3.0 * k, cy - rad, 1.4 * k, ink, a);
            let (ax, ay) = (cx - 3.0 * k, cy - rad);
            s.tri((ax - 4.0 * k, ay), (ax + 0.5 * k, ay - 4.0 * k), (ax + 0.5 * k, ay + 4.0 * k), ink, a);
        }
    }
}

fn blank() -> Save {
    Save {
        version: VERSION,
        opt: [0],
        level: 0,
        phase: DRAW,
        retries: 0,
        level_ms: 0,
        time_ms: 0,
        all_stung: 0,
        lines: 0,
        done: 0,
        score: 0,
        cleared: -1,
        cleared_score: 0,
    }
}

impl HiveGuard {
    /// The sky, the hills and trees, the ground and the ledges: drawn once
    /// a level.
    fn draw_scene(&self, s: &mut Screen) {
        let lv = &self.lv;
        let (x0, y0, x1, _) = s.visible();
        s.rect_grad(x0, y0, x1 - x0, levels::GROUND - y0, 0.0, hex(0x7cc4ef), hex(0xdff3fb), 1.0);
        let mut rng = wg::Rng::new(53 + self.s.level as u32);
        for _ in 0..4 {
            let (cx, cy) = (x0 + rng.float() * (x1 - x0), 70.0 + rng.float() * 110.0);
            for k in 0..4 {
                let r = 11.0 + rng.float() * 9.0;
                s.disc(cx + k as f32 * 13.0 - 20.0, cy - (k % 2) as f32 * 5.0, r, hex(0xffffff), 0.75);
            }
        }
        for (i, c) in [(0, 0xa6d77f), (1, 0x8cc765)] {
            let mut x = x0 - 50.0 + i as f32 * 80.0;
            while x < x1 + 60.0 {
                s.ellipse(x, levels::GROUND + 4.0, 95.0, 70.0 - i as f32 * 22.0, hex(c), 1.0);
                x += 160.0;
            }
        }
        // trees on the skyline, behind everything
        let kinds = [art::TREE_OAK, art::TREE_CONE, art::TREE_DEFAULT];
        let mut x = x0 - 10.0 + rng.float() * 30.0;
        let mut k = self.s.level as usize;
        while x < x1 + 20.0 {
            self.put(s, kinds[k % 3], x, levels::GROUND + 2.0, 13.0 + rng.float() * 4.0, false, 0.9);
            x += 46.0 + rng.float() * 50.0;
            k += 1;
        }
        for r in &lv.solids {
            slab(s, r);
        }
        for &bx in &[x0 + 18.0, x1 - 22.0] {
            self.put(s, art::BUSHLARGE, bx, levels::GROUND + 3.0, 14.0, false, 1.0);
        }
    }

    fn draw_card(&self, s: &mut Screen, w: f32) {
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0x10141a), 0.35);
        let (px, py, pw, ph) = self.card();
        s.shadow(px, py + 4.0, pw, ph, 16.0, 14.0, 0, 0.45);
        s.rect(px, py, pw, ph, 16.0, hex(0x2f3540), 1.0);
        let stung = self.s.phase == STUNG;
        let title = if stung {
            tr(&["Stung!", "被蜇了！", "刺された！", "쏘였어요!", "¡Picado!", "Picado!", "Piqué !", "Gestochen!", "Ужален!", "被螫了！"])
        } else {
            tr(&["Level cleared", "过关", "クリア", "레벨 클리어", "Nivel superado", "Nível concluído", "Niveau réussi", "Level geschafft", "Уровень пройден", "過關"])
        };
        let size = s.fit_size(title, 21.0, pw - 32.0);
        s.text_c(title, w / 2.0, py + 34.0, size, hex(0xffffff), 1.0);
        let ink_l = tr(&["Ink used", "用墨", "使ったインク", "사용한 잉크", "Tinta usada", "Tinta usada", "Encre utilisée", "Tinte verbraucht", "Чернил ушло", "用墨"]);
        let used = (100.0 * self.ink() / self.lv.ink).round() as i32;
        let line = if stung {
            format!("{} {}%", ink_l, used)
        } else {
            let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
            format!("{} +{}    {} {}%", score_l, self.s.cleared_score, ink_l, used)
        };
        let size = s.fit_size(&line, 14.0, pw - 32.0);
        s.text_c(&line, w / 2.0, py + 64.0, size, hex(0xb9c2cf), 1.0);
        if self.s.phase != DONE {
            let b = self.button();
            let label = if stung {
                tr(&["Try again", "再试一次", "もう一度", "다시 하기", "Reintentar", "Tentar de novo", "Réessayer", "Nochmal", "Ещё раз", "再試一次"])
            } else {
                tr(&["Next level", "下一关", "次のレベル", "다음 레벨", "Siguiente nivel", "Próximo nível", "Niveau suivant", "Nächstes Level", "Следующий уровень", "下一關"])
            };
            let fill = hex(0x3d7be0);
            let fill = if self.hover_button { mix(hex(0xffffff), fill, 0.15) } else { fill };
            s.rect(b.0, b.1, b.2, b.3, b.3 / 2.0, fill, 1.0);
            let size = s.fit_size(label, 16.0, b.2 - 16.0);
            s.text_c(label, b.0 + b.2 / 2.0, b.1 + b.3 / 2.0, size, hex(0xffffff), 1.0);
        }
    }
}

/// Ground or a ledge: earth with grass on top.
fn slab(s: &mut Screen, r: &Rect) {
    let grass = 6.0_f32.min(r.h);
    s.rect(r.x, r.y + 2.0, r.w, r.h, 4.0, hex(0x000000), 0.15);
    s.rect_grad(r.x, r.y, r.w, r.h, 4.0, hex(0xa0703f), hex(0x6e4a28), 1.0);
    // stones in the earth
    let mut rng = wg::Rng::new((r.x as i32 * 31 + r.y as i32) as u32);
    for _ in 0..(r.w * r.h / 260.0) as i32 {
        let (x, y) = (r.x + 4.0 + rng.float() * (r.w - 8.0), r.y + grass + 3.0 + rng.float() * (r.h - grass - 6.0).max(0.0));
        s.ellipse(x, y, 2.0 + rng.float() * 2.5, 1.5 + rng.float() * 1.5, hex(0x5a3b20), 0.5);
    }
    s.rect(r.x, r.y, r.w, grass, 4.0, hex(0x6fbf45), 1.0);
    s.rect(r.x, r.y, r.w, grass * 0.4, 3.0, hex(0x95d96a), 1.0);
}

wg::game!(HiveGuard);

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn run(g: &mut HiveGuard, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    /// Draws one line through `pts` as a player would, skipping what is in the way.
    fn stroke(g: &mut HiveGuard, pts: impl IntoIterator<Item = (f32, f32)>) {
        g.drawing = true;
        for p in pts {
            g.draw_to(p);
        }
        g.release();
    }

    /// An arch around (x, y) from the ground on one side to the other.
    fn arch(rx: f32, ry: f32, x: f32, y: f32) -> impl Iterator<Item = (f32, f32)> {
        (0..=80).map(move |k| {
            let a = PI * (1.0 + k as f32 / 80.0);
            (x + rx * a.cos(), y - world::LINE_R - 1.0 + ry * a.sin())
        })
    }

    /// Lets the bees out and runs the level to its end: whether it was cleared.
    fn play(g: &mut HiveGuard) -> bool {
        assert_eq!(g.go(), wg::EV_BLIP);
        let ms = (g.lv.secs * 1000.0) as i32 + 200;
        run(g, ms) & wg::EV_LEVEL != 0
    }

    #[test]
    fn a_dome_saves_the_dog_on_every_level() {
        let mut g = HiveGuard::new();
        g.start(1);
        for level in 0..LEVELS {
            assert_eq!(g.s.level, level);
            let (dx, dy, _) = g.lv.dog;
            stroke(&mut g, arch(levels::DOME_R, levels::DOME_R, dx, dy));
            assert!(g.lines[0].0.len() > 10, "level {}: dome not drawn", level + 1);
            assert!(play(&mut g), "level {}: stung (ink {:.0} of {:.0})", level + 1, g.ink(), g.lv.ink);
            if level + 1 < LEVELS {
                g.press_button();
            }
        }
        assert!(g.over());
    }

    #[test]
    fn bees_shove_a_cap_off_the_hive() {
        let mut g = HiveGuard::new();
        for level in 0..LEVELS {
            g.s.opt[0] = level;
            g.start(1);
            for (hx, hy) in g.lv.hives.clone() {
                stroke(&mut g, arch(36.0, 60.0, hx, hy));
            }
            assert!(!play(&mut g), "level {}: a cap of {:.0} ink held", level + 1, g.ink());
        }
    }

    /// Counts, over several swarms and shapes, the domes that let a bee
    /// through and the caps that held: `cargo test tune -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn tune() {
        let (mut domes, mut caps, mut runs) = (0, 0, 0);
        let mut g = HiveGuard::new();
        for level in 0..LEVELS {
            for seed in 0..4u32 {
                runs += 1;
                g.s.opt[0] = level;
                g.start(1);
                let (dx, dy, _) = g.lv.dog;
                let r = levels::DOME_R + [0.0, 3.0, -2.0, 5.0][seed as usize];
                stroke(&mut g, arch(r, r, dx, dy));
                g.swarm = world::Swarm::new(&g.lv, 1000 + seed + 7 * level as u32);
                if !play(&mut g) {
                    domes += 1;
                    eprintln!("dome stung: level {} seed {seed}", level + 1);
                }
                g.start(1);
                for (hx, hy) in g.lv.hives.clone() {
                    stroke(&mut g, arch(36.0 + seed as f32 * 3.0, 60.0 + seed as f32 * 2.0, hx, hy));
                }
                g.swarm = world::Swarm::new(&g.lv, 2000 + seed + 7 * level as u32);
                if play(&mut g) {
                    caps += 1;
                    eprintln!("cap held: level {} seed {seed} ink {:.0}", level + 1, g.ink());
                }
            }
        }
        eprintln!("TUNE domes stung {domes}/{runs}, caps held {caps}/{runs}");
    }

    #[test]
    fn bees_fly_around_a_wall() {
        // the hive up on its ledge, walled off toward the dog and roofed over
        let mut g = HiveGuard::new();
        g.s.opt[0] = 1;
        g.start(1);
        let (hx, hy) = g.lv.hives[0];
        stroke(&mut g, [(hx - 30.0, hy - 6.0), (hx - 30.0, hy - 60.0), (hx + 40.0, hy - 60.0)]);
        assert!(!play(&mut g));
    }

    #[test]
    fn no_line_gets_the_dog_stung() {
        let mut g = HiveGuard::new();
        g.start(1);
        stroke(&mut g, [(20.0, 60.0), (30.0, 60.0)]);
        assert!(run(&mut g, 15000) & wg::EV_LOSE == 0, "the bees wait for go");
        g.go();
        assert!(run(&mut g, 15000) & wg::EV_LOSE != 0);
        assert_eq!(g.s.phase, STUNG);
        g.press_button();
        assert_eq!((g.s.phase, g.s.retries), (DRAW, 1));
    }

    #[test]
    fn lines_share_the_ink_undo_and_join() {
        let mut g = HiveGuard::new();
        g.start(1);
        stroke(&mut g, [(40.0, 100.0), (80.0, 100.0)]);
        let one = g.ink();
        stroke(&mut g, [(80.0, 104.0), (80.0, 140.0)]);
        stroke(&mut g, [(200.0, 100.0), (240.0, 100.0)]);
        assert_eq!(g.lines.len(), 3);
        g.press_tool(Tool::Undo);
        assert_eq!(g.lines.len(), 2);
        assert!((g.ink() - one - 36.0).abs() < 5.0, "{} {}", g.ink(), one);
        stroke(&mut g, [(200.0, 100.0), (240.0, 100.0)]);
        g.go();
        // the two that touch drop as one
        assert_eq!(g.bars.len(), 2);
    }

    #[test]
    fn the_line_stops_at_ledges_and_runs_out_of_ink() {
        let mut g = HiveGuard::new();
        g.start(1);
        // straight down into the ground: stops short of it
        g.drawing = true;
        g.draw_to((150.0, 300.0));
        g.draw_to((150.0, 390.0));
        assert!(g.pts.last().unwrap().1 < levels::GROUND - world::LINE_R);
        // a long scribble: never more than the ink
        for k in 0..400 {
            g.draw_to((40.0 + (k % 2) as f32 * 200.0, 100.0 + k as f32 * 0.1));
        }
        assert!(g.ink() <= g.lv.ink + 0.01);
    }
}
