//! Slider - an original wasmga.me game. MIT licensed.
//! Put the tiles of an n x n sliding puzzle back in order, 1 to n²-1, with
//! the gap in the bottom right. Click or tap a tile in the gap's row or
//! column to slide it (and any tiles between) into the gap, swipe, or use
//! the arrows. The page shows the setup, pause and game over screens.
mod puzzle;
mod glyphs;

use puzzle::{Board, MAXN};
use wg::{hex, mix, Game, Input, KeyRepeat, Pointer, Rng, Screen, Sprite};

const VERSION: u32 = 1;
/// The solved board stays in view this long before game over.
const END_MS: i32 = 1300;

/// options, in the catalog `setup` order
const O_SIZE: usize = 0;
const O_STYLE: usize = 1;
const O_THEME: usize = 2;
const SIZES: [i32; 6] = [3, 4, 5, 6, 7, 8];
const THEMES: usize = 4;

const PLAY: i32 = 0;
const SOLVED: i32 = 1;

/// The board's square, in plan units, and the panel beside or under it.
const BX: f32 = 12.0;
const BY: f32 = 12.0;
const BS: f32 = 276.0;
const PAD: f32 = 6.0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 3],
    rng: Rng,
    board: Board,
    phase: i32,
    end_ms: i32,
    /// tiles moved, and slides (a slide can move a whole row of tiles)
    moves: i32,
    slides: i32,
    time_ms: i32,
    /// how far the tiles were from home at the start, in moves
    start_dist: i32,
    score: i32,
    rep: KeyRepeat,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}

struct Theme {
    bg: u32,
    well: u32,
    tile_hi: u32,
    tile: u32,
    tile_lo: u32,
    ink: u32,
    dim: u32,
    panel: u32,
    /// numbers on the panel
    text: u32,
}

const THEME: [Theme; THEMES] = [
    // Walnut: maple tiles on a dark wood tray
    Theme { bg: 0x2a1d17, well: 0x1b120e, tile_hi: 0xf6dfb4, tile: 0xe8c48a, tile_lo: 0xb98a52, ink: 0x3b2618, dim: 0xb59a84, panel: 0x36261e, text: 0xf6dfb4 },
    // Night
    Theme { bg: 0x161a2e, well: 0x0d1020, tile_hi: 0x6d78f2, tile: 0x4f5bd5, tile_lo: 0x2f3896, ink: 0xf5f3ff, dim: 0x8d93c0, panel: 0x20253f, text: 0xf5f3ff },
    // Mint
    Theme { bg: 0xe6f2ec, well: 0xc4dccf, tile_hi: 0xffffff, tile: 0xf3faf6, tile_lo: 0x9cc2ad, ink: 0x1f4535, dim: 0x5f8574, panel: 0xd5e8de, text: 0x1f4535 },
    // Coral
    Theme { bg: 0xfff1e8, well: 0xf2d4c4, tile_hi: 0xff9f86, tile: 0xff7a5c, tile_lo: 0xc94a33, ink: 0xffffff, dim: 0xa86f5d, panel: 0xfbe0d2, text: 0x5a2a1c },
];

