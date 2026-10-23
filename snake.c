/* Snake - an original wasmga.me game. MIT licensed.
 * The classic: eat, grow, don't hit a wall or yourself. The page shows the
 * setup, pause and game over screens (see wg.h), so this file only plays.
 *
 * Draws at the screen's own size (wg_resize, wgfb.h): the snake glides
 * between squares instead of jumping, and the text is Inter (sdf_font.h). */
#define WG_OWN_FB
#define WGFB_LAYERS 2 /* the screen, and the field without the snake and apple */
#include "wg.h"
#include "sdf_font.h"
#include "wgfb.h"

#define MAXC 30
#define MAXR 24
#define MAXN (MAXC * MAXR)
#define MAGIC 0x534E4B33u /* "SNK3" */

/* options, in catalog order: speed, map, obstacles, theme */
enum { O_SPEED, O_MAP, O_OBST, O_THEME, NOPT };
static const i32 CHOICES[NOPT] = {3, 3, 3, 4};
static const i32 STEP_MS[3] = {180, 130, 105};
static const i32 COLS[3] = {15, 20, 30}, ROWS[3] = {12, 16, 24};

enum { READY, PLAY, DYING, OVER };
enum { EMPTY, BODY, WALL };

/* everything a save needs; the animation state below is not saved */
static struct {
  u32 magic, version, rng;
  i32 opt[NOPT];
  i32 phase, cols, rows;
  i32 dir, queue[2], nq;
  i32 head, len, food;
  i32 score, acc_ms, timer, prev_buttons;
  i32 best[3][3][3]; /* by speed, map and obstacles */
  u8 bx[MAXN], by[MAXN];
  u8 map[MAXR][MAXC];
  i32 time_ms, turns, steps; /* for the game over screen */
  i32 fruit; /* which fruit the food is */
} S;

static const i32 DX[4] = {1, 0, -1, 0}, DY[4] = {0, 1, 0, -1};

/* ---------- rules ---------- */

/* Obstacles are drawn from the map's shape, never at random, so every free
 * cell stays reachable and the starting row is always clear. */
static void wall_h(i32 y, i32 x0, i32 x1) {
  for (i32 x = x0; x <= x1; x++) S.map[y][x] = WALL;
}
static void wall_v(i32 x, i32 y0, i32 y1) {
  for (i32 y = y0; y <= y1; y++) S.map[y][x] = WALL;
}
static void build_walls(void) {
  i32 c = S.cols, r = S.rows, mid = r / 2;
  if (S.opt[O_OBST] >= 1) {
    wall_h(r / 4, c / 4, c - 1 - c / 4);
    wall_h(r - 1 - r / 4, c / 4, c - 1 - c / 4);
  }
  if (S.opt[O_OBST] >= 2) {
    /* a post each side, open across the two middle rows, and a notch in the
     * top and bottom edges */
    for (i32 y = r / 4 + 1; y <= r - 2 - r / 4; y++)
      if (y != mid - 1 && y != mid) S.map[y][c / 6] = S.map[y][c - 1 - c / 6] = WALL;
    wall_v(c / 2, 0, r / 4 - 2);
    wall_v(c / 2, r - r / 4 + 1, r - 1);
  }
}

enum { APPLE, ORANGE, CHERRIES, GRAPES, STRAWBERRY, LEMON, NFRUIT };

static void place_food(void) {
  i32 free = 0;
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++) free += S.map[y][x] == EMPTY;
  if (!free) { S.food = -1; return; }
  i32 k = (i32)(xorshift(&S.rng) % (u32)free);
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++)
      if (S.map[y][x] == EMPTY && k-- == 0) {
        S.food = y * S.cols + x;
        S.fruit = (i32)(xorshift(&S.rng) % NFRUIT);
        return;
      }
}

static i32 *best(void) { return &S.best[S.opt[O_SPEED]][S.opt[O_MAP]][S.opt[O_OBST]]; }

/* ---------- animation (not saved) ---------- */

static struct {
  float now;          /* ms since start, for wobbles and blinks */
  float eat_t;        /* ms since the last bite */
  i32 eat_cell;       /* where it was */
  i32 eat_fruit;      /* and which fruit */
  float spawn_t;      /* ms since the apple appeared */
  i32 under_ok;       /* the field layer is up to date */
  float bend_from;    /* where in the step the head was turned, or -1 */
} A;

static void layout(void);

WG_EXPORT(wg_init) void wg_init(u32 seed) {
  i32 keep = S.magic == MAGIC, opt[NOPT], bests[3][3][3];
  for (i32 i = 0; i < NOPT; i++) opt[i] = keep ? S.opt[i] : (i == O_SPEED || i == O_MAP);
  if (keep) memcpy(bests, S.best, sizeof bests);
  memset(&S, 0, sizeof S);
  S.magic = MAGIC; S.version = 3; S.rng = seed | 1;
  memcpy(S.opt, opt, sizeof opt);
  if (keep) memcpy(S.best, bests, sizeof bests);
  S.cols = COLS[S.opt[O_MAP]]; S.rows = ROWS[S.opt[O_MAP]];
  build_walls();
  S.len = 4; S.head = 3;
  for (i32 i = 0; i < 4; i++) {
    S.bx[i] = (u8)(1 + i); S.by[i] = (u8)(S.rows / 2);
    S.map[S.rows / 2][1 + i] = BODY;
  }
  S.dir = 0;
  S.phase = READY;
  place_food();
  A.eat_t = 1e9f;
  A.spawn_t = 0;
  A.bend_from = -1;
  ensure_fb();
  layout();
}

