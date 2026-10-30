//! Hexpile - an original wasmga.me game. MIT licensed.
//! Stacks of hex tiles come three at a time. Drop each on a free cell of the
//! board: tiles of the same colour on touching stacks slide together, and a
//! run of ten or more clears. The run ends when the board is full. The page
//! shows the setup, pause and game over screens.
mod glyphs;
mod puzzle;

use puzzle::{Board, Step, CLEAR, TRAYH};
use wg::{hex, mix, tr, Game, Input, KeyRepeat, Pointer, Rng, Screen};

const VERSION: u32 = 1;

/// options, in the catalog `setup` order
const O_BOARD: usize = 0;
const O_THEME: usize = 1;
const THEMES: usize = 3;

const PLAY: i32 = 0;
const OVER: i32 = 1;

const TRAY: usize = 3;
/// the board seen from a little above: rows squashed
const SQUASH: f32 = 0.8;
/// tiles in flight leave this far apart and take this long, ms
const GAP_MS: f32 = 55.0;
const FLY_MS: f32 = 260.0;
const POP_MS: f32 = 200.0;
const POP_GAP_MS: f32 = 28.0;
const DROP_MS: f32 = 150.0;
/// the full board stays on screen this long before game over
const END_MS: i32 = 1200;

#[repr(C)]
#[derive(Clone, Copy)]
struct Save {
    version: u32,
    opt: [i32; 2],
    phase: i32,
    seed: u32,
    score: i32,
    /// tiles cleared, stacks placed, the longest chain of clears from one stack
    tiles: i32,
    placed: i32,
    best_chain: i32,
    time_ms: i32,
    end_ms: i32,
    /// keyboard cursor, a cell, and the tray stack it holds
    cur: i32,
    pick: i32,
    rep: KeyRepeat,
    board: Board,
    tray: [[u8; TRAYH]; TRAY],
    tray_h: [u8; TRAY],
    _pad: u8,
}
// SAFETY: #[repr(C)], integers and Plain types only, padding spelled out
unsafe impl wg::Plain for Save {}

struct Theme {
    bg: u32,
    bg2: u32,
    well: u32,
    slot: u32,
    text: u32,
    dim: u32,
    tiles: [u32; 7],
}

const THEME: [Theme; THEMES] = [
    // Night
    Theme { bg: 0x1d2238, bg2: 0x11141f, well: 0x272d47, slot: 0x343b5c, text: 0xf2f4fb, dim: 0x8f97b8, tiles: [0xef4f5a, 0xf7923a, 0xf4d03f, 0x49c26c, 0x33bfd6, 0x4f6ff0, 0xa35ee6] },
    // Sand
    Theme { bg: 0xf6eedd, bg2: 0xe8dcc2, well: 0xe3d5b8, slot: 0xd3c3a2, text: 0x3a2f24, dim: 0x8a7a66, tiles: [0xe0434f, 0xef8428, 0xe6bd1f, 0x3aa95a, 0x22a6bb, 0x4263d8, 0x9150d0] },
    // Mint
    Theme { bg: 0xe2f2ea, bg2: 0xcbe5d8, well: 0xbfdccd, slot: 0xa9cdbb, text: 0x1f4535, dim: 0x5f8574, tiles: [0xe24a55, 0xf08a30, 0xe9c42a, 0x2f9d58, 0x1f9fb5, 0x3f62d6, 0x9353cf] },
];

#[derive(Clone, Copy)]
struct Drag {
    slot: usize,
    x: f32,
    y: f32,
    from_x: f32,
    from_y: f32,
    /// moved far enough to be a drag rather than a tap
    far: bool,
    touch: bool,
}

/// what plays on screen; the save already has the result
#[derive(Clone, Copy)]
enum Anim {
    /// a stack from the tray settling onto a cell, from where it was let go
    Drop { cell: usize, x: f32, y: f32, tiles: [u8; TRAYH], n: usize },
    Board(Step),
}

struct Popup {
    x: f32,
    y: f32,
    points: i32,
    t: f32,
}

struct Hexpile {
    s: Save,
    tall: bool,
    keys: bool,
    /// the board as drawn, one step behind the save while steps play
    view: Board,
    anims: Vec<Anim>,
    anim_t: f32,
    popups: Vec<Popup>,
    drag: Option<Drag>,
    hover: i32,
    /// the tray sliding in after a refill, 0 to 1
    tray_in: f32,
    cells: Vec<(i32, i32)>,
    /// the accessible board's focus is on this tray stack, -1 on the board
    on_tray: i32,
}

