//! Hive Guard - an original wasmga.me game. MIT licensed; the dog, the
//! bees, the hive and the trees are cartoons drawn by art/draw.py with
//! games-src/tools/cartoon.
//! A dog sits near a beehive. Draw one line with the ink you have: when
//! you let go it drops as a solid bar, and the bees pour out and fly at
//! the dog. Keep every bee off the dog until the time runs out. 30 levels.
mod art;
mod glyphs;
mod levels;
mod world;

use levels::{Level, Rect, LEVELS};
use wg::{hex, mix, tr, Game, Image, Input, Pointer, Screen};
use world::{Bar, Swarm};

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
    /// the line being drawn, and its length
    pts: Vec<(f32, f32)>,
    ink: f32,
    drawing: bool,
    bar: Option<Bar>,
    swarm: Swarm,
    /// ms since the bees were let out, and physics time owed
    clock: f32,
    owed: f32,
    /// the keyboard pen, when it was used last
    pen: Option<(f32, f32)>,
    hover_button: bool,
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
        self.restart();
    }
    /// The same level again, counted as a retry once a line went down.
    fn retry(&mut self) {
        if !self.pts.is_empty() || self.s.phase != DRAW {
            self.s.retries += 1;
        }
        self.restart();
    }
    /// The level from its start: no line, no bees.
    fn restart(&mut self) {
        self.s.phase = DRAW;
        self.pts.clear();
        self.ink = 0.0;
        self.drawing = false;
        self.bar = None;
        self.swarm = Swarm::new(self.s.level as u32);
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
            let left = self.lv.ink - self.ink;
            if left <= 0.0 {
                return;
            }
            let k = STEP.min(left) / d;
            let next = (at.0 + dx * k, at.1 + dy * k);
            if !self.clear(at, next) {
                return;
            }
            self.ink += STEP.min(left);
            self.pts.push(next);
            at = next;
        }
    }

    /// Lets the line drop and the bees out.
    fn release(&mut self) -> u32 {
        self.drawing = false;
        if self.s.phase != DRAW || self.pts.len() < 2 {
            self.pts.clear();
            self.ink = 0.0;
            return 0;
        }
        self.bar = Some(Bar::new(&self.pts));
        self.s.lines += 1;
        self.s.phase = SWARM;
        wg::EV_BLIP
    }

    fn clear_level(&mut self) -> u32 {
        let s = &mut self.s;
        // more points for ink left over and no retries
        let left = 1.0 - self.ink / self.lv.ink;
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
            if let Some(bar) = &mut self.bar {
                bar.step(lv);
            }
            if self.swarm.step(lv, self.bar.as_mut(), self.clock) {
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
        HiveGuard {
            s: blank(),
            lv: levels::level(0),
            sprites,
            pts: Vec::new(),
            ink: 0.0,
            drawing: false,
            bar: None,
            swarm: Swarm::new(0),
            clock: 0.0,
            owed: 0.0,
            pen: None,
            hover_button: false,
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
        if matches!(self.s.phase, DRAW | SWARM) && (self.s.phase == SWARM || !self.pts.is_empty()) {
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
            ev |= self.release();
        }
        if self.s.phase != DRAW && self.s.phase != SWARM && input.pressed & wg::A != 0 {
            ev |= self.press_button();
        }
        ev
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        let mut ev = 0;
        self.hover_button = !p.touch && p.inside && self.on_button(p.x, p.y);
        if p.pressed & wg::M_LEFT != 0 {
            self.pen = None;
            if self.on_button(p.x, p.y) {
                return self.press_button();
            }
            if self.s.phase == DRAW && self.pts.is_empty() {
                self.drawing = true;
                self.draw_to((p.x, p.y));
            }
        } else if self.drawing && self.pen.is_none() {
            if !p.inside {
                // a cancelled touch takes back the line
                self.restart();
            } else if p.held & wg::M_LEFT != 0 {
                self.draw_to((p.x, p.y));
            } else {
                ev |= self.release();
            }
        }
        ev
    }

    fn picture(&mut self) -> u32 {
        let shown = [self.hover_button as i32, self.pts.len() as i32, self.pen.map_or(-1, |p| (p.0 as i32) << 12 | p.1 as i32)];
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
        match &self.bar {
            Some(bar) => line(s, &bar.points().collect::<Vec<_>>()),
            None => line(s, &self.pts),
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
        for (dy, col, a) in [(1.2, 0x1d2a44, 0.45), (0.0, 0xffffff, 1.0)] {
            s.text(&format!("{} {}", level_l, self.s.level + 1), 14.0, 20.0 + dy, 17.0, hex(col), a);
        }
        if self.s.phase == DRAW {
            let (bx, by, bw, bh) = (w - 124.0, 13.0, 110.0, 14.0);
            let left = 1.0 - self.ink / lv.ink;
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

/// The drawn line: a dark bar with a lighter edge on top.
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
        let used = (100.0 * self.ink / self.lv.ink).round() as i32;
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

    fn run(g: &mut HiveGuard, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    /// Draws a dome over the dog, as a player would: an arch from the ground
    /// on one side to the other, skipping what is in the way.
    fn dome(g: &mut HiveGuard) {
        let (dx, dy, _) = g.lv.dog;
        let r = levels::DOME_R;
        let n = 80;
        for k in 0..=n {
            let a = std::f32::consts::PI * (1.0 + k as f32 / n as f32);
            let p = (dx + r * a.cos(), dy - world::LINE_R - 1.0 + r * a.sin());
            if g.pts.is_empty() {
                g.drawing = true;
            }
            g.draw_to(p);
            if g.pts.is_empty() {
                g.draw_to(p);
            }
        }
    }

    #[test]
    fn a_dome_saves_the_dog_on_every_level() {
        let mut g = HiveGuard::new();
        g.start(1);
        for level in 0..LEVELS {
            assert_eq!(g.s.level, level);
            dome(&mut g);
            assert!(g.pts.len() > 10, "level {}: dome not drawn", level + 1);
            assert_eq!(g.release(), wg::EV_BLIP);
            let ms = (g.lv.secs * 1000.0) as i32 + 200;
            let ev = run(&mut g, ms);
            assert!(ev & wg::EV_LEVEL != 0, "level {}: stung (ink {:.0} of {:.0})", level + 1, g.ink, g.lv.ink);
            if level + 1 < LEVELS {
                g.press_button();
            }
        }
        assert!(g.over());
    }

    #[test]
    fn no_line_gets_the_dog_stung() {
        let mut g = HiveGuard::new();
        g.start(1);
        g.pts = vec![(20.0, 60.0), (30.0, 60.0)];
        g.release();
        assert!(run(&mut g, 15000) & wg::EV_LOSE != 0);
        assert_eq!(g.s.phase, STUNG);
        g.press_button();
        assert_eq!((g.s.phase, g.s.retries), (DRAW, 1));
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
        assert!(g.ink <= g.lv.ink + 0.01);
    }
}
