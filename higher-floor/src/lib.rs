//! Higher Floor - an original wasmga.me game. MIT licensed; the hero,
//! monsters, items and trees are cartoons drawn by art/draw.py with
//! games-src/tools/cartoon.
//! A tower of monsters stands next to the hero's. Tap a room the hero can
//! reach (the ground floor, or next to a room taken) and he jumps in: he
//! beats a weaker monster and takes its power, and loses to one as strong
//! or stronger. Potions add to his power, chests double it, traps halve
//! it. Beat every monster to finish the level. 30 levels.
mod art;
mod glyphs;
mod puzzle;

use puzzle::{take, Room, Tower, CHEST, EMPTY, LEVELS, MONSTER, POTION};
use wg::controls::{self, Icon, Row};
use wg::{hex, mix, tr, Game, Image, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 1;

const PLAY: i32 = 0;
const CLEARED: i32 = 1;
const DONE: i32 = 2;
/// the hero lost a fight
const LOST: i32 = 3;

const PLAN: (f32, f32) = (300.0, 400.0);
/// the ground line, and the towers' plinths on it
const GROUND: f32 = 372.0;
const BASE: f32 = 8.0;
/// the enemy tower's right edge, its walls, a room's width; the hero's
/// tower's left edge and width
const TOWER_X1: f32 = 290.0;
const WALL: f32 = 7.0;
const ROOM_W: f32 = 86.0;
const HOME_X: f32 = 12.0;
const HOME_W: f32 = 84.0;
/// a move: the jump, the fight (monsters only) and the outcome, ms
const JUMP_MS: f32 = 420.0;
const FIGHT_MS: f32 = 560.0;
const END_MS: f32 = 460.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 1],
    level: i32,
    phase: i32,
    tower: Tower,
    /// rooms taken, a bit each
    taken: u32,
    power: i32,
    /// the room the hero stands in, -1 on his own tower
    at: i32,
    retries: i32,
    level_ms: i32,
    time_ms: i32,
    all_beaten: i32,
    all_lost: i32,
    done: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
    /// keyboard cursor: a room's column and floor from the top
    cx: i32,
    cy: i32,
    rep: KeyRepeat,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}
unsafe impl wg::Plain for Tower {}
unsafe impl wg::Plain for Room {}

/// The hero on his way into a room: where from, how long since, and his
/// power after it (None if its monster wins).
#[derive(Clone, Copy)]
struct Move {
    to: usize,
    from: (f32, f32),
    t: f32,
    after: Option<i32>,
}

impl Move {
    fn fight(&self, room: &Room) -> f32 {
        if room.kind == MONSTER { FIGHT_MS } else { 0.0 }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    None,
    Room(usize),
    Button,
    Ctl(Icon),
}

struct HigherFloor {
    s: Save,
    hover: Hit,
    keys: bool,
    sprites: Vec<Image>,
    walk: Option<Move>,
    /// a room the hero cannot reach yet, shaken when tapped: room and ms
    deny: Option<(usize, f32)>,
    /// each room taken this level, with power and the hero's room before
    /// it, for Undo; not saved, so a loaded save starts it empty
    hist: Vec<(usize, i32, i32)>,
}

impl HigherFloor {
    /// a room's height: the tower's floors fill the space under the top bar
    fn room_h(&self) -> f32 {
        ((GROUND - BASE - 80.0) / self.s.tower.floors as f32).min(92.0)
    }
    /// the enemy tower's left edge
    fn tower_x0(&self) -> f32 {
        TOWER_X1 - 2.0 * WALL - ROOM_W * self.s.tower.cols as f32
    }
    /// room r's box
    fn room_box(&self, r: usize) -> (f32, f32, f32, f32) {
        let (f, c) = self.s.tower.place(r);
        let rh = self.room_h();
        (self.tower_x0() + WALL + c as f32 * ROOM_W, GROUND - BASE - (f + 1) as f32 * rh, ROOM_W, rh)
    }
    /// the hero's room on his own tower
    fn home_box(&self) -> (f32, f32, f32, f32) {
        let rh = self.room_h();
        (HOME_X + WALL, GROUND - BASE - rh, HOME_W - 2.0 * WALL, rh)
    }
    /// plan units per model unit: a figure stands a bit over half a room tall
    fn unit(&self) -> f32 {
        self.room_h() * 0.56 / 0.9
    }
    /// where the hero stands in a room (feet), and where its occupant does
    fn hero_spot(b: (f32, f32, f32, f32)) -> (f32, f32) {
        (b.0 + b.2 * 0.3, b.1 + b.3 - 7.0)
    }
    fn thing_spot(b: (f32, f32, f32, f32)) -> (f32, f32) {
        (b.0 + b.2 * 0.66, b.1 + b.3 - 7.0)
    }
    fn hero_at(&self) -> (f32, f32) {
        Self::hero_spot(if self.s.at < 0 { self.home_box() } else { self.room_box(self.s.at as usize) })
    }

    fn card(&self) -> (f32, f32, f32, f32) {
        let (w, h) = PLAN;
        let (pw, ph) = (248.0, if self.s.phase == DONE { 96.0 } else { 158.0 });
        (w / 2.0 - pw / 2.0, h * 0.42 - ph / 2.0, pw, ph)
    }
    fn button(&self) -> (f32, f32, f32, f32) {
        let (px, py, pw, ph) = self.card();
        let (bw, bh) = (pw - 48.0, 42.0);
        (px + (pw - bw) / 2.0, py + ph - bh - 20.0, bw, bh)
    }

