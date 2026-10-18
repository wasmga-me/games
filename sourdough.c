/* Sourdough Solitaire - an original wasmga.me game. MIT licensed.
 * Build the four suits up from ace to king. On the table, cards go down in
 * alternating colours, and only a king fills an empty column.
 * The page shows the setup, pause and game over screens (see wg.h); the
 * table draws at the screen's own size (wgview.h). */
#define WG_OWN_FB
#define WGFB_LAYERS 2 /* the screen, and the empty table */
#include "wg.h"
#include "sdf_font.h"
#include "sourdough.glyphs.h"
#include "wgfb.h"
#include "wgview.h"

/* the plan everything is laid out on, scaled to the screen */
#define PW 480
#define PH 420

#define MAGIC 0x53524431u /* "SRD1" */
#define VERSION 5
#define END_MS 1600 /* the solved table stays in view this long before game over */
#define CW 60
#define CH 84
#define GAP 8
#define LEFT 6
#define BAR 24
#define TOP_Y 30
#define TAB_Y 126
#define MAXP 24
#define MAXUNDO 80
#define FLY_MS 170
#define TURN_MS 220

/* time left in each column's top card turning over; pictures only, so
 * not in the save */
static i32 turn_ms[7];

enum { PLAY, WON };
enum { STOCK, WASTE, F0, F1, F2, F3, T0, NPILES = 13 };
/* options, in catalog order: draw, passes, theme */
enum { O_DRAW, O_PASSES, O_THEME, NOPT };
#define THEMES 4
static const i32 CHOICES[NOPT] = {2, 3, THEMES};

typedef struct {
  u8 c[NPILES][MAXP];
  u8 n[NPILES];
  u8 down[7];
  i32 points, passes;
} Table;

static struct {
  u32 magic, version, rng;
  i32 phase, end_ms;
  i32 opt[NOPT];
  i32 time_ms, moves, score, bonus, started, undos;
  i32 kx, krow, kidx, holding, hold_pile, hold_idx, keyboard;
  i32 mx, my, mbtn, press, press_pile, press_idx, press_x, press_y, dragging;
  i32 auto_ms, pend_ev, prev_buttons, rep_ms, blink;
  i32 fly_n, fly_t;
  i32 fly_card[13], fly_x[13], fly_y[13];
  i32 undo_n;
  Table t;
  Table undo[MAXUNDO];
} S;

typedef struct {
  const char *name;
  u32 table, table_lo, slot, text, dim, accent, back, back_hi, panel;
} Theme;

static const Theme THEME[THEMES] = {
  {"MEADOW", RGB(36, 104, 72), RGB(28, 84, 58), RGB(48, 122, 88), RGB(240, 250, 240), RGB(160, 206, 180), RGB(255, 214, 102),
   RGB(176, 52, 58), RGB(214, 92, 90), RGB(22, 70, 48)},
  {"HARBOUR", RGB(30, 62, 104), RGB(22, 48, 84), RGB(44, 80, 126), RGB(236, 244, 255), RGB(150, 180, 220), RGB(255, 196, 92),
   RGB(232, 156, 48), RGB(250, 196, 104), RGB(18, 38, 70)},
  {"CELLAR", RGB(92, 30, 44), RGB(72, 22, 34), RGB(112, 44, 60), RGB(255, 238, 236), RGB(214, 160, 168), RGB(255, 200, 120),
   RGB(40, 64, 98), RGB(80, 110, 150), RGB(58, 16, 26)},
  {"SLATE", RGB(23, 20, 46), RGB(16, 13, 34), RGB(40, 34, 78), RGB(255, 247, 232), RGB(150, 140, 190), RGB(46, 196, 182),
   RGB(101, 79, 240), RGB(150, 132, 255), RGB(40, 34, 78)},
};
#define T (THEME[S.opt[O_THEME]])

static const u32 FACE = RGB(252, 249, 242), EDGE = RGB(120, 112, 104), RED = RGB(200, 38, 52), BLACK = RGB(28, 28, 40);
static const char *RANKS[13] = {"A", "2", "3", "4", "5", "6", "7", "8", "9", "10", "J", "Q", "K"};

#define RANK(c) ((c) % 13)
#define SUIT(c) ((c) / 13)
#define IS_RED(c) (SUIT(c) == 1 || SUIT(c) == 2)
#define TB (S.t)

/* ---------- layout ---------- */

static i32 pile_x(i32 p) {
  if (p == STOCK) return LEFT;
  if (p == WASTE) return LEFT + CW + GAP;
  if (p < T0) return LEFT + (p - F0 + 3) * (CW + GAP);
  return LEFT + (p - T0) * (CW + GAP);
}

/* spacing of a tableau column, squeezed so the column fits the screen */
static void tab_offsets(i32 p, i32 *down_off, i32 *up_off) {
  i32 d = TB.down[p - T0], u = TB.n[p] - d;
  *down_off = 7;
  *up_off = 20;
  i32 room = PH - 4 - TAB_Y - CH;
  if (u > 1 && d * 7 + (u - 1) * 20 > room) {
    *down_off = 4;
    *up_off = (room - d * 4) / (u - 1);
    if (*up_off > 20) *up_off = 20;
  }
}

