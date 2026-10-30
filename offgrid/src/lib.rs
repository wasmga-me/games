//! Offgrid - an original wasmga.me game. MIT licensed.
//! A board full of arrows. Tap an arrow to send it off the board the way it
//! points; if another arrow is in its way it bumps into it and comes back,
//! and that costs one of three lives. Clear the board to finish the level.
//! 30 levels, from 4 x 4 to 9 x 9. The page shows the setup, pause and game
//! over screens.
mod glyphs;
mod puzzle;

use puzzle::{Board, DX, DY, LEVELS, MAXN};
use wg::{hex, mix, tr, Game, Input, KeyRepeat, Pointer, Screen};

const VERSION: u32 = 1;

/// options, in the catalog `setup` order
const O_LEVEL: usize = 0;
const O_THEME: usize = 1;
const THEMES: usize = 3;

const PLAY: i32 = 0;
const CLEARED: i32 = 1;
const DONE: i32 = 2;
const LOST: i32 = 3;

const LIVES: i32 = 3;
/// an arrow leaving, ms per square, and a bump there and back
const FLY_MS: f32 = 55.0;
const BUMP_MS: f32 = 320.0;
/// the last arrow's way off, or the last life's bump, plays out before game over
const END_MS: i32 = 900;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 2],
    level: i32,
    phase: i32,
    board: Board,
    lives: i32,
    level_ms: i32,
    time_ms: i32,
    /// arrows sent off, this level and the whole run
    gone: i32,
    all_gone: i32,
    mistakes: i32,
    done: i32,
    score: i32,
    cleared: i32,
    cleared_score: i32,
    end_ms: i32,
    /// keyboard cursor, a square
    cx: i32,
    cy: i32,
    rep: KeyRepeat,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}

struct Theme {
    bg: u32,
    bg2: u32,
    well: u32,
    dot: u32,
    text: u32,
    dim: u32,
    overlay: u32,
    /// arrows by direction: up, right, down, left
    arrow: [u32; 4],
    ink: u32,
}

const THEME: [Theme; THEMES] = [
    // Night
    Theme { bg: 0x1a1f33, bg2: 0x10131f, well: 0x262c45, dot: 0x3a4263, text: 0xf2f4fb, dim: 0x8f97b8, overlay: 0x0b0d16, arrow: [0x4f7cff, 0xff6b5a, 0x35c48a, 0xffbe3d], ink: 0xffffff },
    // Paper
    Theme { bg: 0xf6f1e6, bg2: 0xe7dfcc, well: 0xe4dac6, dot: 0xcdbfa6, text: 0x3a2f24, dim: 0x8a7a66, overlay: 0x5a4a38, arrow: [0x3d6fd6, 0xd9503f, 0x2c9a6a, 0xd99a1e], ink: 0xffffff },
    // Mint
    Theme { bg: 0xe3f2ec, bg2: 0xcfe6dc, well: 0xc2dccf, dot: 0xa5c7b6, text: 0x1f4535, dim: 0x5f8574, overlay: 0x1f4535, arrow: [0x3a7bd5, 0xe0604a, 0x1f8f62, 0xe39b20], ink: 0xffffff },
];

#[derive(Clone, Copy, PartialEq)]
enum Hit {
    None,
    Cell(i32, i32),
    Next,
}

/// arrows on the move, drawn over the board; the save already has the result
#[derive(Clone, Copy)]
struct Move {
    x: i32,
    y: i32,
    dir: u8,
    /// squares it travels: off the board, or to the arrow in its way
    steps: i32,
    bump: bool,
    t: f32,
}

struct Offgrid {
    s: Save,
    tall: bool,
    hover: Hit,
    keys: bool,
    moves: Vec<Move>,
    /// the arrow an arrow bumped into shakes, ms left
    shake: (i32, i32, f32),
}

