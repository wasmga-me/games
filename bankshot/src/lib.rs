//! Bankshot - an original wasmga.me game. MIT licensed; the people, the
//! blasters and the crates are Kenney's Mini Characters and Blaster Kit
//! (CC0), rendered by games-src/tools/render3d.
//! Gunmen hide behind walls and ledges. Aim the agent's shot, and it
//! bounces off the walls, the floor, the ceiling and steel plates. Hit
//! every gunman with the shots you have, and never a hostage. 30 levels.
mod art;
mod glyphs;
mod room;

use room::{dist, Path, Room, FLOOR, LEFT, LEVELS, RIGHT, TOP};
use wg::{hex, mix, tr, Game, Image, Input, Pointer, Screen};

const VERSION: u32 = 1;

const AIM: i32 = 0;
/// a shot on its way
const FLY: i32 = 1;
const CLEARED: i32 = 2;
const DONE: i32 = 3;
/// a hostage hit, or the shots ran out
const FAILED: i32 = 4;

const PLAN: (f32, f32) = (300.0, 400.0);
/// a shot's speed, plan units a second
const SPEED: f32 = 820.0;
/// the keyboard's turn, radians a second
const TURN: f32 = 1.1;
/// plan units to a model unit for people and crates
const PERSON_U: f32 = 58.0;
/// how far past the first bounce the aim line shows
const PEEK: f32 = 46.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 1],
    level: i32,
    phase: i32,
    retries: i32,
    /// shots left, foes down as bits, and the hostage hit (or -1)
    shots: i32,
    down: u32,
    hurt: i32,
    level_ms: i32,
    time_ms: i32,
    fired: i32,
    bounces: i32,
    done: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
    /// the aim, radians
    aim: f32,
}
// SAFETY: #[repr(C)], integers, floats and Plain types only
unsafe impl wg::Plain for Save {}

struct Bankshot {
    s: Save,
    room: Room,
    sprites: Vec<Image>,
    /// where the aim goes now, and the shot in the air with how far it got
    preview: Path,
    shot: Path,
    flown: f32,
    /// the pointer is down and aiming
    aiming: bool,
    /// ms since the agent fired, for the recoil
    since_fire: f32,
    hover_button: bool,
    /// where the pointer was last
    hover: (f32, f32),
}

impl Bankshot {
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
        self.room = room::level(level);
        self.restart();
    }
    /// The same level again, counted as a retry once a shot was fired.
    fn retry(&mut self) {
        if self.s.shots < self.room.shots || self.s.phase != AIM {
            self.s.retries += 1;
        }
        self.restart();
    }
    /// The level from its start, aimed across the room.
    fn restart(&mut self) {
        self.s.phase = AIM;
        self.s.shots = self.room.shots;
        self.s.down = 0;
        self.s.hurt = -1;
        self.s.aim = if self.room.agent.0 < RIGHT / 2.0 { -0.35 } else { std::f32::consts::PI + 0.35 };
        self.shot = Path::default();
        self.flown = 0.0;
        self.aiming = false;
        self.aim_at(self.s.aim);
    }

    fn aim_at(&mut self, a: f32) {
        self.s.aim = a;
        self.preview = self.room.trace(a);
    }
    fn aim_to(&mut self, x: f32, y: f32) {
        let g = self.room.gun();
        if (x - g.0).abs() + (y - g.1).abs() > 4.0 {
            self.aim_at((y - g.1).atan2(x - g.0));
        }
    }

    fn fire(&mut self) -> u32 {
        self.aiming = false;
        if self.s.phase != AIM || self.s.shots <= 0 {
            return 0;
        }
        self.shot = self.room.trace(self.s.aim);
        self.flown = 0.0;
        self.since_fire = 0.0;
        self.s.shots -= 1;
        self.s.fired += 1;
        self.s.bounces += self.shot.bounces() as i32;
        self.s.phase = FLY;
        wg::EV_BLIP
    }

    /// Moves the shot on; foes it passes go down.
    fn fly(&mut self, dt: f32) -> u32 {
        let mut ev = 0;
        self.flown = (self.flown + SPEED * dt / 1000.0).min(self.shot.len);
        for &(i, d) in &self.shot.foes {
            if d <= self.flown && self.s.down & 1 << i == 0 {
                self.s.down |= 1 << i;
                ev |= wg::EV_HIT;
            }
        }
        if self.flown < self.shot.len {
            return ev;
        }
        if let Some(h) = self.shot.hostage {
            self.s.hurt = h as i32;
            self.s.phase = FAILED;
            return ev | wg::EV_LOSE;
        }
        if self.s.down == self.room.all_foes() {
            return ev | self.clear_level();
        }
        if self.s.shots == 0 {
            self.s.phase = FAILED;
            return ev | wg::EV_LOSE;
        }
        self.s.phase = AIM;
        self.aim_at(self.s.aim);
        ev
    }

    fn clear_level(&mut self) -> u32 {
        let s = &mut self.s;
        // more points for shots to spare and no retries
        let score = (100 + 50 * s.shots - 30 * s.retries).max(30);
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

    /// Draws sprite `i` with its model's origin at (x, y), facing left
    /// when `flip`.
    fn put(&self, s: &mut Screen, i: usize, x: f32, y: f32, flip: bool, alpha: f32) {
        let a = &art::ARTS[i];
        let k = PERSON_U / a.ppu;
        let (w, h) = (a.w as f32 * k, a.h as f32 * k);
        if flip {
            s.image(&self.sprites[i], x - (a.w as f32 - a.ax) * k, y - a.ay * k, -w, h, alpha);
        } else {
            s.image(&self.sprites[i], x - a.ax * k, y - a.ay * k, w, h, alpha);
        }
    }
}

