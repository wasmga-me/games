/* Sapper - an original wasmga.me game. MIT licensed.
 * Open every safe square; the numbers count the mines next to each one.
 * The page shows the setup, pause and game over screens (see wg.h); the
 * board draws at the screen's own size (wgview.h). */
#define WG_OWN_FB
#define WGFB_LAYERS 2 /* the screen, and the empty board */
#include "wg.h"
#include "sdf_font.h"
#include "wgfb.h"
#include "wgview.h"

/* the plan everything is laid out on, scaled to the screen */
#define PW 400
#define PH 300

#define MAXC 30
#define MAXR 16
#define HUD 30
#define MAGIC 0x53415031u /* "SAP1" */
#define VERSION 5

#define END_MS 1200 /* the board stays in view this long before game over */

enum { PLAY, WON, LOST };
enum { CLOSED, OPEN, FLAG, MARK }; /* MARK: a question mark, still covered */
/* options, in catalog order: board, mines, theme */
enum { O_SIZE, O_DENSITY, O_THEME, NOPT };
#define THEMES 4
static const i32 CHOICES[NOPT] = {3, 3, THEMES};

static const i32 SIZES[3][2] = {{9, 9}, {16, 16}, {30, 16}};
static const i32 MINES[3][3] = {{8, 10, 14}, {30, 40, 52}, {75, 99, 120}};

static struct {
  u32 magic, version, rng;
  i32 phase, end_ms;
  i32 opt[NOPT];
  i32 cols, rows, mines, cell, ox, oy;
  i32 placed, opened, flags, time_ms, cur_x, cur_y, keyboard;
  i32 boom_x, boom_y, flag_mode;
  i32 mx, my, mbtn, press_on, press_x, press_y, press_ms, press_touch, long_done;
  i32 pend_ev, prev_buttons, rep_ms, blink, score;
  i32 clicks; /* every press that acts on the board, for the stats */
  u8 mine[MAXR][MAXC], st[MAXR][MAXC], adj[MAXR][MAXC];
} S;

typedef struct {
  const char *name;
  u32 bg, hud, text, dim, accent, closed, hi, lo, open, line, flag, mine, boom, cursor, panel;
  u32 num[8];
} Theme;

static const Theme THEME[THEMES] = {
  {"SLATE", RGB(23, 20, 46), RGB(14, 11, 30), RGB(255, 247, 232), RGB(150, 140, 190), RGB(46, 196, 182),
   RGB(84, 66, 210), RGB(122, 106, 245), RGB(54, 41, 150), RGB(34, 29, 66), RGB(44, 38, 82),
   RGB(255, 201, 60), RGB(255, 247, 232), RGB(255, 90, 54), RGB(255, 201, 60), RGB(40, 34, 78),
   {RGB(111, 183, 255), RGB(79, 209, 139), RGB(255, 122, 107), RGB(190, 150, 255), RGB(255, 179, 71),
    RGB(79, 209, 209), RGB(255, 143, 208), RGB(210, 210, 210)}},
  {"PAPER", RGB(243, 234, 216), RGB(228, 214, 187), RGB(42, 33, 24), RGB(120, 104, 84), RGB(101, 79, 240),
   RGB(196, 174, 132), RGB(224, 206, 168), RGB(150, 128, 92), RGB(252, 247, 237), RGB(229, 218, 196),
   RGB(214, 64, 43), RGB(42, 33, 24), RGB(255, 140, 110), RGB(101, 79, 240), RGB(250, 243, 228),
   {RGB(47, 95, 208), RGB(46, 139, 74), RGB(200, 55, 45), RGB(75, 45, 154), RGB(138, 59, 18),
    RGB(31, 138, 138), RGB(42, 33, 24), RGB(122, 122, 122)}},
  {"MOSS", RGB(29, 43, 31), RGB(20, 32, 23), RGB(238, 243, 223), RGB(160, 180, 150), RGB(198, 227, 107),
   RGB(79, 125, 58), RGB(110, 158, 78), RGB(56, 92, 42), RGB(222, 207, 166), RGB(206, 189, 145),
   RGB(232, 67, 46), RGB(29, 43, 31), RGB(240, 110, 80), RGB(255, 243, 160), RGB(38, 58, 41),
   {RGB(42, 91, 184), RGB(47, 122, 50), RGB(184, 50, 42), RGB(90, 42, 138), RGB(138, 74, 18),
    RGB(31, 122, 122), RGB(34, 34, 34), RGB(102, 102, 102)}},
  {"EMBER", RGB(26, 20, 20), RGB(16, 12, 12), RGB(255, 233, 214), RGB(170, 140, 125), RGB(255, 138, 61),
   RGB(96, 60, 48), RGB(132, 86, 66), RGB(64, 38, 32), RGB(42, 32, 32), RGB(54, 41, 40),
   RGB(255, 176, 46), RGB(255, 233, 214), RGB(255, 61, 46), RGB(255, 138, 61), RGB(48, 36, 34),
   {RGB(255, 211, 107), RGB(255, 159, 67), RGB(255, 94, 87), RGB(224, 108, 255), RGB(255, 61, 127),
    RGB(127, 214, 255), RGB(255, 255, 255), RGB(170, 170, 170)}},
};

