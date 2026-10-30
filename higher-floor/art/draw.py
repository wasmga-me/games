#!/usr/bin/env python3
"""Draws Higher Floor's hero, monsters, items and trees as cartoon sprites
with ../../tools/cartoon: python3 draw.py OUT_DIR. The hero faces right,
the monsters left."""
import os
import sys
from contextlib import nullcontext

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../tools/cartoon"))
from cartoon import (GOLD, LINE, OUTLINE, STEEL, WOOD, capsule, circle, coin, ellipse, eye, fmt, mouth, person, poly,  # noqa: E402
                     rrect, smooth, sprite, trees, write)

PPU = 160
BOX = {"w": 200, "h": 200, "ppu": PPU, "oy": 0.82}


def sword(grip):
    def draw(a, hand, _):
        with a.g(f"translate({fmt(hand[0])} {fmt(hand[1])}) rotate({fmt(-grip)})"):
            a.part(rrect(-2.6, 3, 5.2, 30, 2.6), STEEL[0], depth=1.2)
            a.flat(poly((-2.6, 31), (2.6, 31), (0, 37)), STEEL[0], line=LINE)
            a.part(rrect(-7, 0, 14, 4, 2), GOLD[0], depth=1)
            a.part(rrect(-2, -7, 4, 8, 2), WOOD[1], depth=0.8, hi=False)
            a.part(circle(0, -8, 2.6), GOLD[0], depth=0.8, hi=False)
    return draw


def tunic(a):
    a.flat(rrect(-12.5, -21, 26, 4, 2), GOLD[1], line=LINE * 0.7)
    a.flat(circle(1, -19, 2.4), GOLD[0], line=LINE * 0.6)
    a.flat(poly((-9, -38), (10, -38), (1, -31)), GOLD[0], line=LINE * 0.7)


def helmet(a, c, r):
    a.part(smooth((c[0] - r * 1.05, c[1] + 2), (c[0] - r * 0.95, c[1] - r * 0.75), (c[0], c[1] - r * 1.12), (c[0] + r * 0.95, c[1] - r * 0.75),
                  (c[0] + r * 1.05, c[1] - 1), (c[0] + r * 0.4, c[1] - 6), (c[0] - r * 0.5, c[1] - 6)), STEEL[0], depth=2.4)
    a.part(rrect(c[0] - r * 1.08, c[1] - 7.5, r * 2.16, 5, 2.5), GOLD[0], depth=1.2)
    # the plume
    a.part(smooth((c[0] - 2, c[1] - r * 1.05), (c[0] - 10, c[1] - r * 1.6), (c[0] - 22, c[1] - r * 1.4), (c[0] - 24, c[1] - r * 0.9),
                  (c[0] - 12, c[1] - r * 1.05)), "#ef3b2f", depth=2)


def shield(a, sh):
    a.part(circle(sh[0] - 10, sh[1] + 10, 11), "#2f7fe6", depth=2)
    a.part(circle(sh[0] - 10, sh[1] + 10, 4), GOLD[0], depth=1)


HERO = {"top": "#2f7fe6", "legs": "#5b6478", "boots": "#7a4318", "body": tunic, "hair": helmet}


def hero(a, pose, grip, down=False):
    if not down:
        shield(a, (-8, -34))
    person(a, dict(HERO, hold=sword(grip)), pose, down)


# the monsters, weakest first: skeleton, zombie, ghost, orc, vampire
def ribs(a):
    for y in (-34, -29, -24):
        a.stroke(f"M-8 {y}Q1 {y + 3} 10 {y}", "#8f8572", LINE * 0.9)
    a.flat(rrect(-1, -38, 3, 20, 1.5), "#8f8572")


def skull(a, c, r):
    a.flat(ellipse(c[0] + 10, c[1] + 8, 2, 2.6), "#2a1830")


def tatters(a):
    a.flat(poly((-13, -16), (-9, -11), (-5, -16), (-1, -11), (3, -16), (8, -11), (13, -16), (12, -21), (-12, -21)), "#5f7a4a", line=LINE * 0.7)
    a.flat(rrect(-3, -32, 6, 6, 2), "#9fb48b", opacity=0.8)


def zombie_hair(a, c, r):
    for dx, dy in ((-12, -14), (-4, -19), (5, -18)):
        a.part(poly((c[0] + dx - 4, c[1] + dy + 6), (c[0] + dx + 1, c[1] + dy - 4), (c[0] + dx + 5, c[1] + dy + 6)), "#3d4b2a", depth=1)
    a.flat(capsule((c[0] + 8, c[1] - 9), (c[0] + 12, c[1] - 4), 1), OUTLINE)