static void card_pos(i32 p, i32 i, i32 *x, i32 *y) {
  *x = pile_x(p);
  *y = TOP_Y;
  if (p == WASTE) {
    /* with three to a draw, the last three cards fan out */
    i32 shown = S.opt[O_DRAW] ? 3 : 1, first = TB.n[p] - shown;
    if (first < 0) first = 0;
    if (i > first) *x += (i - first) * 14;
  } else if (p >= T0) {
    i32 doff, uoff, d = TB.down[p - T0];
    tab_offsets(p, &doff, &uoff);
    *y = TAB_Y + (i < d ? i * doff : d * doff + (i - d) * uoff);
  }
}

/* the pile and card index under a point; idx -1 for an empty pile's slot */
static i32 hit(i32 x, i32 y, i32 *idx) {
  *idx = -1;
  for (i32 p = 0; p < NPILES; p++) {
    if (p >= T0) {
      i32 px = pile_x(p);
      if (x < px || x >= px + CW || y < TAB_Y) continue;
      for (i32 i = TB.n[p] - 1; i >= 0; i--) {
        i32 cx, cy;
        card_pos(p, i, &cx, &cy);
        if (y >= cy && y < cy + CH) { *idx = i; return p; }
      }
      if (y < TAB_Y + CH) return p;
      continue;
    }
    i32 top = TB.n[p] - 1, cx, cy;
    card_pos(p, top < 0 ? 0 : top, &cx, &cy);
    if (inside(x, y, cx, cy, CW, CH) || inside(x, y, pile_x(p), TOP_Y, CW, CH)) { *idx = top; return p; }
  }
  return -1;
}

/* where a card dropped at this point lands: the pile whose column it is over */
static i32 drop_pile(i32 x, i32 y) {
  i32 c = (x - LEFT + GAP / 2) / (CW + GAP);
  if (c < 0) c = 0;
  if (c > 6) c = 6;
  if (y < TAB_Y - 6) return c >= 3 ? F0 + c - 3 : -1;
  return T0 + c;
}

/* ---------- rules ---------- */

static void push_undo(void) {
  if (S.undo_n == MAXUNDO) {
    memcpy(&S.undo[0], &S.undo[1], sizeof(Table) * (MAXUNDO - 1));
    S.undo_n--;
  }
  memcpy(&S.undo[S.undo_n++], &TB, sizeof(Table));
  S.started = 1;
}

static void fly_from(i32 p, i32 from, i32 count) {
  S.fly_n = 0;
  S.fly_t = FLY_MS;
  for (i32 i = 0; i < count && i < 13; i++) {
    i32 x, y;
    card_pos(p, from + i, &x, &y);
    S.fly_card[i] = TB.c[p][from + i];
    S.fly_x[i] = x;
    S.fly_y[i] = y;
    S.fly_n++;
  }
}

static i32 can_found(i32 card, i32 f) {
  i32 n = TB.n[f];
  if (!n) return RANK(card) == 0;
  i32 top = TB.c[f][n - 1];
  return SUIT(top) == SUIT(card) && RANK(card) == RANK(top) + 1;
}

static i32 can_tab(i32 card, i32 t) {
  i32 n = TB.n[t];
  if (!n) return RANK(card) == 12;
  i32 top = TB.c[t][n - 1];
  if (n <= TB.down[t - T0]) return 0;
  return IS_RED(top) != IS_RED(card) && RANK(card) + 1 == RANK(top);
}

static i32 movable(i32 p, i32 i) {
  if (p == STOCK || i < 0 || i >= TB.n[p]) return 0;
  if (p == WASTE || p < T0) return i == TB.n[p] - 1;
  return i >= TB.down[p - T0];
}

static void add_points(i32 v) {
  TB.points += v;
  if (TB.points < 0) TB.points = 0;
}

static void check_win(void) {
  for (i32 f = F0; f <= F3; f++)
    if (TB.n[f] != 13) return;
  S.phase = WON;
  S.end_ms = END_MS;
  i32 secs = S.time_ms / 1000;
  S.bonus = 300000 / (secs + 30);
  S.score = TB.points + S.bonus;
  S.pend_ev |= WG_EV_WIN;
}