#define T (THEME[S.opt[O_THEME]])

/* ---------- board ---------- */

/* board size, mine count and where the board sits, from the options */
static void layout(void) {
  S.cols = SIZES[S.opt[O_SIZE]][0];
  S.rows = SIZES[S.opt[O_SIZE]][1];
  S.mines = MINES[S.opt[O_SIZE]][S.opt[O_DENSITY]];
  i32 cw = (PW - 12) / S.cols, ch = (PH - HUD - 12) / S.rows;
  S.cell = cw < ch ? cw : ch;
  if (S.cell > 34) S.cell = 34;
  S.ox = (PW - S.cell * S.cols) / 2;
  S.oy = HUD + (PH - HUD - S.cell * S.rows) / 2;
}

static void setup_board(void) {
  layout();
  memset(S.mine, 0, sizeof S.mine);
  memset(S.st, 0, sizeof S.st);
  memset(S.adj, 0, sizeof S.adj);
  S.placed = S.opened = S.flags = S.time_ms = 0;
  S.cur_x = S.cols / 2;
  S.cur_y = S.rows / 2;
  S.boom_x = S.boom_y = -1;
  S.press_on = 0;
  S.score = 0;
  S.end_ms = 0;
  S.flag_mode = 0;
  S.phase = PLAY;
}

static i32 on_board(i32 x, i32 y) { return x >= 0 && y >= 0 && x < S.cols && y < S.rows; }

static void place_counts(void);
static i32 rip[MAXR][MAXC], pop[MAXR][MAXC];
static void ripple_from(i32 ox, i32 oy, i32 step);

/* the first square opened and its neighbours never hold a mine */
static void place_mines(i32 sx, i32 sy) {
  i32 total = S.cols * S.rows, spare = total - 9;
  i32 keep_ring = spare >= S.mines;
  i32 left = S.mines;
  while (left) {
    i32 k = (i32)(xorshift(&S.rng) % (u32)total), x = k % S.cols, y = k / S.cols;
    if (S.mine[y][x]) continue;
    i32 dx = x - sx, dy = y - sy;
    if (dx == 0 && dy == 0) continue;
    if (keep_ring && dx >= -1 && dx <= 1 && dy >= -1 && dy <= 1) continue;
    S.mine[y][x] = 1;
    left--;
  }
  place_counts();
  S.placed = 1;
}

/* the number on every square */
static void place_counts(void) {
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++) {
      i32 n = 0;
      for (i32 j = -1; j <= 1; j++)
        for (i32 i = -1; i <= 1; i++)
          if (on_board(x + i, y + j) && S.mine[y + j][x + i]) n++;
      S.adj[y][x] = (u8)n;
    }
}