/// a colour from hue (degrees), saturation and value (0..1)
fn hsv(h: f32, s: f32, v: f32) -> u32 {
    let h = (h % 360.0 + 360.0) % 360.0 / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let to = |f: f32| ((f + m) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
    wg::rgb(to(r), to(g), to(b))
}

struct Slider {
    s: Save,
    /// not saved: each tile's offset from its square, in squares, while it slides
    off: [(f32, f32); MAXN * MAXN],
    /// the solved wave, 0..1
    wave: f32,
    hover: i32,
    press: Option<(f32, f32, i32)>,
    tiles: Vec<Sprite>,
    /// whether each tile's sprite was made as in place
    tile_home: Vec<i32>,
}

fn n_of(opt: &[i32; 3]) -> i32 {
    SIZES[opt[O_SIZE] as usize]
}

/// The panel's box: beside the board on a wide screen, under it on a tall one.
fn panel(s: &Screen) -> (f32, f32, f32, f32) {
    if s.ph > s.pw {
        (BX, BY + BS + 12.0, BS, 88.0)
    } else {
        (BX + BS + 12.0, BY, 88.0, BS)
    }
}

impl Slider {
    fn n(&self) -> i32 {
        n_of(&self.s.opt)
    }
    fn theme(&self) -> &'static Theme {
        &THEME[self.s.opt[O_THEME] as usize]
    }
    fn cell(&self) -> f32 {
        (BS - 2.0 * PAD) / self.n() as f32
    }
    /// the square under plan point x, y, or -1
    fn square_at(&self, x: f32, y: f32) -> i32 {
        let (c, n) = (self.cell(), self.n());
        let (gx, gy) = (((x - BX - PAD) / c).floor() as i32, ((y - BY - PAD) / c).floor() as i32);
        if gx < 0 || gy < 0 || gx >= n || gy >= n {
            -1
        } else {
            gy * n + gx
        }
    }

    /// slides the tiles from square sq into the gap; returns events
    fn slide(&mut self, sq: i32) -> u32 {
        if self.s.phase != PLAY {
            return 0;
        }
        let mut moved = [0i32; MAXN];
        let n = self.n();
        let gap_before = self.s.board.gap;
        let k = self.s.board.slide_to(sq, &mut moved);
        if k == 0 {
            return 0;
        }
        // each moved tile now sits one square toward where the gap was
        let (dx, dy) = ((gap_before % n - sq % n).signum(), (gap_before / n - sq / n).signum());
        for &from in &moved[..k as usize] {
            let to = from + dy * n + dx;
            let t = self.s.board.tile[to as usize] as usize;
            self.off[t].0 -= dx as f32;
            self.off[t].1 -= dy as f32;
        }
        self.s.moves += k;
        self.s.slides += 1;
        if self.s.board.is_solved() {
            self.s.phase = SOLVED;
            self.s.end_ms = END_MS;
            let secs = self.s.time_ms / 1000;
            let n4 = (n * n * n * n) as i64;
            self.s.score = ((n4 * 1000) / (self.s.moves as i64 + 2 * secs as i64 + (n * n) as i64)).max(1) as i32;
            self.wave = 0.0;
            return wg::EV_WIN;
        }
        wg::EV_BLIP
    }

    /// a tile next to the gap moving (dx, dy) into it
    fn push(&mut self, dx: i32, dy: i32) -> u32 {
        let sq = self.s.board.toward_gap(dx, dy);
        if sq < 0 {
            return 0;
        }
        self.slide(sq)
    }

    /// whether the tile on square sq would move if clicked
    fn would_move(&self, sq: i32) -> bool {
        let (n, g) = (self.n(), self.s.board.gap);
        sq >= 0 && sq != g && (sq % n == g % n || sq / n == g / n)
    }

    fn tile_colour(&self, t: u8) -> (u32, u32, u32, u32) {
        let th = self.theme();
        if self.s.opt[O_STYLE] == 0 {
            return (hex(th.tile_hi), hex(th.tile), hex(th.tile_lo), hex(th.ink));
        }
        let n = self.n();
        let h = Board::home(t);
        let (u, v) = ((h % n) as f32 / (n - 1) as f32, (h / n) as f32 / (n - 1) as f32);
        // each row its own hue, red at the top to violet at the bottom,
        // a little lighter along the row
        let face = hsv(v * 285.0 - u * 18.0 + 8.0, 0.62 - 0.12 * u, 0.9 + 0.08 * u);
        let r = (face & 255) as f32;
        let g = (face >> 8 & 255) as f32;
        let b = (face >> 16 & 255) as f32;
        let light = (0.3 * r + 0.59 * g + 0.11 * b) / 255.0;
        let ink = if light > 0.62 { hex(0x2a2320) } else { hex(0xffffff) };
        (mix(hex(0xffffff), face, 0.28), face, mix(hex(0x000000), face, 0.28), ink)
    }
}

/// a tile drawn with its top left at x, y, c units square
fn draw_tile(s: &mut Screen, x: f32, y: f32, c: f32, label: &str, (hi, face, lo, ink): (u32, u32, u32, u32), well: u32) {
    let g = (c * 0.045).max(1.2);
    let (w, r, lip) = (c - 2.0 * g, c * 0.14, (c * 0.07).max(1.4));
    s.rect(x - 2.0, y - 2.0, c + 4.0, c + 4.0, 0.0, well, 1.0);
    s.rect(x + g, y + g, w, w, r, lo, 1.0);
    s.rect_grad(x + g, y + g, w, w - lip, r, hi, face, 1.0);
    s.rect_grad(x + g + lip * 0.7, y + g + lip * 0.7, w - lip * 1.4, w - lip * 2.4, r * 0.75, mix(hi, face, 0.35), face, 1.0);
    let size = s.fit_size(label, c * 0.46, w * 0.8);
    s.text_c(label, x + c / 2.0, y + c / 2.0 - lip * 0.5, size, ink, 1.0);
}

