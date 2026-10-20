/* Flipside - an original wasmga.me game. MIT licensed.
 * Place a disc so it closes a line of the other colour, and the whole line
 * turns over. The colour with more discs when the board fills wins.
 * The page shows the setup, pause and game over screens (see wg.h); the
 * board draws at the screen's own size (wgview.h). */
#define WG_OWN_FB
#define WGFB_LAYERS 2 /* the screen, and the empty board */
#include "wg.h"
#include "sdf_font.h"
#include "flipside.glyphs.h"
#include "wgfb.h"
#include "wgview.h"

/* the plan everything is laid out on, scaled to the screen */
#define PW 400
#define PH 300

#define MAXN 10
#define MAGIC 0x464C5031u /* "FLP1" */
#define VERSION 5
#define END_MS 1500 /* the final board stays in view this long before game over */
#define FLIP_MS 300
#define POP_MS 160
#define MAXHIST 100

enum { PLAY, OVER };
enum { EMPTY, DARK, LIGHT };
/* options, in catalog order */
enum { O_FOE, O_SIDE, O_SIZE, O_THEME, O_HINTS, NOPT };
#define THEMES 4
static const i32 CHOICES[NOPT] = {4, 2, 3, THEMES, 2};
enum { EASY, NORMAL, HARD, FRIEND };

/* the computer's name per level, in each language (en zh ja ko es pt fr de ru) */
static const char *const FOES[4][9] = {
  {"Easy", "简单", "かんたん", "쉬움", "Fácil", "Fácil", "Facile", "Leicht", "Лёгкий"},
  {"Normal", "普通", "ふつう", "보통", "Normal", "Normal", "Normal", "Normal", "Обычный"},
  {"Hard", "困难", "むずかしい", "어려움", "Difícil", "Difícil", "Difficile", "Schwer", "Сложный"},
  {"A friend", "朋友", "友だち", "친구", "Un amigo", "Um amigo", "Un ami", "Ein Freund", "Друг"},
};
static const i32 SIZES[3] = {6, 8, 10};
static const i32 MULT[3] = {1, 2, 4};

/* the board before each move, who made it, and how many discs it turned */
typedef struct { u8 b[MAXN * MAXN]; u8 turn, last, turned, pad; } Snap;

static struct {
  u32 magic, version, rng;
  i32 phase, end_ms;
  i32 opt[NOPT];
  i32 n, cell, bx, by;
  i32 turn, last, cur_x, cur_y, keyboard, used_undo;
  i32 wait_ms, pass_ms, pass_who, result, score;
  i32 mx, my, mbtn;
  i32 pend_ev, prev_buttons, rep_ms, blink, pop_ms;
  i32 hist_n, time_ms;
  u8 b[MAXN * MAXN];
  u8 anim[MAXN * MAXN]; /* flip time left, in 10 ms steps */
  Snap hist[MAXHIST];
} S;

typedef struct {
  const char *name;
  u32 bg, panel, text, dim, accent, board, grid, edge, dark, dark_hi, light, light_lo, hint, cursor;
} Theme;

static const Theme THEME[THEMES] = {
  {"WALNUT", RGB(38, 28, 22), RGB(56, 42, 33), RGB(250, 238, 220), RGB(176, 150, 128), RGB(255, 190, 92),
   RGB(150, 98, 60), RGB(112, 70, 40), RGB(84, 52, 30), RGB(36, 28, 26), RGB(78, 66, 60), RGB(246, 236, 214), RGB(206, 190, 160),
   RGB(196, 140, 96), RGB(255, 214, 120)},
  {"FELT", RGB(18, 34, 28), RGB(28, 50, 41), RGB(236, 246, 238), RGB(140, 176, 156), RGB(120, 220, 160),
   RGB(30, 110, 72), RGB(20, 80, 52), RGB(14, 60, 38), RGB(22, 24, 26), RGB(70, 76, 80), RGB(244, 246, 240), RGB(196, 204, 196),
   RGB(60, 140, 100), RGB(250, 230, 120)},
  {"SLATE", RGB(23, 20, 46), RGB(40, 34, 78), RGB(255, 247, 232), RGB(150, 140, 190), RGB(46, 196, 182),
   RGB(84, 66, 210), RGB(62, 48, 170), RGB(44, 34, 130), RGB(30, 24, 60), RGB(80, 70, 130), RGB(255, 201, 60), RGB(214, 158, 30),
   RGB(110, 94, 230), RGB(46, 196, 182)},
  {"PAPER", RGB(243, 234, 216), RGB(228, 214, 187), RGB(42, 33, 24), RGB(120, 104, 84), RGB(101, 79, 240),
   RGB(226, 208, 172), RGB(176, 156, 118), RGB(150, 128, 92), RGB(42, 33, 24), RGB(96, 84, 70), RGB(255, 252, 244), RGB(214, 202, 182),
   RGB(196, 174, 132), RGB(101, 79, 240)},
};
#define T (THEME[S.opt[O_THEME]])

/* Online play (wg_net below): the side this screen plays, 0 when offline, and
 * the cell of its last move for the page to send. Kept outside the state, so
 * a save of an online game loads as a game between friends on one screen. */
