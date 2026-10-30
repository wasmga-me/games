//! Pinwork - an original wasmga.me game. MIT licensed; the knight, the orc,
//! the coin and the props are Kenney's Mini Dungeon (CC0), rendered by
//! games-src/tools/render3d.
//! Pins hold gold, lava and water in a dungeon's chambers. Pull them in the
//! right order: lava burns the knight and kills the orc, water turns lava
//! to stone, and the gold has to reach the knight. 24 levels.
mod art;
mod glyphs;
mod levels;
mod sim;

use levels::{Level, GOLD, GROUND, LAVA, LEVELS, STONE, WATER};
use sim::World;
use wg::{hex, mix, tr, Game, Image, Input, Pointer, Screen};

const VERSION: u32 = 1;

const PLAY: i32 = 0;
const CLEARED: i32 = 1;
const DONE: i32 = 2;
const FAILED: i32 = 3;

/// why a level failed
const BURNED: i32 = 0;
const CAUGHT: i32 = 1;
const ORC_LIVES: i32 = 2;
const SHORT: i32 = 3;

const PLAN: (f32, f32) = (300.0, 400.0);
/// plan units to a model unit for people and props
const PERSON_U: f32 = 52.0;
/// ms at rest before a level is judged, and the longest wait after a pull
const SETTLE_MS: f32 = 400.0;
const WAIT_MS: f32 = 9000.0;
/// how near a tap must be to a pin, plan units
const REACH: f32 = 14.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 1],
    level: i32,
    phase: i32,
    why: i32,
    retries: i32,
    level_ms: i32,
    time_ms: i32,
    pulled: i32,
    gold: i32,
    done: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}

struct Pinwork {
    s: Save,
    lv: Level,
    world: World,
    sprites: Vec<Image>,
    /// physics time owed, ms at rest, and ms since the last pull
    owed: f32,
    rest: f32,
    since_pull: f32,
    /// the pin the keys point at, when keys were used last
    chosen: Option<usize>,
    hover_pin: Option<usize>,
    hover_button: bool,
}