/* moves cards i.. of pile p onto pile d, if the rules allow */
static i32 move(i32 p, i32 i, i32 d) {
  if (p == d || !movable(p, i)) return 0;
  i32 card = TB.c[p][i], count = TB.n[p] - i;
  if (d >= F0 && d <= F3) {
    if (count != 1 || !can_found(card, d)) return 0;
  } else if (d >= T0) {
    if (!can_tab(card, d)) return 0;
  } else {
    return 0;
  }
  push_undo();
  fly_from(p, i, count);
  for (i32 k = 0; k < count; k++) TB.c[d][TB.n[d]++] = TB.c[p][i + k];
  TB.n[p] = (u8)i;
  if (d <= F3) add_points(10);
  else if (p == WASTE) add_points(5);
  else if (p >= F0 && p <= F3) add_points(-15);
  if (p >= T0 && TB.n[p] && TB.down[p - T0] >= TB.n[p]) {
    TB.down[p - T0] = (u8)(TB.n[p] - 1);
    turn_ms[p - T0] = TURN_MS;
    add_points(5);
  }
  S.moves++;
  S.pend_ev |= d <= F3 ? WG_EV_HIT : WG_EV_BLIP;
  check_win();
  return 1;
}

/* the best place for a card: a foundation, else a column that takes it */
static i32 auto_move(i32 p, i32 i) {
  if (!movable(p, i)) return 0;
  if (i == TB.n[p] - 1)
    for (i32 f = F0; f <= F3; f++)
      if (p != f && move(p, i, f)) return 1;
  for (i32 k = 0; k < 7; k++) {
    i32 d = T0 + k;
    /* a king already at the bottom of its column has nowhere better to go */
    if (p >= T0 && i == 0 && !TB.n[d]) continue;
    if (TB.n[d] && move(p, i, d)) return 1;
  }
  for (i32 k = 0; k < 7; k++)
    if (!TB.n[T0 + k] && !(p >= T0 && i == 0) && move(p, i, T0 + k)) return 1;
  return 0;
}

static i32 passes_left(void) {
  if (S.opt[O_PASSES] == 0) return 1;
  return TB.passes < (S.opt[O_PASSES] == 1 ? 2 : 0);
}

static void draw_stock(void) {
  if (TB.n[STOCK]) {
    push_undo();
    i32 k = S.opt[O_DRAW] ? 3 : 1;
    while (k-- && TB.n[STOCK]) TB.c[WASTE][TB.n[WASTE]++] = TB.c[STOCK][--TB.n[STOCK]];
    fly_from(WASTE, TB.n[WASTE] - 1, 1);
    S.fly_x[0] = pile_x(STOCK);
    S.fly_y[0] = TOP_Y;
    S.moves++;
    S.pend_ev |= WG_EV_BLIP;
  } else if (TB.n[WASTE] && passes_left()) {
    push_undo();
    while (TB.n[WASTE]) TB.c[STOCK][TB.n[STOCK]++] = TB.c[WASTE][--TB.n[WASTE]];
    TB.passes++;
    if (TB.passes > 1) add_points(S.opt[O_DRAW] ? -20 : -50);
    S.moves++;
    S.pend_ev |= WG_EV_HIT;
  }
}

static void undo(void) {
  if (!S.undo_n || S.phase != PLAY) return;
  memcpy(&TB, &S.undo[--S.undo_n], sizeof(Table));
  add_points(-20);
  S.undos++;
  S.holding = 0;
  S.fly_n = 0;
  S.pend_ev |= WG_EV_BLIP;
}

/* cards are all face up and the stock is used: nothing left to decide */
static i32 solved(void) {
  if (TB.n[STOCK] || TB.n[WASTE]) return 0;
  for (i32 k = 0; k < 7; k++)
    if (TB.down[k]) return 0;
  return 1;
}

static void deal(void) {
  u8 deck[52];
  for (i32 i = 0; i < 52; i++) deck[i] = (u8)i;
  for (i32 i = 51; i > 0; i--) {
    i32 j = (i32)(xorshift(&S.rng) % (u32)(i + 1));
    u8 t = deck[i];
    deck[i] = deck[j];
    deck[j] = t;
  }
  memset(&TB, 0, sizeof TB);
  i32 k = 0;
  for (i32 c = 0; c < 7; c++) {
    for (i32 r = 0; r <= c; r++) TB.c[T0 + c][r] = deck[k++];
    TB.n[T0 + c] = (u8)(c + 1);
    TB.down[c] = (u8)c;
  }
  while (k < 52) TB.c[STOCK][TB.n[STOCK]++] = deck[k++];
}

/* ---------- drawing: cards ---------- */

static float ease(float t) { t = clampf(t, 0, 1); return t * t * (3 - 2 * t); }