impl Offgrid {
    fn theme(&self) -> &'static Theme {
        &THEME[self.s.opt[O_THEME] as usize]
    }
    fn plan_wh(&self) -> (f32, f32) {
        if self.tall { (300.0, 400.0) } else { (400.0, 300.0) }
    }
    /// the board's box: x, y, side, and a square's side
    fn board_box(&self) -> (f32, f32, f32, f32) {
        let w = self.plan_wh().0;
        let side = if self.tall { 280.0 } else { 244.0 };
        let (x, y) = ((w - side) / 2.0, if self.tall { 70.0 } else { 46.0 });
        (x, y, side, side / self.s.board.n as f32)
    }
    /// the card over the board, in the middle of the screen
    fn card(&self) -> (f32, f32, f32, f32) {
        let (w, h) = self.plan_wh();
        let (pw, ph) = (248.0, 158.0);
        (w / 2.0 - pw / 2.0, h / 2.0 - ph / 2.0, pw, ph)
    }
    /// its button, inside the card along the bottom
    fn next_box(&self) -> (f32, f32, f32, f32) {
        let (px, py, pw, ph) = self.card();
        let (bw, bh) = (pw - 48.0, 42.0);
        (px + (pw - bw) / 2.0, py + ph - bh - 20.0, bw, bh)
    }
    fn hit(&self, x: f32, y: f32) -> Hit {
        if self.s.phase != PLAY {
            let b = self.next_box();
            let on = x >= b.0 && x < b.0 + b.2 && y >= b.1 && y < b.1 + b.3;
            return if self.s.phase == CLEARED && on { Hit::Next } else { Hit::None };
        }
        let (bx, by, _, c) = self.board_box();
        let (gx, gy) = (((x - bx) / c).floor() as i32, ((y - by) / c).floor() as i32);
        if self.s.board.inside(gx, gy) { Hit::Cell(gx, gy) } else { Hit::None }
    }

    fn begin(&mut self, level: i32) {
        let s = &mut self.s;
        s.level = level;
        s.phase = PLAY;
        s.board = puzzle::deal(level);
        s.lives = LIVES;
        s.level_ms = 0;
        s.gone = 0;
        s.end_ms = 0;
        let n = s.board.n;
        s.cx = s.cx.clamp(0, n - 1);
        s.cy = s.cy.clamp(0, n - 1);
        self.moves.clear();
    }

    fn tap(&mut self, x: i32, y: i32) -> u32 {
        if self.s.phase != PLAY || !self.s.board.inside(x, y) {
            return 0;
        }
        let d = self.s.board.at(x, y);
        // an arrow already bumping waits until it is back
        if d == 0 || self.moves.iter().any(|m| m.bump && m.x == x && m.y == y) {
            return 0;
        }
        match self.s.board.blocked(x, y) {
            None => {
                let steps = self.s.board.exit(x, y);
                self.s.board.cell[(y * self.s.board.n + x) as usize] = 0;
                self.moves.push(Move { x, y, dir: d, steps, bump: false, t: 0.0 });
                self.s.gone += 1;
                self.s.all_gone += 1;
                if self.s.board.count() == 0 {
                    return self.clear_level();
                }
                wg::EV_BLIP
            }
            Some(steps) => {
                self.moves.push(Move { x, y, dir: d, steps, bump: true, t: 0.0 });
                let du = d as usize;
                self.shake = (x + DX[du] * (steps + 1), y + DY[du] * (steps + 1), 260.0);
                self.s.lives -= 1;
                self.s.mistakes += 1;
                if self.s.lives <= 0 {
                    self.s.phase = LOST;
                    self.s.end_ms = END_MS;
                    return wg::EV_LOSE;
                }
                wg::EV_HIT
            }
        }
    }

    fn clear_level(&mut self) -> u32 {
        let s = &mut self.s;
        let score = 10 * s.gone + 100 * s.lives;
        s.score += score;
        s.cleared = s.level;
        s.cleared_score = score;
        s.done += 1;
        if s.level + 1 >= LEVELS {
            s.phase = DONE;
            s.end_ms = END_MS;
            return wg::EV_LEVEL | wg::EV_WIN;
        }
        s.phase = CLEARED;
        wg::EV_LEVEL | wg::EV_WIN
    }

    fn next(&mut self) -> u32 {
        if self.s.phase != CLEARED {
            return 0;
        }
        self.begin(self.s.level + 1);
        wg::EV_BLIP
    }
}

