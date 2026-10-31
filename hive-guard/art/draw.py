#!/usr/bin/env python3
"""Draws Hive Guard's puppy, bees, hive and trees as cartoon sprites with
../../tools/cartoon: python3 draw.py OUT_DIR. The puppy and the bees face
right."""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "../../tools/cartoon"))
from cartoon import GOLD, LINE, OUTLINE, circle, ellipse, eye, mouth, poly, rrect, smooth, sprite, trees, write  # noqa: E402

FUR = "#f6e3c4"
PATCH = "#c27a3a"
EAR = "#8a4f24"


def puppy(a, pose):
    """A puppy sitting, its middle about 71 art units up: the circle the
    bees must not reach is 69 across that."""
    happy, sad, pant = pose == "happy", pose == "sad", pose == "breathe"
    a.shadow(110, 18)
    # the tail, wagging up when happy
    tail = ((-40, -30), (-62, -52), (-66, -78)) if happy else ((-40, -26), (-64, -30), (-74, -46))
    a.part(smooth(tail[0], tail[1], tail[2], (tail[2][0] + 6, tail[2][1] + 2), (tail[1][0] + 6, tail[1][1] + 8), (-36, -18)), PATCH, depth=2)
    # back leg and body
    a.part(ellipse(-26, -22, 26, 22), FUR, depth=4)
    a.part(smooth((-40, -10), (-36, -58), (-6, -86), (22, -78), (30, -40), (24, -6), (-10, -2)), FUR, depth=5)
    a.part(ellipse(-24, -60, 14, 18), PATCH, depth=3, hi=False)
    a.part(rrect(-44, -12, 30, 12, 6), FUR, depth=2)
    # front legs, one raised when happy
    a.part(rrect(2, -44, 14, 44, 7), FUR, depth=2.4)
    if happy:
        a.part(smooth((14, -56), (40, -76), (48, -66), (24, -44)), FUR, depth=2.4)
    else:
        a.part(rrect(16, -44, 14, 44, 7), FUR, depth=2.4)
    # the collar and its tag
    a.part(smooth((-12, -84), (20, -80), (26, -72), (-8, -74)), "#e23d3d", depth=1.4)
    a.part(circle(16, -68, 5.5), GOLD[0], depth=1.2)
    # the head: big and round
    hy = -104 + (4 if sad else 0)
    with a.g(f"rotate({-8 if happy else 8 if sad else 0} 12 {hy})"):
        ear_y = 12 if sad else 0
        a.part(smooth((-26, hy - 22), (-44, hy - 6 + ear_y), (-40, hy + 26 + ear_y), (-26, hy + 14), (-18, hy - 12)), EAR, depth=2.4)
        a.part(circle(10, hy, 38), FUR, depth=5)
        a.part(ellipse(-4, hy - 12, 16, 15), PATCH, depth=2.4, hi=False)
        # the muzzle
        a.part(ellipse(32, hy + 16, 20, 15), "#fff6e6", depth=2.4)
        a.part(ellipse(44, hy + 6, 8, 6), OUTLINE, depth=1, hi=False)
        a.flat(ellipse(42, hy + 4, 3, 2), "#ffffff", opacity=0.7)
        kind = "happy" if happy else "worried" if sad else "open"
        eye(a, 6, hy - 10, 11, (1, 0.2), kind)
        eye(a, 30, hy - 12, 9.5, (1, 0.2), kind)
        if pant or happy:
            a.part(smooth((30, hy + 22), (40, hy + 22), (42, hy + 38), (35, hy + 42), (30, hy + 34)), "#ef6a6a", depth=1.4)
            mouth(a, 36, hy + 17, 14, "smile")
        elif sad:
            mouth(a, 34, hy + 24, 12, "frown")
            a.flat(smooth((-6, hy + 2), (-2, hy + 12), (-6, hy + 16), (-10, hy + 12)), "#7fc8ff", line=LINE * 0.6)
            a.part(circle(-6, hy - 34, 7), "#ff8a8a", depth=1.4)
        else:
            mouth(a, 36, hy + 19, 14, "smile")
        a.flat(ellipse(12, hy + 14, 7, 4), "#ff8a8a", opacity=0.45)
        # the front ear, flopping
        a.part(smooth((-6, hy - 34), (-26, hy - 26 + ear_y), (-32, hy + 8 + ear_y), (-20, hy + 14 + ear_y), (-10, hy - 10)), EAR, depth=2.4)


def bee(a, frame):
    up = frame == "a"
    rot = -30 if up else 20
    for dx, k in ((-6, 0.85), (4, 1.0)):
        with a.g(f"translate({dx} -48) rotate({rot})"):
            a.part(ellipse(0, -14 * k, 11 * k, 16 * k), "#e8f6ff", depth=1.4)
    a.part(poly((-36, -36), (-48, -32), (-36, -28)), OUTLINE, depth=0, hi=False)
    body = ellipse(-4, -32, 32, 24)
    a.part(body, "#ffcc1e", depth=4)
    with a.clip(body):
        for x in (-24, -8):
            a.flat(rrect(x, -60, 9, 60, 0), "#2a1810")
    a.flat(body, "none", line=LINE)
    eye(a, 14, -38, 8.5, (1, 0.1), "angry")
    a.stroke("M12 -56Q16 -68 24 -70M20 -54Q28 -62 34 -60")
    a.flat(circle(24, -70, 2.8), OUTLINE)
    a.flat(circle(34, -60, 2.8), OUTLINE)


def hive(a):
    a.shadow(130, 20)
    a.part(smooth((-60, 0), (-62, -50), (-44, -104), (0, -128), (44, -104), (62, -50), (60, 0)), "#f2b531", depth=6)
    for y, w in zip((-20, -44, -68, -92, -112), (58, 60, 54, 42, 24)):
        a.stroke(f"M{-w} {y}Q0 {y + 8} {w} {y}", "#b97a12", LINE * 1.2)
    a.part(smooth((-16, 0), (-15, -18), (0, -28), (15, -18), (16, 0)), "#3a2414", depth=1.5, hi=False)
    a.flat(ellipse(-30, -84, 8, 14), "#ffffff", opacity=0.35)


# the puppy at the game's 35 plan units to a model unit, 3.4 pixels a
# plan unit; bees are scaled to their height in the game
BOX = {"w": 260, "h": 260, "ppu": 120, "oy": 0.85}
sprites = [sprite("dog-" + pose, lambda a, p=pose: puppy(a, p), BOX, w=280, h=280, oy=0.88) for pose in ("idle", "breathe", "sad", "happy")]
sprites += [sprite("bee-" + frame, lambda a, f=frame: bee(a, f), BOX, w=140, h=140, ppu=96, oy=0.9) for frame in ("a", "b")]
sprites.append(sprite("hive", hive, BOX))
write(sprites + trees(), sys.argv[1])
