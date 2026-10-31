#!/usr/bin/env python3
"""Draws Pinwork's art as SVG: the knight, the orc, the chests, the torch
and the coins, chunky and outlined.

Units are tenths of a plan unit; each sprite's origin is where the game
puts it (a person's feet, a coin's middle). The knight faces right and the
orc left; the game mirrors them.

    python3 draw.py OUT_DIR

writes each sprite's SVG and a jobs.json for render.mjs and render3d's
pack.py into OUT_DIR (seen side on, `pitch` 0, with no `center`: the
origin is in the middle of each picture)."""
import json
import os
import sys

PPU = 6  # sheet pixels per plan unit
INK = "#2a1810"
W = 12  # outline width

SKIN, SKIN_D = "#ffd0a1", "#e9a978"
STEEL, STEEL_D, STEEL_L = "#c7d2e2", "#8392aa", "#eef3fa"
BLUE, BLUE_D = "#2f7fe6", "#1f5fb8"
RED, RED_D = "#ef3b2f", "#b8241c"
GOLD, GOLD_D, GOLD_L = "#ffc928", "#d18a0c", "#fff1a0"
WOOD, WOOD_D = "#a8642c", "#7a4318"
GREEN, GREEN_D, GREEN_L = "#69bd35", "#46902a", "#9ad866"
CHAR, CHAR_D = "#4a4038", "#2e2722"


def g(*parts, t=""):
    return (f'<g transform="{t}">' if t else "<g>") + "".join(parts) + "</g>"


def shape(tag, fill, outline=True, **a):
    attrs = " ".join(f'{k.replace("_", "-")}="{v}"' for k, v in a.items())
    s = ""
    if outline:
        s = ' stroke-linejoin="round" stroke-linecap="round"'
        if "stroke" not in a:
            s += f' stroke="{INK}"'
        if "stroke_width" not in a:
            s += f' stroke-width="{W}"'
    return f'<{tag} {attrs} fill="{fill}"{s}/>'


def path(d, fill, outline=True, **a):
    return shape("path", fill, outline, d=d, **a)


def circle(cx, cy, r, fill, outline=True, **a):
    return shape("circle", fill, outline, cx=cx, cy=cy, r=r, **a)


def ellipse(cx, cy, rx, ry, fill, outline=True, **a):
    return shape("ellipse", fill, outline, cx=cx, cy=cy, rx=rx, ry=ry, **a)


def rect(x, y, w, h, rx, fill, outline=True, **a):
    return shape("rect", fill, outline, x=x, y=y, width=w, height=h, rx=rx, **a)


def line(d, colour, width, **a):
    return shape("path", "none", d=d, stroke=colour, stroke_width=width, **a)


def shadow(rx):
    return ellipse(0, -4, rx, 18, "#000", False, opacity=0.25)


def sparkle(x, y, r, colour="#fff6b0"):
    d = f"M{x},{y - r} Q{x + r * 0.18},{y - r * 0.18} {x + r},{y} Q{x + r * 0.18},{y + r * 0.18} {x},{y + r} Q{x - r * 0.18},{y + r * 0.18} {x - r},{y} Q{x - r * 0.18},{y - r * 0.18} {x},{y - r} Z"
    return path(d, colour, False) + path(d, "none", True, stroke_width=5)


def smoke(x, y, s=1.0):
    return g(
        circle(0, 0, 26, "#9b958f", False, opacity=0.75),
        circle(24, -30, 20, "#b3ada7", False, opacity=0.7),
        circle(-6, -62, 15, "#c9c4bf", False, opacity=0.6),
        t=f"translate({x},{y}) scale({s})",
    )


# --- the knight ---------------------------------------------------------


def sword(x, y, angle, burnt=False):
    blade, guard = (CHAR, CHAR_D) if burnt else (STEEL_L, GOLD)
    return g(
        path("M-14,-34 L-16,-190 L0,-226 L16,-190 L14,-34 Z", blade),
        line("M0,-48 L0,-196", STEEL if not burnt else CHAR_D, 6, opacity=0.8),
        rect(-44, -44, 88, 22, 10, guard),
        rect(-11, -22, 22, 44, 8, WOOD_D if not burnt else CHAR_D),
        circle(0, 30, 13, guard),
        t=f"translate({x},{y}) rotate({angle})",
    )


