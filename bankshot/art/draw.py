#!/usr/bin/env python3
"""Draws Bankshot's agent, gunmen, hostage and crates as cartoon sprites
with ../../tools/cartoon: python3 draw.py OUT_DIR. Everyone faces right."""
import math
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../tools/cartoon"))
from cartoon import GOLD, LINE, OUTLINE, at, circle, ellipse, fmt, person, poly, rrect, smooth, sprite, write  # noqa: E402

# 4 pixels a plan unit at the game's 58 plan units to a model unit
PPU = 232
BOX = {"w": 240, "h": 240, "ppu": PPU, "oy": 0.82}


def pistol(a, hand, ang):
    """A chunky pistol in the hand, its barrel along the arm."""
    with a.g(f"translate({fmt(hand[0])} {fmt(hand[1])}) rotate({fmt(90 - ang)})"):
        a.part(rrect(-2, -4.5, 19, 7, 2.5), "#5d6573", depth=1.4)
        a.part(rrect(14, -3.6, 5, 5.2, 1.5), "#3c424d", depth=1, hi=False)
        a.part(rrect(-3, 0, 6.5, 9, 2.5), "#8a5a34", depth=1.2)
        a.flat(rrect(3, -3.4, 9, 1.8, 0.9), GOLD[0])


def flash(a, hand, ang):
    """The muzzle flash past the pistol's barrel."""
    tip = at(hand, ang, 24)
    pts = []
    for k in range(16):
        r = 9 if k % 2 == 0 else 4
        t = k / 16 * math.tau
        pts.append((tip[0] + r * 1.2 * math.cos(t), tip[1] + r * math.sin(t)))
    a.flat(poly(*pts), "#ffd23f", line=LINE * 0.8)
    a.flat(circle(tip[0], tip[1], 3.4), "#fff6c8")


def suit(a):
    """The agent's jacket: a white shirt, a red tie, lapels."""
    a.flat(poly((5, -39), (10, -26), (14, -37)), "#ffffff", line=LINE * 0.7)
    a.flat(poly((9.5, -37), (12, -37), (12.4, -27), (10.4, -24.5), (8.8, -27)), "#e23d3d", line=LINE * 0.6)
    a.flat(rrect(-12, -19, 25, 3.4, 1.7), "#1f2a44")


def quiff(a, c, r):
    a.part(smooth((c[0] - r * 0.95, c[1] + 2), (c[0] - r * 0.9, c[1] - r * 0.7), (c[0] - r * 0.1, c[1] - r * 1.2),
                  (c[0] + r * 0.9, c[1] - r * 1.05), (c[0] + r * 1.05, c[1] - r * 0.45), (c[0] + r * 0.3, c[1] - r * 0.62),
                  (c[0] - r * 0.4, c[1] - r * 0.35), (c[0] - r * 0.55, c[1] + 4)), "#5b3420", depth=2.2)


AGENT = {"top": "#2f7fe6", "legs": "#24324f", "boots": "#3a2a20", "body": suit, "hair": quiff}


def agent(a, pose, down=False):
    look = dict(AGENT, hold=pistol) if pose.get("armed", True) and not down else AGENT
    if pose.get("fire"):
        look = dict(look, hold=lambda a, h, g: (flash(a, h, g), pistol(a, h, g)))
    person(a, look, pose, down)


def stripes(a):
    for y in (-35, -29, -23, -17):
        a.flat(rrect(-12.5, y, 26, 2.6, 1.3), "#2d2f38", opacity=0.9)


def beanie(a, c, r):
    a.part(smooth((c[0] - r * 1.02, c[1] - 1), (c[0] - r * 0.8, c[1] - r * 0.85), (c[0] + r * 0.2, c[1] - r * 1.15),
                  (c[0] + r * 1.0, c[1] - r * 0.6), (c[0] + r * 1.04, c[1] - 2)), "#30323d", depth=2.2)
    a.part(rrect(c[0] - r * 1.08, c[1] - 5, r * 2.16, 6, 3), "#454857", depth=1.4)


def eye_mask(a, c, r):
    a.flat(rrect(c[0] - 6, c[1] - 4.8, 30, 9.6, 4.8), "#1d1d26", opacity=0.92)


def bald(a, c, r):
    a.flat(ellipse(c[0] + 2, c[1] - r * 0.62, r * 0.5, r * 0.22), "#ffffff", opacity=0.35)


def beard(a, c, r):
    a.part(smooth((c[0] - 2, c[1] + 3), (c[0] + 8, c[1] + 6), (c[0] + r * 1.02, c[1] + 4), (c[0] + r * 0.9, c[1] + r * 0.85),
                  (c[0] + 6, c[1] + r * 1.18), (c[0] - 4, c[1] + r * 0.8)), "#c8501f", depth=2)
    a.flat(rrect(c[0] + 6, c[1] + 8.5, 9, 2.6, 1.3), "#7a2a12")


def vest(a):
    a.flat(poly((-12, -36), (-2, -38), (-3, -15), (-12.5, -15)), "#6b4a2e", line=LINE * 0.7)
    a.flat(poly((13, -36), (7, -38), (8, -15), (13.5, -15)), "#6b4a2e", line=LINE * 0.7)


def hood_back(a, c, r):
    a.part(circle(c[0] - 2, c[1] + 1, r * 1.16), "#3e9b4f", depth=2.4)


def hood(a, c, r):
    a.part(smooth((c[0] - r * 1.15, c[1] + 4), (c[0] - r * 1.0, c[1] - r * 0.8), (c[0] + r * 0.1, c[1] - r * 1.2),
                  (c[0] + r * 1.0, c[1] - r * 0.6), (c[0] + r * 0.55, c[1] - r * 0.5), (c[0] - r * 0.45, c[1] - r * 0.2),
                  (c[0] - r * 0.6, c[1] + 6)), "#3e9b4f", depth=2.2)