static i32 net_side, net_out = -1;

static const i32 DX[8] = {1, 1, 0, -1, -1, -1, 0, 1}, DY[8] = {0, 1, 1, 1, 0, -1, -1, -1};

/* ---------- rules ---------- */

static i32 flips_dir(const u8 *b, i32 n, i32 x, i32 y, i32 d, i32 me) {
  i32 them = 3 - me, k = 0;
  x += DX[d];
  y += DY[d];
  while (x >= 0 && y >= 0 && x < n && y < n && b[y * MAXN + x] == them) { x += DX[d]; y += DY[d]; k++; }
  if (k && x >= 0 && y >= 0 && x < n && y < n && b[y * MAXN + x] == me) return k;
  return 0;
}

static i32 legal(const u8 *b, i32 n, i32 x, i32 y, i32 me) {
  if (b[y * MAXN + x]) return 0;
  i32 t = 0;
  for (i32 d = 0; d < 8; d++) t += flips_dir(b, n, x, y, d, me);
  return t;
}

static i32 count_moves(const u8 *b, i32 n, i32 me) {
  i32 m = 0;
  for (i32 y = 0; y < n; y++)
    for (i32 x = 0; x < n; x++)
      if (legal(b, n, x, y, me)) m++;
  return m;
}

/* places a disc; anim marks the turned discs for the flip animation */
static void apply(u8 *b, i32 n, i32 x, i32 y, i32 me, u8 *anim) {
  b[y * MAXN + x] = (u8)me;
  for (i32 d = 0; d < 8; d++) {
    i32 k = flips_dir(b, n, x, y, d, me), cx = x, cy = y;
    while (k--) {
      cx += DX[d];
      cy += DY[d];
      b[cy * MAXN + cx] = (u8)me;
      if (anim) anim[cy * MAXN + cx] = FLIP_MS / 10;
    }
  }
}

static void counts(const u8 *b, i32 n, i32 *dark, i32 *light) {
  *dark = *light = 0;
  for (i32 y = 0; y < n; y++)
    for (i32 x = 0; x < n; x++) {
      if (b[y * MAXN + x] == DARK) (*dark)++;
      else if (b[y * MAXN + x] == LIGHT) (*light)++;
    }
}

/* ---------- computer player ---------- */

static i32 weight(i32 n, i32 x, i32 y) {
  i32 ex = x == 0 || x == n - 1, ey = y == 0 || y == n - 1;
  i32 nx = x == 1 || x == n - 2, ny = y == 1 || y == n - 2;
  if (ex && ey) return 120;
  if (nx && ny) return -50;               /* diagonal to a corner */
  if ((ex && ny) || (ey && nx)) return -22; /* edge square next to a corner */
  if (ex || ey) return 12;
  if (nx || ny) return -4;
  return 2;
}

/* The search stops after this many positions, so a move never holds up a
 * frame for long; it then plays the best move of the last depth it finished. */
#define NODE_BUDGET 6000

static i32 nodes, aborted;

static i32 evaluate(const u8 *b, i32 n, i32 me, i32 mobility) {
  i32 s = 0;
  for (i32 y = 0; y < n; y++)
    for (i32 x = 0; x < n; x++) {
      i32 v = b[y * MAXN + x];
      if (v) s += v == me ? weight(n, x, y) : -weight(n, x, y);
    }
  if (mobility) s += (count_moves(b, n, me) - count_moves(b, n, 3 - me)) * 6;
  return s;
}

/* legal squares, best-looking first, which lets alpha-beta cut far more */
static i32 moves(const u8 *b, i32 n, i32 me, u8 *out) {
  i32 m = 0;
  i32 key[MAXN * MAXN];
  for (i32 y = 0; y < n; y++)
    for (i32 x = 0; x < n; x++) {
      if (!legal(b, n, x, y, me)) continue;
      i32 k = weight(n, x, y), j = m++;
      while (j && key[j - 1] < k) { key[j] = key[j - 1]; out[j] = out[j - 1]; j--; }
      key[j] = k;
      out[j] = (u8)(y * MAXN + x);
    }
  return m;
}

static i32 search(const u8 *b, i32 n, i32 me, i32 depth, i32 alpha, i32 beta, i32 passed, i32 exact) {
  if (++nodes > NODE_BUDGET) { aborted = 1; return 0; }
  if (depth == 0) return exact ? 0 : evaluate(b, n, me, S.opt[O_FOE] == HARD);
  u8 nb[MAXN * MAXN], list[MAXN * MAXN];
  i32 m = moves(b, n, me, list);
  if (!m) {
    if (passed) {
      i32 d, l;
      counts(b, n, &d, &l);
      return (me == DARK ? d - l : l - d) * 1000;
    }
    return -search(b, n, 3 - me, depth, -beta, -alpha, 1, exact);
  }
  for (i32 i = 0; i < m && !aborted; i++) {
    memcpy(nb, b, sizeof nb);
    apply(nb, n, list[i] % MAXN, list[i] / MAXN, me, 0);
    i32 v = -search(nb, n, 3 - me, depth - 1, -beta, -alpha, 0, exact);
    if (v > alpha) alpha = v;
    if (alpha >= beta) break;
  }
  return alpha;
}