def knight_body(c):
    """Boots, tunic and belt."""
    return "".join(
        [
            rect(-56, -72, 46, 72, 16, c["boot"]),
            rect(10, -72, 46, 72, 16, c["boot"]),
            path("M-80,-206 Q-96,-130 -88,-58 L88,-58 Q96,-130 80,-206 Z", c["tunic"]),
            path("M-90,-86 L90,-86 L88,-58 L-88,-58 Z", c["trim"]),
            rect(-90, -128, 180, 26, 10, c["belt"]),
            rect(-20, -134, 40, 38, 8, c["trim"]),
        ]
    )


def knight_head(c, face):
    return "".join(
        [
            # the plume, behind the helmet
            path("M-10,-392 C-60,-470 -150,-450 -178,-392 C-140,-418 -96,-404 -70,-370 Z", c["plume"]),
            circle(0, -290, 114, c["skin"]),
            # the helmet: a dome with a band, open at the face
            path("M-122,-282 C-128,-372 -60,-414 4,-414 C70,-414 128,-372 122,-282 Z", c["steel"]),
            path("M-124,-300 L124,-300 L122,-268 L-122,-268 Z", c["steel_d"]),
            line("M-76,-372 C-46,-396 -6,-400 26,-392", c["shine"], 14, opacity=0.9),
            circle(-92, -284, 7, c["shine"], False),
            circle(92, -284, 7, c["shine"], False),
            face,
        ]
    )


def knight_face(mood):
    if mood == "burnt":
        return circle(16, -226, 14, "#fff", False) + circle(76, -226, 12, "#fff", False) + line("M30,-186 Q50,-178 70,-188", "#fff", 7)
    eyes = {
        "idle": (ellipse(16, -228, 22, 28, "#fff") + circle(24, -224, 13, INK, False) + circle(29, -231, 5, "#fff", False))
        + (ellipse(78, -228, 18, 26, "#fff") + circle(85, -224, 12, INK, False) + circle(89, -231, 4, "#fff", False)),
        "worried": (ellipse(16, -228, 24, 30, "#fff") + circle(22, -224, 8, INK, False))
        + (ellipse(80, -228, 20, 28, "#fff") + circle(85, -224, 7, INK, False)),
        "cheer": line("M-4,-226 Q16,-252 36,-226", INK, 11) + line("M62,-226 Q80,-250 98,-226", INK, 11),
        "down": line("M2,-244 L34,-212 M34,-244 L2,-212", INK, 10) + line("M64,-244 L94,-212 M94,-244 L64,-212", INK, 10),
    }[mood]
    brows = {
        "idle": line("M-6,-268 Q16,-278 36,-270", INK, 10) + line("M62,-270 Q80,-278 98,-268", INK, 10),
        "worried": line("M-4,-262 Q14,-276 34,-282", INK, 10) + line("M64,-282 Q84,-276 100,-262", INK, 10),
        "cheer": "",
        "down": "",
    }[mood]
    mouth = {
        "idle": line("M34,-190 Q52,-174 70,-192", INK, 9),
        "worried": ellipse(54, -182, 12, 16, "#7a1f1a"),
        "cheer": path("M22,-200 Q54,-200 86,-204 Q82,-150 52,-150 Q24,-152 22,-200 Z", "#8c1f1a") + path("M36,-166 Q52,-176 70,-166 Q62,-152 50,-152 Q40,-154 36,-166 Z", "#ff7a7a", False),
        "down": line("M36,-186 Q48,-196 58,-186 Q68,-176 78,-186", INK, 8),
    }[mood]
    cheeks = ellipse(96, -196, 16, 10, "#ff8a7a", False, opacity=0.55) + ellipse(-2, -196, 14, 9, "#ff8a7a", False, opacity=0.45)
    return eyes + brows + mouth + cheeks


def colours(burnt):
    if burnt:
        return dict(skin=CHAR, steel="#5a524b", steel_d=CHAR_D, shine="#7a7068", plume=CHAR_D, tunic=CHAR, trim=CHAR_D, belt=CHAR_D, boot=CHAR_D)
    return dict(skin=SKIN, steel=STEEL, steel_d=STEEL_D, shine=STEEL_L, plume=RED, tunic=BLUE, trim=GOLD, belt=WOOD_D, boot=WOOD_D)