static void suit_icon(float cx, float cy, float s, i32 suit, u32 c, float a) {
  float r = s * 0.27f;
  if (suit == 1) { /* heart */
    v_disc(cx - s * 0.235f, cy - s * 0.14f, r, c, a);
    v_disc(cx + s * 0.235f, cy - s * 0.14f, r, c, a);
    float p[8] = {cx - s * 0.49f, cy - s * 0.06f, cx, cy - s * 0.02f, cx + s * 0.49f, cy - s * 0.06f, cx, cy + s * 0.5f};
    v_poly(p, 4, c, a);
  } else if (suit == 2) { /* diamond */
    float p[8] = {cx, cy - s * 0.52f, cx + s * 0.38f, cy, cx, cy + s * 0.52f, cx - s * 0.38f, cy};
    v_poly(p, 4, c, a);
  } else if (suit == 0) { /* spade */
    float p[8] = {cx, cy - s * 0.52f, cx + s * 0.48f, cy + s * 0.06f, cx, cy + s * 0.06f, cx - s * 0.48f, cy + s * 0.06f};
    v_poly(p, 4, c, a);
    v_disc(cx - s * 0.235f, cy + s * 0.1f, r, c, a);
    v_disc(cx + s * 0.235f, cy + s * 0.1f, r, c, a);
    v_tri(cx, cy + s * 0.1f, cx + s * 0.17f, cy + s * 0.5f, cx - s * 0.17f, cy + s * 0.5f, c, a);
  } else { /* club */
    float q = s * 0.22f, o = s * 0.25f;
    v_disc(cx, cy - o, q, c, a);
    v_disc(cx - o, cy + s * 0.07f, q, c, a);
    v_disc(cx + o, cy + s * 0.07f, q, c, a);
    v_disc(cx, cy, q * 0.6f, c, a);
    v_tri(cx, cy - s * 0.05f, cx + s * 0.17f, cy + s * 0.5f, cx - s * 0.17f, cy + s * 0.5f, c, a);
  }
}

/* the card's outline and shadow; w < CW while it turns over */
static void card_base(float x, float y, float w, u32 edge) {
  float r = minf(5.5f, w / 2);
  /* only the bottom edge's shadow shows past the cards around it */
  v_shadow(x + 1, y + CH - 8, w - 2, 9.5f, r, 2.5f, RGB(0, 0, 0), 0.32f);
  v_box(x, y, w, CH, r, edge, 1);
  v_box(x + 0.7f, y + 0.7f, w - 1.4f, CH - 1.4f, r - 0.7f, FACE, 1);
}

/* the patterned middle of a card back, over the card's face colour */
static void back_pattern(float x, float y) {
  v_box(x, y, CW - 6, CH - 6, 0, FACE, 1);
  float ix = x + 1, iy = y + 1, iw = CW - 8, ih = CH - 8;
  v_box_grad(ix, iy, iw, ih, 3.5f, T.back_hi, T.back, 1);
  /* a lattice of small diamonds */
  for (i32 j = 0; j < 9; j++)
    for (i32 i = 0; i < 7; i++) {
      float cx = ix + 4 + i * 8 + (j & 1) * 4, cy = iy + 4.5f + j * 8.5f;
      if (cx > ix + iw - 3) continue;
      float p[8] = {cx, cy - 2.4f, cx + 2, cy, cx, cy + 2.4f, cx - 2, cy};
      v_poly(p, 4, T.back_hi, 0.55f);
    }
}

static Sprite back_sprite;

static void card_back(float x, float y, float w) {
  card_base(x, y, w, EDGE);
  if (w < 10) return;
  if (w >= CW && sprite_make(&back_sprite, (u32)S.opt[O_THEME] + 1, 0, 0, CW - 6, CH - 6, back_pattern)) {
    sprite_put(&back_sprite, x + 3, y + 3);
    return;
  }
  float ix = x + 4, iy = y + 4, iw = w - 8, ih = CH - 8;
  v_box_grad(ix, iy, iw, ih, 3.5f, T.back_hi, T.back, 1);
}

static void card_face(float x, float y, i32 card, i32 lit, float w) {
  i32 r = RANK(card), s = SUIT(card);
  u32 c = IS_RED(card) ? RED : BLACK;
  card_base(x, y, w, lit ? T.accent : EDGE);
  if (lit) {
    v_box(x + 0.7f, y + 0.7f, w - 1.4f, CH - 1.4f, 4.8f, T.accent, 0.14f);
  }
  if (w < CW - 4) {
    /* turning over: just the middle */
    suit_icon(x + w / 2, y + CH / 2, minf(24, w * 0.5f), s, c, 1);
    return;
  }
  v_text(RANKS[r], x + 5, y + 11, r == 9 ? 14 : 16, c, 1);
  suit_icon(x + CW - 11, y + 11, 11, s, c, 1);
  if (r >= 10) {
    /* court cards: the letter, framed, with the suit under it */
    float t = 0.8f;
    v_box(x + 12, y + 24, CW - 24, CH - 31, 3, c, 1);
    v_box(x + 12 + t, y + 24 + t, CW - 24 - 2 * t, CH - 31 - 2 * t, 2.4f, FACE, 1);
    v_text_c(RANKS[r], x + CW / 2.0f, y + 43, 22, c, 1);
    suit_icon(x + CW / 2.0f, y + 64, 11, s, c, 1);
  } else {
    suit_icon(x + CW / 2.0f, y + 52, 26, s, c, 1);
  }
}

static void slot(float x, float y, const char *label) {
  v_box(x, y, CW, CH, 6, T.slot, 0.9f);
  v_box(x + 1.6f, y + 1.6f, CW - 3.2f, CH - 3.2f, 4.6f, T.table_lo, 1);
  if (label) v_text_c(label, x + CW / 2.0f, y + CH / 2.0f, 22, T.slot, 1);
}