/* 3BV: the fewest clicks that clear the board, one per opening and one per
 * number not on an opening's edge; with only_open, those already done */
static i32 bbbv(i32 only_open) {
  static u8 seen[MAXR][MAXC];
  static u16 stack[MAXR * MAXC];
  memset(seen, 0, sizeof seen);
  i32 n = 0;
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++) {
      if (S.mine[y][x] || S.adj[y][x] || seen[y][x]) continue;
      /* an opening: the zeros joined to this one and the numbers round them */
      i32 sp = 0, done = 1;
      seen[y][x] = 1;
      stack[sp++] = (u16)(y * MAXC + x);
      while (sp) {
        i32 k = stack[--sp], cx = k % MAXC, cy = k / MAXC;
        if (S.st[cy][cx] != OPEN) done = 0;
        if (S.adj[cy][cx]) continue;
        for (i32 j = -1; j <= 1; j++)
          for (i32 i = -1; i <= 1; i++) {
            i32 nx = cx + i, ny = cy + j;
            if (!on_board(nx, ny) || seen[ny][nx]) continue;
            seen[ny][nx] = 1;
            stack[sp++] = (u16)(ny * MAXC + nx);
          }
      }
      if (!only_open || done) n++;
    }
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++)
      if (!S.mine[y][x] && !seen[y][x] && (!only_open || S.st[y][x] == OPEN)) n++;
  return n;
}

static void win(void) {
  S.phase = WON;
  S.end_ms = END_MS;
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++)
      if (S.mine[y][x]) S.st[y][x] = FLAG;
  S.flags = S.mines;
  i32 t = S.time_ms / 100;
  S.score = (i32)((u32)S.mines * 60000u / (u32)(t + 300));
  if (S.score < 1) S.score = 1;
  S.pend_ev |= WG_EV_WIN;
}

static void lose(i32 x, i32 y) {
  S.phase = LOST;
  S.end_ms = END_MS;
  S.boom_x = x;
  S.boom_y = y;
  for (i32 j = 0; j < S.rows; j++)
    for (i32 i = 0; i < S.cols; i++)
      if (S.mine[j][i] && S.st[j][i] != FLAG && !(i == x && j == y)) rip[j][i] = -1;
  ripple_from(x, y, 45);
  S.score = 0;
  S.pend_ev |= WG_EV_LOSE;
}

static void reveal(i32 x, i32 y) {
  if (!on_board(x, y) || S.st[y][x] == OPEN || S.st[y][x] == FLAG) return;
  if (!S.placed) place_mines(x, y);
  if (S.mine[y][x]) { lose(x, y); return; }
  static u16 stack[MAXR * MAXC];
  i32 sp = 0, before = S.opened;
  S.st[y][x] = OPEN;
  S.opened++;
  rip[y][x] = -1;
  stack[sp++] = (u16)(y * MAXC + x);
  while (sp) {
    i32 k = stack[--sp], cx = k % MAXC, cy = k / MAXC;
    if (S.adj[cy][cx]) continue;
    for (i32 j = -1; j <= 1; j++)
      for (i32 i = -1; i <= 1; i++) {
        i32 nx = cx + i, ny = cy + j;
        if (!on_board(nx, ny) || S.st[ny][nx] == OPEN || S.st[ny][nx] == FLAG) continue;
        S.st[ny][nx] = OPEN;
        S.opened++;
        rip[ny][nx] = -1;
        stack[sp++] = (u16)(ny * MAXC + nx);
      }
  }
  ripple_from(x, y, 28);
  S.pend_ev |= S.opened - before > 1 ? WG_EV_HIT : WG_EV_BLIP;
  if (S.opened == S.cols * S.rows - S.mines) win();
}

/* an open number with as many flags around it as it says opens the rest */
static void chord(i32 x, i32 y) {
  if (!on_board(x, y) || S.st[y][x] != OPEN || !S.adj[y][x]) return;
  i32 f = 0;
  for (i32 j = -1; j <= 1; j++)
    for (i32 i = -1; i <= 1; i++)
      if (on_board(x + i, y + j) && S.st[y + j][x + i] == FLAG) f++;
  if (f != S.adj[y][x]) return;
  for (i32 j = -1; j <= 1 && S.phase == PLAY; j++)
    for (i32 i = -1; i <= 1 && S.phase == PLAY; i++) reveal(x + i, y + j);
}

