//! Tilestack - an original wasmga.me game. MIT licensed.
//! Mahjong solitaire: take the 144 tiles off a stack two by two, a pair of
//! the same face (any two flowers, any two seasons) whose tops are clear and
//! that can slide out to the left or right. Every deal can be cleared
//! (board.rs). Hints, undo and three shuffles; the page draws setup, pause
//! and game over.
mod board;
mod faces;
#[rustfmt::skip]
mod glyphs;

use board::{class, Shape, LAYOUT_COUNT, N};
use faces::{Size, Theme, THEMES};
use wg::{ease_out, hex, mix, Game, Input, KeyRepeat, Pointer, Rng, Screen, Sprite};

const VERSION: u32 = 1;
/// The cleared or blocked board stays in view this long before game over.
const END_MS: i32 = 1500;
const SHUFFLES: i32 = 3;

/// options, in the catalog `setup` order
const O_LAYOUT: usize = 0;
const O_THEME: usize = 1;

const PLAY: i32 = 0;
const WON: i32 = 1;
const LOST: i32 = 2;

/// buttons
const B_HINT: usize = 0;
const B_UNDO: usize = 1;
const B_SHUFFLE: usize = 2;

/// a tile's face height over its width, and the block below it over the width
const TALL: f32 = 1.3;
const DEPTH: f32 = 0.11;

/// Points: each pair taken, clearing the board, and what help costs.
const PAIR_POINTS: i32 = 10;
const CLEAR_POINTS: i32 = 500;
const HINT_COST: i32 = 20;
const UNDO_COST: i32 = 10;
const SHUFFLE_COST: i32 = 50;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 2],
    rng: Rng,
    rep: KeyRepeat,
    /// the layout this deal is on (the option may change since)
    layout: i32,
    face: [u8; N],
    gone: [u8; N],
    /// the tiles taken, in order, two by two
    taken: [u8; N],
    ntaken: i32,
    sel: i32,
    hint: [i32; 2],
    phase: i32,
    end_ms: i32,
    time_ms: i32,
    hints: i32,
    undos: i32,
    shuffles: i32,
    bonus: i32,
}
// SAFETY: #[repr(C)], integers and Plain types only
unsafe impl wg::Plain for Save {}