impl Hexpile {
    fn theme(&self) -> &'static Theme {
        &THEME[self.s.opt[O_THEME] as usize]
    }
    fn radius(&self) -> i32 {
        2 + self.s.opt[O_BOARD]
    }
    fn plan_wh(&self) -> (f32, f32) {
        if self.tall { (300.0, 400.0) } else { (400.0, 300.0) }
    }
    /// the board's middle and a cell's size (centre to corner)
    fn geo(&self) -> (f32, f32, f32) {
        let r = self.radius() as f32;
        let across = 3f32.sqrt() * (2.0 * r + 1.0);
        let down = (3.0 * r + 2.0) * SQUASH;
        if self.tall {
            let size = (276.0 / across).min(220.0 / down).min(28.0);
            (150.0, 182.0, size)
        } else {
            let size = (276.0 / across).min(224.0 / down).min(28.0);
            (152.0, 162.0, size)
        }
    }
    fn cell_xy(&self, i: usize) -> (f32, f32) {
        let (ox, oy, size) = self.geo();
        let (q, r) = self.cells[i];
        (ox + size * 3f32.sqrt() * (q as f32 + r as f32 / 2.0), oy + size * 1.5 * r as f32 * SQUASH)
    }
    fn slot_xy(&self, k: usize) -> (f32, f32) {
        let slide = (1.0 - wg::ease_out(self.tray_in)) * 220.0;
        if self.tall {
            (60.0 + k as f32 * 90.0 + slide, 352.0)
        } else {
            (346.0 + slide, 86.0 + k as f32 * 78.0)
        }
    }
    fn tray_size(&self) -> f32 {
        self.geo().2.min(21.0)
    }
    /// a layer's thickness on a stack this high
    fn layer(size: f32, h: usize) -> f32 {
        (size * 0.17).min(size * 2.3 / h.max(1) as f32)
    }
    /// the cell under a point, empty or not
    fn cell_at(&self, x: f32, y: f32) -> i32 {
        let size = self.geo().2;
        let mut best = (-1, size * 0.95);
        for i in 0..self.cells.len() {
            let (cx, cy) = self.cell_xy(i);
            let d = ((x - cx).powi(2) + ((y - cy) / SQUASH).powi(2)).sqrt();
            if d < best.1 {
                best = (i as i32, d);
            }
        }
        best.0
    }
    fn slot_at(&self, x: f32, y: f32) -> i32 {
        let size = self.tray_size();
        for k in 0..TRAY {
            let (sx, sy) = self.slot_xy(k);
            if self.s.tray_h[k] > 0 && (x - sx).abs() < size * 1.6 && y > sy - size * 2.6 && y < sy + size * 1.2 {
                return k as i32;
            }
        }
        -1
    }
    /// where a dragged stack sits: above the finger on a touch screen
    fn drag_xy(d: &Drag) -> (f32, f32) {
        if d.touch && d.far { (d.x, d.y - 46.0) } else { (d.x, d.y) }
    }
    fn busy(&self) -> bool {
        !self.anims.is_empty()
    }

    fn refill(&mut self) {
        let mut rng = Rng(self.s.seed);
        let colours = puzzle::colours(self.radius(), self.s.tiles);
        for k in 0..TRAY {
            let (t, h) = puzzle::deal_stack(&mut rng, colours, self.s.tiles);
            self.s.tray[k] = t;
            self.s.tray_h[k] = h;
        }
        self.s.seed = rng.0;
        self.tray_in = 0.0;
    }

    /// drops tray stack `k` on cell `i`, let go at (x, y)
    fn place(&mut self, k: usize, i: usize, x: f32, y: f32) -> u32 {
        if self.s.phase != PLAY || self.busy() || i >= self.cells.len() || self.s.board.h[i] != 0 || self.s.tray_h[k] == 0 {
            return 0;
        }
        let n = self.s.tray_h[k] as usize;
        let tiles = self.s.tray[k];
        self.s.board.put(i, &tiles[..n]);
        self.s.tray_h[k] = 0;
        self.s.tray[k] = [0; TRAYH];
        self.s.placed += 1;
        self.anims.push(Anim::Drop { cell: i, x, y, tiles, n });
        self.anim_t = 0.0;
        let steps = self.s.board.settle(i);
        let mut chain = 0;
        for st in &steps {
            if let Step::Clear { n, points, .. } = *st {
                chain += 1;
                self.s.score += points;
                self.s.tiles += n as i32;
            }
        }
        self.s.best_chain = self.s.best_chain.max(chain);
        self.anims.extend(steps.into_iter().map(Anim::Board));
        if self.s.tray_h.iter().all(|&h| h == 0) {
            self.refill();
        } else if self.s.tray_h[self.s.pick as usize] == 0 {
            self.s.pick = (0..TRAY).find(|&j| self.s.tray_h[j] > 0).unwrap_or(0) as i32;
        }
        if self.s.board.empty_cells() == 0 {
            self.s.phase = OVER;
            self.s.end_ms = END_MS;
        }
        wg::EV_BLIP
    }

    fn anim_ms(a: &Anim) -> f32 {
        match a {
            Anim::Drop { .. } => DROP_MS,
            Anim::Board(Step::Move { n, .. }) => GAP_MS * (*n as f32 - 1.0) + FLY_MS,
            Anim::Board(Step::Clear { n, .. }) => POP_GAP_MS * (*n as f32 - 1.0) + POP_MS,
        }
    }

    /// tiles of a Move that have landed by time t
    fn landed(n: usize, t: f32) -> usize {
        (0..n).filter(|&k| t >= k as f32 * GAP_MS + FLY_MS).count()
    }
    fn left(n: usize, t: f32) -> usize {
        (0..n).filter(|&k| t >= k as f32 * GAP_MS).count()
    }

    /// arrow keys: the nearest cell that way
    fn step_cursor(&mut self, dx: f32, dy: f32) {
        let (x0, y0) = self.cell_xy(self.s.cur as usize);
        let mut best = (-1i32, f32::MAX);
        for i in 0..self.cells.len() {
            let (x, y) = self.cell_xy(i);
            let (ex, ey) = (x - x0, (y - y0) / SQUASH);
            let along = ex * dx + ey * dy;
            if along <= 1.0 {
                continue;
            }
            let side = (ex * dy - ey * dx).abs();
            if side > along * 1.2 {
                continue;
            }
            let cost = along + side;
            if cost < best.1 {
                best = (i as i32, cost);
            }
        }
        if best.0 >= 0 {
            self.s.cur = best.0;
        }
    }
}