static void toggle_flag(i32 x, i32 y) {
  if (!on_board(x, y)) return;
  /* covered, flag, question mark, covered again */
  if (S.st[y][x] == CLOSED) { S.st[y][x] = FLAG; S.flags++; pop[y][x] = 140; }
  else if (S.st[y][x] == FLAG) { S.st[y][x] = MARK; S.flags--; pop[y][x] = 140; }
  else if (S.st[y][x] == MARK) S.st[y][x] = CLOSED;
  else return;
  S.pend_ev |= WG_EV_BLIP;
}

/* the main action on a square: dig, or open around a satisfied number */
static void act(i32 x, i32 y) {
  if (!on_board(x, y)) return;
  if (S.st[y][x] == OPEN) chord(x, y);
  else if (S.flag_mode) toggle_flag(x, y);
  else reveal(x, y);
}

/* ---------- layout ---------- */

#define BTN 24
#define FLAG_BTN_X (PW - BTN - 6)
#define BTN_Y 3

/* ---------- drawing ---------- */

static float ease(float t) { t = clampf(t, 0, 1); return t * t * (3 - 2 * t); }

static void draw_mine(float cx, float cy, float s, u32 c, float a) {
  float l = s * 0.36f, d = l * 0.72f, t = maxf(s * 0.045f, 0.5f);
  v_line(cx - l, cy, cx + l, cy, t, c, a);
  v_line(cx, cy - l, cx, cy + l, t, c, a);
  v_line(cx - d, cy - d, cx + d, cy + d, t, c, a);
  v_line(cx - d, cy + d, cx + d, cy - d, t, c, a);
  v_disc(cx, cy, s * 0.24f, c, a);
  v_disc(cx - s * 0.08f, cy - s * 0.08f, s * 0.065f, RGB(255, 255, 255), 0.55f * a);
}

/* a flag in a square of size s whose top left is x, y */
static void draw_flag(float x, float y, float s, u32 c, u32 pole, float a) {
  float px = x + s * 0.38f, t = maxf(s * 0.04f, 0.45f);
  v_box(x + s * 0.24f, y + s * 0.72f, s * 0.48f, s * 0.09f, s * 0.045f, pole, a);
  v_line(px, y + s * 0.2f, px, y + s * 0.74f, t, pole, a);
  v_tri(px + t * 0.5f, y + s * 0.16f, px + s * 0.42f, y + s * 0.33f, px + t * 0.5f, y + s * 0.5f, c, a);
}

static void draw_closed(float x, float y, float s, i32 hover) {
  float g = maxf(s * 0.05f, 0.5f), b = s * 0.09f, r = s * 0.16f, w = s - 2 * g;
  u32 face = hover ? mixc(T.hi, T.closed, 0.4f) : T.closed;
  v_box(x + g, y + g, w, w, r, T.lo, 1);
  v_box(x + g, y + g, w, w - b, r, T.hi, 1);
  v_box_grad(x + g + b * 0.6f, y + g + b * 0.6f, w - b * 1.2f, w - b * 2, r * 0.7f, mixc(T.hi, face, 0.25f), face, 1);
}

/* rip: time left in each square's opening, so a cleared patch ripples
 * open; pop: a flag going up. Pictures only, so not in the save. */
#define RIP_MS 170

static void ripple_from(i32 ox, i32 oy, i32 step) {
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++)
      if (rip[y][x] < 0) {
        i32 dx = x > ox ? x - ox : ox - x, dy = y > oy ? y - oy : oy - y, d = dx > dy ? dx : dy;
        rip[y][x] = RIP_MS + (d * step > 700 ? 700 : d * step);
      }
}