#[derive(Clone, Copy, Default)]
struct Rect {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}
impl Rect {
    fn has(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// Where everything sits for a plan of pw x ph.
#[derive(Clone, Copy, Default)]
struct Layout {
    tall: bool,
    /// a tile at half-tile x, y on layer z has its face's top left at
    /// (ox + x * u - z * d, oy + y * u * TALL - z * d)
    u: f32,
    ox: f32,
    oy: f32,
    size: Size,
    panel: Rect,
    btn: [Rect; 3],
    /// the stats: label and value centres
    stat: [(f32, f32); 3],
    stat_w: f32,
    /// the line for a message
    msg: (f32, f32, f32),
}

const M: f32 = 10.0;
/// the panel beside the board (wide) or under it (tall)
const SIDE_W: f32 = 112.0;
const UNDER_H: f32 = 104.0;

/// The board's extent in tile widths: left, top, right, bottom of every
/// tile and its block at u = 1.
type Extent = (f32, f32, f32, f32);

fn extent(shape: &Shape) -> Extent {
    let d = DEPTH * 2.0;
    let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for p in shape.pos.iter() {
        let (x, y) = (
            p.x as f32 - p.z as f32 * d,
            p.y as f32 * TALL - p.z as f32 * d,
        );
        x0 = x0.min(x);
        y0 = y0.min(y);
        x1 = x1.max(x + 2.0 + d * 1.6);
        y1 = y1.max(y + 2.0 * TALL + d * 1.6);
    }
    (x0, y0, x1, y1)
}

/// The board's room and the panel's in a plan of pw x ph: the panel goes
/// beside the board (wide) or under it (tall).
fn areas(pw: f32, ph: f32) -> (Rect, Rect) {
    if ph > pw {
        (
            Rect {
                x: M,
                y: M,
                w: pw - 2.0 * M,
                h: ph - 3.0 * M - UNDER_H,
            },
            Rect {
                x: M,
                y: ph - M - UNDER_H,
                w: pw - 2.0 * M,
                h: UNDER_H,
            },
        )
    } else {
        (
            Rect {
                x: M,
                y: M,
                w: pw - 3.0 * M - SIDE_W,
                h: ph - 2.0 * M,
            },
            Rect {
                x: pw - M - SIDE_W,
                y: M,
                w: SIDE_W,
                h: ph - 2.0 * M,
            },
        )
    }
}

/// Half a tile's width, for the board to fill its room.
fn unit(board: &Rect, (x0, y0, x1, y1): Extent) -> f32 {
    (board.w / (x1 - x0)).min(board.h / (y1 - y0))
}

fn layout(pw: f32, ph: f32, ext: Extent) -> Layout {
    let mut l = Layout::default();
    l.tall = ph > pw;
    let (board, panel) = areas(pw, ph);
    l.panel = panel;
    let (x0, y0, x1, y1) = ext;
    // u is half a tile's width
    let u = unit(&board, ext);
    l.u = u;
    let gap = u * 0.04;
    l.size = Size {
        w: 2.0 * u - gap,
        h: 2.0 * u * TALL - gap,
        depth: 2.0 * u * DEPTH,
    };
    l.ox = board.x + (board.w - (x1 - x0) * u) / 2.0 - x0 * u;
    l.oy = board.y + (board.h - (y1 - y0) * u) / 2.0 - y0 * u;
    let p = l.panel;
    if l.tall {
        let cw = p.w / 3.0;
        for i in 0..3 {
            l.stat[i] = (p.x + cw * (i as f32 + 0.5), p.y + 24.0);
        }
        l.stat_w = cw - 8.0;
        l.msg = (p.x + p.w / 2.0, p.y + 24.0, p.w - 16.0);
        let g = 8.0;
        let bw = (p.w - 16.0 - 2.0 * g) / 3.0;
        for i in 0..3 {
            l.btn[i] = Rect {
                x: p.x + 8.0 + i as f32 * (bw + g),
                y: p.y + 56.0,
                w: bw,
                h: 40.0,
            };
        }
    } else {
        for i in 0..3 {
            l.stat[i] = (p.x + p.w / 2.0, p.y + 22.0 + i as f32 * 50.0);
        }
        l.stat_w = p.w - 12.0;
        let (bh, g) = (40.0, 8.0);
        let top = p.y + p.h - 8.0 - 3.0 * bh - 2.0 * g;
        for i in 0..3 {
            l.btn[i] = Rect {
                x: p.x + 8.0,
                y: top + i as f32 * (bh + g),
                w: p.w - 16.0,
                h: bh,
            };
        }
        l.msg = (p.x + p.w / 2.0, top - 18.0, p.w - 12.0);
    }
    l
}

struct Tilestack {
    s: Save,
    shape: Shape,
    /// the shape's extent()
    ext: Extent,
    lay: Layout,
    sprites: Vec<Sprite>,
    /// matching pairs of free tiles now
    open: i32,
    hover: i32,
    hover_btn: i32,
    press: i32,
    /// the keyboard's tile, -1 until an arrow is pressed
    cursor: i32,
    /// a blocked tile tapped, and how long it still shows
    flash: i32,
    flash_ms: i32,
    /// tiles leaving: slot, face, ms since
    leaving: Vec<(usize, u8, i32)>,
    shuffle_ms: i32,
    ev: u32,
}

/// How long a pair takes to leave, and a shuffle to settle.
const LEAVE_MS: i32 = 320;
const SHUFFLE_MS: i32 = 450;
const FLASH_MS: i32 = 380;

impl Tilestack {
    fn set_shape(&mut self, layout: i32) {
        self.shape = Shape::new(layout as usize);
        self.ext = extent(&self.shape);
    }
    fn t(&self) -> &'static Theme {
        &THEMES[self.s.opt[O_THEME] as usize]
    }
    fn free(&self, i: usize) -> bool {
        self.shape.free(&self.s.gone, i)
    }
    fn left(&self) -> i32 {
        N as i32 - self.s.ntaken
    }

    /// The face's top left of tile i.
    fn face_at(&self, i: usize) -> (f32, f32) {
        let p = self.shape.pos[i];
        let l = &self.lay;
        (
            l.ox + p.x as f32 * l.u - p.z as f32 * l.size.depth,
            l.oy + p.y as f32 * l.u * TALL - p.z as f32 * l.size.depth,
        )
    }

    /// The tile drawn on top at x, y, or -1.
    fn tile_at(&self, x: f32, y: f32) -> i32 {
        let z = self.lay.size;
        for &i in self.shape.order.iter().rev() {
            let i = i as usize;
            if self.s.gone[i] != 0 {
                continue;
            }
            let (fx, fy) = self.face_at(i);
            if x >= fx && y >= fy && x < fx + z.w + z.depth && y < fy + z.h + z.depth {
                return i as i32;
            }
        }
        -1
    }

    fn button_at(&self, x: f32, y: f32) -> i32 {
        self.lay
            .btn
            .iter()
            .position(|b| b.has(x, y))
            .map_or(-1, |i| i as i32)
    }

    /// Works out the open pairs again, and whether the run ended.
    fn settle(&mut self) {
        let (open, _) = self.shape.open_pairs(&self.s.gone, &self.s.face);
        self.open = open;
        if self.s.phase != PLAY {
            return;
        }
        if self.left() == 0 {
            self.s.phase = WON;
            self.s.end_ms = END_MS;
            self.s.bonus = (1500 - 2 * (self.s.time_ms / 1000)).max(0);
            self.ev |= wg::EV_WIN;
        } else if open == 0 && self.s.shuffles >= SHUFFLES {
            self.s.phase = LOST;
            self.s.end_ms = END_MS;
            self.ev |= wg::EV_LOSE;
        }
        if self.cursor >= 0 && !self.free(self.cursor as usize) {
            self.cursor = self.nearest_free(self.cursor as usize);
        }
    }

    /// The free tile closest to tile i on screen, or -1.
    fn nearest_free(&self, i: usize) -> i32 {
        let (x, y) = self.face_at(i);
        let mut best = (-1, f32::MAX);
        for j in self.shape.free_tiles(&self.s.gone) {
            let (a, b) = self.face_at(j);
            let d = (a - x) * (a - x) + (b - y) * (b - y);
            if d < best.1 {
                best = (j as i32, d);
            }
        }
        best.0
    }

    /// The free tile from the cursor in direction (dx, dy).
    fn step_cursor(&mut self, dx: f32, dy: f32) {
        let free = self.shape.free_tiles(&self.s.gone);
        if self.cursor < 0 || !self.free(self.cursor as usize) {
            self.cursor = free.first().map_or(-1, |&i| i as i32);
            return;
        }
        let (x, y) = self.face_at(self.cursor as usize);
        let mut best = (-1, f32::MAX);
        for j in free {
            let (a, b) = self.face_at(j);
            let (along, across) = (
                (a - x) * dx + (b - y) * dy,
                ((a - x) * dy - (b - y) * dx).abs(),
            );
            if along <= self.lay.u * 0.2 {
                continue;
            }
            let cost = along + across * 2.5;
            if cost < best.1 {
                best = (j as i32, cost);
            }
        }
        if best.0 >= 0 {
            self.cursor = best.0;
        }
    }

    /// A tile clicked, tapped or picked with the keyboard.
    fn pick(&mut self, i: usize) {
        if self.s.phase != PLAY || self.s.gone[i] != 0 {
            return;
        }
        if !self.free(i) {
            self.flash = i as i32;
            self.flash_ms = FLASH_MS;
            return;
        }
        let sel = self.s.sel;
        if sel == i as i32 {
            self.s.sel = -1;
            return;
        }
        if sel >= 0 && class(self.s.face[sel as usize]) == class(self.s.face[i]) {
            self.take(sel as usize, i);
            return;
        }
        self.s.sel = i as i32;
        self.ev |= wg::EV_BLIP;
    }

    fn take(&mut self, a: usize, b: usize) {
        let s = &mut self.s;
        for i in [a, b] {
            s.gone[i] = 1;
            s.taken[s.ntaken as usize] = i as u8;
            s.ntaken += 1;
            self.leaving.push((i, s.face[i], 0));
        }
        s.sel = -1;
        s.hint = [-1, -1];
        self.ev |= wg::EV_HIT;
        self.settle();
    }

    fn undo(&mut self) {
        if self.s.phase != PLAY || self.s.ntaken < 2 {
            return;
        }
        let s = &mut self.s;
        for _ in 0..2 {
            s.ntaken -= 1;
            let i = s.taken[s.ntaken as usize] as usize;
            s.gone[i] = 0;
            self.leaving.retain(|l| l.0 != i);
        }
        s.undos += 1;
        s.sel = -1;
        s.hint = [-1, -1];
        self.ev |= wg::EV_BLIP;
        self.settle();
    }

    fn hint(&mut self) {
        if self.s.phase != PLAY || self.s.hint[0] >= 0 {
            return;
        }
        if let (_, Some((a, b))) = self.shape.open_pairs(&self.s.gone, &self.s.face) {
            self.s.hint = [a as i32, b as i32];
            self.s.hints += 1;
            self.s.sel = -1;
            self.ev |= wg::EV_BLIP;
        }
    }

    /// Deals the tiles left again, so they can still all be cleared when
    /// their places allow it.
    fn shuffle(&mut self) {
        if self.s.phase != PLAY || self.s.shuffles >= SHUFFLES || self.left() == 0 {
            return;
        }
        let s = &mut self.s;
        let Some(mut pairs) = board::pairs_left(&s.gone, &s.face, &mut s.rng) else {
            return;
        };
        if !self
            .shape
            .deal(&s.gone, &mut pairs, &mut s.face, &mut s.rng)
        {
            // no order clears them: mix the faces, keeping a pair to take
            let live: Vec<usize> = (0..N).filter(|&i| s.gone[i] == 0).collect();
            for _ in 0..50 {
                let mut f: Vec<u8> = live.iter().map(|&i| s.face[i]).collect();
                s.rng.shuffle(&mut f);
                for (k, &i) in live.iter().enumerate() {
                    s.face[i] = f[k];
                }
                if self.shape.open_pairs(&s.gone, &s.face).0 > 0 {
                    break;
                }
            }
        }
        let s = &mut self.s;
        s.shuffles += 1;
        s.sel = -1;
        s.hint = [-1, -1];
        self.shuffle_ms = SHUFFLE_MS;
        self.ev |= wg::EV_BLIP;
        self.settle();
    }

    /// A new deal on the layout picked.
    fn deal(&mut self, seed: u32) {
        let s = &mut self.s;
        s.layout = s.opt[O_LAYOUT].clamp(0, LAYOUT_COUNT - 1);
        s.rng = Rng::new(seed);
        s.gone = [0; N];
        s.ntaken = 0;
        let layout = s.layout;
        self.set_shape(layout);
        let s = &mut self.s;
        let mut pairs = board::full_pairs(&mut s.rng);
        // tests show every layout deals within a few tries; each retry
        // goes on with the next numbers
        while !self
            .shape
            .deal(&s.gone, &mut pairs, &mut s.face, &mut s.rng)
        {}
        self.settle();
    }

    fn button(&mut self, b: usize) {
        match b {
            B_HINT => self.hint(),
            B_UNDO => self.undo(),
            _ => self.shuffle(),
        }
    }

    fn can(&self, b: usize) -> bool {
        self.s.phase == PLAY
            && match b {
                B_HINT => self.open > 0,
                B_UNDO => self.s.ntaken >= 2,
                _ => self.s.shuffles < SHUFFLES,
            }
    }

    fn score_now(&self) -> i32 {
        let s = &self.s;
        let mut p = s.ntaken / 2 * PAIR_POINTS
            - s.hints * HINT_COST
            - s.undos * UNDO_COST
            - s.shuffles * SHUFFLE_COST;
        if s.phase == WON {
            p += CLEAR_POINTS + s.bonus;
        }
        p.max(0)
    }

    fn draw_tiles(&mut self, s: &mut Screen) {
        let t = self.t();
        let z = self.lay.size;
        let theme = self.s.opt[O_THEME];
        let pad = z.pad();
        let (bw, bh) = (z.w + z.depth + 2.0 * pad, z.h + z.depth + 2.0 * pad);
        if self.sprites.len() < board::FACES as usize {
            self.sprites
                .resize_with(board::FACES as usize, Sprite::default);
        }
        let flash_a = self.flash_ms as f32 / FLASH_MS as f32;
        let shuffle_a = self.shuffle_ms as f32 / SHUFFLE_MS as f32;
        let r = z.radius();
        for k in 0..N {
            let i = self.shape.order[k] as usize;
            if self.s.gone[i] != 0 {
                continue;
            }
            let f = self.s.face[i];
            let key = wg::hash(
                0,
                wg::bytes_of(&[f as i32, theme, z.w.to_bits() as i32, z.h.to_bits() as i32]),
            );
            s.sprite_make_alpha(&mut self.sprites[f as usize], key, 0.0, 0.0, bw, bh, |s| {
                faces::tile(s, pad, pad, z, f, t);
            });
            let (fx, fy) = self.face_at(i);
            s.sprite(&self.sprites[f as usize], fx - pad, fy - pad);
            // lower layers a little darker, so the stack reads at a glance
            let low = (self.shape.top - self.shape.pos[i].z) as f32;
            if low > 0.0 {
                s.rect(fx, fy, z.w, z.h, r, hex(0x3a2a10), 0.045 * low);
            }
            // what marks it, drawn before the tiles after it cover its edges
            let ii = i as i32;
            if ii == self.s.sel {
                s.rect(fx, fy, z.w, z.h, r, t.accent, 0.34);
                frame(s, fx, fy, z.w, z.h, r, (z.w * 0.06).max(1.5), t.accent);
            } else if ii == self.s.hint[0] || ii == self.s.hint[1] {
                s.rect(fx, fy, z.w, z.h, r, t.hint, 0.3);
                frame(s, fx, fy, z.w, z.h, r, (z.w * 0.06).max(1.5), t.hint);
            } else if ii == self.hover {
                s.rect(fx, fy, z.w, z.h, r, t.accent, 0.14);
            }
            if ii == self.cursor {
                let i2 = z.w * 0.06;
                frame(
                    s,
                    fx + i2,
                    fy + i2,
                    z.w - 2.0 * i2,
                    z.h - 2.0 * i2,
                    r * 0.7,
                    (z.w * 0.08).max(1.6),
                    hex(0x1d2a3a),
                );
            }
            if ii == self.flash && self.flash_ms > 0 {
                s.rect(fx, fy, z.w, z.h, r, hex(0xd8432f), 0.4 * flash_a);
            }
            if self.shuffle_ms > 0 {
                s.rect(fx, fy, z.w, z.h, r, t.face_hi, 0.9 * shuffle_a);
            }
        }
        // the pair just taken rises and fades
        for &(i, f, ms) in self.leaving.iter() {
            let a = ms as f32 / LEAVE_MS as f32;
            let (fx, fy) = self.face_at(i);
            let lift = ease_out(a) * z.h * 0.35;
            let (x, y) = (fx - lift * 0.3, fy - lift);
            if a < 0.35 {
                s.sprite(&self.sprites[f as usize], x - pad, y - pad);
            } else {
                let k = 1.0 - (a - 0.35) / 0.65;
                s.rect(x + z.depth, y + z.depth, z.w, z.h, r, t.back, 0.9 * k);
                s.rect(x, y, z.w, z.h, r, t.face_hi, k);
                let ring = z.w * (0.3 + 0.6 * (1.0 - k));
                s.ring(
                    x + z.w / 2.0,
                    y + z.h / 2.0,
                    ring,
                    (z.w * 0.06).max(1.0),
                    t.accent,
                    k,
                );
            }
        }
    }

    fn draw_panel(&self, s: &mut Screen) {
        let t = self.t();
        let l = &self.lay;
        let p = l.panel;
        s.rect(p.x, p.y, p.w, p.h, 12.0, t.panel, 0.55);
        let stuck = self.s.phase != WON && self.open == 0 && self.left() > 0;
        let rows: [(&str, String); 3] = [
            (
                wg::tr(&[
                    "Tiles",
                    "剩余",
                    "残り",
                    "남은 패",
                    "Fichas",
                    "Peças",
                    "Tuiles",
                    "Steine",
                    "Фишки",
                ]),
                format!("{}", self.left()),
            ),
            (
                wg::tr(&[
                    "Open pairs",
                    "可配对",
                    "取れる組",
                    "가능한 짝",
                    "Parejas libres",
                    "Pares livres",
                    "Paires libres",
                    "Freie Paare",
                    "Свободные пары",
                ]),
                format!("{}", self.open),
            ),
            (
                wg::tr(&[
                    "Time",
                    "时间",
                    "タイム",
                    "시간",
                    "Tiempo",
                    "Tempo",
                    "Temps",
                    "Zeit",
                    "Время",
                ]),
                wg::clock(self.s.time_ms),
            ),
        ];
        let msg = if stuck {
            Some(wg::tr(&[
                "No pairs left",
                "没有可配对的牌了",
                "取れる組がありません",
                "가능한 짝이 없습니다",
                "No quedan parejas",
                "Não há mais pares",
                "Plus de paires",
                "Keine Paare mehr",
                "Пар больше нет",
            ]))
        } else {
            None
        };
        for (i, (label, value)) in rows.iter().enumerate() {
            // on a phone the message takes the stats' place
            if l.tall && msg.is_some() {
                break;
            }
            let (x, y) = l.stat[i];
            let size = s.fit_size(label, 11.0, l.stat_w);
            s.text_c(label, x, y - 9.0, size, t.dim, 1.0);
            let col = if i == 1 && stuck { t.accent } else { t.text };
            s.text_c(
                value,
                x,
                y + 10.0,
                s.fit_size(value, 20.0, l.stat_w),
                col,
                1.0,
            );
        }
        if let Some(m) = msg {
            let (x, y, w) = l.msg;
            s.text_c(m, x, y, s.fit_size(m, 13.0, w), t.accent, 1.0);
        }
        let names = [
            wg::tr(&[
                "Hint",
                "提示",
                "ヒント",
                "힌트",
                "Pista",
                "Dica",
                "Indice",
                "Tipp",
                "Подсказка",
            ]),
            wg::tr(&[
                "Undo",
                "撤销",
                "元に戻す",
                "되돌리기",
                "Deshacer",
                "Desfazer",
                "Annuler",
                "Zurück",
                "Отмена",
            ]),
            wg::tr(&[
                "Shuffle",
                "洗牌",
                "シャッフル",
                "섞기",
                "Mezclar",
                "Embaralhar",
                "Mélanger",
                "Mischen",
                "Перемешать",
            ]),
        ];
        // icons above the names when a name beside its icon would come out
        // small (narrow buttons, long words); the same for all three
        let stacked = (l.tall && l.btn[0].w < 120.0)
            || names.iter().any(|n| {
                let room = l.btn[0].w - (l.btn[0].h * 0.42).min(18.0) - 22.0 - 24.0;
                s.fit_size(n, 13.0, room) < 11.0
            });
        for (i, r) in l.btn.iter().enumerate() {
            let can = self.can(i);
            let lit = i == B_SHUFFLE && stuck && can;
            let hot = can && (self.hover_btn == i as i32 || self.press == i as i32);
            let face = if lit {
                t.accent
            } else if hot {
                mix(t.accent, t.btn, 0.22)
            } else {
                t.btn
            };
            s.shadow(r.x, r.y + 1.5, r.w, r.h, 8.0, 3.0, 0, 0.2);
            s.rect(r.x, r.y, r.w, r.h, 8.0, face, 1.0);
            let ink = if lit { hex(0x2a1c08) } else { t.btn_ink };
            let a = if can { 1.0 } else { 0.4 };
            let icon = (r.h * 0.42).min(18.0);
            let name = names[i];
            // the shuffles left, after the name
            let count = if i == B_SHUFFLE {
                format!("{}", SHUFFLES - self.s.shuffles)
            } else {
                String::new()
            };
            let cw = if count.is_empty() {
                0.0
            } else {
                s.text_width(&count, 11.0) + 10.0
            };
            // where the name ends, and the line it sits on
            let (end, cy) = if stacked {
                draw_icon(s, i, r.x + r.w / 2.0, r.y + r.h * 0.36, icon * 0.9, ink, a);
                let size = s.fit_size(name, 10.5, r.w - 8.0 - cw);
                let tw = s.text_width(name, size);
                let x0 = r.x + (r.w - tw - cw) / 2.0;
                s.text(name, x0, r.y + r.h * 0.76, size, ink, a);
                (x0 + tw, r.y + r.h * 0.76)
            } else {
                let size = s.fit_size(name, 13.0, r.w - icon - 22.0 - cw);
                let tw = s.text_width(name, size);
                let x0 = r.x + (r.w - (icon + 7.0 + tw + cw)) / 2.0;
                draw_icon(s, i, x0 + icon / 2.0, r.y + r.h / 2.0, icon, ink, a);
                s.text(name, x0 + icon + 7.0, r.y + r.h / 2.0, size, ink, a);
                (x0 + icon + 7.0 + tw, r.y + r.h / 2.0)
            };
            if !count.is_empty() {
                badge(s, end + 5.0 + (cw - 5.0) / 2.0, cy, &count, ink, face, a);
            }
        }
    }
}

/// A small round number.
fn badge(s: &mut Screen, cx: f32, cy: f32, text: &str, ink: u32, face: u32, a: f32) {
    s.disc(cx, cy, 7.5, ink, a * 0.9);
    s.text_c(text, cx, cy, 10.0, face, 1.0);
}

/// The outline of a rounded box, `th` thick, inside its edge.
fn frame(s: &mut Screen, x: f32, y: f32, w: f32, h: f32, r: f32, th: f32, c: u32) {
    let h2 = th / 2.0;
    let (x0, y0, x1, y1) = (x + h2, y + h2, x + w - h2, y + h - h2);
    let r = (r - h2).max(0.0);
    s.line(x0 + r, y0, x1 - r, y0, h2, c, 1.0);
    s.line(x0 + r, y1, x1 - r, y1, h2, c, 1.0);
    s.line(x0, y0 + r, x0, y1 - r, h2, c, 1.0);
    s.line(x1, y0 + r, x1, y1 - r, h2, c, 1.0);
    let corners = [
        (x1 - r, y0 + r, -0.5f32),
        (x0 + r, y0 + r, -1.0),
        (x0 + r, y1 - r, 0.5),
        (x1 - r, y1 - r, 0.0),
    ];
    for (cx, cy, start) in corners {
        let mut prev = None;
        for k in 0..=4 {
            let a = (start + k as f32 * 0.125) * core::f32::consts::PI;
            let p = (cx + a.cos() * r, cy + a.sin() * r);
            if let Some((px, py)) = prev {
                s.line(px, py, p.0, p.1, h2, c, 1.0);
            }
            prev = Some(p);
        }
    }
}

/// A button's picture, centred on cx, cy, about `z` across.
fn draw_icon(s: &mut Screen, b: usize, cx: f32, cy: f32, z: f32, c: u32, a: f32) {
    let w = (z * 0.09).max(0.8);
    let pi = core::f32::consts::PI;
    match b {
        B_HINT => {
            // a light bulb
            s.disc(cx, cy - z * 0.1, z * 0.28, c, a);
            s.rect(
                cx - z * 0.14,
                cy + z * 0.08,
                z * 0.28,
                z * 0.2,
                z * 0.04,
                c,
                a,
            );
            s.rect(
                cx - z * 0.11,
                cy + z * 0.32,
                z * 0.22,
                z * 0.07,
                z * 0.03,
                c,
                a,
            );
        }
        B_UNDO => {
            // an arrow curling back to the left
            let r = z * 0.32;
            let (ox, oy) = (cx + z * 0.06, cy + z * 0.05);
            let mut prev = (ox + r, oy);
            for i in 1..=10 {
                let t = -pi * i as f32 / 10.0;
                let p = (ox + r * t.cos(), oy + r * t.sin());
                s.line(prev.0, prev.1, p.0, p.1, w, c, a);
                prev = p;
            }
            let (tx, ty) = (ox - r, oy);
            s.tri(
                (tx - z * 0.2, ty - z * 0.04),
                (tx + z * 0.2, ty - z * 0.04),
                (tx, ty + z * 0.24),
                c,
                a,
            );
        }
        _ => {
            // two arrows crossing
            let (l, r) = (cx - z * 0.42, cx + z * 0.3);
            let (t, bm) = (cy - z * 0.24, cy + z * 0.24);
            s.line(l, t, cx - z * 0.1, t, w, c, a);
            s.line(cx - z * 0.1, t, cx + z * 0.1, bm, w, c, a);
            s.line(cx + z * 0.1, bm, r, bm, w, c, a);
            s.line(l, bm, cx - z * 0.1, bm, w, c, a);
            s.line(cx - z * 0.1, bm, cx + z * 0.1, t, w, c, a);
            s.line(cx + z * 0.1, t, r, t, w, c, a);
            for y in [t, bm] {
                s.tri(
                    (r + z * 0.16, y),
                    (r - z * 0.04, y - z * 0.15),
                    (r - z * 0.04, y + z * 0.15),
                    c,
                    a,
                );
            }
        }
    }
}

impl Game for Tilestack {
    type Save = Save;
    const CHOICES: &'static [i32] = &[LAYOUT_COUNT, THEMES.len() as i32];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        // SAFETY: Save is Plain, so all zeroes is a valid value
        let mut s: Save = unsafe { core::mem::zeroed() };
        s.version = VERSION;
        s.sel = -1;
        s.hint = [-1, -1];
        let shape = Shape::new(0);
        let ext = extent(&shape);
        let lay = layout(480.0, 360.0, ext);
        let mut g = Tilestack {
            s,
            shape,
            ext,
            lay,
            sprites: Vec::new(),
            open: 0,
            hover: -1,
            hover_btn: -1,
            press: -1,
            cursor: -1,
            flash: -1,
            flash_ms: 0,
            leaving: Vec::new(),
            shuffle_ms: 0,
            ev: 0,
        };
        // a deal to show behind the setup screen
        g.deal(1);
        g
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn start(&mut self, seed: u32) {
        let opt = self.s.opt;
        let sprites = core::mem::take(&mut self.sprites);
        *self = Tilestack::new();
        self.sprites = sprites;
        self.s.opt = opt;
        self.deal(seed);
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt;
        self.flash_ms = (self.flash_ms - dt).max(0);
        self.shuffle_ms = (self.shuffle_ms - dt).max(0);
        for l in self.leaving.iter_mut() {
            l.2 += dt;
        }
        self.leaving.retain(|l| l.2 < LEAVE_MS);
        let mut ev = core::mem::take(&mut self.ev);
        if self.s.phase != PLAY {
            if self.s.end_ms > 0 {
                self.s.end_ms -= dt;
                if self.s.end_ms <= 0 {
                    self.s.end_ms = 0;
                    ev |= wg::EV_GAMEOVER;
                }
            }
            return ev;
        }
        self.s.time_ms = self.s.time_ms.saturating_add(dt);
        let dirs = self.s.rep.dirs(input);
        if dirs != 0 {
            let (dx, dy) = (
                (dirs & wg::RIGHT != 0) as i32 as f32 - (dirs & wg::LEFT != 0) as i32 as f32,
                (dirs & wg::DOWN != 0) as i32 as f32 - (dirs & wg::UP != 0) as i32 as f32,
            );
            self.step_cursor(dx, dy);
            self.hover = -1;
        }
        if input.pressed & wg::A != 0 {
            if self.cursor < 0 || !self.free(self.cursor as usize) {
                self.step_cursor(0.0, 0.0);
            } else {
                self.pick(self.cursor as usize);
            }
        }
        if input.pressed & wg::B != 0 {
            self.s.sel = -1;
        }
        ev | core::mem::take(&mut self.ev)
    }