WG_EXPORT(wg_option) i32 wg_option(i32 i, i32 v) {
  if (S.magic != MAGIC) wg_init(1);
  return wg_pick(S.opt, CHOICES, NOPT, i, v);
}

/* the run ends as the snake crashes, not when the crash has played out */
WG_EXPORT(wg_over) i32 wg_over(void) { return S.phase >= DYING; }
/* the page's best on the board these picks play on, so the bar shows the same as its scores */
WG_EXPORT(wg_best) i32 wg_best(i32 v) {
  if (S.magic != MAGIC) wg_init(1);
  if (v >= 0) *best() = v > S.score ? v : S.score;
  return *best();
}

/* the way the head came into its square */
static i32 came(void) {
  i32 p = (S.head - 1 + MAXN) % MAXN;
  for (i32 i = 0; i < 4; i++)
    if (S.bx[p] + DX[i] == S.bx[S.head] && S.by[p] + DY[i] == S.by[S.head]) return i;
  return S.dir;
}

/* S.dir is the way the step in progress goes. A turn in the first 30% of a
 * step still bends that step (the head curves onto it); later ones wait in
 * the queue for the next square, so the head never jumps sideways. */
static void turn(i32 d) {
  if (S.phase != READY && S.phase != PLAY) return;
  i32 last = S.nq ? S.queue[S.nq - 1] : S.dir;
  if (d == last || (d + 2) % 4 == last || S.nq == 2) return;
  if (!S.nq && S.acc_ms * 10 < STEP_MS[S.opt[O_SPEED]] * 3 && (d + 2) % 4 != came()) {
    S.dir = d;
    A.bend_from = (float)S.acc_ms / STEP_MS[S.opt[O_SPEED]];
    return;
  }
  S.queue[S.nq++] = d;
}

static void next_turn(void) {
  if (!S.nq) return;
  S.dir = S.queue[0];
  S.queue[0] = S.queue[1];
  S.nq--;
}

static i32 step(void) {
  S.turns += S.dir != came();
  S.steps++;
  i32 hx = S.bx[S.head] + DX[S.dir], hy = S.by[S.head] + DY[S.dir];
  if (hx < 0 || hy < 0 || hx >= S.cols || hy >= S.rows) goto dead;
  i32 eating = hy * S.cols + hx == S.food;
  if (eating) {
    S.len++;
  } else {
    /* the tail moves out first, so following it closely is fine */
    i32 tail = (S.head - S.len + 1 + MAXN) % MAXN;
    S.map[S.by[tail]][S.bx[tail]] = EMPTY;
  }
  if (S.map[hy][hx] != EMPTY) goto dead;
  S.head = (S.head + 1) % MAXN;
  S.bx[S.head] = (u8)hx; S.by[S.head] = (u8)hy;
  S.map[hy][hx] = BODY;
  A.bend_from = -1;
  next_turn();
  if (eating) {
    S.score++;
    if (S.score > *best()) *best() = S.score;
    A.eat_t = 0;
    A.eat_cell = S.food;
    A.eat_fruit = S.fruit;
    A.spawn_t = 0;
    place_food();
    if (S.food < 0) { S.phase = OVER; return WG_EV_WIN | WG_EV_GAMEOVER; }
    return S.score % 10 ? WG_EV_BLIP : WG_EV_BLIP | WG_EV_LEVEL;
  }
  return 0;
dead:
  S.phase = DYING;
  S.timer = 0;
  return WG_EV_LOSE;
}

/* ---------- looks ---------- */

typedef struct {
  u32 frame, bar, text, field, field2, wall, wall_top, body, eye, pupil, apple, apple_hi, leaf, stem, gold;
} Theme;
static const Theme THEMES[4] = {
  /* Meadow */ {RGB(87, 138, 52), RGB(74, 117, 44), RGB(255, 255, 255), RGB(170, 215, 81), RGB(162, 209, 73),
                RGB(96, 112, 120), RGB(130, 146, 152), RGB(78, 124, 246), RGB(255, 255, 255),
                RGB(28, 32, 48), RGB(231, 71, 29), RGB(255, 160, 130), RGB(76, 160, 54), RGB(110, 70, 40), RGB(255, 202, 40)},
  /* LCD */    {RGB(48, 98, 48), RGB(36, 80, 36), RGB(202, 220, 159), RGB(155, 188, 15), RGB(147, 180, 13),
                RGB(48, 98, 48), RGB(70, 118, 60), RGB(15, 56, 15), RGB(155, 188, 15),
                RGB(15, 56, 15), RGB(15, 56, 15), RGB(110, 150, 30), RGB(48, 98, 48), RGB(15, 56, 15), RGB(202, 220, 159)},
  /* Night */  {RGB(20, 17, 42), RGB(14, 11, 30), RGB(255, 247, 232), RGB(36, 31, 70), RGB(31, 27, 62),
                RGB(72, 62, 130), RGB(98, 86, 168), RGB(101, 79, 240), RGB(255, 255, 255),
                RGB(20, 17, 42), RGB(255, 201, 60), RGB(255, 236, 170), RGB(46, 196, 182), RGB(120, 90, 60), RGB(255, 201, 60)},
  /* Paper */  {RGB(232, 222, 200), RGB(26, 23, 48), RGB(251, 246, 234), RGB(251, 246, 234), RGB(243, 235, 216),
                RGB(196, 186, 164), RGB(214, 205, 186), RGB(26, 23, 48), RGB(255, 255, 255),
                RGB(26, 23, 48), RGB(255, 90, 54), RGB(255, 190, 170), RGB(46, 160, 120), RGB(90, 60, 40), RGB(255, 180, 0)},
};
#define TH (&THEMES[S.opt[O_THEME]])

