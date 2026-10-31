"""Cartoon sprites drawn as SVG: outlined, shaded shapes in the chunky style
of mobile puzzle games, for render.mjs and ../render3d/pack.py.

A game's art/draw.py builds its sprites with these helpers and calls
`write(sprites, folder)`, which writes <name>.svg for each and a jobs.json
that pack.py reads like render3d's. Sprites are drawn facing right in art
units, 100 to a model unit, with the origin on the ground under the middle
and y down.
"""
import json
import math
import os
from contextlib import contextmanager

OUTLINE = "#2a1810"
LINE = 2.6
SKIN = "#ffd0a1"
GOLD = ("#ffc928", "#d18a0c", "#fff1a0")
STEEL = ("#c7d2e2", "#8392aa", "#eef3fa")
WOOD = ("#a8642c", "#7a4318")


def mix(a, b, k):
    """Colour a, k of the way to colour b."""
    pa = [int(a[i : i + 2], 16) for i in (1, 3, 5)]
    pb = [int(b[i : i + 2], 16) for i in (1, 3, 5)]
    return "#" + "".join(f"{round(x + (y - x) * k):02x}" for x, y in zip(pa, pb))


def shade(c):
    return mix(c, "#1a0c20", 0.28)


def light(c):
    return mix(c, "#ffffff", 0.45)


def fmt(v):
    return f"{v:.2f}".rstrip("0").rstrip(".")


# shapes: path data
def circle(x, y, r):
    return ellipse(x, y, r, r)


def ellipse(x, y, rx, ry):
    return (f"M{fmt(x - rx)} {fmt(y)}a{fmt(rx)} {fmt(ry)} 0 1 0 {fmt(2 * rx)} 0"
            f"a{fmt(rx)} {fmt(ry)} 0 1 0 {fmt(-2 * rx)} 0Z")


def rrect(x, y, w, h, r):
    r = min(r, w / 2, h / 2)
    return (f"M{fmt(x + r)} {fmt(y)}h{fmt(w - 2 * r)}a{fmt(r)} {fmt(r)} 0 0 1 {fmt(r)} {fmt(r)}v{fmt(h - 2 * r)}"
            f"a{fmt(r)} {fmt(r)} 0 0 1 {fmt(-r)} {fmt(r)}h{fmt(2 * r - w)}a{fmt(r)} {fmt(r)} 0 0 1 {fmt(-r)} {fmt(-r)}"
            f"v{fmt(2 * r - h)}a{fmt(r)} {fmt(r)} 0 0 1 {fmt(r)} {fmt(-r)}Z")


def poly(*pts):
    return "M" + "L".join(f"{fmt(x)} {fmt(y)}" for x, y in pts) + "Z"


def capsule(a, b, r1, r2=None):
    """A limb from a to b, r1 thick at a and r2 at b."""
    r2 = r1 if r2 is None else r2
    dx, dy = b[0] - a[0], b[1] - a[1]
    l = math.hypot(dx, dy) or 1e-3
    nx, ny = -dy / l, dx / l
    return (f"M{fmt(a[0] + nx * r1)} {fmt(a[1] + ny * r1)}L{fmt(b[0] + nx * r2)} {fmt(b[1] + ny * r2)}"
            f"A{fmt(r2)} {fmt(r2)} 0 0 0 {fmt(b[0] - nx * r2)} {fmt(b[1] - ny * r2)}"
            f"L{fmt(a[0] - nx * r1)} {fmt(a[1] - ny * r1)}A{fmt(r1)} {fmt(r1)} 0 0 0 {fmt(a[0] + nx * r1)} {fmt(a[1] + ny * r1)}Z")