fn hex_pts(cx: f32, cy: f32, size: f32, sq: f32) -> [(f32, f32); 6] {
    let mut p = [(0.0, 0.0); 6];
    for (k, pt) in p.iter_mut().enumerate() {
        let a = (-90.0 + 60.0 * k as f32).to_radians();
        *pt = (cx + size * a.cos(), cy + size * a.sin() * sq);
    }
    p
}

/// one tile: its top face centred on (cx, cy), `thick` deep, squashed
/// further by `flip` while it turns over in the air
fn draw_tile(s: &mut Screen, cx: f32, cy: f32, size: f32, thick: f32, c: u32, flip: f32, alpha: f32) {
    let sq = SQUASH * flip;
    let top = hex_pts(cx, cy, size, sq);
    let mut side = top;
    // a little short of the next tile down, so a light rim parts the layers
    for k in 2..=4 {
        side[k].1 += thick * 0.8;
    }
    s.poly(&side, mix(hex(0x000000), c, 0.6), alpha);
    s.poly(&top, mix(hex(0xffffff), c, 0.7), alpha);
    s.poly(&hex_pts(cx, cy + size * 0.03 * sq, size * 0.86, sq), c, alpha);
}

/// a stack of tiles with its bottom on (cx, cy)
fn draw_stack(s: &mut Screen, th: &Theme, cx: f32, cy: f32, size: f32, tiles: &[u8], alpha: f32) -> f32 {
    let t = Hexpile::layer(size, tiles.len());
    let mut y = cy;
    for &c in tiles {
        draw_tile(s, cx, y, size * 0.94, t, hex(th.tiles[c as usize - 1]), 1.0, alpha);
        y -= t;
    }
    y + t
}

