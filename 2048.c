/* 2048 - a wasmga.me game. MIT licensed.
 * Slide the tiles, join equal numbers, reach 2048.
 *
 * Draws at whatever size the page asks for (wg_resize), so tiles and text
 * stay sharp on any screen. Text comes from signed distance fields of Inter
 * (sdf_font.h). Swipes and taps arrive through wg_mouse. The page shows the
 * setup, pause and game over screens (wg.h), so this file only plays. */
#define WG_OWN_FB
#include "wg.h"
#include "sdf_font.h"
#include "2048.glyphs.h"

#define MAXN 6
#define UNDO_MAX 64
#define MAGIC 0x32303438u /* "2048" */
#define VERSION 4
#define WIN_EXP 11 /* 2^11 = 2048 */
#define TILE_R 0.09f /* tile corner radius, of the tile size */

enum { PLAY, WON, OVER };
/* options, in catalog order: board, undo, theme */
enum { O_BOARD, O_UNDO, O_THEME, NOPT };

typedef struct {
  u8 cell[MAXN * MAXN];
  i32 score;
} Snap;

/* everything a save needs; the animation state below is not saved */
static struct {
  u32 magic, version, rng;
  i32 n, theme, undo_on;
  i32 phase, sel; /* sel: the button picked on the win screen */
  u8 cell[MAXN * MAXN];
  i32 score, best[MAXN + 1];
  i32 kept_going, undos, moves, merges, time_ms;
  i32 hist_n;
  Snap hist[UNDO_MAX];
} S;

/* ---------- framebuffer ---------- */

extern unsigned char __heap_base;
static u32 *fb, *under; /* under: the board without tiles, copied in each frame */
static i32 FW, FH, under_ok;
/* finished tiles by exponent, colour in the low bytes and coverage in the top one */
#define NSPRITES 18
static u32 *sprites;
static i32 sprite_px, sprite_ok[NSPRITES];

static float minf(float a, float b) { return a < b ? a : b; }
static float clampf(float v, float lo, float hi) { return v < lo ? lo : v > hi ? hi : v; }
static float absf(float v) { return v < 0 ? -v : v; }
static i32 floori(float v) { return (i32)__builtin_floorf(v); }
static i32 ceili(float v) { return (i32)__builtin_ceilf(v); }

static void set_size(i32 w, i32 h) {
  if (w < 64) w = 64;
  if (h < 64) h = 64;
  if (w > 4096) w = 4096;
  if (h > 4096) h = 4096;
  unsigned long side = (unsigned long)(w < h ? w : h) / 3 + 2;
  unsigned long need = (unsigned long)&__heap_base + (unsigned long)w * (unsigned long)h * 8 + side * side * NSPRITES * 4;
  unsigned long have = __builtin_wasm_memory_size(0) * 65536ul;
  if (need > have && __builtin_wasm_memory_grow(0, (need - have + 65535) / 65536) == (unsigned long)-1) return;
  fb = (u32 *)&__heap_base;
  under = fb + w * h;
  sprites = under + w * h;
  sprite_px = (i32)side;
  FW = w;
  FH = h;
}

/* blends colour c over the pixel with coverage a (0..256) */
static void blend(u32 *p, u32 c, u32 a) {
  if (a >= 256) { *p = c; return; }
  if (!a) return;
  u32 d = *p, ia = 256 - a;
  u32 rb = (((c & 0xFF00FFu) * a + (d & 0xFF00FFu) * ia) >> 8) & 0xFF00FFu;
  u32 g = (((c & 0x00FF00u) * a + (d & 0x00FF00u) * ia) >> 8) & 0x00FF00u;
  *p = 0xFF000000u | rb | g;
}

static void clear(u32 c) {
  u32 *p = fb, *end = fb + FW * FH;
  while (p < end) *p++ = c;
}

static void span(u32 *row, i32 a, i32 b, u32 c, u32 al) {
  if (al >= 256)
    for (i32 i = a; i < b; i++) row[i] = c;
  else
    for (i32 i = a; i < b; i++) blend(row + i, c, al);
}

/* coverage of a pixel near a rounded box edge; qx, qy: distance past the inner box, r: radius */
static float edge_cov(float qx, float qy, float r) {
  float d = qy <= 0 ? qx - r : qx <= 0 ? qy - r : __builtin_sqrtf(qx * qx + qy * qy) - r;
  return clampf(0.5f - d, 0, 1);
}

