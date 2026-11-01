//! Boarding: passengers leave a crowded platform for a matching bus.
//! Original MIT code; Kenney's CC0 character sprites are retained.
mod art;
mod glyphs;
mod puzzle;
use puzzle::{Yard, BENCH, LEVELS};
use wg::controls::{self, Icon, Row};
use wg::{hex, tr, Game, Image, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 3;
const PLAY: i32 = 0;
const CLEARED: i32 = 1;
const DONE: i32 = 2;
const STUCK: i32 = 3;
const PLAN: (f32, f32) = (300.0, 400.0);
const COLOURS: [u32; 6] = [0xe84a5f, 0xf18b31, 0xe9c539, 0x40ad75, 0x438de1, 0x9b61d1];
const INK: u32 = hex(0x26374a);
#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 1],
    level: i32,
    phase: i32,
    yard: Yard,
    retries: i32,
    time_ms: i32,
    all_retries: i32,
    all_boarded: i32,
    done: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
    cx: i32,
    cy: i32,
    rep: KeyRepeat,
}
unsafe impl wg::Plain for Save {}
struct Flight {
    before: Yard,
    selected: usize,
    path: Vec<(f32, f32)>,
    t: f32,
}
struct Boarding {
    s: Save,
    sprites: Vec<Image>,
    flight: Option<Flight>,
    hover: Option<usize>,
    keys: bool,
    blocked: Option<(usize, f32)>,
}
fn blank() -> Save {
    Save {
        version: VERSION,
        opt: [0],
        level: 0,
        phase: PLAY,
        yard: Yard::empty(4, 4),
        retries: 0,
        time_ms: 0,
        all_retries: 0,
        all_boarded: 0,
        done: 0,
        score: 0,
        cleared: -1,
        cleared_score: 0,
        cx: 0,
        cy: 0,
        rep: KeyRepeat(0),
    }
}
fn colour(c: u8) -> u32 {
    hex(COLOURS[c.saturating_sub(1).min(5) as usize])
}
impl Boarding {
    fn begin(&mut self, level: i32) {
        self.s.level = level;
        self.s.phase = PLAY;
        self.s.yard = puzzle::deal(level);
        self.s.cx = 0;
        self.s.cy = 0;
        self.s.retries = 0;
        self.flight = None;
        self.blocked = None;
        self.hover = None;
    }
    fn controls(&self) -> Row {
        let row = controls::row(&[Icon::Restart], 292.0, 21.0);
        if self.s.phase == PLAY {
            row
        } else {
            row.hidden()
        }
    }
    fn restart(&mut self) -> u32 {
        if !matches!(self.s.phase, PLAY | STUCK) {
            return 0;
        }
        let retries = self.s.retries + 1;
        self.s.all_retries += 1;
        self.begin(self.s.level);
        self.s.retries = retries;
        wg::EV_BLIP
    }
    fn next(&mut self) -> u32 {
        if self.s.phase == STUCK {
            return self.restart();
        }
        if self.s.phase != CLEARED {
            return 0;
        }
        self.begin(self.s.level + 1);
        wg::EV_BLIP
    }
    fn board_box(&self) -> (f32, f32, f32) {
        let c = (252.0 / self.s.yard.w as f32).min(180.0 / self.s.yard.h as f32).min(42.0);
        ((300.0 - c * self.s.yard.w as f32) / 2.0, 198.0, c)
    }
    fn place(&self, i: usize) -> (f32, f32) {
        let (x, y, c) = self.board_box();
        (x + (i as i32 % self.s.yard.w) as f32 * c + c / 2.0, y + (i as i32 / self.s.yard.w) as f32 * c + c * 0.8)
    }
    fn hit(&self, x: f32, y: f32) -> Option<usize> {
        let (bx, by, c) = self.board_box();
        let (gx, gy) = (((x - bx) / c).floor() as i32, ((y - by) / c).floor() as i32);
        (gx >= 0 && gy >= 0 && gx < self.s.yard.w && gy < self.s.yard.h).then_some((gy * self.s.yard.w + gx) as usize)
    }
    fn tap(&mut self, i: usize) -> u32 {
        if self.s.phase != PLAY || self.flight.is_some() {
            return 0;
        }
        let before = self.s.yard;
        let Some(route) = before.path(i) else {
            self.blocked = Some((i, 450.0));
            return wg::EV_HIT;
        };
        let mut path: Vec<_> = route.iter().map(|&j| self.place(j)).collect();
        let last = *path.last().unwrap();
        path.push((last.0, 185.0));
        let c = before.cell[i];
        if c == before.current() {
            path.extend([(225.0, 185.0), (225.0, 94.0), (188.0, 94.0)]);
        } else {
            let k = before.bench[..BENCH].iter().position(|&v| v == 0).unwrap();
            path.push((66.0 + k as f32 * 42.0, 156.0));
        }
        if !self.s.yard.dispatch(i) {
            return 0;
        }
        self.s.all_boarded += self.s.yard.boarded - before.boarded;
        self.flight = Some(Flight { before, selected: i, path, t: 0.0 });
        self.blocked = None;
        wg::EV_BLIP
    }
    fn finish(&mut self) -> u32 {
        if self.s.phase != PLAY || self.flight.is_some() {
            return 0;
        }
        if self.s.yard.lost() {
            self.s.phase = STUCK;
            return wg::EV_LOSE;
        }
        if !self.s.yard.won() {
            return 0;
        }
        let score = (self.s.yard.boarded * 50 - self.s.retries * 30).max(50);
        self.s.score += score;
        self.s.cleared = self.s.level;
        self.s.cleared_score = score;
        self.s.done += 1;
        if self.s.level == LEVELS - 1 {
            self.s.phase = DONE;
            wg::EV_LEVEL | wg::EV_WIN | wg::EV_GAMEOVER
        } else {
            self.s.phase = CLEARED;
            wg::EV_LEVEL | wg::EV_WIN
        }
    }
    fn person(&self, s: &mut Screen, c: u8, x: f32, y: f32, height: f32, walking: bool) {
        let pose = if walking { 1 } else { 0 };
        let i = art::FEMALE_A_0_STAND + (c - 1) as usize * 6 + pose;
        let a = &art::ARTS[i];
        let k = height / a.h as f32;
        s.ellipse(x, y + 1.0, height * 0.23, height * 0.075, hex(0x26374a), 0.18);
        s.image(&self.sprites[i], x - a.ax * k, y - a.ay * k, a.w as f32 * k, a.h as f32 * k, 1.0);
        // The colour number also appears on the bus and bench, so hue
        // isn't the only way to identify the passenger's destination.
        s.disc(x + height * 0.22, y - 4.0, 5.2, colour(c), 1.0);
        s.text_c(&c.to_string(), x + height * 0.22, y - 4.0, 7.0, hex(0xffffff), 1.0);
    }
    fn draw_bus(&self, s: &mut Screen, yard: &Yard) {
        let c = yard.current().max(1);
        s.disc(90.0, 100.0, 8.0, INK, 1.0);
        s.disc(200.0, 100.0, 8.0, INK, 1.0);
        s.rect(65.0, 48.0, 160.0, 52.0, 12.0, colour(c), 1.0);
        s.rect(193.0, 54.0, 23.0, 24.0, 5.0, hex(0xd6f2ff), 1.0);
        for k in 0..3 {
            let x = 91.0 + k as f32 * 32.0;
            s.rect(x - 12.0, 56.0, 25.0, 30.0, 5.0, hex(0xe8f6fa), 1.0);
            if k < yard.filled {
                self.person(s, c, x, 81.0, 25.0, false);
            }
        }
        s.text_c(&format!("{}   {}/3", c, yard.filled), 145.0, 94.0, 10.0, hex(0xffffff), 1.0);
        s.text(tr(&["Next", "接下来"]), 234.0, 53.0, 10.0, INK, 1.0);
        for k in 1..=2 {
            let j = yard.bus + k;
            if j < yard.buses {
                let c = yard.queue[j as usize];
                let y = 66.0 + (k - 1) as f32 * 22.0;
                s.rect(238.0, y - 8.0, 32.0, 17.0, 5.0, colour(c), 1.0);
                s.text_c(&c.to_string(), 254.0, y, 10.0, hex(0xffffff), 1.0);
            }
        }
    }
    fn card(&self, s: &mut Screen) {
        s.rect(0.0, 0.0, 300.0, 400.0, 0.0, hex(0x172b42), 0.48);
        s.shadow(30.0, 126.0, 240.0, 150.0, 16.0, 10.0, 0, 0.25);
        s.rect(30.0, 122.0, 240.0, 150.0, 16.0, hex(0xffffff), 1.0);
        let failed = self.s.phase == STUCK;
        s.text_c(if failed { tr(&["Bench full", "候车区满了"]) } else { tr(&["Everyone aboard!", "全部上车！"]) }, 150.0, 151.0, 21.0, INK, 1.0);
        let detail = if failed {
            tr(&["Plan around the next bus colour.", "先考虑下一辆车的颜色。"]).to_owned()
        } else {
            format!("+{}", self.s.cleared_score)
        };
        s.text_c(&detail, 150.0, 183.0, s.fit_size(&detail, 13.0, 214.0), INK, 1.0);
        s.rect(50.0, 215.0, 200.0, 40.0, 20.0, hex(0x3479c7), 1.0);
        s.text_c(if failed { tr(&["Retry level", "重试本关"]) } else { tr(&["Next level", "下一关"]) }, 150.0, 235.0, 16.0, hex(0xffffff), 1.0);
    }
}
impl Game for Boarding {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS];
    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let sheet = Image::png(art::SHEET);
        let sprites = art::ARTS.iter().map(|a| sheet.crop(a.x, a.y, a.w, a.h)).collect();
        Self { s: blank(), sprites, flight: None, hover: None, keys: false, blocked: None }
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
        if let Some(f) = &mut self.flight {
            f.t += dt;
            if f.t >= 650.0 {
                self.flight = None;
            }
        }
        if let Some((_, t)) = &mut self.blocked {
            *t -= dt;
            if *t <= 0.0 {
                self.blocked = None;
            }
        }
        let mut ev = self.finish();
        if self.s.phase == PLAY {
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
            let d = self.s.rep.dirs(input);
            if d != 0 {
                self.keys = true;
                self.hover = None;
            }
            if d & wg::LEFT != 0 {
                self.s.cx = (self.s.cx - 1).max(0);
            }
            if d & wg::RIGHT != 0 {
                self.s.cx = (self.s.cx + 1).min(self.s.yard.w - 1);
            }
            if d & wg::UP != 0 {
                self.s.cy = (self.s.cy - 1).max(0);
            }
            if d & wg::DOWN != 0 {
                self.s.cy = (self.s.cy + 1).min(self.s.yard.h - 1);
            }
            if input.pressed & wg::A != 0 {
                ev |= self.tap((self.s.cy * self.s.yard.w + self.s.cx) as usize);
            }
        } else if input.pressed & wg::A != 0 {
            ev |= self.next();
        }
        ev
    }
    fn key(&mut self, code: i32) -> bool {
        controls::is_restart_key(code) && self.restart() != 0
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        self.hover = if p.inside && !p.touch { self.hit(p.x, p.y) } else { None };
        if !p.inside || p.pressed & wg::M_LEFT == 0 {
            return 0;
        }
        self.keys = false;
        if self.controls().hit(p.x, p.y).is_some() {
            return self.restart();
        }
        if self.s.phase != PLAY {
            return if p.x >= 50.0 && p.x <= 250.0 && p.y >= 215.0 && p.y <= 255.0 { self.next() } else { 0 };
        }
        if let Some(i) = self.hit(p.x, p.y) {
            self.s.cx = i as i32 % self.s.yard.w;
            self.s.cy = i as i32 / self.s.yard.w;
            return self.tap(i);
        }
        0
    }
    fn picture(&mut self) -> u32 {
        let mut s = self.s;
        s.time_ms = 0;
        s.rep = KeyRepeat(0);
        wg::hash(wg::hash(0, wg::bytes_of(&s)), wg::bytes_of(&[self.hover.map_or(-1, |i| i as i32), self.keys as i32]))
    }
    fn moving(&self) -> bool {
        self.flight.is_some() || self.blocked.is_some()
    }
    fn draw(&mut self, s: &mut Screen) {
        let yard = self.flight.as_ref().map_or(self.s.yard, |f| f.before);
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(0xeaf2e8), 1.0);
        s.rect(0.0, 39.0, 300.0, 74.0, 0.0, hex(0x65758b), 1.0);
        for k in 0..8 {
            s.rect(k as f32 * 42.0, 105.0, 24.0, 2.0, 1.0, hex(0xf7e6a9), 1.0);
        }
        s.text(&format!("{} {}", tr(&["Level", "关卡"]), self.s.level + 1), 12.0, 21.0, 17.0, INK, 1.0);
        s.text_r(&format!("{}/{}", yard.boarded, yard.buses * 3), 250.0, 21.0, 14.0, INK, 1.0);
        self.controls().draw(s, None, |_| true, 300.0);
        self.draw_bus(s, &yard);
        s.text_c(&format!("{}  {}/{}", tr(&["Waiting", "候车"]), yard.waiting(), BENCH), 150.0, 126.0, 12.0, INK, 1.0);
        s.rect(43.0, 158.0, 214.0, 9.0, 3.0, hex(0xb4906a), 1.0);
        for k in 0..BENCH {
            let x = 66.0 + k as f32 * 42.0;
            s.disc(x, 152.0, 14.0, hex(0xd7e0dc), 1.0);
            if yard.bench[k] != 0 {
                self.person(s, yard.bench[k], x, 156.0, 33.0, false);
            } else {
                s.text_c("·", x, 152.0, 18.0, hex(0x809a99), 1.0);
            }
        }
        s.text_c(tr(&["Clear a path to the front", "清出通往前方的路"]), 150.0, 185.0, 11.0, hex(0x526979), 1.0);
        let (bx, by, c) = self.board_box();
        s.rect(bx - 5.0, by - 4.0, c * yard.w as f32 + 10.0, c * yard.h as f32 + 8.0, 8.0, hex(0xcadad5), 1.0);
        for i in 0..(yard.w * yard.h) as usize {
            let (x, y) = self.place(i);
            let selected = self.hover == Some(i) || (self.keys && i as i32 == self.s.cy * yard.w + self.s.cx);
            let blocked = self.blocked.is_some_and(|b| b.0 == i);
            s.rect(
                x - c * 0.46,
                y - c * 0.76,
                c * 0.92,
                c * 0.92,
                5.0,
                hex(if blocked {
                    0xf0a09a
                } else if selected {
                    0xffffff
                } else {
                    0xe3ece7
                }),
                1.0,
            );
            if yard.cell[i] != 0 && !self.flight.as_ref().is_some_and(|f| f.selected == i) {
                self.person(s, yard.cell[i], x, y, c * 0.92, false);
            }
        }
        if let Some(f) = &self.flight {
            let lengths: Vec<f32> = f.path.windows(2).map(|p| (p[1].0 - p[0].0).hypot(p[1].1 - p[0].1)).collect();
            let mut dist = lengths.iter().sum::<f32>() * (f.t / 650.0).min(1.0);
            for (p, len) in f.path.windows(2).zip(lengths) {
                if dist <= len {
                    let k = dist / len.max(0.001);
                    self.person(s, f.before.cell[f.selected], p[0].0 + (p[1].0 - p[0].0) * k, p[0].1 + (p[1].1 - p[0].1) * k, c * 0.92, true);
                    break;
                }
                dist -= len;
            }
        }
        if matches!(self.s.phase, CLEARED | STUCK) {
            self.card(s);
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
        (self.s.cleared >= 0).then_some((self.s.cleared, self.s.cleared_score as u32))
    }
    fn stat(&self, i: usize) -> i32 {
        match i {
            0 => self.s.done,
            1 => self.s.all_boarded,
            2 => self.s.all_retries,
            3 => self.s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION
            && (0..LEVELS).contains(&s.level)
            && (0..LEVELS).contains(&s.opt[0])
            && (PLAY..=STUCK).contains(&s.phase)
            && s.yard.valid()
            && (0..s.yard.w).contains(&s.cx)
            && (0..s.yard.h).contains(&s.cy);
        self.flight = None;
        self.hover = None;
        self.blocked = None;
        ok
    }
}
impl wg::Board for Boarding {
    fn grid(&self) -> Option<(i32, i32)> {
        if matches!(self.s.phase, CLEARED | STUCK) { return Some((1, 1)); }
        Some((self.s.yard.w, self.s.yard.h))
    }
    fn cursor(&self) -> (i32, i32) {
        if matches!(self.s.phase, CLEARED | STUCK) { return (0, 0); }
        (self.s.cx, self.s.cy)
    }
    fn focus(&mut self, x: i32, y: i32) {
        if x >= 0 && y >= 0 && x < self.s.yard.w && y < self.s.yard.h {
            self.s.cx = x;
            self.s.cy = y;
            self.keys = true;
        }
    }
    fn rect(&self) -> (f32, f32, f32) {
        self.board_box()
    }
    fn cell_box(&self, x: i32, y: i32) -> Option<(f32, f32, f32, f32)> {
        if matches!(self.s.phase, CLEARED | STUCK) { return Some((50.0, 215.0, 200.0, 40.0)); }
        let (bx, by, c) = self.board_box();
        Some((bx + x as f32 * c, by + y as f32 * c, c, c))
    }
    fn cell(&self, x: i32, y: i32, out: &mut String) {
        if matches!(self.s.phase, CLEARED | STUCK) {
            out.push_str(if self.s.phase == STUCK { tr(&["Retry level", "重试本关"]) } else { tr(&["Next level", "下一关"]) });
            return;
        }
        if x < 0 || y < 0 || x >= self.s.yard.w || y >= self.s.yard.h {
            return;
        }
        let i = (y * self.s.yard.w + x) as usize;
        let c = self.s.yard.cell[i];
        if c == 0 {
            out.push_str(tr(&["Empty", "空"]));
            return;
        }
        out.push_str(&format!(
            "{} {}: {}",
            tr(&["Passenger", "乘客"]),
            c,
            if self.s.yard.path(i).is_some() { tr(&["path clear", "道路畅通"]) } else { tr(&["blocked", "被挡住"]) }
        ));
    }
    fn status(&self, out: &mut String) {
        out.push_str(&format!(
            "{} {}. {} {} ({}/3). {} {}/5. {} {:?}",
            tr(&["Level", "关卡"]),
            self.s.level + 1,
            tr(&["Bus", "巴士"]),
            self.s.yard.current(),
            self.s.yard.filled,
            tr(&["Waiting", "候车"]),
            self.s.yard.waiting(),
            tr(&["Next buses", "后续巴士"]),
            &self.s.yard.queue[(self.s.yard.bus + 1).min(self.s.yard.buses) as usize..(self.s.yard.bus + 3).min(self.s.yard.buses) as usize]
        ));
        if self.s.phase == STUCK {
            out.push_str(tr(&[". Bench full. Space retries.", "。候车区已满，按空格重试。"]));
        }
    }
    fn keys(&self) -> &'static str {
        tr(&["Arrows select a passenger. Space sends them along a clear path to the front. Match the bus colour; five waiting passengers lose. R retries. I reads the bus queue and bench.", "方向键选择乘客，空格沿空路送到前方。匹配巴士颜色，五个候车位满则失败。R重试，I读出巴士和候车区。"])
    }
    fn act(&mut self, x: i32, y: i32, _a: i32, said: &mut String) -> u32 {
        if matches!(self.s.phase, CLEARED | STUCK) {
            let ev = self.next();
            self.status(said);
            return ev;
        }
        self.focus(x, y);
        let ev = self.tap((self.s.cy * self.s.yard.w + self.s.cx) as usize);
        self.cell(x, y, said);
        self.status(said);
        ev
    }
    fn rev(&mut self) -> u32 {
        wg::hash(wg::hash(0, wg::bytes_of(&self.s.yard)), wg::bytes_of(&[self.s.phase, self.s.level, wg::lang() as i32]))
    }
}
wg::game!(Boarding);
wg::board!(Boarding);

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plays_every_level_and_restores_in_flight() {
        let mut g = Boarding::new();
        g.start(1);
        for l in 0..LEVELS {
            assert_eq!(g.s.level, l);
            let (_, order) = puzzle::deal_order(l);
            for i in order {
                assert_ne!(g.tap(i), 0);
                assert!(g.loaded());
                g.update(&Input { held: 0, pressed: 0, dt: 16 });
            }
            assert!(g.s.yard.won());
            if l < LEVELS - 1 {
                assert_eq!(g.s.phase, CLEARED);
                g.next();
            } else {
                assert!(g.over());
            }
        }
    }
    #[test]
    fn retry_restores_the_identical_board() {
        let mut g = Boarding::new();
        g.start(1);
        let first = g.s.yard;
        g.tap(0);
        g.restart();
        assert_eq!(first, g.s.yard);
        assert!(g.flight.is_none());
    }
}