/* the board panel, kept in layer 1 */
static void draw_static(void) {
  float s = (float)S.cell, w = s * S.cols, h = s * S.rows;
  clear(T.bg);
  v_box(0, 0, PW, HUD, 0, T.hud, 1);
  v_box(-50, -50, PW + 100, HUD + 50, 0, T.hud, 1);
  v_shadow(S.ox - 4, S.oy - 1, w + 8, h + 8, 7, 6, RGB(0, 0, 0), 0.35f);
  v_box(S.ox - 3, S.oy - 3, w + 6, h + 6, 6, T.line, 1);
  /* every square starts covered; frames draw only what differs */
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++) draw_closed(S.ox + x * s, S.oy + y * s, s, 0);
}

static void draw_board(void) {
  float s = (float)S.cell, hr = s * 0.08f;
  i32 hx = -1, hy = -1;
  if (S.mx >= S.ox && S.my >= S.oy && !S.keyboard) {
    hx = (S.mx - S.ox) / S.cell;
    hy = (S.my - S.oy) / S.cell;
  }
  i32 over = S.phase == LOST || S.phase == WON;
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++) {
      float px = S.ox + x * s, py = S.oy + y * s, cx = px + s / 2, cy = py + s / 2;
      i32 st = S.st[y][x], boom = x == S.boom_x && y == S.boom_y;
      i32 pressed = S.phase == PLAY && S.press_on && S.mbtn & M_LEFT && x == hx && y == hy && (st == CLOSED || st == MARK) && !S.flag_mode;
      i32 shown = st == OPEN || (over && S.mine[y][x] && st != FLAG);
      if (shown || pressed) {
        /* opening: the cover fades and shrinks away after its delay */
        float cover = rip[y][x] > 0 ? clampf((float)rip[y][x] / RIP_MS, 0, 1) : 0;
        if (cover >= 1) continue;
        v_box(px, py, s, s, 0, T.line, 1);
        v_box(px + 0.35f, py + 0.35f, s - 0.7f, s - 0.7f, hr, boom ? T.boom : T.open, 1);
        if (!shown) continue;
        float a = 1 - ease(cover);
        if (st == OPEN && S.adj[y][x]) {
          char d[2] = {(char)('0' + S.adj[y][x]), 0};
          v_text_c(d, cx, cy, s * 0.66f, T.num[S.adj[y][x] - 1], a);
        } else if (st != OPEN) {
          draw_mine(cx, cy, s, boom ? T.open : T.mine, a);
        }
        if (cover > 0) {
          float k = ease(cover), in = (1 - k) * s * 0.3f;
          v_box(px + in, py + in, s - 2 * in, s - 2 * in, s * 0.16f, T.closed, k);
        }
      } else {
        if (x == hx && y == hy && S.phase == PLAY) draw_closed(px, py, s, 1);
        if (st == MARK) {
          float p = pop[y][x] > 0 ? ease(1 - pop[y][x] / 140.0f) : 1;
          v_text_c("?", cx, cy, s * (0.4f + 0.26f * p), T.text, 0.3f + 0.7f * p);
        }
        if (st == FLAG) {
          float p = pop[y][x] > 0 ? ease(1 - pop[y][x] / 140.0f) : 1, z = s * (0.25f + 0.75f * p);
          draw_flag(cx - z / 2, cy - z / 2 - s * 0.03f, z, T.flag, T.text, 0.3f + 0.7f * p);
          if (S.phase == LOST && !S.mine[y][x]) {
            /* a flag on a safe square */
            float m = s * 0.24f, t = maxf(s * 0.05f, 0.6f);
            v_line(px + m, py + m, px + s - m, py + s - m, t, T.boom, 1);
            v_line(px + s - m, py + m, px + m, py + s - m, t, T.boom, 1);
          }
        }
      }
    }
  if (S.keyboard && S.phase == PLAY) {
    float x0 = S.ox + S.cur_x * s - 0.5f, y0 = S.oy + S.cur_y * s - 0.5f, x1 = x0 + s + 1, y1 = y0 + s + 1, t = maxf(s * 0.06f, 0.8f);
    v_line(x0, y0, x1, y0, t, T.cursor, 1);
    v_line(x1, y0, x1, y1, t, T.cursor, 1);
    v_line(x1, y1, x0, y1, t, T.cursor, 1);
    v_line(x0, y1, x0, y0, t, T.cursor, 1);
  }
}