def smooth(*pts):
    """A closed shape through the points, rounded (Catmull-Rom)."""
    n = len(pts)
    out = f"M{fmt(pts[0][0])} {fmt(pts[0][1])}"
    for i in range(n):
        p0, p1, p2, p3 = pts[i - 1], pts[i], pts[(i + 1) % n], pts[(i + 2) % n]
        c1 = (p1[0] + (p2[0] - p0[0]) / 6, p1[1] + (p2[1] - p0[1]) / 6)
        c2 = (p2[0] - (p3[0] - p1[0]) / 6, p2[1] - (p3[1] - p1[1]) / 6)
        out += f"C{fmt(c1[0])} {fmt(c1[1])} {fmt(c2[0])} {fmt(c2[1])} {fmt(p2[0])} {fmt(p2[1])}"
    return out + "Z"


def at(a, ang, l):
    """The point l from a at `ang` degrees (0 straight down, 90 forward)."""
    t = math.radians(ang)
    return (a[0] + math.sin(t) * l, a[1] + math.cos(t) * l)


class Art:
    """Builds one sprite's SVG, back to front."""

    def __init__(self):
        self.out = []
        self.n = 0

    @contextmanager
    def g(self, transform):
        """What is drawn inside, moved by an SVG transform."""
        self.out.append(f'<g transform="{transform}">')
        yield
        self.out.append("</g>")

    @contextmanager
    def clip(self, d):
        """What is drawn inside, kept within the shape d."""
        self.n += 1
        self.out.append(f'<clipPath id="c{self.n}"><path d="{d}"/></clipPath><g clip-path="url(#c{self.n})">')
        yield
        self.out.append("</g>")

    def part(self, d, fill, depth=None, hi=True, line=LINE):
        """A filled shape with a darker lower right, a highlight up left and
        an outline."""
        depth = LINE * 1.6 if depth is None else depth
        with self.clip(d):
            self.out.append(f'<path d="{d}" fill="{shade(fill)}"/>'
                            f'<path d="{d}" fill="{fill}" transform="translate({fmt(-depth)} {fmt(-depth)})"/>')
            if hi:
                self.out.append(f'<path d="{d}" fill="none" stroke="{light(fill)}" stroke-width="{fmt(depth * 1.3)}" '
                                f'stroke-opacity="0.55" transform="translate({fmt(depth * 0.9)} {fmt(depth * 0.9)})"/>')
        if line:
            self.out.append(f'<path d="{d}" fill="none" stroke="{OUTLINE}" stroke-width="{fmt(line)}" stroke-linejoin="round"/>')

    def flat(self, d, fill, line=0.0, opacity=1.0):
        self.out.append(f'<path d="{d}" fill="{fill}" fill-opacity="{fmt(opacity)}"'
                        + (f' stroke="{OUTLINE}" stroke-width="{fmt(line)}" stroke-linejoin="round"' if line else "") + "/>")

    def stroke(self, d, colour=OUTLINE, width=LINE):
        self.out.append(f'<path d="{d}" fill="none" stroke="{colour}" stroke-width="{fmt(width)}" '
                        'stroke-linecap="round" stroke-linejoin="round"/>')

    def shadow(self, w, h=None):
        h = w * 0.22 if h is None else h
        self.flat(ellipse(0, 0, w / 2, h / 2), "#000000", opacity=0.22)

    def svg(self):
        return "".join(self.out)