static i32 is_flying(i32 card) {
  for (i32 i = 0; i < S.fly_n; i++)
    if (S.fly_card[i] == card) return 1;
  return 0;
}

static i32 lifted(i32 p, i32 i) {
  if (S.dragging && S.press_pile == p && i >= S.press_idx) return 1;
  return 0;
}


static void draw_card_at(i32 p, i32 i, float x, float y) {
  i32 card = TB.c[p][i];
  i32 face_up = p == WASTE || (p >= F0 && p <= F3) || (p >= T0 && i >= TB.down[p - T0]);
  i32 lit = S.holding && S.hold_pile == p && i >= S.hold_idx;
  if (p >= T0 && face_up && i == TB.down[p - T0] && i == TB.n[p] - 1 && turn_ms[p - T0] > 0) {
    float k = ease(1 - (float)turn_ms[p - T0] / TURN_MS), w = CW * absf(k * 2 - 1);
    if (w < 1) w = 1;
    if (k < 0.5f) card_back(x + (CW - w) / 2, y, w);
    else card_face(x + (CW - w) / 2, y, card, 0, w);
    return;
  }
  if (!face_up) card_back(x, y, CW);
  else card_face(x, lit ? y - 6 : y, card, lit, CW);
}

/* the felt and the bar, kept in layer 1 */
static void draw_static(void) {
  clear(T.table_lo);
  v_box_grad(-60, BAR - 2, PW + 120, PH + 60, 0, mixc(RGB(255, 255, 255), T.table, 0.05f), T.table_lo, 1);
  v_box(-60, -60, PW + 120, BAR + 58, 0, T.table_lo, 1);
}

static void draw_table(void) {
  /* stock */
  if (TB.n[STOCK]) {
    i32 n = TB.n[STOCK], layers = n > 16 ? 3 : n > 6 ? 2 : 1;
    for (i32 l = layers - 1; l >= 0; l--) card_back(pile_x(STOCK) + l * 1.5f, TOP_Y - l * 1.5f, CW);
  } else {
    slot(pile_x(STOCK), TOP_Y, 0);
    if (TB.n[WASTE] && passes_left()) {
      /* turn the waste over: a circling arrow */
      float cx = pile_x(STOCK) + CW / 2.0f, cy = TOP_Y + CH / 2.0f;
      v_ring(cx, cy, 11, 3, T.slot, 1);
      v_box(cx - 2, cy - 13, 8, 6, 0, T.table_lo, 1);
      v_tri(cx + 1, cy - 16, cx + 7, cy - 9.5f, cx + 1, cy - 3, T.slot, 1);
    }
  }
  for (i32 p = WASTE; p < NPILES; p++) {
    if (p != WASTE && !TB.n[p]) slot(pile_x(p), p >= T0 ? TAB_Y : TOP_Y, p >= F0 && p <= F3 ? "A" : p >= T0 ? "K" : 0);
    i32 from = 0;
    if (p == WASTE || (p >= F0 && p <= F3)) from = TB.n[p] > 3 ? TB.n[p] - 3 : 0;
    for (i32 i = from; i < TB.n[p]; i++) {
      if (lifted(p, i) || is_flying(TB.c[p][i])) continue;
      i32 x, y;
      card_pos(p, i, &x, &y);
      if ((p >= F0 && p <= F3) && i < TB.n[p] - 1) {
        /* only the top card of a foundation shows, but draw one below for the flight */
        x = pile_x(p);
        y = TOP_Y;
      }
      draw_card_at(p, i, (float)x, (float)y);
    }
  }
  /* keyboard cursor */
  if (S.keyboard && S.phase == PLAY) {
    i32 p, x, y, h = CH;
    if (S.krow == 0) {
      p = S.kx == 0 ? STOCK : S.kx <= 2 ? WASTE : F0 + S.kx - 3;
      i32 top = TB.n[p] - 1;
      card_pos(p, top < 0 ? 0 : top, &x, &y);
    } else {
      p = T0 + S.kx;
      i32 n = TB.n[p], idx = S.kidx < n ? S.kidx : n - 1;
      if (idx < 0) { x = pile_x(p); y = TAB_Y; }
      else {
        card_pos(p, idx, &x, &y);
        i32 bx, by;
        card_pos(p, n - 1, &bx, &by);
        h = by + CH - y;
      }
    }
    float x0 = x - 3.5f, y0 = y - 3.5f, x1 = x + CW + 3.5f, y1 = y + h + 3.5f, t = 1.1f;
    v_line(x0, y0, x1, y0, t, T.accent, 1);
    v_line(x1, y0, x1, y1, t, T.accent, 1);
    v_line(x1, y1, x0, y1, t, T.accent, 1);
    v_line(x0, y1, x0, y0, t, T.accent, 1);
  }
  /* cards in flight, then the dragged stack */
  if (S.fly_n) {
    float k = ease((float)(FLY_MS - S.fly_t) / FLY_MS);
    for (i32 f = 0; f < S.fly_n; f++) {
      i32 card = S.fly_card[f];
      for (i32 p = 0; p < NPILES; p++)
        for (i32 i = 0; i < TB.n[p]; i++)
          if (TB.c[p][i] == card) {
            i32 x, y;
            card_pos(p, i, &x, &y);
            if (p >= F0 && p <= F3) { x = pile_x(p); y = TOP_Y; }
            draw_card_at(p, i, S.fly_x[f] + (x - S.fly_x[f]) * k, S.fly_y[f] + (y - S.fly_y[f]) * k);
          }
    }
  }
  if (S.dragging) {
    i32 p = S.press_pile;
    for (i32 i = S.press_idx; i < TB.n[p]; i++) {
      i32 x, y;
      card_pos(p, i, &x, &y);
      float dx = x + S.mx - S.press_x, dy = y + S.my - S.press_y;
      v_shadow(dx + 2, dy + 5, CW, CH, 6, 6, RGB(0, 0, 0), 0.25f);
      card_face(dx, dy, TB.c[p][i], 0, CW);
    }
  }
}