impl Game for Slider {
    type Save = Save;
    const CHOICES: &'static [i32] = &[SIZES.len() as i32, 2, THEMES as i32];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        let opt = [1, 0, 0];
        let n = n_of(&opt);
        Slider {
            s: Save {
                version: VERSION,
                opt,
                rng: Rng::new(1),
                board: Board::solved(n),
                phase: PLAY,
                end_ms: 0,
                moves: 0,
                slides: 0,
                time_ms: 0,
                start_dist: 0,
                score: 0,
                rep: KeyRepeat(0),
            },
            off: [(0.0, 0.0); MAXN * MAXN],
            wave: 1.0,
            hover: -1,
            press: None,
            tiles: Vec::new(),
            tile_home: Vec::new(),
        }
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        let opt = self.s.opt;
        let mut rng = Rng::new(seed);
        let board = Board::shuffled(n_of(&opt), &mut rng);
        self.s = Save { opt, rng, board, start_dist: board.distance(), ..Slider::new().s };
        self.off = [(0.0, 0.0); MAXN * MAXN];
        self.wave = 1.0;
        self.press = None;
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        if h as f32 > w as f32 * 1.15 {
            (300.0, 400.0)
        } else {
            (400.0, 300.0)
        }
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        // a sliding tile closes half its distance every 30 ms
        let k = 0.5f32.powf(dt / 30.0);
        for o in self.off.iter_mut() {
            o.0 *= k;
            o.1 *= k;
            if o.0.abs() < 0.004 {
                o.0 = 0.0;
            }
            if o.1.abs() < 0.004 {
                o.1 = 0.0;
            }
        }
        if self.s.phase == SOLVED {
            self.wave = (self.wave + dt / 1000.0).min(1.0);
            if self.s.end_ms > 0 {
                self.s.end_ms -= input.dt;
                if self.s.end_ms <= 0 {
                    self.s.end_ms = 0;
                    return wg::EV_GAMEOVER;
                }
            }
            return 0;
        }
        if self.s.moves > 0 {
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        let dirs = self.s.rep.dirs(input);
        let mut ev = 0;
        // an arrow moves the tile beside the gap that way
        if dirs & wg::LEFT != 0 { ev |= self.push(-1, 0); }
        if dirs & wg::RIGHT != 0 { ev |= self.push(1, 0); }
        if dirs & wg::UP != 0 { ev |= self.push(0, -1); }
        if dirs & wg::DOWN != 0 { ev |= self.push(0, 1); }
        if dirs != 0 {
            self.hover = -1;
        }
        ev
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.hover = -1;
            self.press = None;
            return 0;
        }
        let sq = self.square_at(p.x, p.y);
        // fingers don't hover, before or just after a tap
        self.hover = if p.touch || self.press.is_some() { -1 } else { sq };
        let mut ev = 0;
        if p.pressed & wg::M_LEFT != 0 {
            if p.touch {
                // a touch acts when it lifts: a tap slides, a swipe pushes
                self.press = Some((p.x, p.y, sq));
            } else {
                ev |= self.slide(sq);
            }
        }
        if p.released & wg::M_LEFT != 0 {
            if let Some((x0, y0, sq0)) = self.press.take() {
                let (dx, dy) = (p.x - x0, p.y - y0);
                let far = self.cell() * 0.3;
                if dx.abs().max(dy.abs()) < far.max(8.0) {
                    ev |= self.slide(sq0);
                } else if dx.abs() > dy.abs() {
                    // a swipe that starts on a tile in the gap's line slides it there
                    let along = sq0 >= 0 && sq0 / self.n() == self.s.board.gap / self.n() && (self.s.board.gap - sq0).signum() == dx.signum() as i32;
                    ev |= if along { self.slide(sq0) } else { self.push(dx.signum() as i32, 0) };
                } else {
                    let along = sq0 >= 0 && sq0 % self.n() == self.s.board.gap % self.n() && (self.s.board.gap - sq0).signum() == dy.signum() as i32;
                    ev |= if along { self.slide(sq0) } else { self.push(0, dy.signum() as i32) };
                }
            }
        }
        ev
    }

    fn picture(&mut self) -> u32 {
        let s = &self.s;
        let shown = [s.opt[0], s.opt[1], s.opt[2], s.phase, s.moves, s.time_ms / 1000, self.hover];
        wg::hash(wg::hash(0, wg::bytes_of(&s.board)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        (self.s.phase == SOLVED && self.wave < 1.0) || self.off.iter().any(|o| o.0 != 0.0 || o.1 != 0.0)
    }

    fn draw(&mut self, s: &mut Screen) {
        let th = self.theme();
        let n = self.n();
        let c = self.cell();
        let (px, py, pw, ph) = panel(s);
        let tall = s.ph > s.pw;
        let key = wg::hash(0, wg::bytes_of(&[self.s.opt[O_THEME], n, tall as i32]));
        s.background(key, |s| {
            s.clear(hex(th.bg));
            s.shadow(BX, BY + 2.0, BS, BS, 12.0, 8.0, 0, 0.35);
            s.rect(BX, BY, BS, BS, 12.0, hex(th.well), 1.0);
            s.rect(px, py, pw, ph, 12.0, hex(th.panel), 1.0);
        });

        // the tiles: each drawn once into a sprite, then copied
        let count = (n * n) as usize;
        if self.tiles.len() < count {
            self.tiles.resize_with(count, Sprite::default);
            self.tile_home.resize(count, -1);
        }
        let style = self.s.opt[O_STYLE];
        let solved = self.s.phase == SOLVED;
        for sq in 0..count {
            let t = self.s.board.tile[sq];
            if t == 0 {
                continue;
            }
            let colours = self.tile_colour(t);
            let label = format!("{}", t);
            let key = wg::hash(0, wg::bytes_of(&[t as i32, n, style, self.s.opt[O_THEME]]));
            let well = hex(th.well);
            s.sprite_make(&mut self.tiles[t as usize - 1], key, 0.0, 0.0, c, c, |s| {
                draw_tile(s, 0.0, 0.0, c, &label, colours, well);
            });
            let (x, y) = ((sq as i32 % n) as f32, (sq as i32 / n) as f32);
            let (ox, oy) = self.off[t as usize];
            let (tx, ty) = (BX + PAD + (x + ox) * c, BY + PAD + (y + oy) * c);
            s.sprite(&self.tiles[t as usize - 1], tx, ty);
            if solved {
                // a light runs across the board from the top left
                let d = (x + y) / (2.0 * (n - 1) as f32);
                let w = 1.0 - ((self.wave * 1.5 - d) * 4.0 - 1.0).abs();
                if w > 0.0 {
                    s.rect(tx + c * 0.045, ty + c * 0.045, c * 0.91, c * 0.91, c * 0.14, hex(0xffffff), 0.35 * w);
                }
            } else if self.hover >= 0 && self.would_move(self.hover) && self.in_line(sq as i32) {
                s.rect(tx + c * 0.045, ty + c * 0.045, c * 0.91, c * 0.91, c * 0.14, hex(0xffffff), 0.14);
            }
        }
        if solved {
            // the last tile slides into the gap
            let t = (n * n) as u8;
            let a = wg::ease_out((self.wave * 2.0).min(1.0));
            let g = self.s.board.gap;
            let (x, y) = ((g % n) as f32, (g / n) as f32);
            let colours = self.tile_colour(t);
            let label = format!("{}", t);
            let well = hex(th.well);
            s.sprite_make(&mut self.tiles[t as usize - 1], wg::hash(0, wg::bytes_of(&[t as i32, n, style, self.s.opt[O_THEME]])), 0.0, 0.0, c, c, |s| {
                draw_tile(s, 0.0, 0.0, c, &label, colours, well);
            });
            let z = 0.6 + 0.4 * a;
            if a >= 1.0 {
                s.sprite(&self.tiles[t as usize - 1], BX + PAD + x * c, BY + PAD + y * c);
            } else {
                let (cx, cy) = (BX + PAD + (x + 0.5) * c, BY + PAD + (y + 0.5) * c);
                let zc = c * z;
                draw_tile_alpha(s, cx - zc / 2.0, cy - zc / 2.0, zc, colours, a);
            }
        }
        self.draw_panel(s, px, py, pw, ph, tall);
    }

    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.phase == SOLVED
    }
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        match i {
            0 => s.time_ms,
            1 => s.moves,
            2 => s.slides,
            3 => {
                let t = s.time_ms / 10;
                if t > 0 { (s.moves as i64 * 10000 / t as i64) as i32 } else { -1 }
            }
            4 => s.start_dist,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION
            && s.opt.iter().zip(Self::CHOICES).all(|(&v, &c)| (0..c).contains(&v))
            && s.board.valid(n_of(&s.opt))
            && (s.phase == PLAY || s.phase == SOLVED)
            && s.moves >= 0
            && s.slides >= 0
            && s.time_ms >= 0;
        self.off = [(0.0, 0.0); MAXN * MAXN];
        self.wave = 1.0;
        self.press = None;
        self.hover = -1;
        ok
    }
}