impl Pinwork {
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
        matches!(self.s.phase, CLEARED | FAILED) && x >= b.0 && x < b.0 + b.2 && y >= b.1 && y < b.1 + b.3
    }

    fn begin(&mut self, level: i32) {
        self.s.level = level;
        self.s.level_ms = 0;
        self.lv = levels::level(level);
        self.restart();
    }
    /// The same level again, counted as a retry once a pin was pulled.
    fn retry(&mut self) {
        if self.world.pins.iter().any(|p| p.pulled) || self.s.phase != PLAY {
            self.s.retries += 1;
        }
        self.restart();
    }
    /// The level from its start.
    fn restart(&mut self) {
        self.s.phase = PLAY;
        self.world = World::new(&self.lv);
        self.owed = 0.0;
        self.rest = 0.0;
        self.since_pull = 0.0;
        self.chosen = self.chosen.map(|_| 0);
    }

    /// The pin nearest (x, y) still in, if near enough.
    fn pin_at(&self, x: f32, y: f32) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for (i, pin) in self.world.pins.iter().enumerate() {
            if pin.pulled {
                continue;
            }
            let (a, b) = pin.now();
            let d = sim::capsule((x, y), 0.0, a, b, REACH).map_or(REACH, |(_, depth)| REACH - depth);
            let ring = ((x - b.0).powi(2) + (y - b.1).powi(2)).sqrt() - 6.0;
            let d = d.min(ring.max(0.0));
            if d < REACH && best.is_none_or(|bb| d < bb.0) {
                best = Some((d, i));
            }
        }
        best.map(|b| b.1)
    }

    fn pull(&mut self, i: usize) -> u32 {
        if self.s.phase != PLAY || !self.world.pull(i) {
            return 0;
        }
        self.s.pulled += 1;
        self.since_pull = 0.0;
        self.rest = 0.0;
        if self.chosen.is_some() {
            self.chosen = self.next_pin(i, 1);
        }
        wg::EV_BLIP
    }
    /// the next pin still in from i, going `dir`
    fn next_pin(&self, i: usize, dir: i32) -> Option<usize> {
        let n = self.world.pins.len() as i32;
        (1..=n).map(|k| ((i as i32 + dir * k).rem_euclid(n)) as usize).find(|&j| !self.world.pins[j].pulled)
    }

    /// the gold that has reached the knight
    fn gold_in(&self) -> usize {
        let (x0, y0, x1, y1) = self.lv.zone;
        self.world.grains.iter().filter(|g| g.kind == GOLD && g.p.0 > x0 && g.p.0 < x1 && g.p.1 > y0 && g.p.1 < y1).count()
    }
    fn orc_down(&self) -> bool {
        self.world.orc.is_none_or(|o| !o.alive)
    }

    /// Runs the physics, then sees whether the level is won or lost.
    fn simulate(&mut self, dt: f32) -> u32 {
        self.owed = (self.owed + dt).min(100.0);
        while self.owed >= sim::STEP_MS {
            self.owed -= sim::STEP_MS;
            // nothing moves once settled, until the next pull
            if self.rest < SETTLE_MS {
                self.world.step(&self.lv);
            }
        }
        self.since_pull += dt;
        self.rest = if self.world.resting() { self.rest + dt } else { 0.0 };
        if self.world.burned {
            return self.fail(BURNED);
        }
        if self.world.caught {
            return self.fail(CAUGHT);
        }
        if self.rest < SETTLE_MS && self.since_pull < WAIT_MS {
            return 0;
        }
        let gold = self.gold_in();
        if gold >= self.lv.need && self.orc_down() {
            return self.clear_level(gold);
        }
        if self.world.pins.iter().all(|p| p.pulled) {
            return self.fail(if self.orc_down() { SHORT } else { ORC_LIVES });
        }
        0
    }

    fn fail(&mut self, why: i32) -> u32 {
        self.s.phase = FAILED;
        self.s.why = why;
        wg::EV_LOSE
    }

    fn clear_level(&mut self, gold: usize) -> u32 {
        let s = &mut self.s;
        // more points for gold past what was needed, none for retries
        let score = (100 + 3 * (gold - self.lv.need) as i32 - 30 * s.retries).max(30);
        s.gold += gold as i32;
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
            FAILED => self.retry(),
            _ => return 0,
        }
        wg::EV_BLIP
    }

    /// Draws sprite `i` with its model's origin at (x, y), mirrored when
    /// `flip`.
    fn put(&self, s: &mut Screen, i: usize, x: f32, y: f32, u: f32, flip: bool) {
        let a = &art::ARTS[i];
        let k = u / a.ppu;
        let (w, h) = (a.w as f32 * k, a.h as f32 * k);
        if flip {
            s.image(&self.sprites[i], x - (a.w as f32 - a.ax) * k, y - a.ay * k, -w, h, 1.0);
        } else {
            s.image(&self.sprites[i], x - a.ax * k, y - a.ay * k, w, h, 1.0);
        }
    }
}

impl Game for Pinwork {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let sheet = Image::png(art::SHEET);
        let sprites = art::ARTS.iter().map(|a| sheet.crop(a.x, a.y, a.w, a.h)).collect();
        let lv = levels::level(0);
        let world = World::new(&lv);
        Pinwork {
            s: blank(),
            lv,
            world,
            sprites,
            owed: 0.0,
            rest: 0.0,
            since_pull: 0.0,
            chosen: None,
            hover_pin: None,
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
        self.chosen = None;
        self.begin(opt[0]);
    }
    fn plan(&self, _w: i32, _h: i32) -> (f32, f32) {
        PLAN
    }