/* screen positions: the score bar on top, the field below it */
static struct { float bar, cs, fx, fy, m; } L;

static void layout(void) {
  float w = (float)FW, h = (float)FH;
  L.m = minf(w, h) * 0.025f;
  L.bar = clampf(minf(w, h) * 0.1f, 28, 120);
  float cs = minf((w - 2 * L.m) / S.cols, (h - L.bar - L.m) / S.rows);
  L.cs = (float)floori(cs);
  if (L.cs < 2) L.cs = 2;
  L.fx = (float)floori((w - L.cs * S.cols) / 2);
  L.fy = (float)floori(L.bar + (h - L.bar - L.m - L.cs * S.rows) / 2);
  A.under_ok = 0;
}

static void fb_resized(void) {
  if (S.cols) layout();
}

WG_EXPORT(wg_resize) void wg_resize(i32 w, i32 h) {
  if (fb_size(w, h, WGFB_LAYERS)) fb_resized();
}

static float cx_of(i32 x) { return L.fx + (x + 0.5f) * L.cs; }
static float cy_of(i32 y) { return L.fy + (y + 0.5f) * L.cs; }

/* the field, walls and score bar background, kept in layer 1 */
static void draw_under(void) {
  const Theme *t = TH;
  u32 *keep = fb;
  fb = fb_layer(1);
  clear(t->frame);
  box(0, 0, FW, (i32)L.bar, t->bar);
  i32 cs = (i32)L.cs, fx = (i32)L.fx, fy = (i32)L.fy;
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++) box(fx + x * cs, fy + y * cs, cs, cs, (x + y) & 1 ? t->field2 : t->field);
  /* walls: raised blocks with a lighter top, joined to their neighbours */
  float r = L.cs * 0.22f, in = L.cs * 0.06f;
  for (i32 y = 0; y < S.rows; y++)
    for (i32 x = 0; x < S.cols; x++) {
      if (S.map[y][x] != WALL) continue;
      float x0 = fx + x * L.cs + in, y0 = fy + y * L.cs + in, x1 = x0 + L.cs - 2 * in, y1 = y0 + L.cs - 2 * in;
      if (x > 0 && S.map[y][x - 1] == WALL) x0 -= in * 2;
      if (x + 1 < S.cols && S.map[y][x + 1] == WALL) x1 += in * 2;
      if (y > 0 && S.map[y - 1][x] == WALL) y0 -= in * 2;
      if (y + 1 < S.rows && S.map[y + 1][x] == WALL) y1 += in * 2;
      rrect(x0, y0 + in, x1 - x0, y1 - y0, r, mixc(RGB(0, 0, 0), t->wall, 0.25f), 1);
      rrect(x0, y0, x1 - x0, y1 - y0 - in, r, t->wall, 1);
      rrect(x0 + in, y0 + in, x1 - x0 - 2 * in, (y1 - y0) * 0.45f, r * 0.8f, t->wall_top, 0.55f);
    }
  fb = keep;
  A.under_ok = 1;
}

static void apple(float x, float y, float r, float alpha) {
  const Theme *t = TH;
  if (r <= 0) return;
  disc(x, y + r * 0.12f, r, RGB(0, 0, 0), alpha * 0.15f); /* shadow */
  disc(x - r * 0.3f, y, r * 0.78f, t->apple, alpha);
  disc(x + r * 0.3f, y, r * 0.78f, t->apple, alpha);
  disc(x, y + r * 0.12f, r * 0.82f, t->apple, alpha);
  disc(x - r * 0.38f, y - r * 0.28f, r * 0.2f, t->apple_hi, alpha * 0.8f);
  capsule(x, y - r * 0.6f, x + r * 0.08f, y - r * 1.0f, r * 0.09f, t->stem, alpha);
  capsule(x + r * 0.18f, y - r * 0.82f, x + r * 0.52f, y - r * 1.0f, r * 0.16f, t->leaf, alpha);
}

/* the other fruits keep their own colours, except on the LCD theme */
#define T_LCD 1
static u32 fruit_c(u32 c, u32 lcd) { return S.opt[O_THEME] == T_LCD ? lcd : c; }