/// an arrow tile with its top left at x, y, c units square
fn draw_arrow(s: &mut Screen, x: f32, y: f32, c: f32, dir: u8, face: u32, ink: u32, alpha: f32) {
    let g = (c * 0.07).max(1.0);
    let (w, r) = (c - 2.0 * g, c * 0.2);
    let lip = (c * 0.06).max(1.0);
    s.rect(x + g, y + g + lip * 0.5, w, w, r, mix(hex(0x000000), face, 0.7), alpha);
    s.rect_grad(x + g, y + g, w, w - lip * 0.5, r, mix(hex(0xffffff), face, 0.82), face, alpha);
    // the arrow: a shaft and a head, pointing right, then turned
    let (cx, cy) = (x + c / 2.0, y + c / 2.0);
    let k = c * 0.5;
    let pts: [(f32, f32); 7] = [
        (-0.30, -0.08),
        (0.02, -0.08),
        (0.02, -0.24),
        (0.32, 0.0),
        (0.02, 0.24),
        (0.02, 0.08),
        (-0.30, 0.08),
    ];
    let (ca, sa) = match dir {
        1 => (0.0, -1.0),
        2 => (1.0, 0.0),
        3 => (0.0, 1.0),
        _ => (-1.0, 0.0),
    };
    let turn = |(px, py): (f32, f32)| (cx + (px * ca - py * sa) * k * 1.5, cy + (px * sa + py * ca) * k * 1.5);
    // the head and the shaft as two convex shapes
    let head = [turn(pts[2]), turn(pts[3]), turn(pts[4])];
    let shaft = [turn(pts[0]), turn((0.06, -0.08)), turn((0.06, 0.08)), turn(pts[6])];
    s.poly(&shaft, ink, alpha);
    s.poly(&head, ink, alpha);
}