def orc_cap(a, c, r):
    a.part(smooth((c[0] - r * 1.05, c[1] - 1), (c[0] - r * 0.9, c[1] - r * 0.8), (c[0], c[1] - r * 1.1), (c[0] + r * 0.9, c[1] - r * 0.8),
                  (c[0] + r * 1.05, c[1] - 1), (c[0], c[1] - 6)), "#7d8696", depth=2.2)
    for dx in (-1, 1):
        hx = c[0] + dx * r * 0.85
        a.part(smooth((hx, c[1] - r * 0.6), (hx + dx * 9, c[1] - r * 1.15), (hx + dx * 7, c[1] - r * 1.65), (hx + dx * 3, c[1] - r * 1.0)),
               "#f3ead2", depth=1.2)


def orc_body(a):
    a.flat(smooth((-6, -16), (-8, -28), (2, -34), (11, -27), (9, -16)), "#9ad866", line=LINE * 0.7)
    a.flat(rrect(-13, -20, 27, 5, 2), "#7a4318", line=LINE * 0.7)


def club(a, hand, ang):
    with a.g(f"translate({fmt(hand[0])} {fmt(hand[1])}) rotate({fmt(150 - ang)})"):
        a.part(smooth((-3, 0), (-6, 22), (0, 30), (6, 22), (3, 0)), WOOD[0], depth=1.6)


def slick_hair(a, c, r):
    a.part(smooth((c[0] - r * 1.05, c[1] + 4), (c[0] - r * 0.95, c[1] - r * 0.8), (c[0], c[1] - r * 1.12), (c[0] + r * 0.95, c[1] - r * 0.7),
                  (c[0] + r * 0.7, c[1] - r * 0.35), (c[0] + 6, c[1] - r * 0.55), (c[0] + 1, c[1] - r * 0.25), (c[0] - 4, c[1] - r * 0.55),
                  (c[0] - r * 0.6, c[1] + 2)), "#2b2140", depth=2)


def cape_back(a, c, r):
    a.part(poly((-11, -40), (-26, -6), (-6, -10), (14, -38)), "#c42a3c", depth=2)
    a.part(poly((-16, -43), (-24, -52), (-6, -42)), "#2b2140", depth=1)
    a.part(poly((14, -43), (22, -52), (6, -42)), "#2b2140", depth=1)


def collar(a):
    a.flat(poly((-4, -39), (1, -28), (7, -39)), "#ffffff", line=LINE * 0.7)
    a.flat(circle(1.5, -31, 2.2), "#c42a3c", line=LINE * 0.5)


MONSTERS = {
    "skeleton": {"idle": (15, 20), "swing": (70, 95), "skin": "#f1ead8", "top": "#e9e1cc", "legs": "#e9e1cc", "boots": "#cfc6ad", "sleeve": "#e9e1cc", "body": ribs,
                 "face": skull, "ear": False, "blush": False, "eyes": "socket", "mouth": "teeth"},
    "zombie": {"idle": (20, 70), "swing": (70, 95), "skin": "#a9c98b", "top": "#7d9b67", "legs": "#4f5a7a", "boots": "#3a3f4c", "body": tatters, "hair": zombie_hair,
               "blush": False, "eyes": "worried", "mouth": "frown"},
    "orc": {"idle": (15, 20), "swing": (60, 150), "skin": "#69bd35", "top": "#69bd35", "legs": "#6b4a2e", "boots": "#46301e", "body": orc_body, "hair": orc_cap,
            "blush": False, "eyes": "angry", "mouth": "tusks", "hold": club},
    "vampire": {"idle": (15, 20), "swing": (70, 95), "skin": "#d9d2f0", "top": "#2b2140", "legs": "#2b2140", "boots": "#151020", "body": collar, "hair": slick_hair,
                "hair_back": cape_back, "blush": False, "eyes": "angry", "mouth": "fangs"},
}


def ghost(a, pose):
    lift, arms, eyes, m = {"idle": (8, -10, "open", "o"), "swing": (10, 25, "angry", "grin"), "down": (0, -10, "x", "o")}[pose]
    if pose != "down":
        a.shadow(34, 7)
    # knocked down: squashed flat on the floor
    with a.g("translate(0 4) scale(1.15 0.55)") if pose == "down" else nullcontext():
        body = smooth((-16, -lift - 2), (-18, -lift - 30), (-10, -lift - 56), (6, -lift - 62), (20, -lift - 48), (19, -lift - 20), (16, -lift),
                      (9, -lift - 5), (3, -lift + 1), (-4, -lift - 5), (-10, -lift + 1))
        a.part(body, "#eef4ff", depth=3)
        for x, d in ((-14, -1), (17, 1)):
            a.part(ellipse(x + d * arms * 0.15, -lift - 30 - abs(arms) * 0.3, 6, 4.5), "#eef4ff", depth=1.2)
        eye(a, 2, -lift - 40, 5.2, (1, 0), eyes)
        eye(a, 13, -lift - 40, 4.6, (1, 0), eyes)
        mouth(a, 8, -lift - 29, 8, m)