/* ---------- drawing: bar, end ---------- */

#define BTN_W 64
#define UNDO_X (PW - BTN_W - 4)

static i32 undo_hot(void) { return !S.keyboard && inside(S.mx, S.my, UNDO_X, 2, BTN_W, 19) && S.undo_n; }

static void draw_bar(void) {
  char buf[48], num[12];
  float mid = (BAR - 2) / 2.0f + 0.5f;
  v_text(join(buf, tr("Score ", "得分 ", "スコア ", "점수 ", "Puntos ", "Pontos ", "Score ", "Punkte ", "Очки "), itoa10(TB.points, num)), 8, mid, 12, T.text, 1);
  v_text(clock_text(S.time_ms / 1000, buf), 120, mid, 12, T.text, 1);
  v_text(join(buf, tr("Moves ", "步数 ", "手数 ", "이동 ", "Movimientos ", "Movimentos ", "Coups ", "Züge ", "Ходы "), itoa10(S.moves, num)), 190, mid, 12, T.dim, 1);
  i32 hot = undo_hot();
  float a = S.undo_n ? 1 : 0.4f;
  v_box(UNDO_X, 2, BTN_W, 19, 6, hot ? T.accent : T.panel, a);
  v_text_c(tr("Undo", "撤销", "元に戻す", "되돌리기", "Deshacer", "Desfazer", "Annuler", "Zurück", "Отмена"), UNDO_X + BTN_W / 2.0f, 2 + 9.5f, 12, hot ? T.panel : T.text, a);
}

static void draw_won(void) {
  char buf[48], num[12];
  float pw = 240, ph = 100, px = (PW - pw) / 2, py = 160;
  v_shadow(px, py + 4, pw, ph, 14, 12, RGB(0, 0, 0), 0.4f);
  v_box(px, py, pw, ph, 14, T.panel, 0.96f);
  v_text_c(tr("Solved", "完成", "完成", "완성", "Resuelto", "Resolvido", "Résolu", "Gelöst", "Решено"), PW / 2.0f, py + 28, 26, T.accent, 1);
  v_text_c(join(buf, tr("Time bonus ", "时间奖励 ", "タイムボーナス ", "시간 보너스 ", "Bonus de tiempo ", "Bônus de tempo ", "Bonus de temps ", "Zeitbonus ", "Бонус за время "), itoa10(S.bonus, num)), PW / 2.0f, py + 56, 12, T.dim, 1);
  v_text_c(join(buf, tr("Score ", "得分 ", "スコア ", "점수 ", "Puntos ", "Pontos ", "Score ", "Punkte ", "Очки "), itoa10(S.score, num)), PW / 2.0f, py + 78, 17, T.text, 1);
}

static void draw(void) {
  view_background(hash_int(S.opt[O_THEME] + 1, 7), draw_static);
  draw_table();
  draw_bar();
  if (S.phase == WON) draw_won();
}

/* what the picture depends on, so a still table isn't repainted every frame */
static u32 picture(void) {
  u32 h = hash_bytes(0, &TB, sizeof TB);
  i32 v[] = {S.phase, S.holding, S.hold_pile, S.hold_idx, S.keyboard, S.kx, S.krow, S.kidx, S.dragging,
             S.press_pile, S.press_idx, S.dragging ? S.mx : 0, S.dragging ? S.my : 0, undo_hot(), S.undo_n > 0,
             S.time_ms / 1000, S.moves, S.score, S.opt[O_THEME], S.opt[O_DRAW]};
  return hash_bytes(h, v, sizeof v);
}