/* each fruit's main colour, for the sparks it leaves */
static u32 fruit_colour(i32 kind) {
  static const u32 C[NFRUIT] = {0, RGB(255, 146, 28), RGB(206, 22, 52), RGB(132, 62, 178), RGB(236, 40, 62), RGB(250, 212, 46)};
  return kind == APPLE ? TH->apple_hi : fruit_c(C[kind], TH->apple_hi);
}

static void fruit(i32 kind, float x, float y, float r, float alpha) {
  const Theme *t = TH;
  if (r <= 0) return;
  if (kind == APPLE) return apple(x, y, r, alpha);
  if (kind == CHERRIES || kind == GRAPES || kind == STRAWBERRY) r *= 1.2f; /* smaller shapes, drawn up to the apple's size */
  u32 hi = fruit_c(RGB(255, 255, 255), t->apple_hi), leaf = t->leaf, stem = t->stem;
  if (kind == ORANGE) {
    u32 c = fruit_c(RGB(255, 146, 28), t->apple);
    disc(x, y + r * 0.17f, r * 0.86f, RGB(0, 0, 0), alpha * 0.15f); /* shadow */
    disc(x, y + r * 0.05f, r * 0.86f, c, alpha);
    disc(x - r * 0.32f, y - r * 0.22f, r * 0.2f, hi, alpha * 0.5f);
    disc(x, y - r * 0.74f, r * 0.12f, stem, alpha);
    capsule(x + r * 0.08f, y - r * 0.8f, x + r * 0.46f, y - r * 0.98f, r * 0.15f, leaf, alpha);
  } else if (kind == CHERRIES) {
    u32 c = fruit_c(RGB(206, 22, 52), t->apple);
    float tx = x + r * 0.12f, ty = y - r * 0.92f;
    capsule(x - r * 0.38f, y + r * 0.2f, tx, ty, r * 0.07f, stem, alpha);
    capsule(x + r * 0.42f, y + r * 0.08f, tx, ty, r * 0.07f, stem, alpha);
    capsule(tx, ty, tx + r * 0.42f, ty - r * 0.08f, r * 0.14f, leaf, alpha);
    disc(x - r * 0.38f, y + r * 0.42f, r * 0.46f, c, alpha);
    disc(x + r * 0.42f, y + r * 0.3f, r * 0.46f, c, alpha);
    disc(x - r * 0.52f, y + r * 0.28f, r * 0.12f, hi, alpha * 0.6f);
    disc(x + r * 0.28f, y + r * 0.16f, r * 0.12f, hi, alpha * 0.6f);
  } else if (kind == GRAPES) {
    u32 c = fruit_c(RGB(132, 62, 178), t->apple), c2 = fruit_c(RGB(158, 92, 204), t->apple);
    static const float G[6][2] = {{-0.46f, -0.3f}, {0, -0.3f}, {0.46f, -0.3f}, {-0.23f, 0.1f}, {0.23f, 0.1f}, {0, 0.5f}};
    capsule(x, y - r * 0.55f, x + r * 0.1f, y - r * 0.95f, r * 0.08f, stem, alpha);
    capsule(x + r * 0.1f, y - r * 0.8f, x + r * 0.5f, y - r * 0.9f, r * 0.14f, leaf, alpha);
    for (i32 i = 0; i < 6; i++) {
      float gx = x + G[i][0] * r, gy = y + G[i][1] * r;
      disc(gx, gy, r * 0.3f, i < 3 ? c : c2, alpha);
      disc(gx - r * 0.09f, gy - r * 0.09f, r * 0.07f, hi, alpha * 0.55f);
    }
  } else if (kind == STRAWBERRY) {
    u32 c = fruit_c(RGB(236, 40, 62), t->apple), seed = fruit_c(RGB(255, 222, 120), t->apple_hi);
    capsule_grad(x, y - r * 0.18f, x, y + r * 0.62f, r * 0.72f, r * 0.24f, c, c, alpha);
    static const float P[6][2] = {{-0.35f, -0.15f}, {0.05f, -0.2f}, {0.4f, -0.1f}, {-0.2f, 0.2f}, {0.2f, 0.22f}, {0, 0.5f}};
    for (i32 i = 0; i < 6; i++) disc(x + P[i][0] * r, y + P[i][1] * r, r * 0.06f, seed, alpha);
    for (i32 i = -1; i <= 1; i++)
      capsule(x, y - r * 0.75f, x + i * r * 0.42f, y - r * (0.62f - (i ? 0 : 0.2f)), r * 0.13f, leaf, alpha);
  } else { /* LEMON */
    u32 c = fruit_c(RGB(250, 212, 46), t->apple);
    capsule(x - r * 0.42f, y + r * 0.17f, x + r * 0.42f, y + r * 0.17f, r * 0.62f, RGB(0, 0, 0), alpha * 0.15f);
    capsule(x - r * 0.38f, y + r * 0.05f, x + r * 0.38f, y + r * 0.05f, r * 0.6f, c, alpha);
    capsule(x - r * 0.95f, y + r * 0.05f, x - r * 0.6f, y + r * 0.05f, r * 0.2f, c, alpha);
    capsule(x + r * 0.6f, y + r * 0.05f, x + r * 0.95f, y + r * 0.05f, r * 0.2f, c, alpha);
    capsule(x - r * 0.45f, y - r * 0.28f, x + r * 0.05f, y - r * 0.32f, r * 0.08f, hi, alpha * 0.6f);
    capsule(x + r * 0.1f, y - r * 0.55f, x + r * 0.42f, y - r * 0.82f, r * 0.14f, leaf, alpha);
  }
}