    fn hit(&self, x: f32, y: f32) -> Hit {
        let inside = |(bx, by, bw, bh): (f32, f32, f32, f32)| x >= bx && x < bx + bw && y >= by && y < by + bh;
        if let Some(i) = self.controls().hit(x, y) {
            return Hit::Ctl(i);
        }
        match self.s.phase {
            PLAY => (0..self.s.tower.n()).find(|&r| inside(self.room_box(r))).map_or(Hit::None, Hit::Room),
            CLEARED | LOST if inside(self.button()) => Hit::Button,
            _ => Hit::None,
        }
    }

    /// Undo and Restart, in the top bar while a level is on
    fn controls(&self) -> Row {
        let row = controls::row(&[Icon::Undo, Icon::Restart], PLAN.0 - 8.0, 20.0);
        if self.s.phase == PLAY { row } else { row.hidden() }
    }
    fn can_use(&self, i: Icon) -> bool {
        self.s.phase == PLAY && self.walk.is_none() && if i == Icon::Undo { !self.hist.is_empty() } else { self.s.taken != 0 }
    }
    /// Takes back the last room; it costs what a retry does.
    fn undo(&mut self) -> u32 {
        if !self.can_use(Icon::Undo) {
            return 0;
        }
        let Some((room, power, at)) = self.hist.pop() else { return 0 };
        let s = &mut self.s;
        s.taken &= !(1 << room);
        (s.power, s.at) = (power, at);
        if s.tower.rooms[room].kind == MONSTER {
            s.all_beaten -= 1;
        }
        s.retries += 1;
        wg::EV_BLIP
    }
    fn restart(&mut self) -> u32 {
        if !self.can_use(Icon::Restart) {
            return 0;
        }
        self.retry();
        wg::EV_BLIP
    }
    /// The level again, counted as a retry.
    fn retry(&mut self) {
        self.s.retries += 1;
        self.begin(self.s.level);
    }

    fn begin(&mut self, level: i32) {
        self.hist.clear();
        let s = &mut self.s;
        s.level = level;
        s.phase = PLAY;
        s.tower = puzzle::deal(level);
        s.taken = 0;
        s.power = s.tower.start;
        s.at = -1;
        s.level_ms = 0;
        s.cx = s.cx.clamp(0, s.tower.cols - 1);
        s.cy = s.cy.clamp(0, s.tower.floors - 1);
        self.walk = None;
        self.deny = None;
    }

    /// Sends the hero into room r.
    fn enter(&mut self, r: usize) -> u32 {
        if self.s.phase != PLAY || self.walk.is_some() || r >= self.s.tower.n() || self.s.taken & 1 << r != 0 {
            return 0;
        }
        if !self.s.tower.reachable(self.s.taken, r) {
            self.deny = Some((r, 0.0));
            return wg::EV_HIT;
        }
        let after = take(self.s.power, &self.s.tower.rooms[r]);
        self.walk = Some(Move { to: r, from: self.hero_at(), t: 0.0, after });
        wg::EV_BLIP
    }

    /// The move's end: the room is taken, or the hero has lost.
    fn arrive(&mut self, m: Move) -> u32 {
        let s = &mut self.s;
        let room = s.tower.rooms[m.to];
        let Some(power) = m.after else {
            s.phase = LOST;
            s.all_lost += 1;
            return wg::EV_LOSE;
        };
        self.hist.push((m.to, s.power, s.at));
        s.power = power;
        s.taken |= 1 << m.to;
        s.at = m.to as i32;
        if room.kind == MONSTER {
            s.all_beaten += 1;
        }
        if s.tower.won(s.taken) { self.clear_level() } else { wg::EV_HIT }
    }