/* anti-aliased rounded rectangle; alpha 0..1 */
static void rrect(float x, float y, float w, float h, float r, u32 c, float alpha) {
  if (w <= 0 || h <= 0 || alpha <= 0) return;
  r = minf(r, minf(w, h) / 2);
  float cx = x + w / 2, cy = y + h / 2, hw = w / 2 - r, hh = h / 2 - r;
  i32 x0 = floori(x), x1 = ceili(x + w), y0 = floori(y), y1 = ceili(y + h);
  if (x0 < 0) x0 = 0;
  if (y0 < 0) y0 = 0;
  if (x1 > FW) x1 = FW;
  if (y1 > FH) y1 = FH;
  /* columns between the corners (|x - cx| <= hw), where coverage depends on the row only */
  i32 m0 = ceili(cx - hw - 0.5f), m1 = floori(cx + hw - 0.5f) + 1;
  if (m0 < x0) m0 = x0;
  if (m1 > x1) m1 = x1;
  if (m1 < m0) m1 = m0;
  for (i32 j = y0; j < y1; j++) {
    float qy = absf(j + 0.5f - cy) - hh;
    float rowcov = clampf(r - qy + 0.5f, 0, 1);
    if (rowcov <= 0) continue;
    u32 *row = fb + j * FW;
    span(row, m0, m1, c, (u32)(rowcov * alpha * 256));
    for (i32 i = x0; i < x1; i++) {
      if (i == m0 && m1 > m0) { i = m1 - 1; continue; }
      float cov = minf(edge_cov(absf(i + 0.5f - cx) - hw, qy, r), rowcov);
      if (cov > 0) blend(row + i, c, (u32)(cov * alpha * 256));
    }
  }
}

/* ---------- text ---------- */

/* Inter, and the Chinese glyphs this game draws (2048.glyphs.h) */
static const struct sdf_glyph *glyph(u32 ch, const unsigned char **data) {
  *data = SDF_DATA;
  if (ch < 128) {
    for (i32 i = 0; SDF_CHARS[i]; i++)
      if ((u32)SDF_CHARS[i] == ch) return &SDF_GLYPHS[i];
  } else {
    for (i32 i = 0; i < SDF_EXTRA_N; i++)
      if (SDF_EXTRA_CHARS[i] == ch) {
        *data = SDF_EXTRA_DATA;
        return &SDF_EXTRA_GLYPHS[i];
      }
  }
  return &SDF_GLYPHS[0];
}

static float text_width(const char *s, float size) {
  float k = size / SDF_EM, w = 0;
  const unsigned char *d;
  for (u32 ch; (ch = utf8_next(&s));) w += glyph(ch, &d)->adv * k;
  return w;
}

static void draw_glyph(const struct sdf_glyph *g, const unsigned char *data, float gx, float gy, float k, u32 c, float alpha) {
  const unsigned char *f = data + g->off;
  i32 w = g->w, h = g->h;
  if (!w) return;
  i32 x0 = floori(gx), y0 = floori(gy), x1 = ceili(gx + w * k), y1 = ceili(gy + h * k);
  if (x0 < 0) x0 = 0;
  if (y0 < 0) y0 = 0;
  if (x1 > FW) x1 = FW;
  if (y1 > FH) y1 = FH;
  float inv = 1 / k, scale = SDF_SPREAD / 127.0f * k;
  /* bilinear neighbours differ from the nearest texel by at most 127 / SPREAD * 1.5 */
  float slack = 127 / SDF_SPREAD * 1.5f, lo = 128 - 0.5f / scale - slack, hi = 128 + 0.5f / scale + slack;
  u32 full = (u32)(alpha * 256);
  for (i32 j = y0; j < y1; j++) {
    float fy = (j + 0.5f - gy) * inv - 0.5f;
    i32 iy = floori(fy);
    float ty = fy - iy;
    if (iy < -1 || iy >= h) continue;
    for (i32 i = x0; i < x1; i++) {
      float fx = (i + 0.5f - gx) * inv - 0.5f;
      i32 ix = floori(fx);
      float tx = fx - ix;
      if (ix < -1 || ix >= w) continue;
      /* far from the edge: the nearest texel decides, no filtering needed */
      i32 nx = ix + (tx >= 0.5f), ny = iy + (ty >= 0.5f);
      float near = nx >= 0 && nx < w && ny >= 0 && ny < h ? f[ny * w + nx] : 0;
      if (near < lo) continue;
      if (near > hi) { blend(fb + j * FW + i, c, full); continue; }
      /* bilinear sample, outside the field counts as far outside */
      float s00 = 0, s10 = 0, s01 = 0, s11 = 0;
      if (iy >= 0) {
        if (ix >= 0) s00 = f[iy * w + ix];
        if (ix + 1 < w) s10 = f[iy * w + ix + 1];
      }
      if (iy + 1 < h) {
        if (ix >= 0) s01 = f[(iy + 1) * w + ix];
        if (ix + 1 < w) s11 = f[(iy + 1) * w + ix + 1];
      }
      float v = (s00 * (1 - tx) + s10 * tx) * (1 - ty) + (s01 * (1 - tx) + s11 * tx) * ty;
      float cov = clampf((v - 128) * scale + 0.5f, 0, 1);
      if (cov > 0) blend(fb + j * FW + i, c, (u32)(cov * alpha * 256));
    }
  }
}

/* centre of the digits' height below the line top, in em units */
#define DIGIT_MID 24.6f