static i32 animate(i32 dt) {
  i32 busy = S.fly_n > 0;
  for (i32 k = 0; k < 7; k++)
    if (turn_ms[k] > 0) { turn_ms[k] -= dt; busy = 1; }
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

static void click_pile(i32 p, i32 idx) {
  if (p == STOCK) { draw_stock(); return; }
  if (idx >= 0) auto_move(p, idx);
}

WG_EXPORT(wg_mouse) void wg_mouse(i32 x, i32 y, i32 b) {
  i32 held = b & 7, pressed = held & ~S.mbtn, released = S.mbtn & ~held;
  S.mbtn = held;
  if (x < 0) { S.mx = S.my = -1; S.press = S.dragging = 0; return; }
  x = plan_x(x);
  y = plan_y(y);
  if (x != S.mx || y != S.my) S.keyboard = 0;
  S.mx = x;
  S.my = y;
  if (S.phase != PLAY) return;
  if (pressed & M_LEFT) {
    S.holding = 0;
    if (y < BAR) {
      if (inside(x, y, UNDO_X, 2, BTN_W, 19)) undo();
      return;
    }
    i32 idx, p = hit(x, y, &idx);
    S.press = p >= 0;
    S.press_pile = p;
    S.press_idx = idx;
    S.press_x = x;
    S.press_y = y;
    S.dragging = 0;
    return;
  }
  if (S.press && !S.dragging && held & M_LEFT) {
    i32 dx = x - S.press_x, dy = y - S.press_y;
    if (dx * dx + dy * dy > 25 && movable(S.press_pile, S.press_idx)) S.dragging = 1;
  }
  if (released & M_LEFT && S.press) {
    if (S.dragging) {
      /* drop where the dragged card's middle is */
      i32 cx, cy;
      card_pos(S.press_pile, S.press_idx, &cx, &cy);
      i32 d = drop_pile(cx + CW / 2 + x - S.press_x, cy + 20 + y - S.press_y);
      S.dragging = 0;
      if (d >= 0) move(S.press_pile, S.press_idx, d);
      S.fly_n = 0; /* dropped cards land where they are let go */
    } else {
      click_pile(S.press_pile, S.press_idx);
    }
    S.press = 0;
  }
}

static i32 key_pile(void) {
  if (S.krow == 0) return S.kx == 0 ? STOCK : S.kx <= 2 ? WASTE : F0 + S.kx - 3;
  return T0 + S.kx;
}

static i32 key_index(i32 p) {
  if (p < T0) return TB.n[p] - 1;
  i32 n = TB.n[p];
  if (!n) return -1;
  i32 d = TB.down[p - T0];
  if (S.kidx < d) S.kidx = d;
  if (S.kidx > n - 1) S.kidx = n - 1;
  return S.kidx;
}

static void keys(u32 buttons, u32 pressed, i32 dt) {
  u32 dirs = key_repeat(buttons, pressed, dt, &S.rep_ms);
  if (pressed) S.keyboard = 1;
  if (S.phase != PLAY) return;
  if (dirs & B_LEFT) S.kx = (S.kx + 6) % 7;
  if (dirs & B_RIGHT) S.kx = (S.kx + 1) % 7;
  if (S.krow == 0 && S.kx == 2) S.kx = dirs & B_LEFT ? 1 : 3;
  if (dirs & B_DOWN) {
    if (S.krow == 0) { S.krow = 1; S.kidx = TB.n[T0 + S.kx] - 1; }
    else if (S.kidx < TB.n[T0 + S.kx] - 1) S.kidx++;
  }
  if (dirs & B_UP) {
    if (S.krow == 1) {
      i32 p = T0 + S.kx;
      if (TB.n[p] && S.kidx > TB.down[p - T0]) S.kidx--;
      else { S.krow = 0; if (S.kx == 2) S.kx = 1; }
    }
  }
  if (dirs & (B_LEFT | B_RIGHT) && S.krow == 1) S.kidx = TB.n[T0 + S.kx] - 1;
  i32 p = key_pile(), idx = key_index(p);
  if (pressed & B_A) {
    if (S.holding && S.hold_pile == p && S.hold_idx == idx) {
      /* pressed twice on the same cards: send them where they fit */
      S.holding = 0;
      auto_move(p, idx);
    } else if (S.holding) {
      S.holding = 0;
      move(S.hold_pile, S.hold_idx, p);
    } else if (p == STOCK) {
      draw_stock();
    } else if (movable(p, idx)) {
      S.holding = 1;
      S.hold_pile = p;
      S.hold_idx = idx;
    }
  }
  if (pressed & B_B) {
    if (S.holding) S.holding = 0;
    else undo();
  }
}

WG_EXPORT(wg_init) void wg_init(u32 seed) {
  static const i32 DEFAULTS[NOPT] = {0, 0, 0};
  i32 opt[NOPT];
  for (i32 i = 0; i < NOPT; i++) opt[i] = S.magic == MAGIC && S.version == VERSION ? S.opt[i] : DEFAULTS[i];
  memset(&S, 0, sizeof S);
  S.magic = MAGIC;
  S.version = VERSION;
  S.rng = seed | 1;
  memcpy(S.opt, opt, sizeof opt);
  S.mx = S.my = -1;
  deal();
  S.phase = PLAY;
  memset(turn_ms, 0, sizeof turn_ms);
}

WG_EXPORT(wg_option) i32 wg_option(i32 i, i32 v) {
  if (S.magic != MAGIC) wg_init(1);
  return wg_pick(S.opt, CHOICES, NOPT, i, v);
}

WG_EXPORT(wg_over) i32 wg_over(void) { return S.phase == WON; }

WG_EXPORT(wg_frame) i32 wg_frame(i32 buttons, i32 dt_ms) {
  u32 pressed = (u32)buttons & ~(u32)S.prev_buttons;
  S.prev_buttons = buttons;
  if (dt_ms > 100) dt_ms = 100;
  S.blink++;
  xorshift(&S.rng);
  keys((u32)buttons, pressed, dt_ms);
  if (S.phase == WON && S.end_ms > 0) {
    S.end_ms -= dt_ms;
    if (S.end_ms <= 0) S.pend_ev |= WG_EV_GAMEOVER;
  }
  i32 moving = animate(dt_ms);
  if (S.fly_n) {
    S.fly_t -= dt_ms;
    if (S.fly_t <= 0) S.fly_n = 0;
  }
  if (S.phase == PLAY) {
    if (S.started) S.time_ms += dt_ms;
    if (solved() && !S.dragging && !S.holding) {
      /* the rest plays itself, one card at a time */
      S.auto_ms -= dt_ms;
      if (S.auto_ms <= 0 && !S.fly_n) {
        S.auto_ms = 60;
        i32 best = -1, rank = 99;
        for (i32 k = 0; k < 7; k++) {
          i32 p = T0 + k, n = TB.n[p];
          if (n && RANK(TB.c[p][n - 1]) < rank) { rank = RANK(TB.c[p][n - 1]); best = p; }
        }
        if (best >= 0) auto_move(best, TB.n[best] - 1);
      }
    }
  }
  ensure_fb();
  view_present(picture(), moving, draw);
  i32 ev = S.pend_ev;
  S.pend_ev = 0;
  return ev;
}

/* for the game over screen, in the catalog's `stats` order: time, moves,
 * cards home, times through the deck, undos, points, time bonus */
WG_EXPORT(wg_stat) i32 wg_stat(i32 i) {
  switch (i) {
    case 0: return S.time_ms;
    case 1: return S.moves;
    case 2: return TB.n[F0] + TB.n[F1] + TB.n[F2] + TB.n[F3];
    case 3: return TB.passes + 1;
    case 4: return S.undos;
    case 5: return TB.points;
    case 6: return S.phase == WON ? S.bonus : -1;
  }
  return -1;
}

WG_EXPORT(wg_state) void *wg_state(void) { return &S; }
WG_EXPORT(wg_state_size) i32 wg_state_size(void) { return sizeof S; }
WG_EXPORT(wg_score) i32 wg_score(void) { return S.score; }

/* every card exactly once, piles no taller than they can grow, and each
 * column's face-down cards under at least one face-up card */
static i32 valid_table(const Table *t) {
  u8 seen[52];
  i32 total = 0;
  memset(seen, 0, sizeof seen);
  for (i32 p = 0; p < NPILES; p++) {
    i32 n = t->n[p], most = p >= F0 && p <= F3 ? 13 : p >= T0 ? 19 : MAXP;
    if (n > most) return 0;
    for (i32 i = 0; i < n; i++) {
      u8 c = t->c[p][i];
      if (c >= 52 || seen[c]) return 0;
      seen[c] = 1;
      total++;
    }
  }
  for (i32 k = 0; k < 7; k++)
    if (t->n[T0 + k] ? t->down[k] >= t->n[T0 + k] : t->down[k]) return 0;
  return total == 52 && t->points >= 0 && t->passes >= 0;
}

static i32 valid_save(void) {
  if (S.magic != MAGIC || S.version != VERSION || !wg_picks_ok(S.opt, CHOICES, NOPT) ||
      S.phase < PLAY || S.phase > WON || S.undo_n < 0 || S.undo_n > MAXUNDO ||
      S.time_ms < 0 || !valid_table(&TB))
    return 0;
  for (i32 i = 0; i < S.undo_n; i++)
    if (!valid_table(&S.undo[i])) return 0;
  if (S.kx < 0 || S.kx > 6) S.kx = 0;
  if (S.krow < 0 || S.krow > 1) S.krow = 0;
  if (S.kidx < 0 || S.kidx > 18) S.kidx = 0;
  return 1;
}

WG_EXPORT(wg_state_loaded) i32 wg_state_loaded(void) {
  if (!valid_save()) {
    S.magic = 0;
    wg_init(0x1234);
    return 0;
  }
  S.prev_buttons = 0xFF;
  S.mbtn = 0;
  S.press = S.dragging = S.holding = 0;
  S.fly_n = 0;
  S.pend_ev = 0;
  memset(turn_ms, 0, sizeof turn_ms);
  view_redraw();
  return 1;
}