impl Game for Hexpile {
    type Save = Save;
    const CHOICES: &'static [i32] = &[2, THEMES as i32];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        Hexpile {
            s: Save {
                version: VERSION,
                opt: [1, 0],
                phase: PLAY,
                seed: 1,
                score: 0,
                tiles: 0,
                placed: 0,
                best_chain: 0,
                time_ms: 0,
                end_ms: 0,
                cur: 0,
                pick: 0,
                rep: KeyRepeat(0),
                board: Board::new(3),
                tray: [[0; TRAYH]; TRAY],
                tray_h: [0; TRAY],
                _pad: 0,
            },
            tall: false,
            keys: false,
            view: Board::new(3),
            anims: Vec::new(),
            anim_t: 0.0,
            popups: Vec::new(),
            drag: None,
            hover: -1,
            tray_in: 1.0,
            cells: puzzle::cells(3),
            on_tray: -1,
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
        self.s = Save { opt, ..Hexpile::new().s };
        self.s.seed = seed | 1;
        let r = self.radius();
        self.s.board = Board::new(r);
        self.cells = puzzle::cells(r);
        self.s.cur = (self.cells.len() / 2) as i32;
        self.view = self.s.board;
        self.anims.clear();
        self.popups.clear();
        self.drag = None;
        self.keys = false;
        self.refill();
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        if h as f32 > w as f32 * 1.1 { (300.0, 400.0) } else { (400.0, 300.0) }
    }

    fn update(&mut self, input: &Input) -> u32 {
        let dt = input.dt as f32;
        let mut ev = 0;
        if self.tray_in < 1.0 && !self.busy() {
            self.tray_in = (self.tray_in + dt / 300.0).min(1.0);
        }
        for p in self.popups.iter_mut() {
            p.t += dt / 900.0;
        }
        self.popups.retain(|p| p.t < 1.0);
        if let Some(a) = self.anims.first().copied() {
            let before = self.anim_t;
            self.anim_t += dt;
            if let Anim::Board(Step::Move { n, .. }) = a {
                if Self::landed(n, self.anim_t) > Self::landed(n, before) {
                    ev |= wg::EV_HIT;
                }
            }
            if self.anim_t >= Self::anim_ms(&a) {
                match a {
                    Anim::Drop { cell, tiles, n, .. } => self.view.put(cell, &tiles[..n]),
                    Anim::Board(st) => {
                        self.view.apply(&st);
                        if let Step::Clear { cell, points, .. } = st {
                            let (x, y) = self.cell_xy(cell);
                            self.popups.push(Popup { x, y: y - self.geo().2 * 0.6, points, t: 0.0 });
                            ev |= wg::EV_WIN;
                        }
                    }
                }
                self.anims.remove(0);
                self.anim_t = 0.0;
            }
        }
        if self.s.phase == PLAY && self.s.placed > 0 {
            self.s.time_ms = self.s.time_ms.saturating_add(input.dt);
        }
        if self.s.end_ms > 0 && !self.busy() {
            self.s.end_ms -= input.dt;
            if self.s.end_ms <= 0 {
                self.s.end_ms = 0;
                ev |= wg::EV_GAMEOVER;
            }
        }
        let dirs = self.s.rep.dirs(input);
        if dirs != 0 {
            self.keys = true;
            self.hover = -1;
        }
        if self.s.phase == PLAY {
            if dirs & wg::LEFT != 0 { self.step_cursor(-1.0, 0.0); }
            if dirs & wg::RIGHT != 0 { self.step_cursor(1.0, 0.0); }
            if dirs & wg::UP != 0 { self.step_cursor(0.0, -1.0); }
            if dirs & wg::DOWN != 0 { self.step_cursor(0.0, 1.0); }
            if input.pressed & wg::B != 0 {
                self.keys = true;
                let k = self.s.pick as usize;
                if let Some(j) = (1..=TRAY).map(|d| (k + d) % TRAY).find(|&j| self.s.tray_h[j] > 0) {
                    self.s.pick = j as i32;
                }
            }
            if input.pressed & wg::A != 0 {
                self.keys = true;
                let (x, y) = self.slot_xy(self.s.pick as usize);
                ev |= self.place(self.s.pick as usize, self.s.cur as usize, x, y);
            }
        }
        ev
    }

    fn pointer(&mut self, p: &Pointer) -> u32 {
        let mut ev = 0;
        if let Some(mut d) = self.drag {
            d.x = p.x;
            d.y = p.y;
            if (d.x - d.from_x).abs() + (d.y - d.from_y).abs() > 8.0 {
                d.far = true;
            }
            self.drag = Some(d);
            if p.released & wg::M_LEFT != 0 || p.held & wg::M_LEFT == 0 || !p.inside {
                self.drag = None;
                let (x, y) = Self::drag_xy(&d);
                let i = self.cell_at(x, y);
                if d.far && i >= 0 {
                    ev |= self.place(d.slot, i as usize, x, y);
                } else if !d.far {
                    self.s.pick = d.slot as i32;
                }
            }
            return ev;
        }
        if !p.inside {
            self.hover = -1;
            return 0;
        }
        let cell = self.cell_at(p.x, p.y);
        self.hover = if p.touch { -1 } else { cell };
        if p.pressed & wg::M_LEFT != 0 && self.s.phase == PLAY {
            self.keys = false;
            let k = self.slot_at(p.x, p.y);
            if k >= 0 {
                self.drag = Some(Drag { slot: k as usize, x: p.x, y: p.y, from_x: p.x, from_y: p.y, far: false, touch: p.touch });
            } else if cell >= 0 {
                // a tap on a cell drops the chosen stack there
                self.s.cur = cell;
                let (x, y) = self.slot_xy(self.s.pick as usize);
                ev |= self.place(self.s.pick as usize, cell as usize, x, y);
            }
        }
        ev
    }

    fn picture(&mut self) -> u32 {
        let d = self.drag.map_or([-1.0; 3], |d| [d.slot as f32, d.x, d.y]);
        let shown = [self.hover, self.keys as i32, self.tall as i32, d[0] as i32, d[1] as i32, d[2] as i32, self.busy() as i32];
        let mut c = self.s;
        c.time_ms = 0;
        c.end_ms = 0;
        wg::hash(wg::hash(0, wg::bytes_of(&c)), wg::bytes_of(&shown))
    }
    fn moving(&self) -> bool {
        self.busy() || !self.popups.is_empty() || self.tray_in < 1.0
    }

    fn draw(&mut self, s: &mut Screen) {
        self.tall = s.ph > s.pw;
        let th = self.theme();
        let (w, _h) = self.plan_wh();
        let (ox, oy, size) = self.geo();
        let n = self.cells.len();
        let key = wg::hash(0, wg::bytes_of(&[self.s.opt[O_THEME], self.s.opt[O_BOARD], self.tall as i32]));
        let slots: Vec<(f32, f32)> = (0..n).map(|i| self.cell_xy(i)).collect();
        let rad_cells = self.radius() as f32;
        s.background(key, |s| {
            let (x0, y0, x1, y1) = s.visible();
            s.rect_grad(x0, y0, x1 - x0, y1 - y0, 0.0, hex(th.bg), hex(th.bg2), 1.0);
            // the board's plate: a big hex with corners on the rows' ends
            let mut plate_r = [(0.0f32, 0.0f32); 6];
            for (k, p) in plate_r.iter_mut().enumerate() {
                let a = (-60.0 + 60.0 * k as f32).to_radians();
                let rad = size * (3f32.sqrt() * rad_cells + 1.35);
                *p = (ox + rad * a.cos(), oy + rad * a.sin() * SQUASH);
            }
            let mut shade_r = plate_r;
            for p in shade_r.iter_mut() {
                p.1 += 6.0;
            }
            s.poly(&shade_r, 0, 0.2);
            s.poly(&plate_r, hex(th.well), 1.0);
            for &(cx, cy) in &slots {
                s.poly(&hex_pts(cx, cy, size * 0.92, SQUASH), hex(th.slot), 1.0);
                s.poly(&hex_pts(cx, cy + size * 0.05, size * 0.8, SQUASH), mix(hex(th.slot), hex(th.well), 0.5), 1.0);
            }
        });

        // the cell a dragged stack would land on, or the keyboard's
        let aim = match self.drag {
            Some(d) if d.far => {
                let (x, y) = Self::drag_xy(&d);
                self.cell_at(x, y)
            }
            _ if self.keys && self.s.phase == PLAY => self.s.cur,
            _ if self.hover >= 0 && self.view.h[self.hover as usize] == 0 && self.s.phase == PLAY => self.hover,
            _ => -1,
        };
        if aim >= 0 {
            let (cx, cy) = slots[aim as usize];
            let free = self.s.board.h[aim as usize] == 0;
            s.poly(&hex_pts(cx, cy, size * 0.92, SQUASH), hex(th.text), if free { 0.35 } else { 0.12 });
        }

        // the stacks, back rows first, with what is moving taken out
        let now = self.anims.first().copied();
        let t = self.anim_t;
        for i in 0..n {
            let mut h = self.view.h[i] as usize;
            let mut extra = 0;
            let mut pop = (0usize, 0.0f32);
            match now {
                Some(Anim::Board(Step::Move { from, to, n: m, .. })) => {
                    if from == i {
                        h -= Self::left(m, t);
                    }
                    if to == i {
                        extra = Self::landed(m, t);
                    }
                }
                Some(Anim::Board(Step::Clear { cell, n: m, .. })) if cell == i => {
                    pop = (m, t);
                }
                _ => {}
            }
            if h + extra == 0 {
                continue;
            }
            let (cx, cy) = slots[i];
            let mut tiles: Vec<u8> = self.view.tile[i][..h].to_vec();
            if extra > 0 {
                if let Some(Anim::Board(Step::Move { c, .. })) = now {
                    tiles.extend(std::iter::repeat(c).take(extra));
                }
            }
            if pop.0 > 0 {
                // the top run shrinks away, the top tile first
                let keep = tiles.len() - pop.0;
                let lay = Self::layer(size, tiles.len());
                draw_stack(s, th, cx, cy, size, &tiles[..keep], 1.0);
                for k in 0..pop.0 {
                    let idx = tiles.len() - 1 - k;
                    let start = k as f32 * POP_GAP_MS;
                    let f = ((pop.1 - start) / POP_MS).clamp(0.0, 1.0);
                    if f >= 1.0 {
                        continue;
                    }
                    let y = cy - idx as f32 * lay - f * size * 0.5;
                    let c = hex(th.tiles[tiles[idx] as usize - 1]);
                    draw_tile(s, cx, y, size * 0.94 * (1.0 + 0.3 * f), lay, mix(hex(0xffffff), c, 1.0 - f * 0.6), 1.0, 1.0 - f);
                }
                continue;
            }
            let top = draw_stack(s, th, cx, cy, size, &tiles, 1.0);
            // the top run's count on its face, from two up
            let run = (0..tiles.len()).rev().take_while(|&k| tiles[k] == tiles[tiles.len() - 1]).count();
            if run >= 2 {
                let txt = format!("{}", run);
                s.text_c(&txt, cx, top + size * 0.02, size * 0.62, hex(0xffffff), if run >= CLEAR - 2 { 1.0 } else { 0.85 });
            }
        }

        // tiles in the air, hopping and turning over
        if let Some(Anim::Board(Step::Move { from, to, n: m, c })) = now {
            let (fx, fy) = slots[from];
            let (tx, ty) = slots[to];
            let fh = self.view.h[from] as usize;
            let th_h = self.view.h[to] as usize;
            let lay_f = Self::layer(size, fh);
            let lay_t = Self::layer(size, th_h + m);
            for k in 0..m {
                let start = k as f32 * GAP_MS;
                if t < start || t >= start + FLY_MS {
                    continue;
                }
                let f = (t - start) / FLY_MS;
                let e = f * f * (3.0 - 2.0 * f);
                let y0 = fy - (fh - 1 - k) as f32 * lay_f;
                let y1 = ty - (th_h + k) as f32 * lay_t;
                let x = fx + (tx - fx) * e;
                let y = y0 + (y1 - y0) * e - (std::f32::consts::PI * f).sin() * size * 1.1;
                let flip = (std::f32::consts::PI * f).cos().abs().max(0.12);
                draw_tile(s, x, y, size * 0.94, lay_t, hex(th.tiles[c as usize - 1]), flip, 1.0);
            }
        }
        if let Some(Anim::Drop { cell, x, y, tiles, n: m }) = now {
            let (cx, cy) = slots[cell];
            let e = wg::ease_out((t / DROP_MS).min(1.0));
            draw_stack(s, th, x + (cx - x) * e, y + (cy - y) * e, size, &tiles[..m], 1.0);
        }

        for p in &self.popups {
            let a = 1.0 - p.t * p.t;
            s.text_c(&format!("+{}", p.points), p.x, p.y - p.t * 26.0, 17.0, hex(th.text), a);
        }

        // the tray
        let ts = self.tray_size();
        for k in 0..TRAY {
            let hgt = self.s.tray_h[k] as usize;
            let (sx, sy) = self.slot_xy(k);
            let picked = self.s.pick == k as i32 && (self.keys || self.drag.is_none());
            s.poly(&hex_pts(sx, sy, ts * 1.02, SQUASH), hex(th.well), 1.0);
            if picked && hgt > 0 && self.s.phase == PLAY {
                s.poly(&hex_pts(sx, sy, ts * 1.02, SQUASH), hex(th.text), 0.3);
            }
            if hgt == 0 || self.drag.is_some_and(|d| d.slot == k && d.far) {
                continue;
            }
            let lift = if picked && self.s.phase == PLAY { 5.0 } else { 0.0 };
            draw_stack(s, th, sx, sy - lift, ts, &self.s.tray[k][..hgt], 1.0);
        }
        if let Some(d) = self.drag.filter(|d| d.far) {
            let (x, y) = Self::drag_xy(&d);
            let k = d.slot;
            let hgt = self.s.tray_h[k] as usize;
            if hgt > 0 {
                draw_stack(s, th, x, y + 6.0, size, &self.s.tray[k][..hgt], 0.25);
                draw_stack(s, th, x, y, size, &self.s.tray[k][..hgt], 1.0);
            }
        }

        // the top bar: the score
        let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
        let ty = if self.tall { 24.0 } else { 22.0 };
        s.text(&format!("{} {}", score_l, self.s.score), 14.0, ty, 17.0, hex(th.text), 1.0);
        if self.s.phase == OVER && !self.busy() {
            let full = tr(&["No room left", "没有空位了", "置く場所がありません", "빈칸이 없습니다", "No queda sitio", "Não há mais espaço", "Plus de place", "Kein Platz mehr", "Места больше нет", "沒有空位了"]);
            let size = s.fit_size(full, 15.0, w - 160.0);
            s.text_r(full, w - 14.0, ty, size, hex(th.dim), 1.0);
        }
    }

    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.phase == OVER
    }
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        match i {
            0 => s.tiles,
            1 => s.placed,
            2 => s.best_chain,
            3 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let r = 2 + s.opt[O_BOARD].clamp(0, 1);
        let ok = s.version == VERSION
            && s.opt.iter().zip(Self::CHOICES).all(|(&v, &c)| (0..c).contains(&v))
            && (PLAY..=OVER).contains(&s.phase)
            && s.board.valid()
            && s.board.radius == r
            && (0..puzzle::count(r) as i32).contains(&s.cur)
            && (0..TRAY as i32).contains(&s.pick)
            && s._pad == 0
            && s.tray.iter().zip(s.tray_h).all(|(t, h)| {
                (h as usize) <= TRAYH && t.iter().enumerate().all(|(k, &c)| if k < h as usize { (1..=puzzle::COLOURS).contains(&c) } else { c == 0 })
            })
            && s.end_ms >= 0
            // a run in play always has a stack to place
            && (s.phase == OVER || s.tray_h.iter().any(|&h| h > 0));
        self.cells = puzzle::cells(r);
        self.view = self.s.board;
        self.anims.clear();
        self.anim_t = 0.0;
        self.popups.clear();
        self.drag = None;
        self.hover = -1;
        self.tray_in = 1.0;
        ok
    }
}