/* draws s centred on (cx, cy); size is the em size in pixels */
static void text_c(const char *s, float cx, float cy, float size, u32 c, float alpha) {
  float k = size / SDF_EM, x = cx - text_width(s, size) / 2, top = cy - DIGIT_MID * k;
  const unsigned char *d;
  for (u32 ch; (ch = utf8_next(&s));) {
    const struct sdf_glyph *g = glyph(ch, &d);
    draw_glyph(g, d, x + g->left * k, top + g->top * k, k, c, alpha);
    x += g->adv * k;
  }
}

/* text fitted inside a width: shrinks the size if needed */
static void text_fit(const char *s, float cx, float cy, float size, float maxw, u32 c, float alpha) {
  float w = text_width(s, size);
  if (w > maxw) size *= maxw / w;
  text_c(s, cx, cy, size, c, alpha);
}

/* ---------- themes ---------- */

#define C(r, g, b) RGB(r, g, b)
typedef struct {
  u32 bg, board, cell, panel, label, value, button, button_text, dark, light;
  i32 light_from; /* first exponent drawn with light text */
  u32 tile[13];   /* 2 .. 4096, then everything bigger */
} Theme;

static const Theme THEMES[] = {
  /* Sand */
  {C(246, 241, 232), C(188, 172, 155), C(208, 195, 179), C(160, 140, 120), C(240, 230, 214), C(255, 255, 255),
   C(124, 104, 86), C(255, 250, 240), C(112, 98, 84), C(255, 250, 242), 3,
   {C(240, 232, 220), C(237, 223, 197), C(242, 175, 121), C(245, 148, 99), C(246, 122, 95), C(246, 92, 59),
    C(237, 206, 114), C(237, 203, 97), C(237, 199, 80), C(237, 196, 63), C(237, 193, 46), C(62, 57, 51), C(36, 33, 30)}},
  /* Night */
  {C(16, 18, 30), C(32, 36, 56), C(44, 49, 74), C(32, 36, 56), C(150, 156, 196), C(240, 242, 255),
   C(98, 90, 200), C(240, 242, 255), C(20, 22, 40), C(240, 242, 255), 1,
   {C(64, 72, 112), C(78, 88, 144), C(96, 88, 198), C(124, 82, 222), C(160, 78, 220), C(200, 70, 196),
    C(232, 72, 150), C(246, 96, 110), C(250, 132, 80), C(250, 174, 58), C(252, 212, 70), C(236, 238, 250),
    C(255, 255, 255)}},
  /* Sea */
  {C(234, 243, 246), C(140, 174, 192), C(168, 198, 211), C(140, 174, 192), C(222, 238, 244), C(255, 255, 255),
   C(36, 104, 140), C(255, 255, 255), C(40, 70, 90), C(255, 255, 255), 3,
   {C(226, 240, 244), C(198, 229, 236), C(120, 206, 204), C(64, 186, 188), C(38, 160, 178), C(30, 128, 168),
    C(38, 96, 160), C(60, 72, 152), C(92, 60, 150), C(132, 58, 140), C(240, 178, 48), C(26, 36, 56),
    C(10, 16, 28)}},
  /* Mono */
  {C(250, 250, 250), C(34, 34, 34), C(60, 60, 60), C(34, 34, 34), C(170, 170, 170), C(255, 255, 255),
   C(34, 34, 34), C(255, 255, 255), C(24, 24, 24), C(255, 255, 255), 6,
   {C(250, 250, 250), C(226, 226, 226), C(204, 204, 204), C(182, 182, 182), C(160, 160, 160), C(138, 138, 138),
    C(112, 112, 112), C(92, 92, 92), C(255, 214, 0), C(255, 150, 0), C(255, 72, 48), C(0, 0, 0), C(0, 0, 0)}},
};
/* how much the board fades behind a message */
#define VEIL 0.88f
#define NTHEMES (i32)(sizeof THEMES / sizeof THEMES[0])

/* ---------- game logic ---------- */

static i32 pending; /* events from taps and swipes, returned by the next wg_frame */

/* animation of the last move, not part of the save */
#define SLIDE_MS 110.0f
#define POP_MS 150.0f
static struct {
  i32 n;               /* sprites sliding */
  u8 fr[MAXN * MAXN], to[MAXN * MAXN], v[MAXN * MAXN];
  u8 merged[MAXN * MAXN], born[MAXN * MAXN];
  float t;             /* ms since the move */
  float overlay_t;     /* ms since an overlay appeared */
  i32 gain;            /* points from the last move */
  float gain_t;
  i32 dirty;
  i32 over_due;        /* the game ended; tell the page after the animation */
} A;

static void anim_done(void) {
  A.n = 0;
  A.t = 1e6f;
  memset(A.merged, 0, sizeof A.merged);
  memset(A.born, 0, sizeof A.born);
}

static i32 empty_count(void) {
  i32 e = 0;
  for (i32 i = 0; i < S.n * S.n; i++) e += !S.cell[i];
  return e;
}