static void trophy(float x, float y, float s, u32 c) {
  rrect(x - s * 0.5f, y - s * 0.55f, s, s * 0.62f, s * 0.3f, c, 1);
  capsule(x - s * 0.5f, y - s * 0.38f, x - s * 0.62f, y - s * 0.2f, s * 0.08f, c, 1);
  capsule(x + s * 0.5f, y - s * 0.38f, x + s * 0.62f, y - s * 0.2f, s * 0.08f, c, 1);
  rrect(x - s * 0.08f, y, s * 0.16f, s * 0.25f, 0, c, 1);
  rrect(x - s * 0.32f, y + s * 0.22f, s * 0.64f, s * 0.16f, s * 0.05f, c, 1);
}

static void hud(void) {
  const Theme *t = TH;
  char buf[12];
  /* the page puts a pause button in the top left corner in full screen */
  float cy = L.bar / 2, s = L.bar * 0.42f, x = maxf(L.fx, L.bar * 1.9f) + s * 0.6f;
  apple(x, cy + s * 0.08f, s * 0.48f, 1);
  text_l(itoa10(S.score, buf), x + s * 0.8f, cy, s * 1.05f, t->text, 1);
  float bx = x + s * 0.8f + text_width(buf, s * 1.05f) + s * 1.3f;
  trophy(bx, cy - s * 0.05f, s * 0.8f, t->gold);
  text_l(itoa10(*best(), buf), bx + s * 0.75f, cy, s * 1.05f, t->text, 1);
}

/* a body square's centre in screen pixels */
static void at(i32 k, float *x, float *y) {
  *x = cx_of(S.bx[k]);
  *y = cy_of(S.by[k]);
}

/* direction from square a of the body to the next square b, or -1 */
static i32 dir_between(i32 a, i32 b) {
  i32 dx = S.bx[b] - S.bx[a], dy = S.by[b] - S.by[a];
  for (i32 i = 0; i < 4; i++)
    if (DX[i] == dx && DY[i] == dy) return i;
  return -1;
}

/* A point on the body's path through a square centred at (x, y), entered going
 * din and left going dout, s from 0 (entry edge) to 1 (exit edge): a straight
 * line, or round the corner between them. bend 1 takes a quarter circle,
 * 0 carries straight on, for a head that turned after it set off. */
static void section_point(float x, float y, i32 din, i32 dout, float s, float bend, float *px, float *py) {
  float h = L.cs * 0.5f;
  if (din == dout) {
    *px = x + DX[din] * h * (2 * s - 1);
    *py = y + DY[din] * h * (2 * s - 1);
    return;
  }
  float ccx = x + (DX[dout] - DX[din]) * h, ccy = y + (DY[dout] - DY[din]) * h;
  float th = s * 1.5707963f, co = sinf_(th + 1.5707963f), si = sinf_(th);
  float ax = ccx + h * (-DX[dout] * co + DX[din] * si), ay = ccy + h * (-DY[dout] * co + DY[din] * si);
  float lx = x + DX[din] * h * (2 * s - 1), ly = y + DY[din] * h * (2 * s - 1);
  *px = lx + (ax - lx) * bend;
  *py = ly + (ay - ly) * bend;
}

/* A quarter circle of radius h round (cx, cy), from cx + h * u0 to
 * cx + h * u1 (two directions at right angles), r thick with round ends:
 * a body square the snake turns in, drawn in one go. */
static void quarter(float cx, float cy, i32 u0, i32 u1, float h, float r, u32 c) {
  float ux0 = (float)DX[u0], uy0 = (float)DY[u0], ux1 = (float)DX[u1], uy1 = (float)DY[u1];
  float fx = cx + (ux0 + ux1) * h, fy = cy + (uy0 + uy1) * h; /* the far corner */
  i32 x0 = floori(minf(cx, fx) - r - 1), x1 = ceili(maxf(cx, fx) + r + 1);
  i32 y0 = floori(minf(cy, fy) - r - 1), y1 = ceili(maxf(cy, fy) + r + 1);
  if (x0 < 0) x0 = 0;
  if (y0 < 0) y0 = 0;
  if (x1 > FW) x1 = FW;
  if (y1 > FH) y1 = FH;
  float ex0 = cx + ux0 * h, ey0 = cy + uy0 * h, ex1 = cx + ux1 * h, ey1 = cy + uy1 * h;
  for (i32 j = y0; j < y1; j++) {
    float vy = j + 0.5f - cy;
    u32 *row = fb + j * FW;
    for (i32 i = x0; i < x1; i++) {
      float vx = i + 0.5f - cx, d;
      if (vx * ux0 + vy * uy0 >= 0 && vx * ux1 + vy * uy1 >= 0) {
        d = absf(sqrtf_(vx * vx + vy * vy) - h); /* beside the arc */
      } else {
        /* past one end: the distance to the nearer end */
        float ax = vx + cx - ex0, ay = vy + cy - ey0, bx = vx + cx - ex1, by = vy + cy - ey1;
        d = sqrtf_(minf(ax * ax + ay * ay, bx * bx + by * by));
      }
      float cov = clampf(r + 0.5f - d, 0, 1);
      if (cov > 0) blend(row + i, c, (u32)(cov * 256));
    }
  }
}

