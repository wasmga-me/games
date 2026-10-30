#!/usr/bin/env python3
"""Writes jobs.json: the cars, buses, props and trees of Unpark, rendered with
../../tools/render3d (sh fetch.sh car-kit nature-kit first). Each vehicle faces 8 ways,
seen from above at the board's tilt."""
import json

KIT = "models/car-kit/Models/GLB format/"
PITCH = 58
PPU = 64
# model, length, width (units, from render.mjs --bbox)
CARS = [
    ("sedan", 2.55, 1.5),
    ("sedan-sports", 2.55, 1.3),
    ("hatchback-sports", 2.85, 1.3),
    ("suv", 2.7, 1.5),
    ("suv-luxury", 2.85, 1.5),
    ("taxi", 2.75, 1.5),
    ("police", 3.1, 1.5),
    ("van", 2.75, 1.5),
    ("truck", 2.95, 1.5),
    ("delivery", 3.25, 1.5),
]
BUSES = [("bus-blue", "#3d7be0"), ("bus-yellow", "#f2b632")]
# a car fills two squares of 1.6, a bus three
CELL = 1.6


def view(name, part, a, size):
    # heading a degrees clockwise from up the screen; the model's front is +z,
    # which faces down the screen unturned
    return {"name": f"{name}-{a // 45}", "parts": [part], "turn": 180 - a, "pitch": PITCH, "yaw": 0,
            "w": size, "h": size, "ppu": PPU, "shadowOpacity": 0.32}


jobs = []
for name, length, width in CARS:
    scale = min(1.35 / width, 2 * CELL * 0.92 / length)
    for a in range(0, 360, 45):
        jobs.append(view(name, {"url": KIT + name + ".glb", "scale": round(scale, 3)}, a, 288))
for name, colour in BUSES:
    for a in range(0, 360, 45):
        part = {"bus": colour, "len": 4.4, "wheel": KIT + "wheel-truck.glb", "scale": 0.9}
        jobs.append(view(name, part, a, 384))
for name in ("cone", "box"):
    jobs.append({"name": name, "parts": [{"url": KIT + name + ".glb", "scale": 1.3}], "pitch": PITCH,
                 "w": 128, "h": 128, "ppu": PPU, "shadowOpacity": 0.32})
NATURE = "models/nature-kit/Models/GLTF format/"
for name, scale in (("tree_default", 3.0), ("tree_oak", 3.2), ("tree_detailed", 2.8), ("tree_fat", 3.0), ("plant_bushLarge", 5.0)):
    # leaves a fresher green than the kit's teal
    colors = {"leafsGreen": "#7fd05c", "grass": "#86d460", "woodBark": "#a8724c"}
    jobs.append({"name": name.replace("plant_", "").lower(), "parts": [{"url": NATURE + name + ".glb", "scale": scale, "colors": colors}],
                 "pitch": PITCH, "w": 384, "h": 288, "ppu": PPU, "shadowOpacity": 0.3, "center": [0, 1.2, 0]})
json.dump(jobs, open("jobs.json", "w"), indent=1)
print(len(jobs), "jobs")