def monster(a, name, pose):
    """A monster in a pose, facing left."""
    with a.g("scale(-1 1)"):
        if name == "ghost":
            ghost(a, pose)
        else:
            look = MONSTERS[name]
            arms = look["swing" if pose == "swing" else "idle"]
            person(a, look, {"arms": arms, "eyes": look["eyes"], "mouth": look["mouth"]}, pose == "down")


def chest(a):
    a.shadow(80, 12)
    a.part(rrect(-34, -34, 68, 34, 4), "#c0392b", depth=2.6)
    a.part(smooth((-34, -32), (-30, -52), (0, -58), (30, -52), (34, -32)), "#d64a3a", depth=2.6)
    for x in (-26, 20):
        a.part(rrect(x, -56, 7, 56, 2), GOLD[0], depth=1.4)
    a.part(rrect(-35, -36, 70, 6, 3), GOLD[0], depth=1.4)
    a.part(rrect(-7, -40, 14, 16, 3), GOLD[0], depth=1.6)
    a.flat(poly((-1.5, -34), (1.5, -34), (2.2, -28), (-2.2, -28)), OUTLINE)
    for x, y in ((-40, -6), (40, -5), (-30, -3)):
        coin(a, x, y, 5.5)


def potion(a):
    a.shadow(52, 10)
    a.part(circle(0, -24, 24), "#4aa8ff", depth=3)
    a.part(rrect(-8, -62, 16, 18, 3), "#bfe3ff", depth=1.4)
    a.part(rrect(-10, -70, 20, 10, 3), WOOD[0], depth=1.4)
    a.flat(ellipse(-9, -32, 5, 8), "#ffffff", opacity=0.7)
    a.flat(circle(10, -14, 3), "#ffffff", opacity=0.5)


def trap(a):
    a.shadow(90, 10)
    for k in range(5):
        x = -32 + k * 16
        a.part(poly((x - 7, -8), (x, -38), (x + 7, -8)), STEEL[0], depth=1.4)
    a.part(rrect(-44, -12, 88, 12, 4), "#6b7180", depth=2)


def banner(a):
    a.part(rrect(-44, -92, 88, 6, 3), WOOD[0], depth=1.4)
    a.part(poly((-36, -88), (36, -88), (36, -10), (0, -24), (-36, -10)), "#c42a3c", depth=3)
    a.part(circle(0, -60, 12), GOLD[0], depth=2)
    a.flat(poly((0, -68), (5, -56), (-5, -56)), "#c42a3c", line=LINE * 0.6)


def barrel(a):
    a.shadow(60, 10)
    a.part(smooth((-24, -2), (-29, -27), (-24, -52), (24, -52), (29, -27), (24, -2)), WOOD[0], depth=2.6)
    for y in (-12, -42):
        a.part(rrect(-27, y - 3, 54, 6, 3), "#6b7180", depth=1)
    a.stroke("M-8 -50Q-10 -27 -8 -4M8 -50Q10 -27 8 -4", WOOD[1], LINE * 0.8)


HERO_POSES = (("idle", {"arms": (25, 25)}, 130), ("swing-a", {"arms": (25, 150), "lean": -8, "eyes": "angry", "mouth": "grin"}, 205),
              ("swing-b", {"arms": (25, 75), "lean": 10, "eyes": "angry", "mouth": "grin", "legs": (-20, 18)}, 95),
              ("jump", {"arms": (40, 140), "legs": (-30, 40), "lift": 8, "mouth": "grin"}, 170), ("down", {}, 30))
sprites = [sprite("hero-" + name, lambda a, p=pose, g=grip, n=name: hero(a, p, g, n == "down"), BOX) for name, pose, grip in HERO_POSES]
# weakest first, as the game counts looks
for name in ("skeleton", "zombie", "ghost", "orc", "vampire"):
    sprites += [sprite(f"{name}-{pose}", lambda a, n=name, p=pose: monster(a, n, p), BOX) for pose in ("idle", "swing", "down")]
for name, fn in (("potion", potion), ("chest", chest), ("trap", trap), ("banner", banner), ("barrel", barrel)):
    sprites.append(sprite(name, fn, BOX, w=220, h=220, scale=0.9))
write(sprites + trees(), sys.argv[1])