#define MAXP 64
static struct { float x, y, t; } P[MAXP];
static i32 np;

/* draws the run of body points in P, narrowing where t < 1, and empties it */
static void flush_points(float r, u32 c) {
  for (i32 j = 0; j < np; j++) {
    float w = r * (0.65f + 0.35f * clampf(P[j].t, 0, 1));
    if (np == 1) disc(P[j].x, P[j].y, w, c, 1);
    if (j == 0) continue;
    float w0 = r * (0.65f + 0.35f * clampf(P[j - 1].t, 0, 1));
    capsule_grad(P[j - 1].x, P[j - 1].y, P[j].x, P[j].y, w0, w, c, c, 1);
  }
  np = 0;
}

static void draw_snake(void) {
  const Theme *t = TH;
  float cs = L.cs, r = cs * 0.4f;
  float u = S.phase == PLAY ? clampf((float)S.acc_ms / STEP_MS[S.opt[O_SPEED]], 0, 1) : 0;
  /* where the head goes next: it glides there unless that square is blocked */
  i32 d = S.dir;
  i32 nx = S.bx[S.head] + DX[d], ny = S.by[S.head] + DY[d];
  i32 open = nx >= 0 && ny >= 0 && nx < S.cols && ny < S.rows && S.map[ny][nx] != WALL;
  i32 eating = open && ny * S.cols + nx == S.food;
  if (open && S.map[ny][nx] == BODY) {
    /* only the tail's square, which is free by the time the head gets there */
    i32 tail = (S.head - S.len + 1 + MAXN) % MAXN;
    open = S.bx[tail] == nx && S.by[tail] == ny;
  }
  float hu = open ? u : 0;
  i32 hurt = S.phase == DYING || S.phase == OVER;

  float shake = 0;
  if (S.phase == DYING) shake = sinf_(S.timer * 0.06f) * cs * 0.06f * (1 - S.timer / 900.0f);
  u32 grey = mixc(RGB(255, 255, 255), t->body, 0.45f);

  /* The body is one path through the squares: straight through a square it
   * crosses, a quarter circle through one where it turns. The tail and head
   * glide along that path, so the snake bends round corners smoothly. One
   * width all along, narrowing over the last square to the tip. The squares
   * between the moving ends are drawn whole: a straight run as one line, a
   * turn as one quarter circle; the ends go through P as short lines. */
  u32 c = t->body;
  if (hurt && (S.phase == OVER || (S.timer / 150) & 1)) c = mixc(grey, c, 0.6f);
  float run_x = 0, run_y = 0, run_ex = 0, run_ey = 0;
  i32 run = 0; /* a straight run is being gathered */
  i32 n = S.len;
  np = 0;
  /* a head turned mid-step curves onto the new way by the time it leaves its square */
  float bend = A.bend_from < 0 ? 1 : clampf((hu - A.bend_from) / (0.5f - A.bend_from), 0, 1);
  float tu = eating || n == 1 ? 0 : hu;
  for (i32 i = n - 1; i >= -1; i--) {
    i32 din, dout;
    float x, y, s0 = 0, s1 = 1;
    if (i >= 0) {
      i32 k = (S.head - i + MAXN) % MAXN;
      at(k, &x, &y);
      dout = i > 0 ? dir_between(k, (k + 1) % MAXN) : open ? d : -1;
      din = i < n - 1 ? dir_between((k - 1 + MAXN) % MAXN, k) : dout;
      if (dout < 0) dout = din;
      if (din < 0) din = dout = d;
    } else {
      /* the square the head is gliding into */
      if (hu <= 0) break;
      at(S.head, &x, &y);
      x += DX[d] * cs;
      y += DY[d] * cs;
      din = d;
      dout = S.nq ? S.queue[0] : d;
    }
    if (i == n - 1) s0 = 0.5f + tu;
    if (i == n - 2 && tu > 0.5f) s0 = tu - 0.5f;
    if (i == n - 1 && tu > 0.5f) continue;
    if (i == 0) s1 = 0.5f + (hu < 0.5f ? hu : 0.5f);
    if (i == -1) s1 = hu - 0.5f;
    if (i == -1 && hu <= 0.5f) break;
    float along = (float)(n - 1 - i) - 0.5f - tu;
    x += shake;
    i32 whole = i > 0 && s0 == 0 && along >= 1; /* a whole square at full width */
    if (run && !(whole && din == dout)) {
      capsule(run_x, run_y, run_ex, run_ey, r, c, 1);
      run = 0;
    }
    if (whole) {
      float h = cs * 0.5f;
      flush_points(r, c);
      if (din != dout) {
        quarter(x + (DX[dout] - DX[din]) * h, y + (DY[dout] - DY[din]) * h, (dout + 2) % 4, din, h, r, c);
      } else {
        if (!run) { run = 1; run_x = x - DX[din] * h; run_y = y - DY[din] * h; }
        run_ex = x + DX[din] * h;
        run_ey = y + DY[din] * h;
      }
      continue;
    }
    i32 steps = din == dout ? 1 : 8;
    for (i32 j = 0; j <= steps; j++) {
      if (j == 0 && np > 0) continue; /* shared with the last section's end */
      float sv = s0 + (s1 - s0) * (float)j / (float)steps;
      if (np < MAXP) {
        section_point(x, y, din, dout, sv, i == 0 ? bend : 1, &P[np].x, &P[np].y);
        P[np].t = along + sv;
        np++;
      }
    }
  }
  /* head and eyes, looking along the path */
  float hx = P[np - 1].x, hy = P[np - 1].y;
  float fx = (float)DX[d], fy = (float)DY[d];
  if (np > 1) {
    float vx = P[np - 1].x - P[np - 2].x, vy = P[np - 1].y - P[np - 2].y, l = vx * vx + vy * vy;
    if (l > 1e-6f) {
      l = 1.0f / sqrtf_(l);
      fx = vx * l;
      fy = vy * l;
    }
  }
  flush_points(r, c);
  if (!open) {
    /* blocked: it faces what it ran into, pressed a little against it */
    fx = (float)DX[d];
    fy = (float)DY[d];
    hx += fx * cs * 0.12f;
    hy += fy * cs * 0.12f;
  }
  float sx = -fy, sy = fx;
  /* a head a little wider than the neck, with the eyes on it */
  disc(hx - fx * r * 0.12f, hy - fy * r * 0.12f, r * 1.1f, c, 1);
  float er = r * 0.42f, ef = r * 0.02f, es = r * 0.55f;
  i32 blink = !hurt && ((i32)(A.now / 100) % 42) == 0;
  for (i32 side = -1; side <= 1; side += 2) {
    float ex = hx + fx * ef + sx * es * side, ey = hy + fy * ef + sy * es * side;
    if (hurt) {
      float k = er * 0.55f;
      capsule(ex - k, ey - k, ex + k, ey + k, er * 0.2f, t->pupil, 1);
      capsule(ex - k, ey + k, ex + k, ey - k, er * 0.2f, t->pupil, 1);
    } else if (blink) {
      capsule(ex - sx * er * 0.8f, ey - sy * er * 0.8f, ex + sx * er * 0.8f, ey + sy * er * 0.8f, er * 0.18f, t->pupil, 1);
    } else {
      disc(ex, ey, er, t->eye, 1);
      disc(ex + fx * er * 0.35f, ey + fy * er * 0.35f, er * 0.55f, t->pupil, 1);
      disc(ex + fx * er * 0.1f - er * 0.18f, ey + fy * er * 0.1f - er * 0.18f, er * 0.17f, t->eye, 0.9f);
    }
  }

  /* before the first move: arrows show the ways it can go */
  if (S.phase == READY) {
    float a = 0.55f + 0.35f * sinf_(A.now * 0.006f), off = cs * 0.95f, k = cs * 0.16f, w = cs * 0.06f;
    for (i32 dd = 0; dd < 4; dd++) {
      if (dd == 2) continue; /* never straight back */
      float ax = hx + DX[dd] * off, ay = hy + DY[dd] * off, px = (float)-DY[dd], py = (float)DX[dd];
      float tipx = ax + DX[dd] * k * 0.6f, tipy = ay + DY[dd] * k * 0.6f;
      capsule(tipx, tipy, ax - DX[dd] * k * 0.4f + px * k, ay - DY[dd] * k * 0.4f + py * k, w, t->text, a);
      capsule(tipx, tipy, ax - DX[dd] * k * 0.4f - px * k, ay - DY[dd] * k * 0.4f - py * k, w, t->text, a);
    }
  }
}