/// the tiles' names, in theme `tiles` order
const NAMES: [[&str; 10]; 7] = [
    ["red", "红", "赤", "빨강", "rojo", "vermelho", "rouge", "Rot", "красный", "紅"],
    ["orange", "橙", "オレンジ", "주황", "naranja", "laranja", "orange", "Orange", "оранжевый", "橙"],
    ["yellow", "黄", "黄", "노랑", "amarillo", "amarelo", "jaune", "Gelb", "жёлтый", "黃"],
    ["green", "绿", "緑", "초록", "verde", "verde", "vert", "Grün", "зелёный", "綠"],
    ["cyan", "青", "水色", "하늘색", "cian", "ciano", "cyan", "Cyan", "голубой", "青"],
    ["blue", "蓝", "青", "파랑", "azul", "azul", "bleu", "Blau", "синий", "藍"],
    ["purple", "紫", "紫", "보라", "morado", "roxo", "violet", "Lila", "фиолетовый", "紫"],
];

/// a stack in words from the top: "4 red, 3 blue"
fn stack_words(tiles: &[u8], out: &mut String) {
    let mut k = tiles.len();
    while k > 0 {
        let c = tiles[k - 1];
        let n = tiles[..k].iter().rev().take_while(|&&t| t == c).count();
        if k < tiles.len() {
            out.push_str(", ");
        }
        out.push_str(&format!("{} {}", n, tr(&NAMES[(c as usize).clamp(1, 7) - 1])));
        k -= n;
    }
}