impl Game for Offgrid {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LEVELS, THEMES as i32];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        Offgrid {
            s: Save {
                version: VERSION,
                opt: [0, 0],
                level: 0,
                phase: PLAY,
                board: Board { n: 4, cell: [0; MAXN * MAXN] },
                lives: LIVES,
                level_ms: 0,
                time_ms: 0,
                gone: 0,
                all_gone: 0,
                mistakes: 0,
                done: 0,
                score: 0,
                cleared: -1,
                cleared_score: 0,
                end_ms: 0,
                cx: 0,
                cy: 0,
                rep: KeyRepeat(0),
            },
            tall: false,
            hover: Hit::None,
            keys: false,
            moves: Vec::new(),
            shake: (-1, -1, 0.0),
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
        self.s = Save { opt, ..Offgrid::new().s };
        self.begin(opt[O_LEVEL]);
        self.keys = false;
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        if h as f32 > w as f32 * 1.1 { (300.0, 400.0) } else { (400.0, 300.0) }
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        for m in self.moves.iter_mut() {
            m.t += dt / if m.bump { BUMP_MS } else { FLY_MS * (m.steps as f32 + 2.0) };
        }
        self.moves.retain(|m| m.t < 1.0);
        if self.shake.2 > 0.0 {
            self.shake.2 = (self.shake.2 - dt).max(0.0);
        }
        if self.s.phase == PLAY && self.s.gone + self.s.mistakes > 0 {
            self.s.level_ms = self.s.level_ms.saturating_add(input.dt);
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        if self.s.end_ms > 0 {
            self.s.end_ms -= input.dt;
            if self.s.end_ms <= 0 {
                self.s.end_ms = 0;
                ev |= wg::EV_GAMEOVER;
            }
        }
        let dirs = self.s.rep.dirs(input);
        if dirs != 0 {
            self.keys = true;
            self.hover = Hit::None;
        }
        let n = self.s.board.n;
        if self.s.phase == PLAY {
            if dirs & wg::LEFT != 0 { self.s.cx = (self.s.cx - 1).max(0); }
            if dirs & wg::RIGHT != 0 { self.s.cx = (self.s.cx + 1).min(n - 1); }
            if dirs & wg::UP != 0 { self.s.cy = (self.s.cy - 1).max(0); }
            if dirs & wg::DOWN != 0 { self.s.cy = (self.s.cy + 1).min(n - 1); }
            if input.pressed & wg::A != 0 {
                self.keys = true;
                ev |= self.tap(self.s.cx, self.s.cy);
            }
        } else if self.s.phase == CLEARED && input.pressed & wg::A != 0 {
            ev |= self.next();
        }
        ev
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.hover = Hit::None;
            return 0;
        }
        let h = self.hit(p.x, p.y);
        self.hover = if p.touch { Hit::None } else { h };
        if p.pressed & wg::M_LEFT != 0 {
            self.keys = false;
            return match h {
                Hit::Cell(x, y) => {
                    self.s.cx = x;
                    self.s.cy = y;
                    self.tap(x, y)
                }
                Hit::Next => self.next(),
                Hit::None => 0,
            };
        }
        0
    }

    fn picture(&mut self) -> u32 {
        let hv = match self.hover {
            Hit::None => -1,
            Hit::Cell(x, y) => y * 16 + x,
            Hit::Next => 1000,
        };
        let shown = [hv, self.keys as i32, self.tall as i32];
        let mut c = self.s;
        c.level_ms = 0;
        c.time_ms = 0;
        c.end_ms = 0;
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        !self.moves.is_empty() || self.shake.2 > 0.0
    }

    fn draw(&mut self, s: &mut Screen) {
        self.tall = s.ph > s.pw;
        let th = self.theme();
        let w = self.plan_wh().0;
        let n = self.s.board.n;
        let (bx, by, side, c) = self.board_box();
        let key = wg::hash(0, wg::bytes_of(&[self.s.opt[O_THEME], n, self.tall as i32]));
        s.background(key, |s| {
            let (x0, y0, x1, y1) = s.visible();
            s.rect_grad(x0, y0, x1 - x0, y1 - y0, 0.0, hex(th.bg), hex(th.bg2), 1.0);
            let pad = 6.0;
            s.shadow(bx - pad, by - pad + 2.0, side + 2.0 * pad, side + 2.0 * pad, 12.0, 8.0, 0, 0.25);
            s.rect(bx - pad, by - pad, side + 2.0 * pad, side + 2.0 * pad, 12.0, hex(th.well), 1.0);
            for y in 0..n {
                for x in 0..n {
                    s.disc(bx + (x as f32 + 0.5) * c, by + (y as f32 + 0.5) * c, (c * 0.06).max(1.0), hex(th.dot), 1.0);
                }
            }
        });

        // the way the hovered arrow would go
        let mark = match self.hover {
            Hit::Cell(x, y) => Some((x, y)),
            _ if self.keys && self.s.phase == PLAY => Some((self.s.cx, self.s.cy)),
            _ => None,
        };
        if let Some((x, y)) = mark {
            let d = self.s.board.at(x, y) as usize;
            if d != 0 {
                let col = hex(th.arrow[d - 1]);
                let (ax, ay) = (bx + (x as f32 + 0.5) * c, by + (y as f32 + 0.5) * c);
                let reach = match self.s.board.blocked(x, y) {
                    Some(k) => k as f32 + 0.5,
                    // past the edge of the board
                    None => self.s.board.exit(x, y) as f32 - 0.1,
                };
                let (ex, ey) = (ax + DX[d] as f32 * reach * c, ay + DY[d] as f32 * reach * c);
                s.line(ax, ay, ex, ey, c * 0.07, col, 0.45);
            }
            if self.keys {
                let (x0, y0) = (bx + x as f32 * c, by + y as f32 * c);
                s.rect(x0 - 1.5, y0 - 1.5, c + 3.0, c + 3.0, c * 0.24, hex(th.text), 0.55);
                s.rect(x0, y0, c, c, c * 0.2, hex(th.well), 1.0);
            }
        }

        let bumping = |m: &Move, x: i32, y: i32| m.bump && m.x == x && m.y == y;
        for y in 0..n {
            for x in 0..n {
                let d = self.s.board.at(x, y);
                if d == 0 || self.moves.iter().any(|m| bumping(m, x, y)) {
                    continue;
                }
                let (mut tx, mut ty) = (bx + x as f32 * c, by + y as f32 * c);
                if self.shake.2 > 0.0 && self.shake.0 == x && self.shake.1 == y {
                    let a = (self.shake.2 * 0.08).sin() * c * 0.06 * self.shake.2 / 260.0;
                    tx += a;
                    ty += a * 0.5;
                }
                let lit = mark == Some((x, y));
                let face = hex(th.arrow[d as usize - 1]);
                let face = if lit { mix(hex(0xffffff), face, 0.85) } else { face };
                draw_arrow(s, tx, ty, c, d, face, hex(th.ink), 1.0);
            }
        }
        // arrows on the move, clipped to the board's tray so they slide out of sight
        let pad = 6.0;
        s.set_clip(bx - pad, by - pad, side + 2.0 * pad, side + 2.0 * pad);
        for m in &self.moves {
            let du = m.dir as usize;
            let dist = if m.bump {
                // out to the arrow in the way and back
                let t = if m.t < 0.4 { wg::ease_out(m.t / 0.4) } else { 1.0 - wg::ease_out((m.t - 0.4) / 0.6) };
                t * (m.steps as f32 + 0.18)
            } else {
                m.t * m.t * (m.steps as f32 + 0.2)
            };
            let tx = bx + (m.x as f32 + DX[du] as f32 * dist) * c;
            let ty = by + (m.y as f32 + DY[du] as f32 * dist) * c;
            let face = hex(th.arrow[du - 1]);
            let face = if m.bump { mix(hex(0xe5484d), face, 0.8) } else { face };
            draw_arrow(s, tx, ty, c, m.dir, face, hex(th.ink), 1.0);
        }
        s.no_clip();

        // the top bar: level, lives, arrows left
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let ty = if self.tall { 24.0 } else { 22.0 };
        s.text(&format!("{} {}", level_l, self.s.level + 1), 14.0, ty, 17.0, hex(th.text), 1.0);
        for i in 0..LIVES {
            let on = i < self.s.lives;
            let x = w - 20.0 - (LIVES - 1 - i) as f32 * 16.0;
            if on {
                s.disc(x, ty, 5.5, hex(0xe5484d), 1.0);
            } else {
                s.ring(x, ty, 5.0, 1.4, hex(th.dim), 0.8);
            }
        }
        let left = self.s.board.count();
        let left_l = tr(&["Arrows left", "剩余箭头", "残りの矢印", "남은 화살표", "Flechas restantes", "Setas restantes", "Flèches restantes", "Pfeile übrig", "Осталось стрелок", "剩餘箭頭"]);
        let line = format!("{} {}", left_l, left);
        let size = s.fit_size(&line, 14.0, w - 190.0);
        s.text_c(&line, w / 2.0, ty, size, hex(th.dim), 1.0);

        if self.s.phase == CLEARED {
            self.draw_cleared(s, th, w);
        }
    }

    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.phase == DONE || self.s.phase == LOST
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
            1 => s.all_gone,
            2 => s.mistakes,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let n = s.board.n;
        let ok = s.version == VERSION
            && s.opt.iter().zip(Self::CHOICES).all(|(&v, &c)| (0..c).contains(&v))
            && (0..LEVELS).contains(&s.level)
            && (PLAY..=LOST).contains(&s.phase)
            && s.board.valid()
            && n == puzzle::side(s.level)
            && (0..=LIVES).contains(&s.lives)
            && (0..n).contains(&s.cx)
            && (0..n).contains(&s.cy)
            && s.end_ms >= 0;
        self.moves.clear();
        self.shake = (-1, -1, 0.0);
        self.hover = Hit::None;
        ok
    }
}