static void spawn(void) {
  i32 e = empty_count();
  if (!e) return;
  i32 k = (i32)(xorshift(&S.rng) % (u32)e);
  for (i32 i = 0; i < S.n * S.n; i++) {
    if (S.cell[i]) continue;
    if (!k--) {
      S.cell[i] = xorshift(&S.rng) % 10 ? 1 : 2;
      A.born[i] = 1;
      return;
    }
  }
}

static i32 can_move(void) {
  i32 n = S.n;
  for (i32 r = 0; r < n; r++)
    for (i32 c = 0; c < n; c++) {
      u8 v = S.cell[r * n + c];
      if (!v) return 1;
      if (c + 1 < n && S.cell[r * n + c + 1] == v) return 1;
      if (r + 1 < n && S.cell[(r + 1) * n + c] == v) return 1;
    }
  return 0;
}

static void new_game(void) {
  memset(S.cell, 0, sizeof S.cell);
  S.score = 0;
  S.kept_going = 0;
  S.undos = 0;
  S.moves = 0;
  S.merges = 0;
  S.time_ms = 0;
  S.hist_n = 0;
  S.phase = PLAY;
  anim_done();
  spawn();
  spawn();
  A.t = SLIDE_MS; /* the first two tiles grow in */
  A.overlay_t = 0;
  A.gain = 0;
  A.dirty = 1;
}

/* dir: 0 up, 1 right, 2 down, 3 left */
static i32 slide(i32 dir) {
  i32 n = S.n, ev = 0, moved = 0, gain = 0;
  Snap before;
  memcpy(before.cell, S.cell, sizeof S.cell);
  before.score = S.score;
  anim_done();
  for (i32 line = 0; line < n; line++) {
    i32 pos[MAXN];
    for (i32 k = 0; k < n; k++) {
      /* k = 0 is the cell at the edge the tiles move towards */
      i32 r, c;
      if (dir == 0) { r = k; c = line; }
      else if (dir == 2) { r = n - 1 - k; c = line; }
      else if (dir == 3) { r = line; c = k; }
      else { r = line; c = n - 1 - k; }
      pos[k] = r * n + c;
    }
    u8 out[MAXN] = {0}, joined[MAXN] = {0};
    i32 dst = 0;
    for (i32 k = 0; k < n; k++) {
      u8 v = S.cell[pos[k]];
      if (!v) continue;
      i32 to;
      if (dst > 0 && out[dst - 1] == v && !joined[dst - 1]) {
        to = dst - 1;
        out[to] = (u8)(v + 1);
        joined[to] = 1;
        gain += 1 << (v + 1);
        S.merges++;
        A.merged[pos[to]] = 1;
        moved = 1;
      } else {
        to = dst++;
        out[to] = v;
        if (to != k) moved = 1;
      }
      A.fr[A.n] = (u8)pos[k];
      A.to[A.n] = (u8)pos[to];
      A.v[A.n] = v;
      A.n++;
    }
    for (i32 k = 0; k < n; k++) S.cell[pos[k]] = out[k];
  }
  if (!moved) {
    anim_done();
    return 0;
  }
  if (S.undo_on) {
    if (S.hist_n == UNDO_MAX) {
      for (i32 i = 1; i < UNDO_MAX; i++) S.hist[i - 1] = S.hist[i];
      S.hist_n--;
    }
    S.hist[S.hist_n++] = before;
  }
  S.score += gain;
  S.moves++;
  if (S.score > S.best[n]) S.best[n] = S.score;
  A.t = 0;
  A.dirty = 1;
  if (gain) {
    A.gain = gain;
    A.gain_t = 0;
    ev |= WG_EV_HIT;
  } else {
    ev |= WG_EV_BLIP;
  }
  spawn();
  if (!S.kept_going) {
    for (i32 i = 0; i < n * n; i++)
      if (S.cell[i] >= WIN_EXP) {
        S.phase = WON;
        S.sel = 0;
        A.overlay_t = 0;
        ev |= WG_EV_WIN;
        break;
      }
  }
  if (S.phase == PLAY && !can_move()) {
    S.phase = OVER;
    A.overlay_t = 0;
    A.over_due = 1; /* reported once the last move has landed */
  }
  return ev;
}

static i32 undo(void) {
  if (!S.undo_on || !S.hist_n) return 0;
  Snap *s = &S.hist[--S.hist_n];
  memcpy(S.cell, s->cell, sizeof S.cell);
  S.score = s->score;
  S.undos++;
  if (S.phase == OVER || S.phase == WON) S.phase = PLAY;
  anim_done();
  A.gain = 0;
  A.dirty = 1;
  return WG_EV_BLIP;
}

static void keep_going(void) {
  S.kept_going = 1;
  S.phase = PLAY;
  A.dirty = 1;
}

static void layout(void);

/* ---------- layout ---------- */

static struct {
  float bx, by, bs;          /* board */
  float gap, cs, rad;        /* cell gap, cell size, tile radius */
  float px, py, pw, ph;      /* score and best boxes, first of them */
  float pdx, pdy;            /* offset to the second box */
  float ux, uy, uw, uh;      /* undo button */
} L;