static void draw_hud(void) {
  char buf[24], num[12];
  float mid = HUD / 2.0f;
  if (S.phase == WON) {
    v_text(join(buf, "Cleared in ", tenths(S.time_ms / 100, num)), 10, mid, 17, T.accent, 1);
    return;
  }
  if (S.phase == LOST) {
    v_text("Boom", 10, mid, 17, T.boom, 1);
    return;
  }
  i32 left = S.mines - S.flags;
  draw_mine(18, mid, 20, T.text, 1);
  v_text(itoa10(left, buf), 34, mid, 17, T.text, 1);
  i32 secs = S.time_ms / 1000;
  if (secs > 999) secs = 999;
  itoa10(secs, buf);
  float tr = FLAG_BTN_X - 12, tw = v_text_w(buf, 17);
  v_text_r(buf, tr, mid, 17, T.text, 1);
  /* clock */
  float cx = tr - tw - 13, cy = mid;
  v_ring(cx, cy, 7.5f, 1.8f, T.dim, 1);
  v_line(cx, cy, cx, cy - 4, 0.9f, T.dim, 1);
  v_line(cx, cy, cx + 3, cy, 0.9f, T.dim, 1);
  i32 hot = S.flag_mode || (!S.keyboard && inside(S.mx, S.my, FLAG_BTN_X, BTN_Y, BTN, BTN));
  v_box(FLAG_BTN_X, BTN_Y, BTN, BTN, 6, S.flag_mode ? T.accent : hot ? mixc(T.accent, T.panel, 0.25f) : T.panel, 1);
  draw_flag(FLAG_BTN_X + 2, BTN_Y + 2, 20, S.flag_mode ? T.hud : T.flag, S.flag_mode ? T.hud : T.text, 1);
}

static void draw(void) {
  view_background(hash_int(hash_int(S.opt[O_THEME] + 1, S.cols), S.rows), draw_static);
  draw_board();
  draw_hud();
}

/* what the picture depends on, so a still board isn't repainted every frame */
static u32 picture(void) {
  u32 h = hash_bytes(0, S.st, sizeof S.st);
  i32 v[] = {S.phase, S.flags, S.mx, S.my, S.keyboard, S.cur_x, S.cur_y, S.press_on, S.mbtn, S.flag_mode,
             S.boom_x, S.time_ms / 1000, S.opt[O_THEME], S.cols, S.rows};
  return hash_bytes(h, v, sizeof v);
}

/* moves the openings on; returns whether anything is still moving */
static i32 animate(i32 dt) {
  i32 busy = 0;
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++) {
      if (rip[y][x] > 0) { rip[y][x] -= dt; busy = 1; }
      if (rip[y][x] < 0) rip[y][x] = 0;
      if (pop[y][x] > 0) { pop[y][x] -= dt; busy = 1; }
    }
  return busy;
}

static void fb_resized(void) {
  view_fit(PW, PH);
  view_redraw();
}

WG_EXPORT(wg_resize) void wg_resize(i32 w, i32 h) {
  if (fb_size(w, h, WGFB_LAYERS)) fb_resized();
}

/* ---------- input ---------- */

static void board_cell(i32 x, i32 y, i32 *cx, i32 *cy) {
  *cx = *cy = -1;
  if (!S.cell || x < S.ox || y < S.oy) return;
  i32 gx = (x - S.ox) / S.cell, gy = (y - S.oy) / S.cell;
  if (on_board(gx, gy)) { *cx = gx; *cy = gy; }
}