def eye(a, x, y, r, look=(1, 0), kind="open"):
    """A big cartoon eye at (x, y): open (white, pupil, glint), shut (a
    curve), happy (an arch), x (knocked out), angry or worried (with a
    brow), or socket (a skull's, with a spark)."""
    if kind == "x":
        k = r * 0.7
        a.stroke(f"M{fmt(x - k)} {fmt(y - k)}L{fmt(x + k)} {fmt(y + k)}M{fmt(x - k)} {fmt(y + k)}L{fmt(x + k)} {fmt(y - k)}", width=LINE * 1.1)
        return
    if kind == "shut":
        a.stroke(f"M{fmt(x - r)} {fmt(y)}Q{fmt(x)} {fmt(y + r * 0.7)} {fmt(x + r)} {fmt(y)}", width=LINE)
        return
    if kind == "happy":
        a.stroke(f"M{fmt(x - r)} {fmt(y + r * 0.3)}Q{fmt(x)} {fmt(y - r * 0.9)} {fmt(x + r)} {fmt(y + r * 0.3)}", width=LINE)
        return
    if kind == "socket":
        a.flat(ellipse(x, y, r * 0.9, r), "#2a1830")
        a.flat(circle(x + look[0] * r * 0.25, y, r * 0.32), "#ffe066")
        return
    a.flat(ellipse(x, y, r * 0.86, r), "#ffffff", line=LINE * 0.45)
    pr = r * 0.58
    px, py = x + look[0] * r * 0.3, y + look[1] * r * 0.3
    a.flat(ellipse(px, py, pr * 0.9, pr), "#1b1424")
    a.flat(circle(px - pr * 0.3, py - pr * 0.4, pr * 0.32), "#ffffff")
    if kind == "angry":
        a.stroke(f"M{fmt(x - r * 1.1)} {fmt(y - r * 1.25)}L{fmt(x + r)} {fmt(y - r * 0.6)}", width=LINE * 1.3)
    if kind == "worried":
        a.stroke(f"M{fmt(x - r)} {fmt(y - r * 0.8)}L{fmt(x + r)} {fmt(y - r * 1.3)}", width=LINE * 1.1)


def mouth(a, x, y, w, kind="smile"):
    """A mouth centred at (x, y), w wide: smile, grin (open), frown, o,
    flat, teeth, tusks or fangs."""
    if kind == "smile":
        a.stroke(f"M{fmt(x - w / 2)} {fmt(y)}Q{fmt(x)} {fmt(y + w * 0.5)} {fmt(x + w / 2)} {fmt(y - w * 0.05)}", width=LINE)
    elif kind == "grin":
        d = f"M{fmt(x - w / 2)} {fmt(y - w * 0.1)}Q{fmt(x)} {fmt(y + w * 0.95)} {fmt(x + w / 2)} {fmt(y - w * 0.15)}Z"
        a.flat(d, "#8a1f2b", line=LINE)
        a.flat(ellipse(x + w * 0.05, y + w * 0.32, w * 0.22, w * 0.12), "#ef6a6a")
    elif kind == "frown":
        a.stroke(f"M{fmt(x - w / 2)} {fmt(y + w * 0.25)}Q{fmt(x)} {fmt(y - w * 0.3)} {fmt(x + w / 2)} {fmt(y + w * 0.25)}", width=LINE)
    elif kind == "o":
        a.flat(ellipse(x, y + w * 0.1, w * 0.26, w * 0.34), "#8a1f2b", line=LINE)
    elif kind == "flat":
        a.stroke(f"M{fmt(x - w / 2)} {fmt(y)}L{fmt(x + w / 2)} {fmt(y + w * 0.06)}", width=LINE)
    elif kind == "teeth":
        a.flat(rrect(x - w / 2, y - w * 0.15, w, w * 0.4, w * 0.1), "#fffbe8", line=LINE * 0.7)
        for k in (1, 2, 3):
            a.stroke(f"M{fmt(x - w / 2 + w * k / 4)} {fmt(y - w * 0.15)}v{fmt(w * 0.4)}", width=LINE * 0.5)
    elif kind == "tusks":
        a.stroke(f"M{fmt(x - w / 2)} {fmt(y)}Q{fmt(x)} {fmt(y + w * 0.3)} {fmt(x + w / 2)} {fmt(y - w * 0.05)}", width=LINE)
        for fx in (x - w * 0.35, x + w * 0.35):
            a.flat(poly((fx - w * 0.12, y + w * 0.08), (fx + w * 0.12, y + w * 0.06), (fx, y - w * 0.4)), "#fff6dc", line=LINE * 0.6)
    elif kind == "fangs":
        a.flat(f"M{fmt(x - w / 2)} {fmt(y)}Q{fmt(x)} {fmt(y + w * 0.5)} {fmt(x + w / 2)} {fmt(y)}Z", "#5a1020", line=LINE * 0.9)
        for fx in (x - w * 0.26, x + w * 0.22):
            a.flat(poly((fx - w * 0.1, y + w * 0.03), (fx + w * 0.1, y + w * 0.03), (fx, y + w * 0.28)), "#ffffff", line=LINE * 0.6)