impl Offgrid {
    fn draw_cleared(&self, s: &mut Screen, th: &Theme, w: f32) {
        let (x0, y0, x1, y1) = s.visible();
        s.rect(x0, y0, x1 - x0, y1 - y0, 0.0, hex(th.overlay), 0.45);
        let (px, py, pw, ph) = self.card();
        s.shadow(px, py + 4.0, pw, ph, 16.0, 14.0, 0, 0.45);
        s.rect(px - 1.0, py - 1.0, pw + 2.0, ph + 2.0, 17.0, hex(th.text), 0.1);
        s.rect(px, py, pw, ph, 16.0, hex(th.bg), 1.0);
        let title = tr(&["Level cleared", "过关", "クリア", "레벨 클리어", "Nivel superado", "Nível concluído", "Niveau réussi", "Level geschafft", "Уровень пройден", "過關"]);
        s.text_c(title, w / 2.0, py + 34.0, 21.0, hex(th.text), 1.0);
        let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
        let lives_l = tr(&["Lives left", "剩余生命", "残りライフ", "남은 목숨", "Vidas", "Vidas", "Vies", "Leben übrig", "Жизни", "剩餘生命"]);
        let line = format!("{} +{}    {} {}", score_l, self.s.cleared_score, lives_l, self.s.lives);
        let size = s.fit_size(&line, 14.0, pw - 32.0);
        s.text_c(&line, w / 2.0, py + 64.0, size, hex(th.dim), 1.0);
        let b = self.next_box();
        let label = tr(&["Next level", "下一关", "次のレベル", "다음 레벨", "Siguiente nivel", "Próximo nível", "Niveau suivant", "Nächstes Level", "Следующий уровень", "下一關"]);
        let hv = self.hover == Hit::Next;
        let fill = hex(th.arrow[0]);
        s.rect(b.0, b.1, b.2, b.3, b.3 / 2.0, if hv { mix(hex(0xffffff), fill, 0.15) } else { fill }, 1.0);
        let size = s.fit_size(label, 16.0, b.2 - 16.0);
        s.text_c(label, b.0 + b.2 / 2.0, b.1 + b.3 / 2.0, size, hex(0xffffff), 1.0);
    }
}