    fn key(&mut self, code: i32) -> bool {
        if self.s.phase != PLAY {
            return false;
        }
        match code {
            0x68 => self.hint(),    // h
            0x75 => self.undo(),    // u
            0x72 => self.shuffle(), // r (s is the page's WASD down)
            _ => return false,
        }
        true
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        if !p.inside {
            self.hover = -1;
            self.hover_btn = -1;
            self.press = -1;
            return 0;
        }
        let tile = self.tile_at(p.x, p.y);
        let btn = self.button_at(p.x, p.y);
        self.hover = if !p.touch && tile >= 0 && self.free(tile as usize) {
            tile
        } else {
            -1
        };
        self.hover_btn = if p.touch { -1 } else { btn };
        if p.pressed & wg::M_LEFT != 0 {
            if btn >= 0 {
                self.press = btn;
            } else if tile >= 0 {
                self.cursor = -1;
                self.pick(tile as usize);
            } else {
                self.s.sel = -1;
            }
        }
        if p.pressed & wg::M_RIGHT != 0 {
            self.s.sel = -1;
        }
        if p.released & wg::M_LEFT != 0 {
            if self.press >= 0 && btn == self.press {
                self.button(btn as usize);
            }
            self.press = -1;
        }
        core::mem::take(&mut self.ev)
    }

    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        let (w, h) = (w.max(1) as f32, h.max(1) as f32);
        let a = w / h;
        // whichever shows the tiles bigger: the panel beside them, or under
        let wide = ((360.0 * a).max(420.0), 360.0);
        let tall = (360.0, (360.0 / a).max(400.0));
        let tile = |(pw, ph): (f32, f32)| unit(&areas(pw, ph).0, self.ext) * (w / pw).min(h / ph);
        if tile(tall) > tile(wide) * 1.04 {
            tall
        } else {
            wide
        }
    }