static void pointer_release(i32 x, i32 y) {
  if (S.phase != PLAY) return;
  if (inside(x, y, FLAG_BTN_X, BTN_Y, BTN, BTN)) { S.flag_mode = !S.flag_mode; S.pend_ev |= WG_EV_BLIP; return; }
  if (!S.press_on || S.long_done) return;
  i32 cx, cy;
  board_cell(x, y, &cx, &cy);
  if (S.press_touch) { cx = S.press_x; cy = S.press_y; }
  if (on_board(cx, cy)) S.clicks++;
  act(cx, cy);
}

/* x, y in game pixels (-1 when the pointer leaves); b = M_* bits held now */
WG_EXPORT(wg_mouse) void wg_mouse(i32 x, i32 y, i32 b) {
  i32 held = b & 7, prev = S.mbtn, pressed = held & ~prev, released = prev & ~held;
  S.mbtn = held;
  if (x < 0) { S.mx = S.my = -1; S.press_on = 0; return; }
  x = plan_x(x);
  y = plan_y(y);
  if (x != S.mx || y != S.my) S.keyboard = 0;
  S.mx = x;
  S.my = y;
  i32 cx, cy;
  board_cell(x, y, &cx, &cy);
  if (S.phase == PLAY) {
    if (pressed & M_RIGHT && cx >= 0) { S.clicks++; toggle_flag(cx, cy); }
    if (pressed & M_MIDDLE && cx >= 0) { S.clicks++; chord(cx, cy); }
    if (pressed & M_LEFT) {
      S.press_on = 1;
      S.press_x = cx;
      S.press_y = cy;
      S.press_ms = 0;
      S.press_touch = (b & M_TOUCH) != 0;
      S.long_done = 0;
    } else if (S.press_on && S.press_touch && (cx != S.press_x || cy != S.press_y)) {
      S.long_done = 1; /* a touch that slid off its square does nothing */
    }
  }
  if (released & M_LEFT) {
    pointer_release(x, y);
    S.press_on = 0;
  }
}

static void keys(u32 buttons, u32 pressed, i32 dt) {
  u32 dirs = key_repeat(buttons, pressed, dt, &S.rep_ms);
  if (pressed) S.keyboard = 1;
  if (S.phase != PLAY) return;
    if (dirs & B_LEFT) S.cur_x = (S.cur_x + S.cols - 1) % S.cols;
    if (dirs & B_RIGHT) S.cur_x = (S.cur_x + 1) % S.cols;
    if (dirs & B_UP) S.cur_y = (S.cur_y + S.rows - 1) % S.rows;
    if (dirs & B_DOWN) S.cur_y = (S.cur_y + 1) % S.rows;
    if (pressed & B_A) { S.clicks++; act(S.cur_x, S.cur_y); }
    if (pressed & B_B) { S.clicks++; toggle_flag(S.cur_x, S.cur_y); }
}

WG_EXPORT(wg_init) void wg_init(u32 seed) {
  static const i32 DEFAULTS[NOPT] = {1, 1, 0};
  i32 opt[NOPT];
  for (i32 i = 0; i < NOPT; i++) opt[i] = S.magic == MAGIC && S.version == VERSION ? S.opt[i] : DEFAULTS[i];
  memset(&S, 0, sizeof S);
  S.magic = MAGIC;
  S.version = VERSION;
  S.rng = seed | 1;
  memcpy(S.opt, opt, sizeof opt);
  S.mx = S.my = -1;
  setup_board();
  memset(rip, 0, sizeof rip);
  memset(pop, 0, sizeof pop);
}

WG_EXPORT(wg_option) i32 wg_option(i32 i, i32 v) {
  if (S.magic != MAGIC) wg_init(1);
  return wg_pick(S.opt, CHOICES, NOPT, i, v);
}

WG_EXPORT(wg_over) i32 wg_over(void) { return S.phase != PLAY; }