    fn clear_level(&mut self) -> u32 {
        let s = &mut self.s;
        let n = s.tower.monsters();
        let score = (50 * n - 40 * s.retries).max(10 * n);
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

    /// The card's button: the next level, or the same one again.
    fn press_button(&mut self) -> u32 {
        match self.s.phase {
            CLEARED => {
                self.s.retries = 0;
                self.begin(self.s.level + 1);
            }
            LOST => self.retry(),
            _ => return 0,
        }
        wg::EV_BLIP
    }

    /// the room under the keyboard cursor
    fn cursor_room(&self) -> usize {
        self.s.tower.room_at(self.s.cx, self.s.cy)
    }

    /// Draws sprite `i` with its model's feet at (x, y), `u` plan units to a
    /// model unit, mirrored if `flip`.
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

    /// A number in a rounded box, centred at (x, y).
    fn badge(s: &mut Screen, text: &str, x: f32, y: f32, size: f32, fill: u32, alpha: f32) {
        let (w, h) = (s.text_width(text, size) + size * 0.9, size * 1.45);
        s.rect(x - w / 2.0, y - h / 2.0 + 1.5, w, h, h / 2.0, hex(0x000000), 0.25 * alpha);
        s.rect(x - w / 2.0, y - h / 2.0, w, h, h / 2.0, hex(fill), alpha);
        s.text_c(text, x, y, size, hex(0xffffff), alpha);
    }

    /// what stands in a room: its sprite, label and badge colour
    fn thing(&self, room: &Room) -> (usize, String, u32) {
        match room.kind {
            MONSTER => {
                let fill = if self.s.power > room.val { 0x2f9e44 } else { 0xd9363e };
                (art::SKELETON_IDLE + room.look as usize * 3, room.val.to_string(), fill)
            }
            POTION => (art::POTION, format!("+{}", room.val), 0x3d7be0),
            CHEST => (art::CHEST, "×2".to_string(), 0xd99a1e),
            _ => (art::TRAP, "÷2".to_string(), 0x7b4bc4),
        }
    }
}

impl Game for HigherFloor {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let sheet = Image::png(art::SHEET);
        let sprites = art::ARTS.iter().map(|a| sheet.crop(a.x, a.y, a.w, a.h)).collect();
        HigherFloor { s: blank(), hover: Hit::None, keys: false, sprites, walk: None, deny: None, hist: Vec::new() }
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
        self.keys = false;
    }
    fn plan(&self, _w: i32, _h: i32) -> (f32, f32) {
        PLAN
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        if let Some(m) = &mut self.walk {
            m.t += dt;
            let m = *m;
            let total = JUMP_MS + m.fight(&self.s.tower.rooms[m.to]) + END_MS;
            if m.t >= total {
                self.walk = None;
                ev |= self.arrive(m);
            }
        }
        if let Some((_, t)) = &mut self.deny {
            *t += dt;
            if *t > 360.0 {
                self.deny = None;
            }
        }
        if self.s.phase == PLAY && self.s.taken != 0 {
            self.s.level_ms = self.s.level_ms.saturating_add(input.dt);
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        let dirs = self.s.rep.dirs(input);
        if dirs != 0 {
            self.keys = true;
            self.hover = Hit::None;
        }
        if self.s.phase == PLAY {
            let t = self.s.tower;
            if dirs & wg::LEFT != 0 { self.s.cx = (self.s.cx - 1).max(0); }
            if dirs & wg::RIGHT != 0 { self.s.cx = (self.s.cx + 1).min(t.cols - 1); }
            if dirs & wg::UP != 0 { self.s.cy = (self.s.cy - 1).max(0); }
            if dirs & wg::DOWN != 0 { self.s.cy = (self.s.cy + 1).min(t.floors - 1); }
            if input.pressed & wg::A != 0 {
                self.keys = true;
                ev |= self.enter(self.cursor_room());
            }
            if input.pressed & wg::B != 0 {
                ev |= self.undo();
            }
        } else if input.pressed & wg::A != 0 {
            ev |= self.press_button();
        }
        ev
    }

    fn key(&mut self, code: i32) -> bool {
        controls::is_restart_key(code) && self.restart() != 0
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.hover = Hit::None;
            return 0;
        }
        let h = self.hit(p.x, p.y);
        self.hover = if p.touch { Hit::None } else { h };
        if p.pressed & wg::M_LEFT == 0 {
            return 0;
        }
        self.keys = false;
        match h {
            Hit::Room(r) => {
                let (f, c) = self.s.tower.place(r);
                self.s.cx = c;
                self.s.cy = self.s.tower.floors - 1 - f;
                self.enter(r)
            }
            Hit::Button => self.press_button(),
            Hit::Ctl(Icon::Undo) => self.undo(),
            Hit::Ctl(_) => self.restart(),
            Hit::None => 0,
        }
    }

    fn picture(&mut self) -> u32 {
        let hv = match self.hover {
            Hit::None => -1,
            Hit::Room(r) => r as i32,
            Hit::Button => 1000,
            Hit::Ctl(i) => 1001 + i as i32,
        };
        let shown = [hv, self.keys as i32];
        let mut c = self.s;
        c.level_ms = 0;
        c.time_ms = 0;
        c.rep = KeyRepeat(0);
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        self.walk.is_some() || self.deny.is_some()
    }