/// The accessible board: a square over each cell and each stack in the
/// tray, laid where they are drawn; arrows go to the nearest one.
impl wg::Board for Hexpile {
    fn grid(&self) -> Option<(i32, i32)> {
        Some((self.cells.len() as i32 + TRAY as i32, 1))
    }
    fn cursor(&self) -> (i32, i32) {
        (if self.on_tray >= 0 { self.cells.len() as i32 + self.on_tray } else { self.s.cur }, 0)
    }
    fn focus(&mut self, x: i32, _y: i32) {
        let n = self.cells.len() as i32;
        if x >= n {
            self.on_tray = (x - n).min(TRAY as i32 - 1);
        } else if x >= 0 {
            self.on_tray = -1;
            self.s.cur = x;
        }
        self.keys = true;
        self.hover = -1;
    }
    fn rect(&self) -> (f32, f32, f32) {
        (0.0, 0.0, 1.0)
    }
    fn cell_box(&self, x: i32, _y: i32) -> Option<(f32, f32, f32, f32)> {
        let n = self.cells.len() as i32;
        if x >= n {
            let size = self.tray_size();
            let (sx, sy) = self.slot_xy((x - n).min(TRAY as i32 - 1) as usize);
            return Some((sx - size * 1.6, sy - size * 2.6, size * 3.2, size * 3.8));
        }
        let size = self.geo().2;
        let (cx, cy) = self.cell_xy(x.max(0) as usize);
        let (w, h) = (size * 3f32.sqrt(), size * 2.0 * SQUASH);
        Some((cx - w / 2.0, cy - h / 2.0, w, h))
    }
    fn cell(&self, x: i32, _y: i32, out: &mut String) {
        let n = self.cells.len() as i32;
        if x >= n {
            let k = (x - n).min(TRAY as i32 - 1) as usize;
            out.push_str(&tr(&["Next stack #", "待放堆 #", "次の山 #", "다음 더미 #", "Pila siguiente #", "Pilha seguinte #", "Pile suivante #", "Nächster Stapel #", "Следующая стопка #", "待放堆 #"]).replace('#', &(k + 1).to_string()));
            out.push_str(": ");
            let h = self.s.tray_h[k] as usize;
            if h == 0 {
                out.push_str(tr(&["placed", "已放下", "置いた", "놓음", "colocada", "colocada", "posée", "gelegt", "поставлена"]));
                return;
            }
            stack_words(&self.s.tray[k][..h], out);
            if self.s.pick == k as i32 {
                out.push_str(", ");
                out.push_str(tr(&["picked", "已选", "選択中", "선택됨", "elegida", "escolhida", "choisie", "gewählt", "выбрана", "已選"]));
            }
            return;
        }
        let i = x.max(0) as usize;
        let (q, r) = self.cells[i];
        let row = self.cells.iter().filter(|c| c.1 == r).position(|c| c.0 == q).unwrap_or(0);
        let rad = self.radius();
        out.push_str(
            &tr(&["Row #, cell #", "第 # 行第 # 格", "# 行目 # 番", "# 행 # 칸", "Fila #, casilla #", "Linha #, casa #", "Rangée #, case #", "Reihe #, Feld #", "Ряд #, клетка #", "第 # 行第 # 格"])
                .replacen('#', &(r + rad + 1).to_string(), 1)
                .replacen('#', &(row + 1).to_string(), 1),
        );
        out.push_str(": ");
        let h = self.s.board.h[i] as usize;
        if h == 0 {
            out.push_str(tr(&["empty", "空", "空き", "빈 칸", "vacía", "vazia", "vide", "leer", "пусто"]));
        } else {
            stack_words(&self.s.board.tile[i][..h], out);
        }
    }
    fn status(&self, out: &mut String) {
        let score_l = tr(&["Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки", "得分"]);
        let free_l = tr(&["empty cells", "空格", "空きマス", "빈 칸", "casillas libres", "casas livres", "cases libres", "freie Felder", "свободных клеток", "空格"]);
        out.push_str(&format!("{} {}, {} {}. ", score_l, self.s.score, free_l, self.s.board.empty_cells()));
        let k = self.s.pick as usize;
        if self.s.tray_h[k] > 0 {
            self.cell(self.cells.len() as i32 + k as i32, 0, out);
        }
    }
    fn keys(&self) -> &'static str {
        tr(&[
            "Arrows move over the board and the next stacks, Space picks a next stack or drops the picked one on a cell, X picks the next one, I reads the score and the picked stack. Stacks read from the top; ten of one colour together clear.",
            "方向键在棋盘和待放堆之间移动，空格选择待放堆或把选中的堆放到格子上，X 换下一堆，I 读出得分和选中的堆。堆从上往下读；同色十片连在一起就消除。",
            "矢印で盤と次の山を移動、スペースで山を選ぶか選んだ山をマスに置く、X で次の山、I でスコアと選んだ山。山は上から読み上げます。同じ色が10枚そろうと消えます。",
            "화살표로 판과 다음 더미 사이 이동, 스페이스로 더미 선택 또는 선택한 더미를 칸에 놓기, X로 다음 더미, I로 점수와 선택한 더미. 더미는 위부터 읽습니다. 같은 색 10장이 모이면 사라집니다.",
            "Flechas para moverte por el tablero y las pilas siguientes, Espacio elige una pila o deja la elegida en una casilla, X elige la siguiente, I lee puntos y la pila elegida. Las pilas se leen desde arriba; diez de un color juntas se eliminan.",
            "Setas movem pelo tabuleiro e pelas pilhas seguintes, Espaço escolhe uma pilha ou solta a escolhida numa casa, X escolhe a próxima, I lê pontos e a pilha escolhida. As pilhas são lidas de cima; dez da mesma cor juntas somem.",
            "Flèches pour parcourir le plateau et les piles suivantes, Espace choisit une pile ou pose celle choisie sur une case, X choisit la suivante, I lit le score et la pile choisie. Les piles se lisent du haut ; dix d'une couleur ensemble disparaissent.",
            "Pfeiltasten bewegen über das Brett und die nächsten Stapel, Leertaste wählt einen Stapel oder legt den gewählten auf ein Feld, X wählt den nächsten, I liest Punkte und den gewählten Stapel vor. Stapel werden von oben gelesen; zehn einer Farbe zusammen verschwinden.",
            "Стрелки ходят по полю и следующим стопкам, пробел выбирает стопку или кладёт выбранную на клетку, X выбирает следующую, I читает очки и выбранную стопку. Стопки читаются сверху; десять одного цвета вместе исчезают.",
            "方向鍵在棋盤和待放堆之間移動，空白鍵選擇待放堆或把選中的堆放到格子上，X 換下一堆，I 讀出得分和選中的堆。堆從上往下讀；同色十片連在一起就消除。",
        ])
    }
    fn act(&mut self, x: i32, y: i32, a: i32, said: &mut String) -> u32 {
        if self.s.phase != PLAY {
            return 0;
        }
        self.focus(x, y);
        let n = self.cells.len() as i32;
        let pick_next = a != 0;
        if pick_next || x >= n {
            let k = if pick_next {
                let k = self.s.pick as usize;
                (1..=TRAY).map(|d| (k + d) % TRAY).find(|&j| self.s.tray_h[j] > 0)
            } else {
                Some((x - n) as usize).filter(|&k| self.s.tray_h[k] > 0)
            };
            if let Some(k) = k {
                self.s.pick = k as i32;
            }
            self.cell(n + if pick_next { self.s.pick } else { x - n }, 0, said);
            return 0;
        }
        let (score, tiles) = (self.s.score, self.s.tiles);
        let (sx, sy) = self.slot_xy(self.s.pick as usize);
        let ev = self.place(self.s.pick as usize, x as usize, sx, sy);
        if ev != 0 && self.s.tiles > tiles {
            said.push_str(&tr(&["Cleared # tiles, +# points. ", "消除 # 片，+# 分。", "# 枚消えた、+# 点。", "# 장 제거, +# 점. ", "# fichas eliminadas, +# puntos. ", "# peças removidas, +# pontos. ", "# tuiles effacées, +# points. ", "# Steine weg, +# Punkte. ", "Убрано плиток: #, +# очков. ", "消除 # 片，+# 分。"])
                .replacen('#', &(self.s.tiles - tiles).to_string(), 1)
                .replacen('#', &(self.s.score - score).to_string(), 1));
        }
        self.cell(x, 0, said);
        if self.s.phase != PLAY {
            said.push_str(". ");
            said.push_str(tr(&["No room left", "没有空位了", "置く場所がありません", "빈칸이 없습니다", "No queda sitio", "Não há mais espaço", "Plus de place", "Kein Platz mehr", "Места больше нет", "沒有空位了"]));
        }
        ev
    }
    fn rev(&mut self) -> u32 {
        let v = [self.s.phase, self.s.pick, self.s.score, self.tall as i32, wg::lang() as i32];
        let h = wg::hash(wg::hash(0, wg::bytes_of(&self.s.board)), wg::bytes_of(&self.s.tray));
        wg::hash(h, wg::bytes_of(&v))
    }
}