impl Game for Bankshot {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let sheet = Image::png(art::SHEET);
        let sprites = art::ARTS.iter().map(|a| sheet.crop(a.x, a.y, a.w, a.h)).collect();
        let mut g = Bankshot {
            s: blank(),
            room: Room::empty(),
            sprites,
            preview: Path::default(),
            shot: Path::default(),
            flown: 0.0,
            aiming: false,
            since_fire: 1e9,
            hover_button: false,
            hover: (-1.0, -1.0),
        };
        g.retry();
        g
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
    }
    fn plan(&self, _w: i32, _h: i32) -> (f32, f32) {
        PLAN
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        self.since_fire += dt;
        if self.s.phase == FLY {
            ev |= self.fly(dt);
        }
        if matches!(self.s.phase, AIM | FLY) {
            self.s.level_ms = self.s.level_ms.saturating_add(input.dt);
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        if self.s.phase == AIM {
            // left and up turn the aim one way, right and down the other
            let turn = |b: u32| if input.held & b != 0 { TURN * dt / 1000.0 } else { 0.0 };
            let d = turn(wg::RIGHT) + turn(wg::DOWN) - turn(wg::LEFT) - turn(wg::UP);
            if d != 0.0 {
                self.aim_at(self.s.aim + d);
            }
            if input.pressed & wg::A != 0 {
                return ev | self.fire();
            }
        }
        if matches!(self.s.phase, CLEARED | FAILED | DONE) && input.pressed & wg::A != 0 {
            ev |= self.press_button();
        }
        ev
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        self.hover_button = !p.touch && p.inside && self.on_button(p.x, p.y);
        if p.pressed & wg::M_LEFT != 0 {
            if self.on_button(p.x, p.y) {
                return self.press_button();
            }
            if self.s.phase == AIM {
                self.aiming = true;
                self.aim_to(p.x, p.y);
            }
        } else if self.aiming {
            // a touch the browser cancelled ends off the screen: no shot
            if !p.inside {
                self.aiming = false;
                return 0;
            }
            self.aim_to(p.x, p.y);
            if p.held & wg::M_LEFT == 0 {
                return self.fire();
            }
        } else if self.s.phase == AIM && !p.touch && p.inside && (p.x, p.y) != self.hover {
            // a mouse aims as it moves; the click fires
            self.aim_to(p.x, p.y);
        }
        self.hover = (p.x, p.y);
        0
    }

    fn picture(&mut self) -> u32 {
        let shown = [self.hover_button as i32, self.aiming as i32];
        let mut c = self.s;
        c.level_ms = 0;
        c.time_ms = 0;
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        self.s.phase == FLY || self.since_fire < 200.0
    }

    fn draw(&mut self, s: &mut Screen) {
        let (w, _) = PLAN;
        let key = wg::hash(0, wg::bytes_of(&[self.s.level]));
        s.background(key, |s| draw_scene(s, self));
        let room = &self.room;

        // the aim: dots to the first bounce and a little past it
        if self.s.phase == AIM {
            let first = if self.preview.pts.len() > 1 { dist(self.preview.pts[0], self.preview.pts[1]) } else { 0.0 };
            let upto = (first + PEEK).min(self.preview.len);
            let mut d = 14.0;
            while d < upto {
                let p = self.preview.at(d);
                let fade = if d > first { 1.0 - (d - first) / PEEK } else { 1.0 };
                s.disc(p.0, p.1, 2.4, hex(0x000000), 0.25 * fade);
                s.disc(p.0, p.1, 1.7, hex(0xffffff), 0.95 * fade);
                d += 8.0;
            }
        }

        // the people
        let (ax, ay) = room.agent;
        let left = self.s.aim.cos() < 0.0;
        let agent = match self.s.phase {
            CLEARED | DONE => art::AGENT_CHEER,
            FAILED => art::AGENT_SAD,
            _ if self.since_fire < 160.0 => art::AGENT_FIRE,
            _ => art::AGENT_AIM,
        };
        self.put(s, agent, ax, ay, left, 1.0);
        for (i, f) in room.foes.iter().enumerate() {
            let down = self.s.down & 1 << i != 0;
            let look = [(art::SUIT_AIM, art::SUIT_DOWN), (art::BEARD_AIM, art::BEARD_DOWN), (art::GREEN_AIM, art::GREEN_DOWN)][f.look as usize % 3];
            self.put(s, if down { look.1 } else { look.0 }, f.x, f.feet, f.x > ax, 1.0);
        }
        for (i, h) in room.hostages.iter().enumerate() {
            let sprite = if self.s.hurt == i as i32 {
                art::HOSTAGE_DOWN
            } else if matches!(self.s.phase, CLEARED | DONE) {
                art::HOSTAGE_CHEER
            } else {
                art::HOSTAGE_SCARED
            };
            self.put(s, sprite, h.x, h.feet, h.x > ax, 1.0);
        }

        // the shot: a bright streak behind the bullet
        if self.s.phase == FLY {
            let tail = 70.0;
            let mut d = (self.flown - tail).max(0.0);
            let mut last = self.shot.at(d);
            while d < self.flown {
                d = (d + 4.0).min(self.flown);
                let p = self.shot.at(d);
                let k = 1.0 - (self.flown - d) / tail;
                s.line(last.0, last.1, p.0, p.1, 2.2, hex(0xffd166), 0.9 * k);
                last = p;
            }
            s.disc(last.0, last.1, 3.2, hex(0xfff3c4), 1.0);
        }

        // the top bar: the level and the shots left
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        for (dy, col, a) in [(1.2, 0x000000, 0.5), (0.0, 0xffffff, 1.0)] {
            s.text(&format!("{} {}", level_l, self.s.level + 1), 12.0, 24.0 + dy, 17.0, hex(col), a);
        }
        for k in 0..self.room.shots {
            let x = w - 18.0 - k as f32 * 15.0;
            let live = k < self.s.shots;
            bullet(s, x, 24.0, if live { 1.0 } else { 0.25 });
        }
        if !matches!(self.s.phase, AIM | FLY) {
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
            1 => s.fired,
            2 => s.bounces,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION
            && (0..LEVELS).contains(&s.opt[0])
            && (0..LEVELS).contains(&s.level)
            && (AIM..=FAILED).contains(&s.phase)
            && s.aim.is_finite();
        if ok {
            let saved = self.s;
            self.room = room::level(saved.level);
            let all = self.room.all_foes();
            if saved.phase == AIM && (0..=self.room.shots).contains(&saved.shots) && saved.down & !all == 0 {
                // mid level: as it was between shots
                self.restart();
                self.s.shots = saved.shots;
                self.s.down = saved.down;
                self.aim_at(saved.aim);
            } else if matches!(saved.phase, CLEARED | DONE) {
                self.restart();
                self.s.phase = saved.phase;
                self.s.down = all;
            } else {
                // a shot in the air is not saved: the level starts over
                self.retry();
            }
        }
        ok
    }
}

/// A bullet standing up: one of the shots left.
fn bullet(s: &mut Screen, x: f32, y: f32, alpha: f32) {
    s.rect(x - 4.0, y - 4.0, 8.0, 14.0, 2.0, hex(0xb07a2a), alpha);
    s.rect(x - 4.0, y - 4.0, 3.0, 14.0, 1.5, hex(0xe0a948), alpha);
    s.ellipse(x, y - 4.0, 4.0, 7.0, hex(0xd9a441), alpha);
    s.ellipse(x - 1.2, y - 6.0, 1.3, 3.0, hex(0xf6d68a), alpha);
}

fn blank() -> Save {
    Save {
        version: VERSION,
        opt: [0],
        level: 0,
        phase: AIM,
        retries: 0,
        shots: 0,
        down: 0,
        hurt: -1,
        level_ms: 0,
        time_ms: 0,
        fired: 0,
        bounces: 0,
        done: 0,
        score: 0,
        cleared: -1,
        cleared_score: 0,
        aim: 0.0,
    }
}

/// The warehouse: a brick wall, a concrete floor, the walls and ledges,
/// the steel plates and a few crates: drawn once a level.
fn draw_scene(s: &mut Screen, g: &Bankshot) {
    let room = &g.room;
    let (x0, y0, x1, y1) = s.visible();
    // outside the room: the building's shell
    s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0x2a2d34), 1.0);
    // the back wall: bricks
    s.rect_grad(LEFT, TOP, RIGHT - LEFT, FLOOR - TOP, 0.0, hex(0x8a5a44), hex(0x6b4434), 1.0);
    let mut rng = wg::Rng::new(77 + g.s.level as u32);
    let (bw, bh) = (24.0f32, 11.0f32);
    let mut row = 0;
    let mut y = TOP;
    while y < FLOOR {
        let mut x = LEFT - if row % 2 == 1 { bw / 2.0 } else { 0.0 };
        while x < RIGHT {
            let shade = rng.float();
            let (cx, cw) = (x.max(LEFT) + 1.0, (x + bw).min(RIGHT) - x.max(LEFT) - 2.0);
            if cw > 0.0 {
                let c = mix(hex(0x9a6650), hex(0x7a4c3a), shade);
                s.rect(cx, y + 1.0, cw, (bh - 2.0).min(FLOOR - y - 1.0), 1.0, c, 1.0);
            }
            x += bw;
        }
        y += bh;
        row += 1;
    }
    // a window with daylight, away from the agent
    let spots = if room.agent.0 < RIGHT / 2.0 { [190.0, 50.0, 120.0] } else { [50.0, 190.0, 120.0] };
    let clear = |wx: f32| {
        let r = room::Rect { x: wx - 3.0, y: 83.0, w: 66.0, h: 52.0 };
        !room.blocks.iter().any(|b| b.overlaps(&r)) && !room.plates.iter().any(|p| [p.a, p.b].iter().any(|q| q.0 > r.x && q.0 < r.x + r.w && q.1 > r.y && q.1 < r.y + r.h))
    };
    if let Some(&wx) = spots.iter().find(|&&wx| clear(wx)) {
        s.rect(wx - 3.0, 86.0 - 3.0, 66.0, 52.0, 3.0, hex(0x3b3f47), 1.0);
        s.rect_grad(wx, 86.0, 60.0, 46.0, 1.0, hex(0xbfe3f5), hex(0x86c3e3), 0.9);
        s.rect(wx + 28.5, 86.0, 3.0, 46.0, 0.0, hex(0x3b3f47), 1.0);
        s.rect(wx, 107.5, 60.0, 3.0, 0.0, hex(0x3b3f47), 1.0);
    }
    // shade toward the ceiling
    for k in 0..6 {
        s.rect(LEFT, TOP + k as f32 * 6.0, RIGHT - LEFT, 6.0, 0.0, hex(0x000000), 0.3 - k as f32 * 0.05);
    }
    // the floor
    s.rect_grad(x0, FLOOR, x1 - x0, y1 - FLOOR, 0.0, hex(0x9da3ab), hex(0x6c727b), 1.0);
    s.rect(x0, FLOOR, x1 - x0, 3.0, 0.0, hex(0xc4c9cf), 1.0);
    let mut x = LEFT + 20.0;
    while x < RIGHT {
        s.rect(x, FLOOR + 3.0, 1.0, y1 - FLOOR, 0.0, hex(0x5a5f67), 0.5);
        x += 60.0;
    }
    // the side walls and the ceiling
    s.rect(x0, TOP - 10.0, x1 - x0, 10.0, 0.0, hex(0x50555e), 1.0);
    s.rect(x0, TOP - 2.0, x1 - x0, 2.0, 0.0, hex(0x7b818b), 1.0);
    for (wx, ww) in [(x0, LEFT - x0), (RIGHT, x1 - RIGHT)] {
        if ww > 0.0 {
            s.rect(wx, TOP - 10.0, ww, FLOOR - TOP + 10.0, 0.0, hex(0x50555e), 1.0);
        }
    }
    s.rect(LEFT - 3.0, TOP, 3.0, FLOOR - TOP, 0.0, hex(0x7b818b), 1.0);
    s.rect(RIGHT, TOP, 3.0, FLOOR - TOP, 0.0, hex(0x7b818b), 1.0);
    // crates against the back wall, where nobody stands
    let mut cx = LEFT + 40.0 + rng.float() * 30.0;
    while cx < RIGHT - 20.0 {
        let near = |x: f32| (x - cx).abs() < 46.0;
        let busy = near(room.agent.0) || room.foes.iter().chain(&room.hostages).any(|p| near(p.x)) || room.blocks.iter().any(|b| b.y + b.h >= FLOOR - 1.0 && cx + 26.0 > b.x && cx - 26.0 < b.x + b.w);
        if !busy {
            let kind = if rng.below(2) == 0 { art::CRATE_WIDE } else { art::CRATE_MEDIUM };
            g.put(s, kind, cx, FLOOR, false, 0.9);
        }
        cx += 70.0 + rng.float() * 50.0;
    }
    // walls and ledges: concrete with a lit top edge; ledges steel
    for b in &room.blocks {
        s.rect(b.x + 2.0, b.y + 3.0, b.w, b.h, 2.0, hex(0x000000), 0.25);
        if b.h <= 12.0 {
            s.rect_grad(b.x, b.y, b.w, b.h, 2.0, hex(0x6d7681), hex(0x434a54), 1.0);
            s.rect(b.x, b.y, b.w, 2.0, 1.0, hex(0xa9b2bd), 1.0);
            let mut rx = b.x + 6.0;
            while rx < b.x + b.w - 3.0 {
                s.disc(rx, b.y + b.h / 2.0 + 1.0, 1.1, hex(0x2c3138), 0.8);
                rx += 12.0;
            }
        } else {
            s.rect_grad(b.x, b.y, b.w, b.h, 2.0, hex(0xb3b0a8), hex(0x86837c), 1.0);
            s.rect(b.x, b.y, 3.0, b.h, 1.0, hex(0xc9c6bf), 1.0);
            s.rect(b.x + b.w - 3.0, b.y, 3.0, b.h, 1.0, hex(0x6c6a64), 1.0);
            let mut ry = b.y + 18.0;
            while ry < b.y + b.h - 4.0 {
                s.rect(b.x + 3.0, ry, b.w - 6.0, 1.0, 0.0, hex(0x6c6a64), 0.5);
                ry += 18.0;
            }
        }
    }
    for p in &room.plates {
        s.line(p.a.0 + 1.5, p.a.1 + 2.5, p.b.0 + 1.5, p.b.1 + 2.5, room::PLATE_R, hex(0x000000), 0.25);
        s.line(p.a.0, p.a.1, p.b.0, p.b.1, room::PLATE_R, hex(0x8d96a3), 1.0);
        s.line(p.a.0, p.a.1 - 0.8, p.b.0, p.b.1 - 0.8, room::PLATE_R * 0.4, hex(0xdfe5ec), 0.9);
        for q in [p.a, p.b] {
            s.disc(q.0, q.1, 1.3, hex(0x4a515b), 1.0);
        }
    }
}