WG_EXPORT(wg_frame) i32 wg_frame(i32 buttons, i32 dt_ms) {
  u32 pressed = (u32)buttons & ~(u32)S.prev_buttons;
  S.prev_buttons = buttons;
  if (dt_ms > 100) dt_ms = 100;
  S.blink++;
  xorshift(&S.rng);
  keys((u32)buttons, pressed, dt_ms);
  if (S.phase != PLAY && S.end_ms > 0) {
    S.end_ms -= dt_ms;
    if (S.end_ms <= 0) S.pend_ev |= WG_EV_GAMEOVER;
  }
  i32 moving = animate(dt_ms);
  if (S.phase == PLAY) {
    if (S.placed) S.time_ms += dt_ms;
    if (S.press_on && S.press_touch && !S.long_done) {
      S.press_ms += dt_ms;
      if (S.press_ms >= 380) {
        S.long_done = 1;
        if (on_board(S.press_x, S.press_y) && S.st[S.press_y][S.press_x] != OPEN) {
          S.clicks++;
          if (S.flag_mode) reveal(S.press_x, S.press_y);
          else toggle_flag(S.press_x, S.press_y);
        }
      }
    }
  }
  ensure_fb();
  view_present(picture(), moving, draw);
  i32 ev = S.pend_ev;
  S.pend_ev = 0;
  return ev;
}

/* for the game over screen, in the catalog's `stats` order: time, 3BV,
 * 3BV solved (after a loss), 3BV/s, clicks, efficiency */
WG_EXPORT(wg_stat) i32 wg_stat(i32 i) {
  if (!S.placed) return -1;
  i32 total = bbbv(0), done = S.phase == WON ? total : bbbv(1), t = S.time_ms / 10;
  switch (i) {
    case 0: return S.time_ms;
    case 1: return total;
    case 2: return S.phase == WON ? -1 : done;
    case 3: return t ? done * 10000 / t : 0;
    case 4: return S.clicks;
    case 5: return S.clicks ? done * 100 / S.clicks : 0;
  }
  return -1;
}

WG_EXPORT(wg_state) void *wg_state(void) { return &S; }
WG_EXPORT(wg_state_size) i32 wg_state_size(void) { return sizeof S; }
WG_EXPORT(wg_score) i32 wg_score(void) { return S.score; }

/* a save is checked square by square, and everything that follows from the
 * squares is worked out again rather than trusted */
static i32 valid_save(void) {
  if (S.magic != MAGIC || S.version != VERSION || !wg_picks_ok(S.opt, CHOICES, NOPT) ||
      S.phase < PLAY || S.phase > LOST || S.time_ms < 0 || (S.placed & ~1))
    return 0;
  layout();
  i32 mines = 0, opened = 0, flags = 0;
  for (i32 y = 0; y < MAXR; y++)
    for (i32 x = 0; x < MAXC; x++) {
      i32 in = on_board(x, y);
      if (S.mine[y][x] > 1 || S.st[y][x] > MARK || (!in && (S.mine[y][x] || S.st[y][x]))) return 0;
      mines += S.mine[y][x];
      opened += S.st[y][x] == OPEN;
      flags += S.st[y][x] == FLAG;
    }
  if (mines != (S.placed ? S.mines : 0) || (!S.placed && opened)) return 0;
  if (S.phase == LOST ? !on_board(S.boom_x, S.boom_y) || !S.mine[S.boom_y][S.boom_x] : S.boom_x != -1) return 0;
  S.opened = opened;
  S.flags = flags;
  if (S.placed) place_counts();
  if (S.cur_x < 0 || S.cur_x >= S.cols) S.cur_x = 0;
  if (S.cur_y < 0 || S.cur_y >= S.rows) S.cur_y = 0;
  return 1;
}

WG_EXPORT(wg_state_loaded) i32 wg_state_loaded(void) {
  if (!valid_save()) {
    S.magic = 0;
    wg_init(0x1234);
    return 0;
  }
  S.prev_buttons = 0xFF;
  S.press_on = 0;
  S.mbtn = 0;
  S.pend_ev = 0;
  memset(rip, 0, sizeof rip);
  memset(pop, 0, sizeof pop);
  view_redraw();
  return 1;
}