static i32 empties(void) {
  i32 e = 0;
  for (i32 y = 0; y < S.n; y++)
    for (i32 x = 0; x < S.n; x++)
      if (!S.b[y * MAXN + x]) e++;
  return e;
}

/* one full search of every root move to this depth; -1 if it ran out of budget */
static i32 root(i32 depth, i32 exact, u8 *list, i32 m) {
  i32 me = S.turn, n = S.n, best = -0x7FFFFFFF, pick = -1;
  u8 nb[MAXN * MAXN];
  for (i32 i = 0; i < m; i++) {
    i32 k = list[i], x = k % MAXN, y = k / MAXN, f = legal(S.b, n, x, y, me);
    memcpy(nb, S.b, sizeof nb);
    apply(nb, n, x, y, me, 0);
    i32 v = -search(nb, n, 3 - me, depth - 1, -0x7FFFFFF, 0x7FFFFFF, 0, exact);
    if (aborted) return -1;
    /* an exact search only scores the final count; prefer it, then position */
    if (exact) v = v * 4 + weight(n, x, y);
    if (S.opt[O_FOE] == EASY) v = v + f * 4 + (i32)(xorshift(&S.rng) % 160);
    else v = v * 8 + (i32)(xorshift(&S.rng) % 8);
    if (v > best) { best = v; pick = i; }
  }
  return pick;
}

static i32 choose_move(void) {
  u8 list[MAXN * MAXN];
  i32 m = moves(S.b, S.n, S.turn, list);
  if (!m) return -1;
  i32 most = S.opt[O_FOE] == EASY ? 1 : S.opt[O_FOE] == NORMAL ? 3 : 8, pick = 0;
  nodes = aborted = 0;
  if (S.opt[O_FOE] == HARD && empties() <= 12) {
    i32 p = root(64, 1, list, m);
    if (p >= 0) return list[p];
    nodes = aborted = 0;
  }
  for (i32 d = 1; d <= most; d++) {
    i32 p = root(d, 0, list, m);
    if (p < 0) break;
    pick = p;
    /* the best move so far is searched first next time */
    u8 t = list[p];
    for (i32 i = p; i > 0; i--) list[i] = list[i - 1];
    list[0] = t;
    pick = 0;
  }
  return list[pick];
}

/* ---------- game flow ---------- */

static i32 human(i32 who) {
  if (net_side) return who == net_side;
  return S.opt[O_FOE] == FRIEND || who == (S.opt[O_SIDE] ? LIGHT : DARK);
}

/* board size and where it sits, from the options */
static void layout(void) {
  S.n = SIZES[S.opt[O_SIZE]];
  S.cell = 272 / S.n;
  S.bx = (300 - S.cell * S.n) / 2;
  S.by = (PH - S.cell * S.n) / 2;
}

static void setup_board(void) {
  layout();
  memset(S.b, 0, sizeof S.b);
  memset(S.anim, 0, sizeof S.anim);
  i32 m = S.n / 2;
  S.b[(m - 1) * MAXN + m - 1] = LIGHT;
  S.b[m * MAXN + m] = LIGHT;
  S.b[(m - 1) * MAXN + m] = DARK;
  S.b[m * MAXN + m - 1] = DARK;
  S.turn = DARK;
  S.last = -1;
  S.cur_x = m - 1;
  S.cur_y = m - 2;
  S.hist_n = 0;
  S.used_undo = 0;
  S.wait_ms = 500;
  S.pass_ms = 0;
  S.result = 0;
  S.score = 0;
  S.phase = PLAY;
}

static void finish(void) {
  i32 d, l;
  counts(S.b, S.n, &d, &l);
  S.phase = OVER;
  S.end_ms = END_MS;
  S.result = d > l ? DARK : l > d ? LIGHT : 3;
  if (net_side) {
    if (S.result == net_side) S.pend_ev |= WG_EV_WIN;
    else if (S.result != 3) S.pend_ev |= WG_EV_LOSE;
    return;
  }
  if (S.opt[O_FOE] == FRIEND) {
    // a draw is a finish, not a win for either screen's sound
    if (S.result != 3) S.pend_ev |= WG_EV_WIN;
    return;
  }
  i32 me = S.opt[O_SIDE] ? LIGHT : DARK;
  if (S.result == me) {
    i32 diff = me == DARK ? d - l : l - d;
    S.score = S.used_undo ? 0 : (1000 + diff * 50) * MULT[S.opt[O_FOE]];
    S.pend_ev |= WG_EV_WIN;
  } else {
    /* a loss or a draw scores the discs you kept, always below any win */
    S.score = S.used_undo ? 0 : (me == DARK ? d : l) * MULT[S.opt[O_FOE]];
    if (S.result != 3) S.pend_ev |= WG_EV_LOSE;
  }
}