    fn picture(&mut self) -> u32 {
        let s = &self.s;
        let mut h = wg::hash(0, &s.gone);
        h = wg::hash(h, &s.face);
        let shown = [
            s.opt[O_THEME],
            s.sel,
            s.hint[0],
            s.hint[1],
            s.phase,
            s.time_ms / 1000,
            s.shuffles,
            self.hover,
            self.hover_btn,
            self.press,
            self.cursor,
        ];
        // an animation ending needs one more picture without it
        let anim = [
            self.leaving.len() as i32,
            (self.flash_ms > 0) as i32,
            (self.shuffle_ms > 0) as i32,
        ];
        h = wg::hash(h, wg::bytes_of(&anim));
        wg::hash(h, wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        !self.leaving.is_empty() || self.flash_ms > 0 || self.shuffle_ms > 0
    }

    fn draw(&mut self, s: &mut Screen) {
        self.lay = layout(s.pw, s.ph, self.ext);
        let t = self.t();
        let key = wg::hash(self.s.opt[O_THEME] as u32 + 1, wg::bytes_of(&[s.pw, s.ph]));
        s.background(key, |s| {
            let (x0, y0, x1, y1) = s.visible();
            s.rect_grad(
                x0,
                y0,
                x1 - x0,
                y1 - y0,
                0.0,
                t.table_top,
                t.table_bottom,
                1.0,
            );
        });
        self.draw_tiles(s);
        self.draw_panel(s);
    }

    fn score(&self) -> u32 {
        self.score_now() as u32
    }
    fn over(&self) -> bool {
        self.s.phase != PLAY
    }
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        match i {
            0 => s.time_ms,
            1 => s.ntaken / 2,
            2 => self.left(),
            3 => s.hints,
            4 => s.undos,
            5 => s.shuffles,
            6 if s.phase == WON => s.bonus,
            _ => -1,
        }
    }