def coin(a, x, y, r):
    """A gold coin seen face on."""
    a.part(circle(x, y, r), GOLD[0], depth=r * 0.18)
    a.flat(circle(x, y, r * 0.62), "none", line=LINE * 0.7)
    a.stroke(f"M{fmt(x - r * 0.15)} {fmt(y - r * 0.32)}L{fmt(x - r * 0.15)} {fmt(y + r * 0.32)}", GOLD[1], width=r * 0.2)


def sprite(name, fn, box, **opts):
    """One sprite for `write`: `fn(a)` draws it on an Art, in a render box
    of `box` with `opts` over it."""
    a = Art()
    fn(a)
    return name, a, dict(box, **opts)


def write(sprites, folder):
    """Writes each sprite's SVG and jobs.json into `folder`. `sprites` is a
    list of (name, Art, opts): opts has `w` and `h` (pixels), `ppu`
    (pixels per model unit), `oy` (where the origin is, as a share of
    the height down) and `scale` (to draw it bigger or smaller)."""
    os.makedirs(folder, exist_ok=True)
    jobs = []
    for name, art, o in sprites:
        w, h, ppu = o["w"], o["h"], o["ppu"]
        oy = o.get("oy", 0.8)
        s = ppu / 100 * o.get("scale", 1)
        svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">'
               f'<g transform="translate({fmt(w / 2)} {fmt(h * oy)}) scale({fmt(s)})">{art.svg()}</g></svg>')
        with open(os.path.join(folder, name + ".svg"), "w") as f:
            f.write(svg)
        jobs.append({"name": name, "svg": name + ".svg", "w": w, "h": h, "ppu": ppu, "origin": [w / 2, h * oy]})
    with open(os.path.join(folder, "jobs.json"), "w") as f:
        json.dump(jobs, f, indent=1)
    print(len(jobs), "sprites")


# a little person: big head, round body, short limbs; feet at the origin,
# facing right, about 80 art units tall
HIP_Y = -17
SHOULDER_Y = -34
HEAD = (3, -54)
HEAD_R = 19


def person(a, look, pose=None, down=False):
    """Draws a person. `look` has skin, top, legs, boots and optional
    hooks hair_back(a, c, r), hair(a, c, r) (over the head), mask(a, c, r)
    (under the eyes), face(a, c, r) (over the mouth), body(a) (over the
    top) and hold(a, hand, angle) (in the front hand). `pose` has lean,
    arms (back, front: degrees from straight down, 90 forward), legs
    (back, front), eyes, mouth, look (where the eyes look) and lift (off
    the ground). `down` lays the person on their back, head behind."""
    p = dict(lean=0, arms=(10, 20), legs=(-8, 8), eyes="open", mouth="smile", look=(1, 0), lift=0)
    p.update(pose or {})
    if down:
        p.update(eyes="x", mouth="o", arms=(-12, 12), legs=(-25, 15), lean=0)
    else:
        a.shadow(40 - p["lift"])
    with a.g("translate(34 -13) rotate(-90)" if down else f"translate(0 {fmt(-p['lift'])})"):
        _person(a, look, p)