/* hands the turn over, passing when the next player has no move */
static void next_turn(void) {
  i32 other = 3 - S.turn;
  if (count_moves(S.b, S.n, other)) { S.turn = other; return; }
  if (count_moves(S.b, S.n, S.turn)) {
    S.pass_ms = 1300;
    S.pass_who = other;
    return;
  }
  finish();
}

static void push_history(void) {
  if (S.hist_n == MAXHIST) {
    memcpy(&S.hist[0], &S.hist[1], sizeof(Snap) * (MAXHIST - 1));
    S.hist_n--;
  }
  Snap *h = &S.hist[S.hist_n++];
  memcpy(h->b, S.b, sizeof h->b);
  h->turn = (u8)S.turn;
  h->last = (u8)(S.last < 0 ? 255 : S.last);
}

static void play_at(i32 x, i32 y) {
  if (!legal(S.b, S.n, x, y, S.turn)) return;
  if (net_side && S.turn == net_side) net_out = y * MAXN + x;
  push_history();
  S.hist[S.hist_n - 1].turned = (u8)legal(S.b, S.n, x, y, S.turn);
  apply(S.b, S.n, x, y, S.turn, S.anim);
  S.last = y * MAXN + x;
  S.pop_ms = POP_MS;
  S.pend_ev |= WG_EV_HIT;
  S.wait_ms = 450;
  next_turn();
}

/* goes back to the last position where a person was to move */
static void undo(void) {
  if (!S.hist_n || S.phase != PLAY || net_side) return;
  do {
    Snap *h = &S.hist[--S.hist_n];
    memcpy(S.b, h->b, sizeof S.b);
    S.turn = h->turn;
    S.last = h->last == 255 ? -1 : h->last;
  } while (S.hist_n && !human(S.turn));
  memset(S.anim, 0, sizeof S.anim);
  S.pass_ms = 0;
  S.wait_ms = 500;
  if (S.opt[O_FOE] != FRIEND) S.used_undo = 1;
  S.pend_ev |= WG_EV_BLIP;
}

/* ---------- layout ---------- */

#define SIDE_X 306
#define SIDE_W 86
#define BTN_H 30
#define BTN1_Y 252

static i32 side_button_at(i32 x, i32 y) { return inside(x, y, SIDE_X, BTN1_Y, SIDE_W, BTN_H) ? 0 : -1; }

/* ---------- drawing ---------- */

static float ease(float t) { t = clampf(t, 0, 1); return t * t * (3 - 2 * t); }

/* a disc seen from above; squeeze narrows it as it turns over */
static void draw_disc(float cx, float cy, float r, i32 who, float squeeze, float a) {
  u32 face = who == DARK ? T.dark : T.light, rim = who == DARK ? T.dark_hi : T.light_lo;
  float rx = r * squeeze;
  v_ellipse(cx, cy + r * 0.14f, rx * 1.02f, r * 1.02f, RGB(0, 0, 0), 0.3f * a);
  v_ellipse(cx, cy, rx, r, rim, a);
  v_ellipse(cx, cy - r * 0.07f, rx * 0.9f, r * 0.86f, face, a);
  v_ellipse(cx - rx * 0.22f, cy - r * 0.38f, rx * 0.46f, r * 0.24f, RGB(255, 255, 255), (who == DARK ? 0.16f : 0.45f) * a);
}

/* the empty board, kept in layer 1 */
static void draw_static(void) {
  i32 n = S.n, s = S.cell, size = s * n;
  float bx = (float)S.bx, by = (float)S.by, line = maxf(0.8f, hair());
  clear(T.bg);
  v_shadow(bx - 9, by - 5, size + 18, size + 18, 12, 9, RGB(0, 0, 0), 0.45f);
  v_box_grad(bx - 9, by - 9, size + 18, size + 18, 12, mixc(T.edge, T.board, 0.5f), T.edge, 1);
  v_box_grad(bx, by, (float)size, (float)size, 3, mixc(RGB(255, 255, 255), T.board, 0.06f), mixc(RGB(0, 0, 0), T.board, 0.08f), 1);
  for (i32 i = 1; i < n; i++) {
    v_box(bx + i * s - line / 2, by, line, (float)size, 0, T.grid, 1);
    v_box(bx, by + i * s - line / 2, (float)size, line, 0, T.grid, 1);
  }
  /* star points, as on many boards */
  if (n >= 8) {
    i32 q = n == 8 ? 2 : 3;
    for (i32 j = 0; j < 2; j++)
      for (i32 i = 0; i < 2; i++)
        v_disc(bx + (i ? n - q : q) * s, by + (j ? n - q : q) * s, 2.2f, T.grid, 1);
  }
}