/// The accessible board: the page lays a square over each one drawn, for
/// screen readers, keyboards and switches.
impl wg::Board for Offgrid {
    fn grid(&self) -> Option<(i32, i32)> {
        Some((self.s.board.n, self.s.board.n))
    }
    fn cursor(&self) -> (i32, i32) {
        (self.s.cx, self.s.cy)
    }
    fn focus(&mut self, x: i32, y: i32) {
        if self.s.board.inside(x, y) {
            self.s.cx = x;
            self.s.cy = y;
            self.keys = true;
            self.hover = Hit::None;
        }
    }
    fn rect(&self) -> (f32, f32, f32) {
        let (bx, by, _, c) = self.board_box();
        (bx, by, c)
    }
    fn cell(&self, x: i32, y: i32, out: &mut String) {
        let d = self.s.board.at(x, y);
        if d == 0 {
            out.push_str(tr(&["empty", "空", "空き", "빈 칸", "vacía", "vazia", "vide", "leer", "пусто"]));
            return;
        }
        out.push_str(match d {
            1 => tr(&["arrow up", "向上箭头", "上向きの矢印", "위쪽 화살표", "flecha arriba", "seta para cima", "flèche vers le haut", "Pfeil nach oben", "стрелка вверх", "向上箭頭"]),
            2 => tr(&["arrow right", "向右箭头", "右向きの矢印", "오른쪽 화살표", "flecha a la derecha", "seta para a direita", "flèche vers la droite", "Pfeil nach rechts", "стрелка вправо", "向右箭頭"]),
            3 => tr(&["arrow down", "向下箭头", "下向きの矢印", "아래쪽 화살표", "flecha abajo", "seta para baixo", "flèche vers le bas", "Pfeil nach unten", "стрелка вниз", "向下箭頭"]),
            _ => tr(&["arrow left", "向左箭头", "左向きの矢印", "왼쪽 화살표", "flecha a la izquierda", "seta para a esquerda", "flèche vers la gauche", "Pfeil nach links", "стрелка влево", "向左箭頭"]),
        });
        out.push_str(", ");
        out.push_str(match self.s.board.blocked(x, y) {
            None => tr(&["free", "可以飞出", "出口が空いている", "나갈 길 열림", "salida libre", "saída livre", "sortie libre", "Weg frei", "путь свободен", "可以飛出"]),
            Some(_) => tr(&["blocked", "被挡住", "ふさがれている", "막힘", "bloqueada", "bloqueada", "bloquée", "blockiert", "заблокирована", "被擋住"]),
        });
    }
    fn status(&self, out: &mut String) {
        let level_l = tr(&["Level", "关卡", "レベル", "레벨", "Nivel", "Nível", "Niveau", "Level", "Уровень", "關卡"]);
        let lives_l = tr(&["lives", "生命", "ライフ", "목숨", "vidas", "vidas", "vies", "Leben", "жизни"]);
        let left_l = tr(&["arrows left", "剩余箭头", "残りの矢印", "남은 화살표", "flechas restantes", "setas restantes", "flèches restantes", "Pfeile übrig", "осталось стрелок", "剩餘箭頭"]);
        out.push_str(&format!("{} {}, {} {}, {} {}", level_l, self.s.level + 1, lives_l, self.s.lives, left_l, self.s.board.count()));
    }
    fn keys(&self) -> &'static str {
        tr(&[
            "Arrows move, Space sends the arrow off the board, I reads level, lives and arrows left.",
            "方向键移动，空格让箭头飞出，I 读出关卡、生命和剩余箭头。",
            "矢印キーで移動、スペースで矢印を飛ばす、I でレベル、ライフ、残りの矢印。",
            "화살표로 이동, 스페이스로 화살표 보내기, I로 레벨, 목숨, 남은 화살표.",
            "Flechas para moverte, Espacio saca la flecha, I lee nivel, vidas y flechas.",
            "Setas movem, Espaço tira a seta, I lê nível, vidas e setas.",
            "Flèches pour bouger, Espace fait sortir la flèche, I lit niveau, vies et flèches.",
            "Pfeiltasten bewegen, Leertaste schickt den Pfeil hinaus, I liest Level, Leben und Pfeile vor.",
            "Стрелки двигают, пробел выпускает стрелку, I читает уровень, жизни и стрелки.",
            "方向鍵移動，空白鍵讓箭頭飛出，I 讀出關卡、生命和剩餘箭頭。",
        ])
    }
    fn act(&mut self, x: i32, y: i32, a: i32, said: &mut String) -> u32 {
        if self.s.phase == CLEARED {
            let ev = self.next();
            self.status(said);
            return ev;
        }
        self.focus(x, y);
        if a != 0 || self.s.board.at(x, y) == 0 {
            self.cell(x, y, said);
            return 0;
        }
        let lives = self.s.lives;
        let ev = self.tap(x, y);
        said.push_str(if self.s.phase == CLEARED || self.s.phase == DONE {
            tr(&["Level cleared! Space for the next level.", "过关！按空格进入下一关。", "クリア！スペースで次のレベル。", "레벨 클리어! 스페이스로 다음 레벨.", "¡Nivel superado! Espacio para el siguiente.", "Nível concluído! Espaço para o próximo.", "Niveau réussi ! Espace pour le suivant.", "Level geschafft! Leertaste für das nächste.", "Уровень пройден! Пробел для следующего.", "過關！按空白鍵進入下一關。"])
        } else if self.s.phase == LOST {
            tr(&["Blocked. Out of lives, game over.", "被挡住。生命用完，游戏结束。", "ぶつかった。ライフがなくなりゲームオーバー。", "막힘. 목숨이 없어 게임 오버.", "Bloqueada. Sin vidas, fin de la partida.", "Bloqueada. Sem vidas, fim de jogo.", "Bloquée. Plus de vies, partie terminée.", "Blockiert. Keine Leben mehr, Spiel vorbei.", "Заблокирована. Жизни кончились, игра окончена.", "被擋住。生命用完，遊戲結束。"])
        } else if self.s.lives < lives {
            tr(&["Blocked, one life lost.", "被挡住，失去一条生命。", "ぶつかった、ライフを1つ失った。", "막힘, 목숨 하나를 잃었습니다.", "Bloqueada, pierdes una vida.", "Bloqueada, perdeu uma vida.", "Bloquée, une vie perdue.", "Blockiert, ein Leben weg.", "Заблокирована, минус жизнь.", "被擋住，失去一條生命。"])
        } else if ev != 0 {
            tr(&["Gone.", "飞出去了。", "飛んでいった。", "나갔습니다.", "Fuera.", "Saiu.", "Partie.", "Weg.", "Улетела.", "飛出去了。"])
        } else {
            ""
        });
        ev
    }
    fn rev(&mut self) -> u32 {
        let v = [self.s.phase, self.s.level, self.s.lives, wg::lang() as i32];
        wg::hash(wg::hash(0, wg::bytes_of(&self.s.board)), wg::bytes_of(&v))
    }
}