impl Bankshot {
    fn draw_card(&self, s: &mut Screen, w: f32) {
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0x10141a), 0.35);
        let (px, py, pw, ph) = self.card();
        s.shadow(px, py + 4.0, pw, ph, 16.0, 14.0, 0, 0.45);
        s.rect(px, py, pw, ph, 16.0, hex(0x2f3540), 1.0);
        let failed = self.s.phase == FAILED;
        let title = if !failed {
            tr(&["Level cleared", "过关", "クリア", "레벨 클리어", "Nivel superado", "Nível concluído", "Niveau réussi", "Level geschafft", "Уровень пройден", "過關"])
        } else if self.s.hurt >= 0 {
            tr(&["Hostage hit", "误伤人质", "人質に命中", "인질 명중", "Rehén herido", "Refém atingido", "Otage touché", "Geisel getroffen", "Заложник ранен", "誤傷人質"])
        } else {
            tr(&["Out of shots", "子弹用完", "弾切れ", "탄약 소진", "Sin balas", "Sem balas", "Plus de balles", "Keine Schüsse mehr", "Патроны кончились", "子彈用完"])
        };
        let size = s.fit_size(title, 21.0, pw - 32.0);
        s.text_c(title, w / 2.0, py + 34.0, size, hex(0xffffff), 1.0);
        let line = if failed {
            let left = self.room.foes.len() as u32 - self.s.down.count_ones();
            let left_l = tr(&["Gunmen left", "剩余枪手", "残りの敵", "남은 적", "Quedan", "Restam", "Restants", "Übrig", "Осталось", "剩餘槍手"]);
            format!("{} {}", left_l, left)
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

wg::game!(Bankshot);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Bankshot, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    #[test]
    fn every_level_plays_through() {
        let mut g = Bankshot::new();
        g.start(1);
        for level in 0..LEVELS {
            assert_eq!((g.s.level, g.s.phase), (level, AIM));
            while g.s.phase == AIM {
                let a = g.room.best_shot(g.s.down).expect("no shot");
                g.aim_at(a);
                assert_eq!(g.fire(), wg::EV_BLIP);
                run(&mut g, 3000);
            }
            assert!(matches!(g.s.phase, CLEARED | DONE), "level {} not cleared", level + 1);
            if level + 1 < LEVELS {
                g.press_button();
            }
        }
        assert!(g.over());
        assert!(g.s.bounces > 0);
    }

    #[test]
    fn hitting_a_hostage_fails_the_level() {
        let mut g = Bankshot::new();
        g.start(1);
        g.begin(10);
        let h = g.room.hostages[0];
        let gun = g.room.gun();
        // straight at the hostage, if nothing is in the way; else any
        // shot that ends on a hostage
        let a = (0..1440).map(Room::step_angle).find(|&a| g.room.trace(a).hostage.is_some()).unwrap_or(((h.feet - 18.0) - gun.1).atan2(h.x - gun.0));
        g.aim_at(a);
        g.fire();
        assert!(run(&mut g, 4000) & wg::EV_LOSE != 0);
        assert_eq!(g.s.phase, FAILED);
        assert!(g.s.hurt >= 0);
        g.press_button();
        assert_eq!((g.s.phase, g.s.retries, g.s.shots), (AIM, 1, g.room.shots));
    }

    #[test]
    fn running_out_of_shots_fails() {
        let mut g = Bankshot::new();
        g.start(1);
        // into the floor by the agent's feet, every time
        let down = std::f32::consts::FRAC_PI_2 * if g.room.agent.0 < 150.0 { 0.98 } else { 1.02 };
        for _ in 0..g.room.shots {
            g.aim_at(down);
            g.fire();
            run(&mut g, 4000);
        }
        if g.s.down != g.room.all_foes() {
            assert_eq!(g.s.phase, FAILED);
            assert!(g.s.hurt < 0);
        }
    }
}