static void draw_board(void) {
  i32 n = S.n, s = S.cell, size = s * n;
  float bx = (float)S.bx, by = (float)S.by;
  i32 hover = -1;
  if (!S.keyboard && inside(S.mx, S.my, S.bx, S.by, size, size))
    hover = ((S.my - S.by) / s) * MAXN + (S.mx - S.bx) / s;
  if (S.keyboard) hover = S.cur_y * MAXN + S.cur_x;
  i32 my_turn = S.phase == PLAY && human(S.turn) && !S.pass_ms;
  for (i32 y = 0; y < n; y++)
    for (i32 x = 0; x < n; x++) {
      i32 k = y * MAXN + x, v = S.b[k];
      float cx = bx + x * s + s / 2.0f, cy = by + y * s + s / 2.0f, r = s * 0.4f;
      if (v) {
        i32 t = S.anim[k] * 10;
        if (t) {
          /* turning over: squeeze to an edge, then open as the new colour */
          float p = ease((float)(FLIP_MS - t) / FLIP_MS), lift = 1 - absf(p * 2 - 1);
          draw_disc(cx, cy - lift * s * 0.08f, r * (1 + lift * 0.08f), p < 0.5f ? 3 - v : v, maxf(absf(p * 2 - 1), 0.04f), 1);
        } else if (k == S.last && S.pop_ms > 0) {
          float p = ease(1 - (float)S.pop_ms / POP_MS);
          draw_disc(cx, cy, r * (1.18f - 0.18f * p), v, 1, 0.4f + 0.6f * p);
        } else {
          draw_disc(cx, cy, r, v, 1, 1);
        }
        /* the last move gets a ring round it, which reads as a marker, not a place to play */
        if (k == S.last) v_ring(cx, cy, s * 0.49f, maxf(s * 0.05f, 1.2f), T.accent, 0.95f);
      } else if (my_turn && legal(S.b, n, x, y, S.turn)) {
        if (k == hover) draw_disc(cx, cy, r, S.turn, 1, 0.5f);
        else if (!S.opt[O_HINTS]) v_disc(cx, cy, s * 0.1f, T.hint, 0.95f); /* choice 0 is On */
      }
    }
  if (S.keyboard && S.phase == PLAY && human(S.turn)) {
    float x0 = bx + S.cur_x * s + 1.5f, y0 = by + S.cur_y * s + 1.5f, x1 = x0 + s - 3, y1 = y0 + s - 3, w = 1.3f;
    v_line(x0, y0, x1, y0, w, T.cursor, 1);
    v_line(x1, y0, x1, y1, w, T.cursor, 1);
    v_line(x1, y1, x0, y1, w, T.cursor, 1);
    v_line(x0, y1, x0, y0, w, T.cursor, 1);
  }
}

/* a size at which s fits in w */
static float fit(const char *s, float size, float w) {
  float t = v_text_w(s, size);
  return t > w ? size * w / t : size;
}

static const char *name_of(i32 who) {
  if (net_side) return who == net_side ? tr("You", "你", "あなた", "나", "Tú", "Você", "Vous", "Du", "Вы") : tr("Friend", "朋友", "友だち", "친구", "Amigo", "Amigo", "Ami", "Freund", "Друг");
  if (S.opt[O_FOE] == FRIEND) return who == DARK ? tr("Dark", "黑棋", "黒", "흑", "Negras", "Pretas", "Noirs", "Schwarz", "Чёрные") : tr("Light", "白棋", "白", "백", "Blancas", "Brancas", "Blancs", "Weiß", "Белые");
  return human(who) ? tr("You", "你", "あなた", "나", "Tú", "Você", "Vous", "Du", "Вы") : wg_tr(FOES[S.opt[O_FOE]], 9);
}

static i32 thinking(void) { return S.phase == PLAY && !human(S.turn) && !S.pass_ms; }

static void draw_side(void) {
  char buf[12];
  i32 d, l;
  counts(S.b, S.n, &d, &l);
  for (i32 i = 0; i < 2; i++) {
    i32 who = i ? LIGHT : DARK;
    float y = 12 + i * 66.0f;
    i32 active = S.phase == PLAY && S.turn == who && !S.pass_ms;
    i32 won = S.phase == OVER && S.result == who;
    if (active || won) {
      v_box(SIDE_X - 4, y - 4, SIDE_W + 8, 58, 10, T.panel, 1);
      v_box(SIDE_X - 4, y + 12, 3, 26, 1.5f, T.accent, 1);
    }
    draw_disc(SIDE_X + 15, y + 15, 12, who, 1, 1);
    v_text(itoa10(who == DARK ? d : l, buf), SIDE_X + 35, y + 15, 22, T.text, 1);
    const char *name = name_of(who);
    v_text(name, SIDE_X + 4, y + 41, fit(name, 12, SIDE_W - 4), active || won ? T.accent : T.dim, 1);
  }
  float mid = SIDE_X + SIDE_W / 2.0f;
  if (S.phase == OVER) {
    const char *msg = S.result == 3 ? tr("Draw", "平局", "引き分け", "무승부", "Empate", "Empate", "Égalité", "Remis", "Ничья") : S.opt[O_FOE] == FRIEND && !net_side ? (S.result == DARK ? tr("Dark wins", "黑棋胜", "黒の勝ち", "흑 승리", "Ganan negras", "Pretas vencem", "Les noirs gagnent", "Schwarz gewinnt", "Победа чёрных") : tr("Light wins", "白棋胜", "白の勝ち", "백 승리", "Ganan blancas", "Brancas vencem", "Les blancs gagnent", "Weiß gewinnt", "Победа белых"))
                    : human(S.result) ? tr("You win", "你赢了", "あなたの勝ち", "이겼습니다", "Ganaste", "Você venceu", "Vous gagnez", "Gewonnen", "Победа") : tr("You lose", "你输了", "あなたの負け", "졌습니다", "Perdiste", "Você perdeu", "Vous perdez", "Verloren", "Поражение");
    v_text_c(msg, mid, 160, fit(msg, 15, SIDE_W + 8), T.accent, 1);
    return;
  }
  if (thinking()) {
    /* three dots that rise in turn */
    for (i32 i = 0; i < 3; i++) {
      float ph = sinf_(S.blink * 0.18f - i * 0.9f);
      v_disc(mid - 12 + i * 12, 160 - maxf(ph, 0) * 4, 3, T.dim, 0.55f + 0.45f * maxf(ph, 0));
    }
  }
  if (net_side) return; /* no undo against a friend elsewhere */
  i32 hot = !S.keyboard && side_button_at(S.mx, S.my) == 0 && S.hist_n;
  float a = S.hist_n ? 1 : 0.4f;
  v_box(SIDE_X, BTN1_Y, SIDE_W, BTN_H, 9, hot ? T.accent : T.panel, a);
  const char *undo = tr("Undo", "撤销", "元に戻す", "되돌리기", "Deshacer", "Desfazer", "Annuler", "Rückgängig", "Отмена");
  v_text_c(undo, mid, BTN1_Y + BTN_H / 2.0f, fit(undo, 14, SIDE_W - 8), hot ? T.bg : T.text, a);
}