static void draw(void) {
  if (!A.under_ok) draw_under();
  u32 *src = fb_layer(1);
  for (i32 i = 0, e = FW * FH; i < e; i++) fb[i] = src[i];
  hud();
  float cs = L.cs;
  if (S.food >= 0) {
    float g = clampf(A.spawn_t / 220, 0, 1);
    g = g * (2.2f - 1.2f * g); /* overshoots a touch, then settles */
    float bob = 1 + 0.04f * sinf_(A.now * 0.005f);
    fruit(S.fruit, cx_of(S.food % S.cols), cy_of(S.food / S.cols), cs * 0.42f * g * bob, 1);
  }
  if (A.eat_t < 350) {
    /* sparks where the last fruit was */
    float k = A.eat_t / 350, x = cx_of(A.eat_cell % S.cols), y = cy_of(A.eat_cell / S.cols);
    float rr = cs * (0.35f + 0.5f * k), w = cs * 0.06f;
    for (i32 i = 0; i < 8; i++) {
      float a = i * 0.785398f;
      disc(x + sinf_(a + 1.570796f) * rr, y + sinf_(a) * rr, w * (1 - k) + 0.5f, fruit_colour(A.eat_fruit), 1 - k);
    }
  }
  draw_snake();
}

/* ---------- frame ---------- */

