#!/usr/bin/env python3
"""Writes jobs.json and the passengers' textures: the buses and passengers of
Boarding, rendered with ../../tools/render3d (sh fetch.sh car-kit
mini-characters nature-kit first). Buses face 8 ways; passengers wear
their bus's colour."""
import colorsys
import json
import os

from PIL import Image

KIT = "models/car-kit/Models/GLB format/"
PEOPLE = "models/mini-characters/Models/GLB format/"
NATURE = "models/nature-kit/Models/GLTF format/"
PITCH = 58
PPU = 64
# the buses' and shirts' colours, in the game's order
COLOURS = ["#e5484d", "#f28c28", "#f5c518", "#3fb950", "#3d7be0", "#9b5de5"]
# a minibus fills two squares of 1.6, a bus three
BUSES = [("mini", 2.9), ("bus", 4.4)]
# the shirt's and the trousers' columns in the characters' shared colour map
BODIES = [("female-a", (448, 256, 512, 384), (320, 256, 384, 384)), ("male-a", (64, 256, 128, 384), (128, 256, 192, 384))]

here = os.path.dirname(os.path.abspath(__file__))
tools = os.path.join(here, "../../tools/render3d/")


def paint(px, box, colour, shade):
    """Gives a block of the colour map `colour`, its lightest pixel at
    `shade` of the colour's lightness and darker ones darker."""
    th, tl, ts = colorsys.rgb_to_hls(*(int(colour[i : i + 2], 16) / 255 for i in (1, 3, 5)))
    top = max(colorsys.rgb_to_hls(*(c / 255 for c in px[x, y]))[1] for y in range(box[1], box[3]) for x in range(box[0], box[2]))
    for y in range(box[1], box[3]):
        for x in range(box[0], box[2]):
            l = colorsys.rgb_to_hls(*(c / 255 for c in px[x, y]))[1]
            r, g, b = colorsys.hls_to_rgb(th, tl * shade * l / top, ts)
            px[x, y] = (int(r * 255), int(g * 255), int(b * 255))


def outfit(body, shirt, trousers, colour):
    """The colour map with this body dressed in `colour`, so the bus it is
    waiting for shows from afar."""
    im = Image.open(tools + PEOPLE + "Textures/colormap.png").convert("RGB")
    px = im.load()
    paint(px, shirt, colour, 1)
    paint(px, trousers, colour, 0.62)
    os.makedirs(os.path.join(here, "tex"), exist_ok=True)
    name = f"tex/{body}-{colour[1:]}.png"
    im.save(os.path.join(here, name))
    return "jobs/" + name


jobs = []
for c, colour in enumerate(COLOURS):
    for kind, length in BUSES:
        for a in range(0, 360, 45):
            part = {"bus": colour, "len": length, "wheel": KIT + "wheel-truck.glb", "scale": 0.9}
            jobs.append({"name": f"{kind}-{c}-{a // 45}", "parts": [part], "turn": 180 - a, "pitch": PITCH,
                         "w": 384 if kind == "bus" else 288, "h": 384 if kind == "bus" else 288, "ppu": PPU, "shadowOpacity": 0.32})
for c, colour in enumerate(COLOURS):
    for body, top, legs in BODIES:
        tex = outfit(body, top, legs, colour)
        for pose, anim, t in (("stand", "idle", 0), ("walk-a", "walk", 0.12), ("walk-b", "walk", 0.45)):
            part = {"url": PEOPLE + "character-" + body + ".glb", "anim": anim, "t": t, "texture": tex, "scale": 1.8}
            jobs.append({"name": f"{body}-{c}-{pose}", "parts": [part], "turn": 40, "pitch": 28,
                         "w": 112, "h": 144, "ppu": PPU, "center": [0, 0.75, 0], "shadowOpacity": 0.25})
for name, scale in (("tree_default", 3.0), ("tree_oak", 3.2), ("tree_detailed", 2.8), ("tree_fat", 3.0), ("plant_bushLarge", 5.0)):
    colors = {"leafsGreen": "#7fd05c", "grass": "#86d460", "woodBark": "#a8724c"}
    jobs.append({"name": name.replace("plant_", "").lower(), "parts": [{"url": NATURE + name + ".glb", "scale": scale, "colors": colors}],
                 "pitch": PITCH, "w": 384, "h": 288, "ppu": PPU, "shadowOpacity": 0.3, "center": [0, 1.2, 0]})
json.dump(jobs, open(os.path.join(here, "jobs.json"), "w"), indent=1)
print(len(jobs), "jobs")