def bandana(a, c, r):
    a.part(smooth((c[0] - 4, c[1] + 4), (c[0] + r * 1.05, c[1] + 4), (c[0] + r * 0.85, c[1] + r * 0.82), (c[0] + 7, c[1] + r * 1.05),
                  (c[0] - 3, c[1] + r * 0.6)), "#d8343a", depth=1.8)
    for x, y in ((c[0] + 6, c[1] + 9), (c[0] + 13, c[1] + 12), (c[0] + 4, c[1] + 15)):
        a.flat(circle(x, y, 1.2), "#ffffff")


GUNMEN = {
    "suit": {"top": "#f1f1ea", "legs": "#2e3140", "boots": "#22242c", "body": stripes, "hair": beanie, "mask": eye_mask},
    "beard": {"skin": "#f2b98a", "top": "#d64a3a", "legs": "#3d4a6b", "boots": "#3a2a20", "body": vest, "hair": bald, "face": beard},
    "green": {"top": "#3e9b4f", "legs": "#3a3f4c", "boots": "#22242c", "hair_back": hood_back, "hair": hood, "face": bandana},
}


def ponytail_back(a, c, r):
    a.part(smooth((c[0] - r * 0.7, c[1] - r * 0.4), (c[0] - r * 1.5, c[1] - r * 0.1), (c[0] - r * 1.7, c[1] + r * 0.9),
                  (c[0] - r * 1.25, c[1] + r * 0.6), (c[0] - r * 0.8, c[1] + r * 0.3)), "#f2c14e", depth=2)


def fringe(a, c, r):
    a.part(smooth((c[0] - r * 1.0, c[1] + 4), (c[0] - r * 0.9, c[1] - r * 0.75), (c[0] + r * 0.1, c[1] - r * 1.12),
                  (c[0] + r * 0.95, c[1] - r * 0.7), (c[0] + r * 1.0, c[1] - r * 0.15), (c[0] + r * 0.2, c[1] - r * 0.5),
                  (c[0] - r * 0.45, c[1] - r * 0.1), (c[0] - r * 0.55, c[1] + 6)), "#f2c14e", depth=2)
    a.part(circle(c[0] - r * 0.75, c[1] - r * 0.55, 3.2), "#ef5b9c", depth=1)


def dress(a):
    a.part(smooth((-12, -24), (12, -24), (17, -10), (0, -8), (-16, -10)), "#ef5b9c", depth=2.4)


HOSTAGE = {"top": "#ef5b9c", "legs": "#f0b78c", "boots": "#7a3b8c", "hair_back": ponytail_back, "hair": fringe, "body": dress}


def crate(a, x, w, h, y=0.0):
    a.part(rrect(x, y - h, w, h, 3), "#c7883f", depth=2.4)
    a.flat(rrect(x + 4, y - h + 4, w - 8, h - 8, 1.5), "none", line=LINE * 0.8)
    a.part(poly((x + 5, y - 7), (x + w - 9, y - h + 4), (x + w - 5, y - h + 7), (x + 9, y - 4)), "#a8642c", depth=1, hi=False)
    for px, py in ((x + 3.5, y - h + 3.5), (x + w - 3.5, y - h + 3.5), (x + 3.5, y - 3.5), (x + w - 3.5, y - 3.5)):
        a.flat(circle(px, py, 1.3), OUTLINE)


def crates_wide(a):
    a.shadow(100, 14)
    crate(a, -46, 52, 48)
    crate(a, 8, 40, 36)


def crates_medium(a):
    a.shadow(96, 14)
    crate(a, -44, 46, 44)
    crate(a, 4, 40, 40)
    crate(a, -30, 34, 30, -44)


AIM = {"arms": (82, 90), "legs": (-14, 14), "look": (1, 0)}
AGENT_POSES = {"aim": AIM, "fire": dict(AIM, fire=True, lean=-6, mouth="flat"),
               "cheer": {"arms": (215, 118), "legs": (-10, 12), "eyes": "happy", "mouth": "grin", "armed": False},
               "sad": {"arms": (6, 8), "lean": 6, "eyes": "worried", "mouth": "frown", "look": (0.2, 1), "armed": False}}
HOSTAGE_POSES = {"scared": {"arms": (210, 118), "eyes": "worried", "mouth": "o", "look": (-0.6, 0)},
                 "cheer": {"arms": (215, 118), "eyes": "happy", "mouth": "grin"}}
sprites = [sprite("agent-" + name, lambda a, p=pose: agent(a, p), BOX) for name, pose in AGENT_POSES.items()]
sprites.append(sprite("agent-down", lambda a: agent(a, {}, down=True), BOX))
for name, look in GUNMEN.items():
    sprites.append(sprite(name + "-aim", lambda a, l=dict(look, hold=pistol): person(a, l, dict(AIM, eyes="angry", mouth="frown")), BOX))
    sprites.append(sprite(name + "-down", lambda a, l=look: person(a, l, down=True), BOX))
sprites.append(sprite("hostage-scared", lambda a: person(a, HOSTAGE, HOSTAGE_POSES["scared"]), BOX))
sprites.append(sprite("hostage-down", lambda a: person(a, HOSTAGE, down=True), BOX))
sprites.append(sprite("hostage-cheer", lambda a: person(a, HOSTAGE, HOSTAGE_POSES["cheer"]), BOX))
sprites.append(sprite("crate-wide", crates_wide, BOX, w=280, h=200))
sprites.append(sprite("crate-medium", crates_medium, BOX, w=280, h=260))
write(sprites, sys.argv[1])