    fn draw(&mut self, s: &mut Screen) {
        let (w, _) = PLAN;
        let t = self.s.tower;
        let key = wg::hash(0, wg::bytes_of(&[self.s.level, t.cols, t.floors]));
        s.background(key, |s| self.draw_scene(s));

        let u = self.unit();
        let rh = self.room_h();
        let size = (rh * 0.22).clamp(9.0, 14.0);
        let walk = self.walk;
        // rooms: dimmed out of reach, marked under the pointer or cursor
        let marked = match (self.hover, self.keys && self.s.phase == PLAY) {
            (Hit::Room(r), _) => Some(r),
            (_, true) => Some(self.cursor_room()),
            _ => None,
        };
        for r in 0..t.n() {
            let mut b = self.room_box(r);
            if let Some((d, ms)) = self.deny {
                if d == r {
                    b.0 += (ms / 30.0).sin() * 3.0 * (1.0 - ms / 360.0);
                }
            }
            let (ix, iy, iw, ih) = (b.0 + 2.0, b.1 + 2.0, b.2 - 4.0, b.3 - 6.0);
            let open = t.reachable(self.s.taken, r);
            if marked == Some(r) {
                s.rect(ix, iy, iw, ih, 3.0, hex(0xffe066), if open { 0.22 } else { 0.1 });
                ring_rect(s, ix, iy, iw, ih, hex(0xffe066), 0.9);
            }
            if self.s.taken & 1 << r != 0 || t.rooms[r].kind == EMPTY {
                continue;
            }
            let room = t.rooms[r];
            let (sprite, label, fill) = self.thing(&room);
            let (x, y) = Self::thing_spot(b);
            let (mut alpha, mut pose) = (1.0, sprite);
            let mut fight = false;
            if let Some(m) = walk.filter(|m| m.to == r) {
                let into = m.t - JUMP_MS;
                let end = into - m.fight(&room);
                fight = into > 0.0 && end < 0.0;
                if room.kind == MONSTER && fight {
                    pose = if (into / 140.0) as usize % 2 == 0 { sprite + 1 } else { sprite };
                }
                if end > 0.0 {
                    if room.kind == MONSTER && m.after.is_some() {
                        pose = sprite + 2;
                        alpha = 1.0 - end / END_MS;
                    } else if room.kind != MONSTER {
                        alpha = 1.0 - end / END_MS;
                    }
                }
            }
            let scale = if room.kind == MONSTER && room.look == puzzle::LOOKS - 1 { 1.15 } else { 1.0 };
            self.put(s, pose, x, y, u * scale, false, alpha);
            if !fight && alpha > 0.99 {
                Self::badge(s, &label, x, y - u * 0.98 * scale - size * 0.3, size, fill, 1.0);
            }
            if !open {
                s.rect(ix, iy, iw, ih, 3.0, hex(0x0b0d18), 0.5);
            }
        }

        // the hero
        let (mut hx, mut hy) = self.hero_at();
        let mut pose = art::HERO_IDLE;
        let mut flip = false;
        let mut shown_power = self.s.power;
        let mut float: Option<(String, f32, f32, f32)> = None;
        if let Some(m) = walk {
            let room = t.rooms[m.to];
            let to = Self::hero_spot(self.room_box(m.to));
            let into = m.t - JUMP_MS;
            let end = into - m.fight(&room);
            if into < 0.0 {
                // a jump: an arc from where he stood
                let k = m.t / JUMP_MS;
                let e = wg::ease_out(k);
                hx = m.from.0 + (to.0 - m.from.0) * e;
                hy = m.from.1 + (to.1 - m.from.1) * k - (k * (1.0 - k) * 4.0) * (rh * 0.55 + (to.1 - m.from.1).abs() * 0.15);
                pose = art::HERO_JUMP;
                flip = to.0 < m.from.0;
            } else {
                (hx, hy) = to;
                if end < 0.0 {
                    pose = if (into / 140.0) as usize % 2 == 0 { art::HERO_SWING_A } else { art::HERO_SWING_B };
                } else if let Some(after) = m.after {
                    // the room's number flies to the hero
                    let k = end / END_MS;
                    let (tx, ty) = Self::thing_spot(self.room_box(m.to));
                    let text = if room.kind == MONSTER { format!("+{}", room.val) } else { self.thing(&room).1 };
                    float = Some((text, tx + (hx - tx) * k, ty - u - (u * 0.4) * k, 1.0 - k * 0.6));
                    if k > 0.6 {
                        shown_power = after;
                    }
                } else {
                    pose = art::HERO_DOWN;
                }
            }
        } else if self.s.phase == LOST {
            pose = art::HERO_DOWN;
        }
        self.put(s, pose, hx, hy, u, flip, 1.0);
        if pose != art::HERO_DOWN {
            Self::badge(s, &shown_power.to_string(), hx, hy - u * 0.98 - size * 0.3, size * 1.1, 0x2f6fd6, 1.0);
        }
        if let Some((text, x, y, a)) = float {
            Self::badge(s, &text, x, y, size, 0x2f9e44, a);
        }

        // the top bar
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let left_l = tr(&["Monsters", "怪物", "モンスター", "몬스터", "Monstruos", "Monstros", "Monstres", "Monster", "Монстры", "怪物"]);
        let left = t.left(self.s.taken);
        let shaded = |s: &mut Screen, t: &str, x: f32, y: f32, size: f32, right: bool| {
            for (dy, col, a) in [(1.2, 0x1d2a44, 0.45), (0.0, 0xffffff, 1.0)] {
                if right {
                    s.text_r(t, x, y + dy, size, hex(col), a);
                } else {
                    s.text(t, x, y + dy, size, hex(col), a);
                }
            }
        };
        shaded(s, &format!("{} {}", level_l, self.s.level + 1), 14.0, 20.0, 17.0, false);
        let hover = if let Hit::Ctl(i) = self.hover { Some(i) } else { None };
        let right = self.controls().draw(s, hover, |i| self.can_use(i), w);
        shaded(s, &format!("{} {}", left_l, left), right - 14.0, 20.0, 15.0, true);
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
            1 => s.all_beaten,
            2 => s.all_lost,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        self.hist.clear();
        let s = &self.s;
        let t = &s.tower;
        let ok = s.version == VERSION
            && (0..LEVELS).contains(&s.opt[0])
            && (0..LEVELS).contains(&s.level)
            && (PLAY..=LOST).contains(&s.phase)
            && t.valid()
            && s.taken >> t.n() == 0
            && s.power > 0
            && (-1..t.n() as i32).contains(&s.at)
            && (0..t.cols).contains(&s.cx)
            && (0..t.floors).contains(&s.cy);
        self.walk = None;
        self.deny = None;
        self.hover = Hit::None;
        ok
    }
}

fn blank() -> Save {
    Save {
        version: VERSION,
        opt: [0],
        level: 0,
        phase: PLAY,
        tower: Tower::empty(),
        taken: 0,
        power: 1,
        at: -1,
        retries: 0,
        level_ms: 0,
        time_ms: 0,
        all_beaten: 0,
        all_lost: 0,
        done: 0,
        score: 0,
        cleared: -1,
        cleared_score: 0,
        cx: 0,
        cy: 0,
        rep: KeyRepeat(0),
    }
}

/// a rectangle's outline, 2 plan units wide
fn ring_rect(s: &mut Screen, x: f32, y: f32, w: f32, h: f32, c: u32, alpha: f32) {
    s.rect(x, y, w, 2.0, 0.0, c, alpha);
    s.rect(x, y + h - 2.0, w, 2.0, 0.0, c, alpha);
    s.rect(x, y + 2.0, 2.0, h - 4.0, 0.0, c, alpha);
    s.rect(x + w - 2.0, y + 2.0, 2.0, h - 4.0, 0.0, c, alpha);
}

const STONE: u32 = 0xe0c08e;
const STONE_DARK: u32 = 0xb9935f;
const INSIDE: u32 = 0x4b4458;
const INSIDE_DARK: u32 = 0x332e3f;
const PLANKS: u32 = 0x9a6a3c;

impl HigherFloor {
    /// The sky, hills, trees and both towers with their empty rooms: drawn
    /// once a level.
    fn draw_scene(&self, s: &mut Screen) {
        let (x0, y0, x1, y1) = s.visible();
        s.rect_grad(x0, y0, x1 - x0, GROUND - y0, 0.0, hex(0x7cc4ef), hex(0xd8effa), 1.0);
        // clouds
        let mut rng = wg::Rng::new(31 + self.s.level as u32);
        for _ in 0..5 {
            let (cx, cy) = (x0 + rng.float() * (x1 - x0), 60.0 + rng.float() * 140.0);
            for k in 0..4 {
                let r = 12.0 + rng.float() * 10.0;
                s.disc(cx + k as f32 * 14.0 - 20.0, cy - (k % 2) as f32 * 6.0, r, hex(0xffffff), 0.75);
            }
        }
        // far hills, then the ground
        for (i, c) in [(0, 0x9fd27a), (1, 0x86c15e)] {
            let mut x = x0 - 60.0 + i as f32 * 70.0;
            while x < x1 + 60.0 {
                s.ellipse(x, GROUND + 6.0, 90.0, 54.0 - i as f32 * 14.0, hex(c), 1.0);
                x += 150.0;
            }
        }
        // trees behind and beside the towers
        let u = 20.0;
        for (i, &x) in [x0 + 4.0, 100.0, x1 - 6.0, -40.0, 340.0].iter().enumerate() {
            let kind = [art::TREE_OAK, art::TREE_CONE, art::TREE_DEFAULT, art::TREE_CONE, art::TREE_OAK][i];
            self.put(s, kind, x, GROUND + 2.0, u, false, 1.0);
        }
        s.rect_grad(x0, GROUND, x1 - x0, y1 - GROUND, 0.0, hex(0x6fb54a), hex(0x4f8e34), 1.0);
        s.rect(x0, GROUND, x1 - x0, 3.0, 0.0, hex(0x8fd067), 1.0);
        for _ in 0..((x1 - x0) * (y1 - GROUND) / 70.0) as i32 {
            let (x, y) = (x0 + rng.float() * (x1 - x0), GROUND + 4.0 + rng.float() * (y1 - GROUND));
            s.disc(x, y, 1.0 + rng.float() * 1.4, hex(0x3f7a2a), 0.2);
        }

        let t = &self.s.tower;
        let rh = self.room_h();
        let top = GROUND - BASE - rh * t.floors as f32;
        self.tower(s, self.tower_x0(), top, TOWER_X1, 0xd9363e);
        for r in 0..t.n() {
            self.room(s, self.room_box(r), r);
        }
        let home = self.home_box();
        self.tower(s, HOME_X, home.1, HOME_X + HOME_W, 0x2f6fd6);
        self.room(s, home, 99);
        // bushes at the towers' feet
        for &x in &[HOME_X + HOME_W + 4.0, self.tower_x0() - 2.0, TOWER_X1 + 6.0] {
            self.put(s, art::BUSHLARGE, x, GROUND + 6.0, 14.0, false, 1.0);
        }
    }