def limb(x0, y0, x1, y1, width, colour, hand, skin):
    """An outlined arm from the shoulder to a round hand."""
    d = f"M{x0},{y0} L{x1},{y1}"
    return line(d, INK, width) + line(d, colour, width - 2 * W) + circle(x1, y1, hand, skin)


def arm(x0, y0, x1, y1, c):
    """A sleeve from the shoulder to a hand."""
    return limb(x0, y0, x1, y1, 52, c["steel"], 22, c["skin"])


def knight(pose):
    burnt = pose == "burnt"
    c = colours(burnt)
    body = knight_body(c)
    head = knight_head(c, knight_face(pose))
    if pose in ("idle", "burnt", "worried"):
        if pose == "worried":
            extra = [path("M-150,-350 Q-128,-310 -136,-294 Q-150,-276 -164,-294 Q-170,-312 -150,-350 Z", "#8fd6ff")]
        else:
            extra = [smoke(-40, -440, 1.2), smoke(70, -470, 0.9)] if burnt else []
        angle = 12 if pose == "worried" else 38
        parts = [arm(-72, -150, -100, -96, c), body, sword(118, -100, angle, burnt), arm(66, -150, 112, -104, c), head]
        return g(shadow(96), *parts, *extra)
    if pose == "cheer":
        parts = [
            arm(-70, -160, -150, -280, c),
            sword(154, -296, 14),
            arm(70, -160, 150, -280, c),
            body,
            head,
            sparkle(-170, -420, 30),
            sparkle(196, -430, 24),
            sparkle(210, -250, 18),
        ]
        return g(shadow(96), *parts)
    if pose == "down":
        # on his back, stars round his head
        lying = g(body, head, arm(66, -150, 112, -104, c), t="translate(-110,-112) rotate(-90)")
        stars = sparkle(-330, -240, 22, GOLD) + sparkle(-260, -270, 18, GOLD) + sparkle(-200, -236, 20, GOLD)
        return g(ellipse(-80, -4, 200, 18, "#000", False, opacity=0.25), sword(150, -24, 90), lying, stars)
    raise ValueError(pose)


# --- the orc --------------------------------------------------------------


def club(x, y, angle, burnt=False):
    wood, wood_d = (CHAR, CHAR_D) if burnt else (WOOD, WOOD_D)
    metal = "#d9dde4" if not burnt else "#6a625a"
    spikes = "".join(path(f"M{x},{y - 12} L{x + dx * 28},{y + dy * 28} L{x},{y + 12} Z", metal) for x, y, dx, dy in [(-36, -186, -1, 0), (36, -200, 1, 0), (-28, -236, -1, -0.4), (28, -236, 1, -0.4)])
    spikes += path("M-12,-252 L0,-282 L12,-252 Z", metal)
    return g(
        spikes,
        path("M-14,0 L-38,-200 C-42,-262 42,-262 38,-200 L14,0 Z", wood),
        line("M-10,-210 C-6,-232 10,-236 18,-224", wood_d, 9),
        t=f"translate({x},{y}) rotate({angle})",
    )


def orc_body(c, face):
    return "".join(
        [
            rect(-100, -76, 64, 76, 20, c["foot"]),
            rect(36, -76, 64, 76, 20, c["foot"]),
            path("M-152,-70 C-176,-210 -120,-326 0,-326 C120,-326 176,-210 152,-70 C110,-40 -110,-40 -152,-70 Z", c["skin"]),
            path("M-96,-170 C-100,-90 -60,-74 -10,-74 C40,-74 70,-100 66,-170 C40,-150 -60,-150 -96,-170 Z", c["belly"], False),
            path("M-156,-96 C-80,-66 80,-66 156,-96 L150,-58 L110,-30 L80,-58 L44,-28 L10,-58 L-26,-28 L-60,-58 L-96,-28 L-130,-58 Z", c["cloth"]),
            # the iron cap and its horns
            path("M-118,-280 C-150,-320 -186,-330 -196,-376 C-160,-352 -128,-350 -98,-322 Z", c["horn"]),
            path("M118,-280 C150,-320 186,-330 196,-376 C160,-352 128,-350 98,-322 Z", c["horn"]),
            path("M-118,-276 C-118,-360 118,-360 118,-276 Z", c["iron"]),
            rect(-126, -292, 252, 30, 12, c["iron_d"]),
            circle(-80, -277, 7, c["rivet"], False),
            circle(0, -277, 7, c["rivet"], False),
            circle(80, -277, 7, c["rivet"], False),
            face,
        ]
    )