static void layout(void) {
  float w = (float)FW, h = (float)FH, m = minf(w, h) * 0.04f;
  if (w > h * 1.25f) {
    /* wide: score column left of the board */
    float bs = minf(h - 2 * m, (w - 3 * m) * 0.75f), pw = minf(bs * 0.3f, w - bs - 3 * m);
    float total = pw + m + bs;
    L.bs = bs;
    L.bx = (w - total) / 2 + pw + m;
    L.by = (h - bs) / 2;
    L.pw = pw;
    L.ph = bs * 0.16f;
    L.px = L.bx - m - pw;
    L.py = L.by;
    L.pdx = 0;
    L.pdy = L.ph + m * 0.6f;
    L.uw = pw;
    L.uh = bs * 0.1f;
    L.ux = L.px;
    L.uy = L.py + 2 * L.pdy;
  } else {
    /* tall: scores above the board */
    float bs = minf(w - 2 * m, (h - 2.5f * m) / 1.19f), head = bs * 0.19f;
    L.bs = bs;
    L.bx = (w - bs) / 2;
    L.by = (h - bs - head - m * 0.5f) / 2 + head + m * 0.5f;
    L.ph = head;
    L.pw = bs * 0.28f;
    L.px = L.bx;
    L.py = L.by - head - m * 0.5f;
    L.pdx = L.pw + m * 0.5f;
    L.pdy = 0;
    L.uw = bs * 0.24f;
    L.uh = head * 0.62f;
    L.ux = L.bx + bs - L.uw;
    L.uy = L.py + (head - L.uh) / 2;
  }
  /* whole pixels for the cells, so tiles drawn from the cache line up */
  float gap = (float)(i32)(L.bs * (S.n <= 4 ? 0.03f : 0.022f) + 0.5f);
  if (gap < 2) gap = 2;
  float cs = (float)(i32)((L.bs - gap * (S.n + 1)) / S.n), bs = cs * S.n + gap * (S.n + 1);
  L.bx = (float)(i32)(L.bx + (L.bs - bs) / 2 + 0.5f);
  L.by = (float)(i32)(L.by + (L.bs - bs) / 2 + 0.5f);
  L.bs = bs;
  L.gap = gap;
  L.cs = cs;
  L.rad = L.cs * TILE_R;
  A.dirty = 1;
  under_ok = 0;
  for (i32 i = 0; i < NSPRITES; i++) sprite_ok[i] = 0;
}

/* ---------- buttons (rebuilt every draw, used by taps) ---------- */

enum { ACT_NONE, ACT_UNDO, ACT_KEEP, ACT_FINISH };
#define MAXBTN 12
static struct { float x, y, w, h; i32 act; } BTN[MAXBTN];
static i32 nbtn;

static void hit(float x, float y, float w, float h, i32 act) {
  if (nbtn < MAXBTN) {
    BTN[nbtn].x = x; BTN[nbtn].y = y; BTN[nbtn].w = w; BTN[nbtn].h = h; BTN[nbtn].act = act;
    nbtn++;
  }
}

static void button(float x, float y, float w, float h, const char *label, i32 act, i32 primary, float alpha) {
  const Theme *t = &THEMES[S.theme];
  rrect(x, y, w, h, h * 0.22f, primary ? t->button : t->panel, alpha);
  text_fit(label, x + w / 2, y + h / 2, h * 0.42f, w * 0.84f, primary ? t->button_text : t->value, alpha);
  hit(x, y, w, h, act);
}

/* ---------- drawing ---------- */

static float ease_out(float u) {
  u = clampf(u, 0, 1);
  return 1 - (1 - u) * (1 - u) * (1 - u);
}

static void cell_xy(i32 i, float *x, float *y) {
  *x = L.bx + L.gap + (i % S.n) * (L.cs + L.gap);
  *y = L.by + L.gap + (i / S.n) * (L.cs + L.gap);
}

static void tile_at(float x, float y, float cs, i32 v, float scale, float alpha) {
  const Theme *t = &THEMES[S.theme];
  i32 k = v - 1 > 12 ? 12 : v - 1;
  float s = cs * scale, o = (cs - s) / 2;
  rrect(x + o, y + o, s, s, cs * TILE_R * scale, t->tile[k], alpha);
  char buf[16];
  if (v > 30) { buf[0] = '2'; buf[1] = '^'; itoa10(v, buf + 2); }
  else itoa10(1 << v, buf);
  i32 digits = 0;
  while (buf[digits]) digits++;
  float size = s * (digits <= 2 ? 0.52f : digits == 3 ? 0.44f : digits == 4 ? 0.36f : digits == 5 ? 0.30f : 0.25f);
  text_fit(buf, x + cs / 2, y + cs / 2, size, s * 0.86f, v >= t->light_from ? t->light : t->dark, alpha);
}