    /// A tower's stonework from x0 to x1, its rooms' top at `top`, with
    /// battlements and a flag of colour `flag`.
    fn tower(&self, s: &mut Screen, x0: f32, top: f32, x1: f32, flag: u32) {
        let crown = 16.0;
        s.rect(x0 - 3.0, GROUND - BASE, x1 - x0 + 6.0, BASE + 2.0, 1.5, hex(0x8e7a62), 1.0);
        s.rect(x0, top - WALL, x1 - x0, GROUND - BASE - top + WALL, 0.0, hex(STONE), 1.0);
        // block courses
        let mut y = top - WALL + 9.0;
        let mut row = 0;
        while y < GROUND - BASE {
            s.rect(x0, y, x1 - x0, 1.0, 0.0, hex(STONE_DARK), 0.6);
            let mut x = x0 + if row % 2 == 0 { 7.0 } else { 15.0 };
            while x < x1 - 2.0 {
                s.rect(x, y - 9.0, 1.0, 9.0, 0.0, hex(STONE_DARK), 0.5);
                x += 16.0;
            }
            y += 9.0;
            row += 1;
        }
        // battlements
        let n = ((x1 - x0) / 14.0) as i32 | 1;
        let mw = (x1 - x0 + 4.0) / n as f32;
        for k in 0..n {
            if k % 2 == 0 {
                s.rect(x0 - 2.0 + k as f32 * mw, top - WALL - crown, mw, crown, 1.0, hex(STONE), 1.0);
                s.rect(x0 - 2.0 + k as f32 * mw, top - WALL - crown, mw, 2.0, 0.0, hex(0xf0d7ab), 1.0);
            }
        }
        s.rect(x0 - 2.0, top - WALL - 4.0, x1 - x0 + 4.0, 4.0, 0.0, hex(STONE_DARK), 1.0);
        // the flag
        let px = (x0 + x1) / 2.0;
        let py = top - WALL - crown;
        s.line(px, py, px, py - 34.0, 1.2, hex(0x5a4a3a), 1.0);
        s.tri((px + 1.0, py - 34.0), (px + 1.0, py - 20.0), (px + 22.0, py - 27.0), hex(flag), 1.0);
    }