def orc_face(mood):
    if mood == "burnt":
        eyes = line("M-96,-238 L-62,-206 M-62,-238 L-96,-206", "#fff", 10) + line("M-30,-238 L4,-206 M4,-238 L-30,-206", "#fff", 10)
        return eyes + line("M-100,-160 Q-60,-176 -20,-158", "#fff", 8)
    eyes = ellipse(-82, -222, 22, 18, "#fff6c4") + circle(-90, -220, 10, INK, False) + ellipse(-16, -222, 20, 17, "#fff6c4") + circle(-24, -220, 9, INK, False)
    brows = path("M-118,-262 L-52,-236 L-50,-252 L-112,-276 Z", INK, False) + path("M-40,-252 L14,-270 L18,-256 L-36,-236 Z", INK, False)
    nose = ellipse(-52, -192, 22, 16, GREEN_D)
    if mood == "idle":
        mouth = path("M-118,-168 Q-50,-140 10,-166 Q-6,-128 -54,-126 Q-104,-128 -118,-168 Z", "#5a1a14")
        tusks = path("M-96,-150 L-104,-196 L-78,-154 Z", "#fffbe8") + path("M-14,-152 L-6,-196 L-32,-156 Z", "#fffbe8")
    else:
        mouth = path("M-126,-176 Q-50,-150 16,-176 Q8,-94 -56,-92 Q-118,-96 -126,-176 Z", "#5a1a14") + ellipse(-56, -114, 30, 12, "#e2605a", False)
        tusks = path("M-104,-130 L-112,-186 L-84,-136 Z", "#fffbe8") + path("M-8,-132 L0,-186 L-28,-138 Z", "#fffbe8")
    return eyes + brows + nose + mouth + tusks


def orc_colours(burnt):
    if burnt:
        return dict(skin=CHAR, belly="#5a5048", cloth=CHAR_D, horn="#6a625a", iron="#3a342f", iron_d=CHAR_D, rivet="#6a625a", foot=CHAR_D)
    return dict(skin=GREEN, belly=GREEN_L, cloth=WOOD, horn="#f3ead2", iron="#7d8696", iron_d="#5b6372", rivet="#c9d0db", foot="#5a3a1e")


def orc_arm(x0, y0, x1, y1, c):
    return limb(x0, y0, x1, y1, 64, c["skin"], 30, c["skin"])


def orc(pose):
    c = orc_colours(pose == "burnt")
    if pose == "idle":
        return g(shadow(150), orc_arm(120, -200, 168, -120, c), club(-196, -104, -24), orc_body(c, orc_face("idle")), orc_arm(-146, -170, -196, -104, c))
    if pose == "swing":
        return g(shadow(150), orc_arm(120, -200, 168, -120, c), club(-170, -380, 30), orc_arm(-140, -220, -170, -380, c), orc_body(c, orc_face("swing")))
    if pose == "burnt":
        flat = g(orc_body(c, orc_face("burnt")), club(-200, -40, -80, True), t="scale(1.08,0.62)")
        return g(shadow(170), flat, smoke(-60, -250, 1.3), smoke(70, -280, 1.0))
    raise ValueError(pose)


# --- chests, torch, coins -------------------------------------------------