    fn update(&mut self, input: &Input) -> u32 {
        let mut ev = 0;
        if self.s.phase == PLAY {
            ev |= self.simulate(input.dt as f32);
            self.s.level_ms = self.s.level_ms.saturating_add(input.dt);
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        if self.s.phase == PLAY {
            // arrows choose a pin, A pulls it
            let dir = if input.pressed & (wg::RIGHT | wg::DOWN) != 0 {
                1
            } else if input.pressed & (wg::LEFT | wg::UP) != 0 {
                -1
            } else {
                0
            };
            if dir != 0 {
                let from = self.chosen.unwrap_or(if dir > 0 { self.world.pins.len() - 1 } else { 0 });
                self.chosen = self.next_pin(from, dir).or(self.chosen);
            }
            if input.pressed & wg::A != 0 {
                match self.chosen {
                    Some(i) => ev |= self.pull(i),
                    None => self.chosen = self.next_pin(self.world.pins.len() - 1, 1),
                }
            }
        } else if input.pressed & wg::A != 0 {
            ev |= self.press_button();
        }
        ev
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        self.hover_button = !p.touch && p.inside && self.on_button(p.x, p.y);
        self.hover_pin = if !p.touch && p.inside && self.s.phase == PLAY { self.pin_at(p.x, p.y) } else { None };
        if p.pressed & wg::M_LEFT != 0 {
            if self.on_button(p.x, p.y) {
                return self.press_button();
            }
            if let Some(i) = self.pin_at(p.x, p.y) {
                self.chosen = None;
                return self.pull(i);
            }
        }
        0
    }

    fn picture(&mut self) -> u32 {
        let shown = [self.hover_button as i32, self.hover_pin.map_or(-1, |i| i as i32), self.chosen.map_or(-1, |i| i as i32)];
        let mut c = self.s;
        c.level_ms = 0;
        c.time_ms = 0;
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        self.s.phase == PLAY && self.rest < SETTLE_MS
    }

    fn draw(&mut self, s: &mut Screen) {
        let (w, _) = PLAN;
        let key = wg::hash(0, wg::bytes_of(&[self.s.level]));
        s.background(key, |s| self.draw_scene(s));
        let lv = &self.lv;

        // the knight, the orc, the grains, then the pins over them
        let knight = lv.knight;
        let face_left = knight > w / 2.0;
        let pose = match (self.s.phase, self.s.why) {
            (CLEARED | DONE, _) => art::KNIGHT_CHEER,
            (FAILED, BURNED | CAUGHT) => art::KNIGHT_DOWN,
            (FAILED, _) => art::KNIGHT_WORRIED,
            _ if self.world.grains.iter().any(|g| g.kind == LAVA && g.p.1 > lv.zone.1 - 60.0) => art::KNIGHT_WORRIED,
            _ => art::KNIGHT_IDLE,
        };
        self.put(s, pose, knight, GROUND, PERSON_U, face_left);
        if let Some(o) = &self.world.orc {
            let sprite = if !o.alive {
                art::ORC_DOWN
            } else if self.world.caught {
                art::ORC_SWING
            } else {
                art::ORC_IDLE
            };
            self.put(s, sprite, o.p.0, o.p.1 + sim::ORC_UP, PERSON_U * 1.1, o.p.0 < knight);
        }
        draw_grains(s, &self.world);
        for (i, pin) in self.world.pins.iter().enumerate() {
            if pin.gone() {
                continue;
            }
            let lit = self.hover_pin == Some(i) || self.chosen == Some(i);
            draw_pin(s, pin.now(), lit, pin.out);
        }

        // the top bar: the level, and the gold that has reached the knight
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        for (dy, col, a) in [(1.2, 0x000000, 0.5), (0.0, 0xffffff, 1.0)] {
            s.text(&format!("{} {}", level_l, self.s.level + 1), 14.0, 22.0 + dy, 17.0, hex(col), a);
        }
        let gold = format!("{}/{}", self.gold_in().min(self.lv.need), self.lv.need);
        let tw = s.text_width(&gold, 16.0);
        let x = w - 14.0 - tw;
        coin(s, x - 12.0, 22.0, 7.0);
        for (dy, col, a) in [(1.2, 0x000000, 0.5), (0.0, 0xffe08a, 1.0)] {
            s.text(&gold, x, 22.0 + dy, 16.0, hex(col), a);
        }
        if self.s.phase != PLAY {
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
            1 => s.pulled,
            2 => s.gold,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION
            && (0..LEVELS).contains(&s.opt[0])
            && (0..LEVELS).contains(&s.level)
            && (PLAY..=FAILED).contains(&s.phase)
            && (BURNED..=SHORT).contains(&s.why);
        if ok {
            // the grains are not saved: a level in play starts over
            self.lv = levels::level(self.s.level);
            let phase = self.s.phase;
            if matches!(phase, CLEARED | DONE) {
                self.restart();
                self.s.phase = phase;
            } else {
                self.retry();
            }
        }
        self.chosen = None;
        ok
    }
}

fn blank() -> Save {
    Save {
        version: VERSION,
        opt: [0],
        level: 0,
        phase: PLAY,
        why: BURNED,
        retries: 0,
        level_ms: 0,
        time_ms: 0,
        pulled: 0,
        gold: 0,
        done: 0,
        score: 0,
        cleared: -1,
        cleared_score: 0,
    }
}

/// A gold coin of radius r.
fn coin(s: &mut Screen, x: f32, y: f32, r: f32) {
    s.disc(x, y, r, hex(0x9a6a12), 1.0);
    s.disc(x, y, r * 0.82, hex(0xf2b829), 1.0);
    s.disc(x - r * 0.15, y - r * 0.15, r * 0.5, hex(0xffd75e), 1.0);
    s.disc(x - r * 0.35, y - r * 0.35, r * 0.18, hex(0xfff4c2), 0.9);
}

/// The grains: lava glowing, water in a blue body, stones and coins.
fn draw_grains(s: &mut Screen, world: &World) {
    let r = levels::GRAIN_R;
    // the liquids' bodies first, so neighbours run together
    for g in &world.grains {
        match g.kind {
            LAVA => s.disc(g.p.0, g.p.1, r * 1.9, hex(0xff7a1a), 0.22),
            WATER => s.disc(g.p.0, g.p.1, r * 1.45, hex(0x2f8fe0), 0.85),
            _ => {}
        }
    }
    for g in &world.grains {
        match g.kind {
            LAVA => {
                s.disc(g.p.0, g.p.1, r * 1.3, hex(0xe8501a), 1.0);
                s.disc(g.p.0 - 0.8, g.p.1 - 0.8, r * 0.7, hex(0xffb347), 0.9);
            }
            WATER => s.disc(g.p.0 - 1.0, g.p.1 - 1.2, r * 0.45, hex(0xbfe4ff), 0.7),
            STONE => {
                s.disc(g.p.0, g.p.1, r * 1.1, hex(0x3a3d44), 1.0);
                s.disc(g.p.0, g.p.1, r * 0.92, hex(0x7c818b), 1.0);
                s.disc(g.p.0 - 1.0, g.p.1 - 1.0, r * 0.4, hex(0xa9aeb7), 0.9);
            }
            _ => coin(s, g.p.0, g.p.1, r * 1.2),
        }
    }
}

/// A pin: a steel rod with a ring at its handle end.
fn draw_pin(s: &mut Screen, (a, b): ((f32, f32), (f32, f32)), lit: bool, out: f32) {
    let rod = if lit { hex(0xffe08a) } else { hex(0xd9b45a) };
    s.line(a.0 + 1.0, a.1 + 2.0, b.0 + 1.0, b.1 + 2.0, levels::PIN_R, hex(0x000000), 0.3);
    s.line(a.0, a.1, b.0, b.1, levels::PIN_R, mix(hex(0x6b4f1d), rod, 0.35), 1.0);
    s.line(a.0, a.1 - 0.8, b.0, b.1 - 0.8, levels::PIN_R * 0.55, rod, 1.0);
    // the ring past the handle end
    let l = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt().max(1.0);
    let c = (b.0 + (b.0 - a.0) / l * 7.0, b.1 + (b.1 - a.1) / l * 7.0);
    s.ring(c.0, c.1 + 1.0, 7.5, 3.0, hex(0x000000), 0.3);
    s.ring(c.0, c.1, 7.5, 3.0, mix(hex(0x6b4f1d), rod, 0.35), 1.0);
    s.ring(c.0, c.1 - 0.5, 7.5, 1.4, rod, 1.0);
    if lit && out == 0.0 {
        s.ring(c.0, c.1, 11.5, 1.5, hex(0xffffff), 0.8);
    }
}

impl Pinwork {
    /// The dungeon: stone blocks behind, the frame, the walls, a chest by
    /// the knight and a few props: drawn once a level.
    fn draw_scene(&self, s: &mut Screen) {
        let lv = &self.lv;
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0x1d1f26), 1.0);
        let (l, r, top, floor) = (levels::FRAME_L, levels::FRAME_R, levels::FRAME_TOP, levels::FLOOR);
        // the back wall: big stone blocks, lighter toward the floor
        s.rect_grad(l, top, r - l, floor - top, 0.0, hex(0x30343e), hex(0x474b55), 1.0);
        let mut rng = wg::Rng::new(91 + self.s.level as u32);
        let (bw, bh) = (34.0f32, 20.0f32);
        let mut y = top;
        let mut row = 0;
        while y < floor {
            let mut x = l - if row % 2 == 1 { bw / 2.0 } else { 0.0 };
            while x < r {
                let (cx, cw) = (x.max(l) + 1.0, (x + bw).min(r) - x.max(l) - 2.0);
                if cw > 0.0 {
                    let c = mix(hex(0x50555f), hex(0x3b3f48), rng.float());
                    s.rect(cx, y + 1.0, cw, (bh - 2.0).min(floor - y - 1.0), 2.0, c, 0.55);
                }
                x += bw;
            }
            y += bh;
            row += 1;
        }
        // banners and props on the back wall, away from the knight
        let side = if lv.knight < 150.0 { 1.0 } else { -1.0 };
        self.put(s, art::BANNER, 150.0 - side * 112.0, 96.0, PERSON_U * 0.9, false);
        // a chest beside the knight, the props by the walls
        let chest_x = (lv.knight + side * 36.0).clamp(l + 18.0, r - 18.0);
        self.put(s, art::CHEST, chest_x, levels::GROUND, PERSON_U * 0.8, side < 0.0);
        // the walls: stone with a lit top edge
        for &(ax, ay, bx, by) in &lv.walls {
            s.line(ax + 1.0, ay + 2.5, bx + 1.0, by + 2.5, levels::WALL_R, hex(0x000000), 0.35);
            s.line(ax, ay, bx, by, levels::WALL_R + 1.2, hex(0x23252b), 1.0);
            s.line(ax, ay, bx, by, levels::WALL_R, hex(0x8a8577), 1.0);
            s.line(ax - 1.0, ay - 1.0, bx - 1.0, by - 1.0, levels::WALL_R * 0.35, hex(0xb9b3a2), 0.8);
        }
        // the floor below the frame
        s.rect(x0, levels::FLOOR + levels::WALL_R, x1 - x0, y1 - levels::FLOOR, 0.0, hex(0x2a2c33), 1.0);
    }