wg::game!(Hexpile);
wg::board!(Hexpile);

#[cfg(test)]
mod tests {
    use super::*;

    fn run(g: &mut Hexpile, ms: i32) -> u32 {
        let mut ev = 0;
        for _ in 0..ms / 16 + 1 {
            ev |= g.update(&Input { held: 0, pressed: 0, dt: 16 });
        }
        ev
    }

    /// plays whole runs, dropping each stack next to a matching top where it can
    #[test]
    fn plays_runs() {
        for board in 0..2 {
            for seed in 1..6u32 {
                let mut g = Hexpile::new();
                g.s.opt[O_BOARD] = board;
                g.start(seed * 7919);
                let mut ev = 0;
                let mut moves = 0;
                while !g.over() {
                    let k = (0..TRAY).find(|&k| g.s.tray_h[k] > 0).unwrap();
                    let b = g.s.board;
                    let n = b.cells();
                    let top = g.s.tray[k][g.s.tray_h[k] as usize - 1];
                    let free: Vec<usize> = (0..n).filter(|&i| b.h[i] == 0).collect();
                    let good = free.iter().copied().max_by_key(|&i| {
                        puzzle::neighbours(b.radius, i).iter().filter(|&&j| b.h[j] > 0 && b.top(j) == top).count() * 10 + (i * 7 + moves) % 9
                    });
                    ev |= g.place(k, good.unwrap(), 0.0, 0.0);
                    ev |= run(&mut g, 20_000);
                    assert!(g.view == g.s.board, "the picture catches up");
                    let mut check = Hexpile::new();
                    check.s = g.s;
                    assert!(check.loaded());
                    moves += 1;
                    assert!(moves < 3000);
                }
                assert!(run(&mut g, END_MS + 100) & wg::EV_GAMEOVER != 0 || ev & wg::EV_GAMEOVER != 0);
                println!("board {} seed {} placed {} tiles {} score {} chain {}", board, seed, g.s.placed, g.s.tiles, g.s.score, g.s.best_chain);
                assert!(g.s.tiles > 0);
            }
        }
    }
}