/* renders a tile once into the cache: colour first, then coverage into the top byte */
static void sprite_make(i32 v) {
  i32 sz = (i32)L.cs;
  u32 *spr = sprites + v * sprite_px * sprite_px, *keep = fb;
  i32 kw = FW, kh = FH;
  i32 k = v - 1 > 12 ? 12 : v - 1;
  fb = spr; FW = sz; FH = sz;
  clear(THEMES[S.theme].tile[k]);
  tile_at(0, 0, L.cs, v, 1, 1);
  /* coverage of the rounded corners, the same as rrect's */
  float r = L.rad, c0 = sz / 2.0f, hw = c0 - r;
  for (i32 j = 0; j < sz; j++)
    for (i32 i = 0; i < sz; i++) {
      float qx = absf(i + 0.5f - c0) - hw, qy = absf(j + 0.5f - c0) - hw;
      if (qx <= 0 && qy <= 0) continue;
      u32 a = (u32)(edge_cov(qx, qy, r) * 255);
      spr[j * sz + i] = (spr[j * sz + i] & 0xFFFFFFu) | a << 24;
    }
  fb = keep; FW = kw; FH = kh;
  sprite_ok[v] = 1;
}

static void tile(float x, float y, i32 v, float scale, float alpha) {
  if (scale != 1 || alpha != 1 || v >= NSPRITES || (i32)L.cs > sprite_px) {
    tile_at(x, y, L.cs, v, scale, alpha);
    return;
  }
  if (!sprite_ok[v]) sprite_make(v);
  i32 sz = (i32)L.cs, x0 = floori(x + 0.5f), y0 = floori(y + 0.5f);
  /* the part of the sprite on screen */
  i32 i0 = x0 < 0 ? -x0 : 0, i1 = x0 + sz > FW ? FW - x0 : sz;
  i32 j0 = y0 < 0 ? -y0 : 0, j1 = y0 + sz > FH ? FH - y0 : sz;
  const u32 *spr = sprites + v * sprite_px * sprite_px;
  for (i32 j = j0; j < j1; j++) {
    u32 *row = fb + (y0 + j) * FW + x0;
    const u32 *src = spr + j * sz;
    for (i32 i = i0; i < i1; i++) {
      u32 p = src[i], a = p >> 24;
      if (a == 255) row[i] = p;
      else if (a) blend(row + i, p | 0xFF000000u, a + (a >> 7));
    }
  }
}

static void score_box(float x, float y, const char *label, i32 value) {
  const Theme *t = &THEMES[S.theme];
  char buf[16];
  rrect(x, y, L.pw, L.ph, L.ph * 0.16f, t->panel, 1);
  text_fit(label, x + L.pw / 2, y + L.ph * 0.3f, L.ph * 0.2f, L.pw * 0.86f, t->label, 1);
  text_fit(itoa10(value, buf), x + L.pw / 2, y + L.ph * 0.66f, L.ph * 0.36f, L.pw * 0.86f, t->value, 1);
}

static u32 title_colour(void) {
  const Theme *t = &THEMES[S.theme];
  return S.theme == 1 ? t->light : t->dark;
}