    fn loaded(&mut self) -> bool {
        let s = &self.s;
        // the runtime has checked the options
        let ok = s.version == VERSION
            && (0..LAYOUT_COUNT).contains(&s.layout)
            && (0..=N as i32).contains(&s.ntaken)
            && s.ntaken % 2 == 0
            && (-1..N as i32).contains(&s.sel)
            && s.hint.iter().all(|h| (-1..N as i32).contains(h))
            && (PLAY..=LOST).contains(&s.phase)
            && s.time_ms >= 0
            && s.hints >= 0
            && s.undos >= 0
            && (0..=SHUFFLES).contains(&s.shuffles)
            && board::full_set(&s.face);
        if !ok {
            return false;
        }
        // the tiles taken are those marked gone, in matching pairs
        let mut gone = [0u8; N];
        for k in 0..s.ntaken as usize {
            let i = s.taken[k] as usize;
            if i >= N || gone[i] != 0 {
                return false;
            }
            gone[i] = 1;
            if k % 2 == 1 && class(s.face[i]) != class(s.face[s.taken[k - 1] as usize]) {
                return false;
            }
        }
        if gone != s.gone || (s.phase == WON) != (s.ntaken == N as i32) {
            return false;
        }
        self.set_shape(s.layout);
        self.leaving.clear();
        self.flash_ms = 0;
        self.shuffle_ms = 0;
        self.hover = -1;
        self.hover_btn = -1;
        self.press = -1;
        self.cursor = -1;
        self.settle();
        self.ev = 0;
        true
    }
}

wg::game!(Tilestack);