wg::game!(Offgrid);
wg::board!(Offgrid);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Offgrid, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    /// taps a free arrow, or a blocked one when `wrong`
    fn pick(g: &Offgrid, wrong: bool) -> Option<(i32, i32)> {
        let b = &g.s.board;
        (0..b.n).flat_map(|y| (0..b.n).map(move |x| (x, y))).find(|&(x, y)| b.at(x, y) != 0 && b.blocked(x, y).is_some() == wrong)
    }

    #[test]
    fn plays_every_level() {
        let mut g = Offgrid::new();
        g.start(3);
        for level in 0..LEVELS {
            assert_eq!(g.s.level, level);
            // two mistakes, then clear the board
            for _ in 0..2 {
                if let Some((x, y)) = pick(&g, true) {
                    g.tap(x, y);
                    run(&mut g, 400);
                }
            }
            let mut ev = 0;
            while let Some((x, y)) = pick(&g, false) {
                ev |= g.tap(x, y);
                ev |= run(&mut g, 50);
                assert!(g.loaded_check());
            }
            assert_eq!(g.s.board.count(), 0, "level {} stuck", level + 1);
            assert!(ev & wg::EV_LEVEL != 0);
            if level + 1 < LEVELS {
                assert_eq!(g.s.phase, CLEARED);
                g.next();
            } else {
                assert!(run(&mut g, 1000) & wg::EV_GAMEOVER != 0);
                assert!(g.over());
            }
        }
        println!("score {} mistakes {}", g.score(), g.s.mistakes);
        // three mistakes end the run
        g.start(1);
        for _ in 0..3 {
            let (x, y) = pick(&g, true).unwrap();
            g.tap(x, y);
            run(&mut g, 400);
        }
        assert!(g.over());
    }

    impl Offgrid {
        fn loaded_check(&mut self) -> bool {
            let keep = std::mem::take(&mut self.moves);
            let ok = self.loaded();
            self.moves = keep;
            ok
        }
    }
}