static void draw(void) {
  const Theme *t = &THEMES[S.theme];
  i32 n = S.n;
  nbtn = 0;
  if (!under_ok) {
    u32 *keep = fb;
    fb = under;
    clear(t->bg);
    rrect(L.bx, L.by, L.bs, L.bs, L.rad * 1.4f, t->board, 1);
    for (i32 i = 0; i < n * n; i++) {
      float x, y;
      cell_xy(i, &x, &y);
      rrect(x, y, L.cs, L.cs, L.rad, t->cell, 1);
    }
    fb = keep;
    under_ok = 1;
  }
  for (i32 i = 0, e = FW * FH; i < e; i++) fb[i] = under[i];

  score_box(L.px, L.py, tr("Score", "得分", "スコア", "점수", "Puntos", "Pontos", "Score", "Punkte", "Очки"), S.score);
  score_box(L.px + L.pdx, L.py + L.pdy, tr("Best", "最高分", "ベスト", "최고", "Récord", "Recorde", "Record", "Rekord", "Рекорд"), S.best[n]);
  if (A.gain && A.gain_t < 700) {
    /* the points of the last move float up beside the score */
    char buf[16];
    buf[0] = '+';
    itoa10(A.gain, buf + 1);
    float u = A.gain_t / 700, gx, gy;
    if (L.pdy) { gx = L.px + L.pw / 2; gy = L.uy + L.uh + L.ph * 0.5f; }
    else { gx = (L.px + L.pdx + L.pw + L.ux) / 2; gy = L.py + L.ph * 0.62f; }
    text_c(buf, gx, gy - L.ph * 0.45f * ease_out(u), L.ph * 0.3f, t->button, 1 - u * u);
  }
  if (S.undo_on) {
    float a = S.hist_n ? 1 : 0.45f;
    button(L.ux, L.uy, L.uw, L.uh, tr("Undo", "撤销", "元に戻す", "되돌리기", "Deshacer", "Desfazer", "Annuler", "Rückgängig", "Отмена"), S.hist_n ? ACT_UNDO : ACT_NONE, 0, a);
  }


  if (A.t < SLIDE_MS && A.n) {
    float u = ease_out(A.t / SLIDE_MS);
    for (i32 k = 0; k < A.n; k++) {
      float x0, y0, x1, y1;
      cell_xy(A.fr[k], &x0, &y0);
      cell_xy(A.to[k], &x1, &y1);
      tile(x0 + (x1 - x0) * u, y0 + (y1 - y0) * u, A.v[k], 1, 1);
    }
  } else {
    float since = A.t - SLIDE_MS;
    for (i32 i = 0; i < n * n; i++) {
      if (!S.cell[i]) continue;
      float x, y, scale = 1, alpha = 1;
      cell_xy(i, &x, &y);
      if (A.merged[i] && since < POP_MS) {
        float u = since / POP_MS;
        scale = 1 + 0.5f * u * (1 - u); /* peaks at 1.125 */
      } else if (A.born[i] && since < POP_MS) {
        float u = ease_out(since / POP_MS);
        scale = 0.2f + 0.8f * u;
        alpha = u;
      }
      tile(x, y, S.cell[i], scale, alpha);
    }
  }

  if (S.phase == WON) {
    /* the page draws game over and pause; a win asks whether to go on */
    float a = ease_out(A.overlay_t / 250);
    if (a > 0) {
      rrect(L.bx, L.by, L.bs, L.bs, L.rad * 1.4f, t->bg, VEIL * a);
      float cx = L.bx + L.bs / 2, cy = L.by + L.bs * 0.4f, bh = L.bs * 0.12f, bw = L.bs * 0.34f;
      float by = L.by + L.bs * 0.56f;
      text_fit("2048!", cx, cy, L.bs * 0.2f, L.bs * 0.9f, title_colour(), a);
      button(cx - bw - L.bs * 0.02f, by, bw, bh, tr("Keep going", "继续", "続ける", "계속하기", "Seguir", "Continuar", "Continuer", "Weiter", "Дальше"), ACT_KEEP, S.sel == 0, a);
      button(cx + L.bs * 0.02f, by, bw, bh, tr("Finish", "结束", "終了", "끝내기", "Terminar", "Encerrar", "Terminer", "Beenden", "Закончить"), ACT_FINISH, S.sel == 1, a);
    }
  }
}

static i32 animating(void) {
  return A.t < SLIDE_MS + POP_MS || (A.gain && A.gain_t < 700) ||
         (S.phase == WON && A.overlay_t < 700);
}

/* ---------- input ---------- */

static i32 act(i32 a) {
  switch (a) {
  case ACT_UNDO: return undo();
  case ACT_KEEP: keep_going(); return WG_EV_BLIP;
  /* a won game ends here, with its score, unless the player goes on */
  case ACT_FINISH: S.phase = OVER; A.dirty = 1; return WG_EV_GAMEOVER;
  }
  return 0;
}

static i32 move(i32 dir) {
  if (S.phase != PLAY) return 0;
  return slide(dir);
}

static struct { i32 down, done; float x, y; } T;

/* pointer in framebuffer pixels, (-1, -1) when it leaves; buttons as in wg.h */
WG_EXPORT(wg_mouse) void wg_mouse(i32 x, i32 y, i32 buttons) {
  float fx = (float)x, fy = (float)y;
  i32 held = buttons & 7;
  if (x < 0 || y < 0) { T.down = 0; return; }
  if (!T.down) {
    if (held) { T.down = 1; T.done = 0; T.x = fx; T.y = fy; }
    return;
  }
  float dx = fx - T.x, dy = fy - T.y, far = minf((float)FW, (float)FH) * 0.05f;
  if (!T.done && (absf(dx) > far || absf(dy) > far)) {
    /* swipe: move as soon as it is clear, not on release */
    T.done = 1;
    if (absf(dx) > absf(dy)) pending |= move(dx > 0 ? 1 : 3);
    else pending |= move(dy > 0 ? 2 : 0);
  }
  if (held) return;
  /* released: a tap presses the button under it */
  T.down = 0;
  if (T.done) return;
  for (i32 i = nbtn - 1; i >= 0; i--)
    if (fx >= BTN[i].x && fx < BTN[i].x + BTN[i].w && fy >= BTN[i].y && fy < BTN[i].y + BTN[i].h) {
      pending |= act(BTN[i].act);
      return;
    }
}

/* ---------- exports ---------- */

WG_EXPORT(wg_resize) void wg_resize(i32 w, i32 h) {
  set_size(w, h);
  layout();
}

/* the page may ask for the size before wg_init */
static void ensure_fb(void) {
  if (!fb) set_size(640, 560);
}
WG_EXPORT(wg_fb) u32 *wg_fb(void) { ensure_fb(); return fb; }
WG_EXPORT(wg_w) i32 wg_w(void) { ensure_fb(); return FW; }
WG_EXPORT(wg_h) i32 wg_h(void) { ensure_fb(); return FH; }