static void draw(void) {
  view_background(hash_int(S.opt[O_THEME] + 1, S.n), draw_static);
  draw_board();
  draw_side();
  if (S.pass_ms) {
    const char *who = net_side && !human(S.pass_who) ? tr("No move for your friend", "朋友无处可下", "友だちは打つ手がありません", "친구는 둘 곳이 없습니다", "Tu amigo no puede mover", "Seu amigo não tem jogada", "Votre ami ne peut pas jouer", "Dein Freund kann nicht ziehen", "Другу некуда ходить")
                    : !net_side && S.opt[O_FOE] == FRIEND ? (S.pass_who == DARK ? tr("No move for dark", "黑棋无处可下", "黒は打つ手がありません", "흑은 둘 곳이 없습니다", "Las negras no pueden mover", "As pretas não têm jogada", "Les noirs ne peuvent pas jouer", "Schwarz kann nicht ziehen", "Чёрным некуда ходить") : tr("No move for light", "白棋无处可下", "白は打つ手がありません", "백은 둘 곳이 없습니다", "Las blancas no pueden mover", "As brancas não têm jogada", "Les blancs ne peuvent pas jouer", "Weiß kann nicht ziehen", "Белым некуда ходить"))
                    : human(S.pass_who) ? tr("No move for you", "你无处可下", "打つ手がありません", "둘 곳이 없습니다", "No puedes mover", "Você não tem jogada", "Vous ne pouvez pas jouer", "Du kannst nicht ziehen", "Вам некуда ходить") : tr("No move for the computer", "电脑无处可下", "コンピューターは打つ手がありません", "컴퓨터는 둘 곳이 없습니다", "La computadora no puede mover", "O computador não tem jogada", "L'ordinateur ne peut pas jouer", "Der Computer kann nicht ziehen", "Компьютеру некуда ходить");
    float w = v_text_w(who, 13) + 28, size = (float)(S.cell * S.n);
    float x = S.bx + (size - w) / 2, y = S.by + size / 2 - 15;
    v_shadow(x, y + 3, w, 30, 10, 8, RGB(0, 0, 0), 0.4f);
    v_box(x, y, w, 30, 10, T.panel, 0.97f);
    v_text_c(who, x + w / 2, y + 15, 13, T.text, 1);
  }
}

/* what the picture depends on, so a still board isn't repainted every frame */
static u32 picture(void) {
  u32 h = hash_bytes(0, S.b, sizeof S.b);
  i32 v[] = {S.turn, S.last, S.phase, S.mx, S.my, S.keyboard, S.cur_x, S.cur_y, S.pass_ms > 0, S.pass_who,
             S.result, S.hist_n > 0, S.opt[O_THEME], S.opt[O_HINTS], S.n};
  return hash_bytes(h, v, sizeof v);
}

static i32 moving(void) {
  if (S.pop_ms > 0 || thinking()) return 1;
  for (i32 k = 0; k < MAXN * MAXN; k++)
    if (S.anim[k]) return 1;
  return 0;
}

static void fb_resized(void) {
  view_fit(PW, PH);
  view_redraw();
}

WG_EXPORT(wg_resize) void wg_resize(i32 w, i32 h) {
  if (fb_size(w, h, WGFB_LAYERS)) fb_resized();
}

/* ---------- input ---------- */

static i32 board_cell(i32 x, i32 y) {
  if (!S.cell || !inside(x, y, S.bx, S.by, S.cell * S.n, S.cell * S.n)) return -1;
  return ((y - S.by) / S.cell) * MAXN + (x - S.bx) / S.cell;
}