    /// A room's inside: stone back wall, planks underfoot, and a banner or
    /// a barrel in some.
    fn room(&self, s: &mut Screen, b: (f32, f32, f32, f32), r: usize) {
        let (x, y, w, h) = (b.0 + 2.0, b.1 + 2.0, b.2 - 4.0, b.3 - 6.0);
        s.rect_grad(x, y, w, h, 2.0, hex(INSIDE_DARK), hex(INSIDE), 1.0);
        let mut yy = y + 7.0;
        let mut row = 0;
        while yy < y + h - 6.0 {
            s.rect(x, yy, w, 0.8, 0.0, hex(0x2a2534), 0.7);
            let mut xx = x + if row % 2 == 0 { 6.0 } else { 13.0 };
            while xx < x + w {
                s.rect(xx, yy - 7.0, 0.8, 7.0, 0.0, hex(0x2a2534), 0.55);
                xx += 14.0;
            }
            yy += 7.0;
            row += 1;
        }
        s.rect(x, y + h - 6.0, w, 6.0, 0.0, hex(PLANKS), 1.0);
        s.rect(x, y + h - 6.0, w, 1.2, 0.0, hex(0xc08a54), 1.0);
        // light from a window high on the wall
        s.shadow(x + w * 0.2, y + 4.0, w * 0.6, h * 0.5, 10.0, 14.0, hex(0xffe2a8), 0.12);
        let u = self.unit();
        let pick = (r * 7 + self.s.level as usize * 3) % 5;
        if pick == 0 || r == 99 {
            self.put(s, art::BANNER, x + w * 0.5, y + h * 0.62, u * 0.7, false, 1.0);
        } else if pick == 2 {
            self.put(s, art::BARREL, x + w - 9.0, y + h - 2.0, u * 0.5, false, 1.0);
        }
        // the inner shadow under the ceiling
        s.rect_grad(x, y, w, 6.0, 0.0, hex(0x000000), hex(INSIDE_DARK), 0.35);
    }

    fn draw_card(&self, s: &mut Screen, w: f32) {
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0x10141a), 0.45);
        let (px, py, pw, ph) = self.card();
        s.shadow(px, py + 4.0, pw, ph, 16.0, 14.0, 0, 0.45);
        s.rect(px, py, pw, ph, 16.0, hex(0x2f3540), 1.0);
        let lost = self.s.phase == LOST;
        let title = if lost {
            tr(&["Defeated", "被打败了", "負けた", "패배", "Derrotado", "Derrotado", "Vaincu", "Besiegt", "Поражение", "被打敗了"])
        } else {
            tr(&["Level cleared", "过关", "クリア", "레벨 클리어", "Nivel superado", "Nível concluído", "Niveau réussi", "Level geschafft", "Уровень пройден", "過關"])
        };
        let size = s.fit_size(title, 21.0, pw - 32.0);
        s.text_c(title, w / 2.0, py + 34.0, size, hex(0xffffff), 1.0);
        let power_l = tr(&["Power", "力量", "パワー", "힘", "Poder", "Poder", "Puissance", "Stärke", "Сила", "力量"]);
        let line = if lost {
            format!("{} {}", power_l, self.s.power)
        } else {
            let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
            format!("{} +{}    {} {}", score_l, self.s.cleared_score, power_l, self.s.power)
        };
        let size = s.fit_size(&line, 14.0, pw - 32.0);
        s.text_c(&line, w / 2.0, py + 64.0, size, hex(0xb9c2cf), 1.0);
        if self.s.phase != DONE {
            let b = self.button();
            let label = if lost {
                tr(&["Try again", "再试一次", "もう一度", "다시 하기", "Reintentar", "Tentar de novo", "Réessayer", "Nochmal", "Ещё раз", "再試一次"])
            } else {
                tr(&["Next level", "下一关", "次のレベル", "다음 레벨", "Siguiente nivel", "Próximo nível", "Niveau suivant", "Nächstes Level", "Следующий уровень", "下一關"])
            };
            let fill = hex(0x3d7be0);
            let fill = if self.hover == Hit::Button { mix(hex(0xffffff), fill, 0.15) } else { fill };
            s.rect(b.0, b.1, b.2, b.3, b.3 / 2.0, fill, 1.0);
            let size = s.fit_size(label, 16.0, b.2 - 16.0);
            s.text_c(label, b.0 + b.2 / 2.0, b.1 + b.3 / 2.0, size, hex(0xffffff), 1.0);
        }
    }
}

const LOOK_NAMES: [[&str; 10]; 5] = [
    ["Skeleton", "骷髅", "スケルトン", "해골", "Esqueleto", "Esqueleto", "Squelette", "Skelett", "Скелет", "骷髏"],
    ["Zombie", "僵尸", "ゾンビ", "좀비", "Zombi", "Zumbi", "Zombie", "Zombie", "Зомби", "殭屍"],
    ["Ghost", "幽灵", "ゴースト", "유령", "Fantasma", "Fantasma", "Fantôme", "Geist", "Призрак", "幽靈"],
    ["Orc", "兽人", "オーク", "오크", "Orco", "Orc", "Orque", "Ork", "Орк", "獸人"],
    ["Vampire", "吸血鬼", "吸血鬼", "뱀파이어", "Vampiro", "Vampiro", "Vampire", "Vampir", "Вампир", "吸血鬼"],
];