static void defaults(void) {
  S.magic = MAGIC;
  S.version = VERSION;
  S.n = 4;
  S.theme = 0;
  S.undo_on = 1;
}

/* starts a run with the options picked */
WG_EXPORT(wg_init) void wg_init(u32 seed) {
  /* options and best scores outlive a restart */
  i32 keep = S.magic == MAGIC;
  i32 n = S.n, theme = S.theme, undo_on = S.undo_on, best[MAXN + 1];
  memcpy(best, S.best, sizeof best);
  memset(&S, 0, sizeof S);
  defaults();
  if (keep) {
    S.n = n; S.theme = theme; S.undo_on = undo_on;
    memcpy(S.best, best, sizeof best);
  }
  S.rng = seed | 1;
  ensure_fb();
  layout();
  new_game();
}

WG_EXPORT(wg_option) i32 wg_option(i32 i, i32 v) {
  if (S.magic != MAGIC) defaults();
  if (i == O_BOARD) {
    if (v >= 0 && v <= MAXN - 3) S.n = 3 + v;
    return S.n - 3;
  }
  if (i == O_UNDO) {
    if (v == 0 || v == 1) S.undo_on = v == 0;
    return !S.undo_on;
  }
  if (i == O_THEME) {
    if (v >= 0 && v < NTHEMES) S.theme = v;
    return S.theme;
  }
  return -1;
}

WG_EXPORT(wg_over) i32 wg_over(void) { return S.phase == OVER; }
/* the page's best on the board these picks play on, so the Best box shows the same as its scores */
WG_EXPORT(wg_best) i32 wg_best(i32 v) {
  if (S.magic != MAGIC) defaults();
  if (v >= 0) { S.best[S.n] = v > S.score ? v : S.score; A.dirty = 1; }
  return S.best[S.n];
}

static i32 prev_buttons, drawn;

WG_EXPORT(wg_drawn) i32 wg_drawn(void) { return drawn; }

WG_EXPORT(wg_frame) i32 wg_frame(i32 buttons, i32 dt_ms) {
  i32 pressed = buttons & ~prev_buttons, ev = pending;
  prev_buttons = buttons;
  pending = 0;
  if (dt_ms > 100) dt_ms = 100;
  if (S.phase == PLAY) S.time_ms += dt_ms;
  if (S.phase == PLAY) {
    if (pressed & B_UP) ev |= move(0);
    else if (pressed & B_RIGHT) ev |= move(1);
    else if (pressed & B_DOWN) ev |= move(2);
    else if (pressed & B_LEFT) ev |= move(3);
    else if (pressed & B_B) ev |= undo();
  } else if (S.phase == WON) {
    if (pressed & (B_LEFT | B_RIGHT)) { S.sel = !S.sel; A.dirty = 1; }
    else if (pressed & B_A) ev |= act(S.sel ? ACT_FINISH : ACT_KEEP);
    else if (pressed & B_B) ev |= undo();
  }
  drawn = animating() || A.dirty || ev;
  if (drawn) {
    A.t += (float)dt_ms;
    A.gain_t += (float)dt_ms;
    A.overlay_t += (float)dt_ms;
    A.dirty = 0;
    draw();
  }
  if (A.over_due && A.t >= SLIDE_MS + POP_MS) {
    A.over_due = 0;
    ev |= WG_EV_LOSE | WG_EV_GAMEOVER;
  }
  return ev;
}

/* the catalog's stats: Moves, Biggest tile, Time, Merges, Undos (hidden with undo off) */
WG_EXPORT(wg_stat) i32 wg_stat(i32 i) {
  i32 top = 0;
  for (i32 k = 0; k < S.n * S.n; k++)
    if (S.cell[k] > top) top = S.cell[k];
  i32 v[] = {S.moves, top ? 1 << top : 0, S.time_ms, S.merges, S.undo_on ? S.undos : -1};
  return i >= 0 && i < 5 ? v[i] : -1;
}

WG_EXPORT(wg_state) void *wg_state(void) { return &S; }
WG_EXPORT(wg_state_size) i32 wg_state_size(void) { return sizeof S; }
WG_EXPORT(wg_score) i32 wg_score(void) { return S.score; }

/* called after the player copied a save into wg_state(); returns 1 if valid */
WG_EXPORT(wg_state_loaded) i32 wg_state_loaded(void) {
  i32 ok = S.magic == MAGIC && S.version == VERSION && S.n >= 3 && S.n <= MAXN && S.theme >= 0 &&
           S.theme < NTHEMES && S.hist_n >= 0 && S.hist_n <= UNDO_MAX && S.phase >= PLAY && S.phase <= OVER && (S.sel == 0 || S.sel == 1);
  for (i32 i = 0; ok && i < MAXN * MAXN; i++) ok = S.cell[i] <= 40;
  if (!ok) {
    S.magic = 0;
    wg_init(0x1234);
    return 0;
  }
  prev_buttons = 0xFF; /* ignore keys held while loading */
  anim_done();
  A.gain = 0;
  A.overlay_t = 1000;
  layout();
  return 1;
}