def chest(open_):
    box = [
        rect(-170, -150, 340, 150, 22, WOOD),
        line("M-160,-100 L160,-100 M-160,-50 L160,-50", WOOD_D, 8),
        rect(-150, -150, 44, 150, 8, GOLD),
        rect(106, -150, 44, 150, 8, GOLD),
    ]
    if not open_:
        lid = [
            path("M-176,-150 L-176,-196 C-176,-262 176,-262 176,-196 L176,-150 Z", WOOD),
            path("M-150,-150 L-150,-236 C-140,-246 -116,-250 -106,-250 L-106,-150 Z", GOLD),
            path("M150,-150 L150,-236 C140,-246 116,-250 106,-250 L106,-150 Z", GOLD),
            rect(-184, -164, 368, 30, 12, GOLD),
            rect(-30, -184, 60, 70, 12, GOLD),
            path("M0,-166 C-12,-166 -14,-150 -6,-144 L-10,-126 L10,-126 L6,-144 C14,-150 12,-166 0,-166 Z", INK, False),
            line("M-150,-200 C-100,-236 -40,-242 0,-242", "#fff", 10, opacity=0.35),
        ]
        return g(shadow(190), *box, *lid)
    lid = [
        path("M-176,-160 L-160,-300 C-150,-350 150,-350 160,-300 L176,-160 Z", WOOD_D),
        rect(-184, -172, 368, 28, 12, GOLD),
    ]
    heap = [path("M-160,-150 C-150,-250 150,-250 160,-150 Z", GOLD)]
    for x, y in [(-110, -180), (-50, -205), (20, -210), (90, -185), (-10, -170), (60, -160), (-80, -160)]:
        heap.append(circle(x, y, 30, GOLD) + circle(x, y, 18, GOLD_L, False) + circle(x - 8, y - 8, 6, "#fff", False))
    return g(shadow(190), *lid, rect(-160, -176, 320, 40, 14, "#3a2010"), *heap, *box, rect(-30, -136, 60, 56, 12, GOLD), sparkle(-120, -300, 26), sparkle(140, -260, 20))


def torch():
    flame = path("M0,-310 C40,-270 54,-236 40,-206 C30,-184 -30,-184 -40,-206 C-54,-236 -30,-260 0,-310 Z", "#ff8a1c")
    core = path("M0,-260 C20,-238 26,-222 18,-208 C10,-196 -10,-196 -18,-208 C-26,-222 -14,-240 0,-260 Z", "#ffe14a", False)
    return g(
        circle(0, -220, 110, "#ffb347", False, opacity=0.13),
        circle(0, -220, 70, "#ffc864", False, opacity=0.16),
        rect(-14, -200, 28, 150, 10, WOOD),
        path("M-36,-200 L36,-200 L24,-168 L-24,-168 Z", "#6c7586"),
        rect(-30, -90, 60, 24, 8, "#6c7586"),
        flame,
        core,
    )


def coin(kind):
    if kind == 0:
        return g(circle(0, 0, 44, GOLD), circle(0, 0, 28, GOLD_L, False), line("M-28,0 A28,28 0 1 0 28,0 A28,28 0 1 0 -28,0", GOLD_D, 6), line("M-26,-14 Q-22,-28 -8,-32", "#fff", 8))
    if kind == 1:
        return g(ellipse(0, 6, 44, 30, GOLD_D), ellipse(0, -2, 44, 30, GOLD), ellipse(0, -2, 26, 16, GOLD_L, False), line("M-24,-14 Q-12,-24 6,-24", "#fff", 7))
    return g(ellipse(4, 0, 18, 44, GOLD_D), ellipse(-2, 0, 18, 44, GOLD), line("M-8,-24 L-8,10", "#fff", 7))


def big(svg, k):
    return g(svg, t=f"scale({k})")


# the knight and the orc are shown bigger than drawn, and so are the
# chests and the torch
SPRITES = [
    ("knight-idle", big(knight("idle"), 1.15)),
    ("knight-worried", big(knight("worried"), 1.15)),
    ("knight-cheer", big(knight("cheer"), 1.15)),
    ("knight-down", big(knight("down"), 1.15)),
    ("knight-burnt", big(knight("burnt"), 1.15)),
    ("orc-idle", big(orc("idle"), 1.15)),
    ("orc-swing", big(orc("swing"), 1.15)),
    ("orc-burnt", big(orc("burnt"), 1.15)),
    ("chest", big(chest(False), 1.1)),
    ("chest-open", big(chest(True), 1.1)),
    ("torch", big(torch(), 1.4)),
    ("coin-a", coin(0)),
    ("coin-b", coin(1)),
    ("coin-c", coin(2)),
]

# each sprite's origin in the middle of its picture, with room enough for
# the biggest (1000 units each way)
out = sys.argv[1]
size = 2000 * PPU // 10
for name, svg in SPRITES:
    with open(os.path.join(out, name + ".svg"), "w") as f:
        f.write(f'<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="-1000 -1000 2000 2000">{svg}</svg>')
json.dump([{"name": name, "ppu": PPU, "pitch": 0} for name, _ in SPRITES], open(os.path.join(out, "jobs.json"), "w"))
print(len(SPRITES), "sprites")