/// The accessible board: the tower's rooms, top floor first.
impl wg::Board for HigherFloor {
    fn grid(&self) -> Option<(i32, i32)> {
        Some((self.s.tower.cols, self.s.tower.floors))
    }
    fn cursor(&self) -> (i32, i32) {
        (self.s.cx, self.s.cy)
    }
    fn focus(&mut self, x: i32, y: i32) {
        self.s.cx = x.clamp(0, self.s.tower.cols - 1);
        self.s.cy = y.clamp(0, self.s.tower.floors - 1);
        self.keys = true;
        self.hover = Hit::None;
    }
    fn rect(&self) -> (f32, f32, f32) {
        let b = self.room_box(self.s.tower.n() - self.s.tower.cols as usize);
        (b.0, b.1, b.2)
    }
    fn cell_box(&self, x: i32, y: i32) -> Option<(f32, f32, f32, f32)> {
        Some(self.room_box(self.s.tower.room_at(x, y)))
    }
    fn cell(&self, x: i32, y: i32, out: &mut String) {
        let t = &self.s.tower;
        let r = t.room_at(x, y);
        let room = &t.rooms[r];
        if self.s.at == r as i32 {
            out.push_str(tr(&["You are here", "你在这里", "ここにいる", "현재 위치", "Estás aquí", "Você está aqui", "Vous êtes ici", "Du bist hier", "Вы здесь", "你在這裡"]));
            return;
        }
        if self.s.taken & 1 << r != 0 || room.kind == EMPTY {
            out.push_str(tr(&["Empty", "空", "空き", "비어 있음", "Vacío", "Vazio", "Vide", "Leer", "Пусто", "空"]));
            return;
        }
        match room.kind {
            MONSTER => {
                out.push_str(&format!("{} {}", tr(&LOOK_NAMES[room.look as usize]), room.val));
                out.push_str(", ");
                out.push_str(if self.s.power > room.val {
                    tr(&["weaker than you", "比你弱", "あなたより弱い", "당신보다 약함", "más débil que tú", "mais fraco que você", "plus faible que vous", "schwächer als du", "слабее вас", "比你弱"])
                } else {
                    tr(&["too strong", "太强了", "強すぎる", "너무 강함", "demasiado fuerte", "forte demais", "trop fort", "zu stark", "слишком силён", "太強了"])
                });
            }
            POTION => out.push_str(&tr(&["Potion, +#", "药水，+#", "ポーション、+#", "물약, +#", "Poción, +#", "Poção, +#", "Potion, +#", "Trank, +#", "Зелье, +#", "藥水，+#"]).replace('#', &room.val.to_string())),
            CHEST => out.push_str(tr(&["Chest, doubles your power", "宝箱，力量翻倍", "宝箱、パワー 2 倍", "상자, 힘 두 배", "Cofre, duplica tu poder", "Baú, dobra seu poder", "Coffre, double votre puissance", "Truhe, verdoppelt deine Stärke", "Сундук, удваивает силу", "寶箱，力量翻倍"])),
            _ => out.push_str(tr(&["Trap, halves your power", "陷阱，力量减半", "罠、パワー半分", "함정, 힘 절반", "Trampa, reduce tu poder a la mitad", "Armadilha, corta seu poder pela metade", "Piège, divise votre puissance par deux", "Falle, halbiert deine Stärke", "Ловушка, вдвое уменьшает силу", "陷阱，力量減半"])),
        }
        if !t.reachable(self.s.taken, r) {
            out.push_str(", ");
            out.push_str(tr(&["out of reach", "够不着", "まだ届かない", "닿지 않음", "fuera de alcance", "fora de alcance", "hors de portée", "nicht erreichbar", "не добраться", "夠不著"]));
        }
    }
    fn status(&self, out: &mut String) {
        let s = &self.s;
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let power_l = tr(&["power", "力量", "パワー", "힘", "poder", "poder", "puissance", "Stärke", "сила", "力量"]);
        let left_l = tr(&["monsters left", "个怪物剩余", "体のモンスターが残り", "마리 남음", "monstruos quedan", "monstros restantes", "monstres restants", "Monster übrig", "монстров осталось", "個怪物剩餘"]);
        let left = s.tower.left(s.taken);
        out.push_str(&format!("{} {}, {} {}, {} {}", level_l, s.level + 1, power_l, s.power, left, left_l));
    }
    fn keys(&self) -> &'static str {
        tr(&[
            "Arrow keys move between rooms, Space enters the selected room, I announces the level, hero power, and remaining monsters. X takes back the last room. R restarts the level.",
            "方向键在房间间移动，空格让英雄进入房间，I 读出关卡、力量和剩余怪物。X 撤销上一步。R 重新开始本关。",
            "矢印で部屋を移動、スペースで勇者をその部屋へ、I でレベル、パワー、残りのモンスター。X で一手戻す。R でレベルをやり直し。",
            "화살표로 방 이동, 스페이스로 영웅을 방에 보내기, I로 레벨, 힘, 남은 몬스터 읽기. X로 한 수 되돌리기. R로 레벨 다시 시작.",
            "Flechas para moverse entre salas, Espacio envía al héroe a la sala, I lee nivel, poder y monstruos restantes. X deshace el último movimiento. R reinicia el nivel.",
            "Setas movem entre as salas, Espaço manda o herói para a sala, I lê nível, poder e monstros restantes. X desfaz o último movimento. R reinicia o nível.",
            "Flèches pour changer de salle, Espace envoie le héros dans la salle, I lit le niveau, la puissance et les monstres restants. X annule le dernier coup. R recommence le niveau.",
            "Pfeiltasten wechseln den Raum, Leertaste schickt den Helden hinein, I liest Level, Stärke und übrige Monster vor. X macht den letzten Zug rückgängig. R startet das Level neu.",
            "Стрелки переходят между комнатами, пробел отправляет героя в комнату, I читает уровень, силу и сколько монстров осталось. X отменяет последний ход. R начинает уровень заново.",
            "方向鍵在房間間移動，空白鍵讓英雄進入房間，I 讀出關卡、力量和剩餘怪物。X 復原上一步。R 重新開始本關。",
        ])
    }
    fn act(&mut self, x: i32, y: i32, a: i32, said: &mut String) -> u32 {
        if self.s.phase != PLAY {
            let ev = self.press_button();
            self.status(said);
            return ev;
        }
        // X takes back the last room, as on the pad
        if a == 1 {
            let ev = self.undo();
            if ev != 0 {
                self.status(said);
            }
            return ev;
        }
        self.focus(x, y);
        let r = self.cursor_room();
        let ev = self.enter(r);
        if ev == wg::EV_BLIP {
            // the move plays out at once for a screen reader
            if let Some(m) = self.walk.take() {
                let ev = self.arrive(m);
                if self.s.phase == LOST {
                    said.push_str(tr(&["Defeated by a stronger monster. Press Space to retry.", "怪物更强。按空格再试一次。", "モンスターの方が強かった。スペースでもう一度。", "몬스터가 더 강했습니다. 스페이스로 다시.", "El monstruo era más fuerte. Espacio para reintentar.", "O monstro era mais forte. Espaço para tentar de novo.", "Le monstre était plus fort. Espace pour réessayer.", "Das Monster war stärker. Leertaste für nochmal.", "Монстр оказался сильнее. Пробел, чтобы повторить.", "怪物更強。按空白鍵再試一次。"]));
                } else if self.s.phase != PLAY {
                    said.push_str(tr(&["Level cleared! Press Space for the next level.", "过关！按空格进入下一关。", "クリア！スペースで次のレベル。", "레벨 클리어! 스페이스로 다음 레벨.", "¡Nivel superado! Espacio para el siguiente.", "Nível concluído! Espaço para o próximo.", "Niveau réussi ! Espace pour le suivant.", "Level geschafft! Leertaste für das nächste.", "Уровень пройден! Пробел для следующего.", "過關！按空白鍵進入下一關。"]));
                } else {
                    self.status(said);
                }
                return ev;
            }
        }
        if ev == wg::EV_HIT {
            said.push_str(tr(&["Out of reach", "够不着", "まだ届かない", "닿지 않음", "Fuera de alcance", "Fora de alcance", "Hors de portée", "Nicht erreichbar", "Не добраться", "夠不著"]));
        } else {
            said.push_str(tr(&["Wait a moment", "请稍等", "少し待って", "잠시 기다리세요", "Espera un momento", "Espere um pouco", "Un instant", "Einen Moment", "Подождите", "請稍等"]));
        }
        ev
    }
    fn rev(&mut self) -> u32 {
        let v = [self.s.phase, self.s.level, self.s.taken as i32, self.s.power, wg::lang() as i32];
        wg::hash(wg::hash(0, wg::bytes_of(&self.s.tower)), wg::bytes_of(&v))
    }
}