    fn draw_card(&self, s: &mut Screen, w: f32) {
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0x10141a), 0.35);
        let (px, py, pw, ph) = self.card();
        s.shadow(px, py + 4.0, pw, ph, 16.0, 14.0, 0, 0.45);
        s.rect(px, py, pw, ph, 16.0, hex(0x2f3540), 1.0);
        let failed = self.s.phase == FAILED;
        let title = if !failed {
            tr(&["Level cleared", "过关", "クリア", "레벨 클리어", "Nivel superado", "Nível concluído", "Niveau réussi", "Level geschafft", "Уровень пройден", "過關"])
        } else {
            match self.s.why {
                BURNED => tr(&["Burned by lava", "被岩浆烧到", "溶岩でやけど", "용암에 데었어요", "Quemado por lava", "Queimado pela lava", "Brûlé par la lave", "Von Lava verbrannt", "Обжёгся лавой", "被岩漿燒到"]),
                CAUGHT => tr(&["The orc got him", "被兽人抓住", "オークにやられた", "오크에게 당했어요", "El orco lo atrapó", "O orc o pegou", "L'orc l'a eu", "Der Ork hat ihn erwischt", "Орк его настиг", "被獸人抓住"]),
                ORC_LIVES => tr(&["The orc still stands", "兽人还活着", "オークが残っている", "오크가 아직 살아 있어요", "El orco sigue en pie", "O orc continua de pé", "L'orc est toujours debout", "Der Ork steht noch", "Орк ещё стоит", "獸人還活著"]),
                _ => tr(&["Not enough gold", "金币不够", "金貨が足りない", "금화가 부족해요", "No hay suficiente oro", "Ouro insuficiente", "Pas assez d'or", "Nicht genug Gold", "Мало золота", "金幣不夠"]),
            }
        };
        let size = s.fit_size(title, 21.0, pw - 32.0);
        s.text_c(title, w / 2.0, py + 34.0, size, hex(0xffffff), 1.0);
        let gold_l = tr(&["Gold", "金币", "金貨", "금화", "Oro", "Ouro", "Or", "Gold", "Золото", "金幣"]);
        let line = if failed {
            format!("{} {}/{}", gold_l, self.gold_in().min(self.lv.need), self.lv.need)
        } else {
            let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
            format!("{} +{}", score_l, self.s.cleared_score)
        };
        let size = s.fit_size(&line, 14.0, pw - 32.0);
        s.text_c(&line, w / 2.0, py + 64.0, size, hex(0xb9c2cf), 1.0);
        if self.s.phase != DONE {
            let b = self.button();
            let label = if failed {
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

wg::game!(Pinwork);

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays a level pulling pins in this order, each once all is at rest;
    /// the phase it ends in and why.
    fn play(level: i32, order: &[usize]) -> i32 {
        let mut g = Pinwork::new();
        g.start(1);
        g.begin(level);
        let step = Input { held: 0, pressed: 0, dt: 16 };
        // let the grains settle first
        for _ in 0..60 {
            g.update(&step);
        }
        for &i in order {
            if g.s.phase != PLAY {
                break;
            }
            g.pull(i);
            for _ in 0..700 {
                g.update(&step);
                if g.s.phase != PLAY || (g.rest > 200.0 && g.since_pull > 300.0) {
                    break;
                }
            }
        }
        for _ in 0..700 {
            if g.s.phase != PLAY {
                break;
            }
            g.update(&step);
        }
        g.s.phase
    }

    fn orders(n: usize) -> Vec<Vec<usize>> {
        // every order of every set of pins
        let mut out = vec![vec![]];
        let mut frontier = vec![vec![]];
        for _ in 0..n {
            let mut next = Vec::new();
            for o in &frontier {
                for i in 0..n {
                    if !o.contains(&i) {
                        let mut m: Vec<usize> = o.clone();
                        m.push(i);
                        next.push(m);
                    }
                }
            }
            out.extend(next.iter().cloned());
            frontier = next;
        }
        out
    }

    #[test]
    fn every_level_has_a_way_and_a_trap() {
        for level in 0..LEVELS {
            let n = levels::level(level).pins.len();
            let mut wins = Vec::new();
            let mut losses = 0;
            for o in orders(n).into_iter().filter(|o| !o.is_empty()) {
                let phase = play(level, &o);
                if matches!(phase, CLEARED | DONE) {
                    wins.push(o);
                } else if phase == FAILED {
                    losses += 1;
                }
            }
            println!("level {:2}: pins {} wins {:?} losses {}", level + 1, n, wins, losses);
            assert!(!wins.is_empty(), "level {} cannot be won", level + 1);
            if level % levels::LAYOUTS != 0 {
                assert!(losses > 0, "level {} cannot be lost", level + 1);
            }
        }
    }
}