static void pointer_release(i32 x, i32 y) {
  if (S.phase != PLAY) return;
  if (side_button_at(x, y) == 0) { undo(); return; }
  i32 k = board_cell(x, y);
  if (k >= 0 && human(S.turn) && !S.pass_ms) {
    S.cur_x = k % MAXN;
    S.cur_y = k / MAXN;
    play_at(S.cur_x, S.cur_y);
  }
}

WG_EXPORT(wg_mouse) void wg_mouse(i32 x, i32 y, i32 b) {
  i32 held = b & 7, released = S.mbtn & ~held;
  S.mbtn = held;
  if (x < 0) { S.mx = S.my = -1; return; }
  x = plan_x(x);
  y = plan_y(y);
  if (x != S.mx || y != S.my) S.keyboard = 0;
  S.mx = x;
  S.my = y;
  if (released & M_LEFT) pointer_release(x, y);
}

static void keys(u32 buttons, u32 pressed, i32 dt) {
  u32 dirs = key_repeat(buttons, pressed, dt, &S.rep_ms);
  if (pressed) S.keyboard = 1;
  if (S.phase != PLAY) return;
    if (pressed & B_B) undo();
    if (dirs & B_LEFT) S.cur_x = (S.cur_x + S.n - 1) % S.n;
    if (dirs & B_RIGHT) S.cur_x = (S.cur_x + 1) % S.n;
    if (dirs & B_UP) S.cur_y = (S.cur_y + S.n - 1) % S.n;
    if (dirs & B_DOWN) S.cur_y = (S.cur_y + 1) % S.n;
    if (pressed & B_A && human(S.turn) && !S.pass_ms) play_at(S.cur_x, S.cur_y);
}

WG_EXPORT(wg_init) void wg_init(u32 seed) {
  static const i32 DEFAULTS[NOPT] = {NORMAL, 0, 1, 0, 0};
  i32 opt[NOPT];
  for (i32 i = 0; i < NOPT; i++) opt[i] = S.magic == MAGIC && S.version == VERSION ? S.opt[i] : DEFAULTS[i];
  memset(&S, 0, sizeof S);
  S.magic = MAGIC;
  S.version = VERSION;
  S.rng = seed | 1;
  memcpy(S.opt, opt, sizeof opt);
  S.mx = S.my = -1;
  net_side = 0;
  net_out = -1;
  setup_board();
}

WG_EXPORT(wg_option) i32 wg_option(i32 i, i32 v) {
  if (S.magic != MAGIC) wg_init(1);
  return wg_pick(S.opt, CHOICES, NOPT, i, v);
}

WG_EXPORT(wg_over) i32 wg_over(void) { return S.phase == OVER; }

/* Online play (src/scripts/online.ts, and the room in worker/src/referee.ts,
 * which runs this module too): the wg_net exports below are what a game needs
 * to be played on two screens (catalog `online`).
 * wg_net(side) starts a run in which this
 * screen plays side (1 dark, 2 light) and a friend elsewhere the other, on the
 * board picked with wg_option; wg_net(0) only reads the side (0 offline, which
 * any wg_init or loaded save goes back to). */
WG_EXPORT(wg_net) i32 wg_net(i32 side) {
  if (side == DARK || side == LIGHT) {
    S.opt[O_FOE] = FRIEND;
    S.opt[O_SIDE] = side == LIGHT;
    wg_init(S.rng);
    net_side = side;
  }
  return net_side;
}

/* A move made on either screen, by side, as the cell y * 10 + x: 0 if it
 * wasn't that side's turn or a legal move (the screens are out of step), else
 * 2 if this screen is to move next, 1 if not. */
WG_EXPORT(wg_net_move) i32 wg_net_move(i32 k, i32 side) {
  if (!net_side || S.phase != PLAY || side != S.turn || k < 0 || k >= MAXN * MAXN) return 0;
  i32 x = k % MAXN, y = k / MAXN;
  if (x >= S.n || y >= S.n || !legal(S.b, S.n, x, y, side)) return 0;
  S.pass_ms = 0;
  play_at(x, y);
  net_out = -1;
  return S.phase == PLAY && S.turn == net_side ? 2 : 1;
}

/* Side gives up (or left the room): the other side wins. 1 if the run was on. */
WG_EXPORT(wg_net_resign) i32 wg_net_resign(i32 side) {
  if (!net_side || S.phase != PLAY || (side != DARK && side != LIGHT)) return 0;
  S.pass_ms = 0;
  S.phase = OVER;
  S.end_ms = END_MS;
  S.result = 3 - side;
  S.pend_ev |= S.result == net_side ? WG_EV_WIN : WG_EV_LOSE;
  return 1;
}

/* How the online run ended for this screen: 1 won, 2 lost, 3 a draw, 0 not yet. */
WG_EXPORT(wg_net_result) i32 wg_net_result(void) {
  if (!net_side || S.phase != OVER) return 0;
  return S.result == 3 ? 3 : S.result == net_side ? 1 : 2;
}

/* The side to move in the online run (1 or 2), or 0 once it's over. */
WG_EXPORT(wg_net_turn) i32 wg_net_turn(void) { return net_side && S.phase == PLAY ? S.turn : 0; }