wg::game!(HigherFloor);
wg::board!(HigherFloor);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut HigherFloor, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    /// a winning order of rooms from here, found by trying them all
    fn solve(t: &Tower, taken: u32, power: i32, path: &mut Vec<usize>) -> bool {
        if t.won(taken) {
            return true;
        }
        for r in 0..t.n() {
            if t.reachable(taken, r) {
                if let Some(p) = take(power, &t.rooms[r]) {
                    path.push(r);
                    if solve(t, taken | 1 << r, p, path) {
                        return true;
                    }
                    path.pop();
                }
            }
        }
        false
    }

    #[test]
    fn plays_every_level() {
        let mut g = HigherFloor::new();
        g.start(1);
        for level in 0..LEVELS {
            assert_eq!(g.s.level, level);
            let mut path = Vec::new();
            assert!(solve(&g.s.tower, 0, g.s.power, &mut path), "level {} unsolved", level + 1);
            let mut ev = 0;
            for r in path {
                assert_eq!(g.enter(r), wg::EV_BLIP);
                ev |= run(&mut g, 2000);
            }
            assert!(ev & wg::EV_LEVEL != 0, "level {} not cleared", level + 1);
            if level + 1 < LEVELS {
                assert_eq!(g.s.phase, CLEARED);
                g.press_button();
            } else {
                assert!(g.over());
            }
        }
        assert_eq!(g.s.done, LEVELS);
    }

    #[test]
    fn loses_to_a_stronger_monster_and_retries() {
        let mut g = HigherFloor::new();
        g.start(1);
        g.begin(12);
        let t = g.s.tower;
        let r = (0..t.n()).find(|&r| t.reachable(0, r) && t.rooms[r].kind == MONSTER && t.rooms[r].val >= g.s.power);
        let r = r.unwrap_or_else(|| {
            // make one too strong
            let r = (0..t.n()).find(|&r| t.reachable(0, r)).unwrap();
            g.s.tower.rooms[r] = Room { kind: MONSTER, look: 0, pad: [0; 2], val: 999 };
            r
        });
        g.enter(r);
        assert!(run(&mut g, 2000) & wg::EV_LOSE != 0);
        assert_eq!(g.s.phase, LOST);
        g.press_button();
        assert_eq!((g.s.phase, g.s.retries, g.s.taken), (PLAY, 1, 0));
    }

    #[test]
    fn out_of_reach_rooms_refuse() {
        let mut g = HigherFloor::new();
        g.start(1);
        g.begin(20);
        let top = g.s.tower.n() - 1;
        assert_eq!(g.enter(top), wg::EV_HIT);
        assert!(g.walk.is_none());
    }
}