WG_EXPORT(wg_frame) i32 wg_frame(i32 buttons, i32 dt_ms) {
  i32 pressed = buttons & ~S.prev_buttons, ev = 0;
  S.prev_buttons = buttons;
  if (dt_ms > 100) dt_ms = 100;
  A.now += (float)dt_ms;
  A.eat_t += (float)dt_ms;
  A.spawn_t += (float)dt_ms;

  if (pressed & B_RIGHT) turn(0);
  if (pressed & B_DOWN) turn(1);
  if (pressed & B_LEFT) turn(2);
  if (pressed & B_UP) turn(3);

  if (S.phase == READY && (pressed & (B_UP | B_DOWN | B_RIGHT | B_LEFT | B_A))) {
    /* the first move starts the run; backing into the tail just goes forward */
    S.phase = PLAY;
    S.acc_ms = 0;
  }
  if (S.phase == PLAY) {
    S.acc_ms += dt_ms;
    S.time_ms += dt_ms;
    while (S.phase == PLAY && S.acc_ms >= STEP_MS[S.opt[O_SPEED]]) {
      S.acc_ms -= STEP_MS[S.opt[O_SPEED]];
      ev |= step();
    }
  } else if (S.phase == DYING) {
    S.timer += dt_ms;
    if (S.timer >= 900) { S.phase = OVER; ev |= WG_EV_GAMEOVER; }
  }
  draw();
  return ev;
}

/* the first move starts the run, as a key does in wg_frame */
static void start(void) {
  if (S.phase != READY) return;
  S.phase = PLAY;
  S.acc_ms = 0;
}

static struct { i32 down, moved, touch; float x, y; } T;

/* Touch and mouse: a swipe anywhere turns as soon as its way is clear, and
 * the finger can keep going for the next turn without lifting. A tap on a
 * touch screen starts the run. */
WG_EXPORT(wg_mouse) void wg_mouse(i32 x, i32 y, i32 buttons) {
  float fx = (float)x, fy = (float)y;
  if (x < 0 || y < 0) { T.down = 0; return; }
  if (!T.down) {
    if (buttons & 7) { T.down = 1; T.moved = 0; T.touch = buttons & 8; T.x = fx; T.y = fy; }
    return;
  }
  float dx = fx - T.x, dy = fy - T.y, far = maxf(minf((float)FW, (float)FH) * 0.04f, L.cs * 0.6f);
  if (absf(dx) > far || absf(dy) > far) {
    i32 d = absf(dx) > absf(dy) ? (dx > 0 ? 0 : 2) : (dy > 0 ? 1 : 3);
    turn(d);
    start();
    T.moved = 1;
    T.x = fx;
    T.y = fy;
  }
  if (buttons & 7) return;
  T.down = 0;
  if (!T.moved && T.touch) start();
}

/* the catalog's stats: Length, Time, Turns, Squares */
WG_EXPORT(wg_stat) i32 wg_stat(i32 i) {
  i32 v[] = {S.len, S.time_ms, S.turns, S.steps};
  return i >= 0 && i < 4 ? v[i] : -1;
}

WG_EXPORT(wg_state) void *wg_state(void) { return &S; }
WG_EXPORT(wg_state_size) i32 wg_state_size(void) { return sizeof S; }
WG_EXPORT(wg_score) i32 wg_score(void) { return S.score; }

/* everything the game indexes with, so a bad save can't reach past a table */
static i32 save_ok(void) {
  if (S.magic != MAGIC || S.version != 3 || !wg_picks_ok(S.opt, CHOICES, NOPT)) return 0;
  if (S.cols != COLS[S.opt[O_MAP]] || S.rows != ROWS[S.opt[O_MAP]]) return 0;
  if (S.phase < READY || S.phase > OVER || S.dir < 0 || S.dir > 3 || S.nq < 0 || S.nq > 2) return 0;
  for (i32 i = 0; i < S.nq; i++)
    if (S.queue[i] < 0 || S.queue[i] > 3) return 0;
  if (S.len < 1 || S.len > S.cols * S.rows || S.head < 0 || S.head >= MAXN) return 0;
  if (S.food < -1 || S.food >= S.cols * S.rows || S.score < 0 || S.fruit < 0 || S.fruit >= NFRUIT) return 0;
  for (i32 i = 0; i < S.len; i++) {
    i32 k = (S.head - i + MAXN) % MAXN;
    if (S.bx[k] >= S.cols || S.by[k] >= S.rows) return 0;
  }
  for (i32 y = 0; y < MAXR; y++)
    for (i32 x = 0; x < MAXC; x++)
      if (S.map[y][x] > WALL) return 0;
  return 1;
}

/* called after the player copied a save into wg_state(); returns 1 if valid */
WG_EXPORT(wg_state_loaded) i32 wg_state_loaded(void) {
  if (!save_ok()) {
    S.magic = 0;
    wg_init(0x1234);
    return 0;
  }
  S.prev_buttons = 0xFF; /* ignore keys held while loading */
  S.nq = 0;
  A.eat_t = 1e9f;
  A.spawn_t = 1e9f;
  A.bend_from = -1;
  layout();
  return 1;
}