/* The cell of this screen's latest move, once, or -1. */
WG_EXPORT(wg_net_out) i32 wg_net_out(void) {
  i32 k = net_out;
  net_out = -1;
  return k;
}

WG_EXPORT(wg_frame) i32 wg_frame(i32 buttons, i32 dt_ms) {
  u32 pressed = (u32)buttons & ~(u32)S.prev_buttons;
  S.prev_buttons = buttons;
  if (dt_ms > 100) dt_ms = 100;
  S.blink++;
  xorshift(&S.rng);
  keys((u32)buttons, pressed, dt_ms);
  if (S.phase == OVER && S.end_ms > 0) {
    S.end_ms -= dt_ms;
    if (S.end_ms <= 0) S.pend_ev |= WG_EV_GAMEOVER;
  }
  if (S.pop_ms > 0) S.pop_ms -= dt_ms;
  if (S.phase == PLAY) S.time_ms += dt_ms;
  if (S.phase == PLAY) {
    i32 busy = 0;
    for (i32 k = 0; k < MAXN * MAXN; k++)
      if (S.anim[k]) {
        i32 t = S.anim[k] * 10 - dt_ms;
        S.anim[k] = (u8)(t > 0 ? (t + 9) / 10 : 0);
        busy = 1;
      }
    if (S.pass_ms) {
      S.pass_ms -= dt_ms;
      if (S.pass_ms <= 0) { S.pass_ms = 0; S.wait_ms = 300; }
    } else if (!human(S.turn) && !busy && !net_side) {
      S.wait_ms -= dt_ms;
      if (S.wait_ms <= 0) {
        i32 k = choose_move();
        if (k >= 0) play_at(k % MAXN, k / MAXN);
        else next_turn();
      }
    }
  }
  ensure_fb();
  view_present(picture(), moving(), draw);
  i32 ev = S.pend_ev;
  S.pend_ev = 0;
  return ev;
}

/* for the game over screen, in the catalog's `stats` order: dark and light
 * discs, dark and light corners, moves, most turned in one move, time */
WG_EXPORT(wg_stat) i32 wg_stat(i32 i) {
  i32 discs[3] = {0}, corners[3] = {0}, n = S.n, most = 0;
  for (i32 y = 0; y < n; y++)
    for (i32 x = 0; x < n; x++) {
      i32 v = S.b[y * MAXN + x];
      if (!v) continue;
      discs[v]++;
      if ((x == 0 || x == n - 1) && (y == 0 || y == n - 1)) corners[v]++;
    }
  for (i32 k = 0; k < S.hist_n; k++)
    if (S.hist[k].turned > most) most = S.hist[k].turned;
  switch (i) {
    case 0: return discs[DARK];
    case 1: return discs[LIGHT];
    case 2: return corners[DARK];
    case 3: return corners[LIGHT];
    case 4: return S.hist_n;
    case 5: return most;
    case 6: return S.time_ms;
  }
  return -1;
}

WG_EXPORT(wg_state) void *wg_state(void) { return &S; }
WG_EXPORT(wg_state_size) i32 wg_state_size(void) { return sizeof S; }
WG_EXPORT(wg_score) i32 wg_score(void) { return S.score; }

/* a board holds only discs inside its size */
static i32 valid_board(const u8 *b) {
  for (i32 y = 0; y < MAXN; y++)
    for (i32 x = 0; x < MAXN; x++) {
      u8 v = b[y * MAXN + x];
      if (v > LIGHT || (v && (x >= S.n || y >= S.n))) return 0;
    }
  return 1;
}

static i32 valid_save(void) {
  if (S.magic != MAGIC || S.version != VERSION || !wg_picks_ok(S.opt, CHOICES, NOPT) ||
      S.phase < PLAY || S.phase > OVER ||
      S.hist_n < 0 || S.hist_n > MAXHIST || (S.turn != DARK && S.turn != LIGHT) ||
      S.pass_who < 0 || S.pass_who > LIGHT || S.result < 0 || S.result > 3)
    return 0;
  layout();
  if (!valid_board(S.b)) return 0;
  for (i32 i = 0; i < S.hist_n; i++) {
    Snap *h = &S.hist[i];
    if (!valid_board(h->b) || (h->turn != DARK && h->turn != LIGHT) || (h->last != 255 && h->last >= MAXN * MAXN)) return 0;
  }
  if (S.last < -1 || S.last >= MAXN * MAXN) S.last = -1;
  if (S.cur_x < 0 || S.cur_x >= S.n) S.cur_x = 0;
  if (S.cur_y < 0 || S.cur_y >= S.n) S.cur_y = 0;
  memset(S.anim, 0, sizeof S.anim);
  S.pop_ms = 0;
  return 1;
}

WG_EXPORT(wg_state_loaded) i32 wg_state_loaded(void) {
  if (!valid_save()) {
    S.magic = 0;
    wg_init(0x1234);
    return 0;
  }
  net_side = 0;
  S.prev_buttons = 0xFF;
  S.mbtn = 0;
  S.pend_ev = 0;
  view_redraw();
  return 1;
}