impl Slider {
    /// whether square sq is between the hovered square and the gap
    fn in_line(&self, sq: i32) -> bool {
        let (n, g, h) = (self.n(), self.s.board.gap, self.hover);
        let between = |a: i32, b: i32, v: i32| (a.min(b)..=a.max(b)).contains(&v);
        if h / n == g / n {
            sq / n == g / n && between(h % n, g % n, sq % n)
        } else {
            sq % n == g % n && between(h / n, g / n, sq / n)
        }
    }

    fn draw_panel(&self, s: &mut Screen, px: f32, py: f32, pw: f32, ph: f32, tall: bool) {
        let th = self.theme();
        let n = self.n();
        // the goal: a small solved board
        let gs = 64.0;
        let (gx, gy) = if tall { (px + 12.0, py + (ph - gs) / 2.0) } else { (px + (pw - gs) / 2.0, py + 12.0) };
        let gc = gs / n as f32;
        for i in 0..n * n - 1 {
            let (hi, face, lo, _) = self.tile_colour((i + 1) as u8);
            let (x, y) = (gx + (i % n) as f32 * gc, gy + (i / n) as f32 * gc);
            let inset = (gc * 0.08).max(0.5);
            s.rect(x + inset, y + inset, gc - 2.0 * inset, gc - 2.0 * inset, gc * 0.18, lo, 1.0);
            s.rect_grad(x + inset, y + inset, gc - 2.0 * inset, gc - 2.0 * inset - (gc * 0.08).max(0.6), gc * 0.18, hi, face, 1.0);
            if n <= 5 {
                let label = format!("{}", i + 1);
                s.text_c(&label, x + gc / 2.0, y + gc / 2.0, gc * 0.42, self.tile_colour((i + 1) as u8).3, 0.9);
            }
        }
        let rows: [(&str, String); 3] = [
            (wg::tr(&["Moves", "步数", "手数", "이동", "Movimientos", "Movimentos", "Coups", "Züge", "Ходы", "步數"]), format!("{}", self.s.moves)),
            (wg::tr(&["Time", "时间", "タイム", "시간", "Tiempo", "Tempo", "Temps", "Zeit", "Время", "時間"]), wg::clock(self.s.time_ms)),
            (wg::tr(&["In place", "已归位", "正しい位置", "제자리", "En su sitio", "No lugar", "En place", "Richtig", "На месте", "已歸位"]), format!("{} / {}", self.s.board.in_place(), n * n - 1)),
        ];
        for (i, (label, value)) in rows.iter().enumerate() {
            let (x, y) = if tall {
                (px + 76.0 + (pw - 76.0) / 3.0 * (i as f32 + 0.5), py + ph / 2.0)
            } else {
                (px + pw / 2.0, py + 104.0 + i as f32 * 58.0)
            };
            let size = s.fit_size(label, 11.0, if tall { (pw - 76.0) / 3.0 - 10.0 } else { pw - 12.0 });
            s.text_c(label, x, y - 11.0, size, hex(th.dim), 1.0);
            let size = s.fit_size(value, 20.0, if tall { 58.0 } else { pw - 12.0 });
            s.text_c(value, x, y + 9.0, size, hex(th.text), 1.0);
        }
    }
}

/// a tile fading in, drawn directly (not from a sprite) since it's see-through
fn draw_tile_alpha(s: &mut Screen, x: f32, y: f32, c: f32, (hi, face, lo, _): (u32, u32, u32, u32), a: f32) {
    let g = (c * 0.045).max(1.2);
    let (w, r, lip) = (c - 2.0 * g, c * 0.14, (c * 0.07).max(1.4));
    s.rect(x + g, y + g, w, w, r, lo, a);
    s.rect_grad(x + g, y + g, w, w - lip, r, hi, face, a);
}

wg::game!(Slider);