def _person(a, look, p):
    lean = f"rotate({fmt(p['lean'])} 0 {HIP_Y})"
    skin = look.get("skin", SKIN)
    hip_b, hip_f = (-4, HIP_Y), (4, HIP_Y)
    sh_b, sh_f = (-8, SHOULDER_Y), (9, SHOULDER_Y)
    leg_l = 14

    def leg(hip, ang):
        foot = at(hip, ang, leg_l)
        a.part(capsule(hip, foot, 5.2, 4.8), look["legs"])
        boot = look.get("boots", "#4a3426")
        a.part(rrect(foot[0] - 5, foot[1] - 3, 13, 7, 3.5), boot, depth=1.6)

    def arm(sh, ang, front):
        hand = at(sh, ang, 15)
        a.part(capsule(sh, hand, 4.6, 4.0), look.get("sleeve", look["top"]))
        if front and "hold" in look:
            look["hold"](a, hand, ang)
        a.part(circle(hand[0], hand[1], 4.4), look.get("glove", skin), depth=1.4)

    # the back arm goes behind the legs, the rest leans in front of them
    with a.g(lean):
        arm(sh_b, p["arms"][0], False)
    leg(hip_b, p["legs"][0])
    leg(hip_f, p["legs"][1])
    with a.g(lean):
        # the body: a rounded barrel
        a.part(smooth((-12, -15), (-13, -28), (-9, -38), (0, -40), (10, -38), (14, -28), (13, -15), (0, -12)), look["top"])
        if "body" in look:
            look["body"](a)
        c, r = HEAD, HEAD_R
        if "hair_back" in look:
            look["hair_back"](a, c, r)
        a.part(circle(c[0], c[1], r), skin, depth=3.0)
        if look.get("ear", True):
            # toward the back
            a.part(ellipse(c[0] - 10, c[1] + 3, 3.6, 4.6), skin, depth=1.2, hi=False)
        if "hair" in look:
            look["hair"](a, c, r)
        if "mask" in look:
            look["mask"](a, c, r)
        lx = p["look"]
        eye(a, c[0] + 4, c[1] + 1, 5.6, lx, p["eyes"])
        eye(a, c[0] + 14.5, c[1] + 1, 4.8, lx, p["eyes"])
        if look.get("blush", True):
            a.flat(ellipse(c[0] + 2, c[1] + 10, 3.4, 2.2), "#ff8a8a", opacity=0.45)
        if p["mouth"]:
            mouth(a, c[0] + 10, c[1] + 11, 8, p["mouth"])
        if "face" in look:
            look["face"](a, c, r)
        arm(sh_f, p["arms"][1], True)


# trees and a bush, about as big as Kenney's Nature Kit ones at the same
# scale: TREE is their render box
TREE = {"w": 260, "h": 400, "ppu": 64, "oy": 0.9, "scale": 1.12}
LEAF = "#7fd05c"


def trunk(a, h, w=14):
    a.part(poly((-w / 2, 0), (-w / 2 + 3, -h), (w / 2 - 3, -h), (w / 2, 0)), WOOD[0], depth=2)


def tree_default(a):
    trunk(a, 170, 26)
    for x, y, r in ((-58, -215, 64), (58, -222, 62), (0, -305, 92)):
        a.part(circle(x, y, r), LEAF, depth=7)


def tree_oak(a):
    trunk(a, 140, 28)
    a.part(smooth((-100, -170), (-96, -270), (-40, -340), (40, -345), (100, -275), (98, -180), (40, -140), (-40, -140)), "#6cc54a", depth=8)
    for x, y in ((-44, -260), (30, -220), (50, -300)):
        a.flat(ellipse(x, y, 16, 10), "#9be27a", opacity=0.8)


def tree_cone(a):
    trunk(a, 90, 22)
    for w, y0, y1 in ((80, -70, -230), (66, -170, -320), (50, -260, -400)):
        a.part(smooth((-w, y0), (0, y0 + 10), (w, y0), (w * 0.35, (y0 + y1) / 2), (0, y1), (-w * 0.35, (y0 + y1) / 2)), "#4fae4a", depth=7)


def bush(a):
    for x, y, r in ((-42, -32, 38), (42, -34, 38), (0, -56, 48)):
        a.part(circle(x, y, r), LEAF, depth=5)


def trees():
    """The trees and the bush as sprites for `write`."""
    return [sprite(name, fn, TREE) for name, fn in
            (("tree_default", tree_default), ("tree_oak", tree_oak), ("tree_cone", tree_cone), ("bushlarge", bush))]
