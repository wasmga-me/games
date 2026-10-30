#!/usr/bin/env python3
"""Writes jobs.json: the dog, the bees, the hive and the trees of Hive
Guard, rendered side on with ../../tools/render3d (sh fetch.sh cube-pets
nature-kit first). The dog and the bees face right."""
import json
import os

PETS = "models/cube-pets/Models/GLB format/"
NATURE = "models/nature-kit/Models/GLTF format/"
PITCH = 12

jobs = []
for pose, anim, t in (("idle", "idle", 0), ("breathe", "idle", 0.5), ("sad", "gesture-negative", 0.12), ("happy", "dance", 0.12)):
    jobs.append({"name": f"dog-{pose}", "parts": [{"url": PETS + "animal-dog.glb", "anim": anim, "t": t}], "turn": 50, "pitch": PITCH,
                 "w": 200, "h": 200, "ppu": 96, "center": [0, 0.8, 0], "shadowOpacity": 0.3})
for pose, t in (("a", 0), ("b", 0.1)):
    jobs.append({"name": f"bee-{pose}", "parts": [{"url": PETS + "animal-bee.glb", "anim": "run", "t": t}], "turn": 80, "pitch": PITCH,
                 "w": 160, "h": 160, "ppu": 48, "center": [0, 1, 0], "shadow": False})
jobs.append({"name": "hive", "parts": [{"hive": True}], "turn": 0, "pitch": PITCH, "w": 220, "h": 220, "ppu": 120, "center": [0, 0.7, 0],
             "shadowOpacity": 0.3})
for name, scale in (("tree_default", 3.0), ("tree_oak", 3.2), ("tree_cone", 3.0), ("plant_bushLarge", 4.0)):
    colors = {"leafsGreen": "#7fd05c", "grass": "#86d460", "woodBark": "#a8724c"}
    jobs.append({"name": name.replace("plant_", "").lower(), "parts": [{"url": NATURE + name + ".glb", "scale": scale, "colors": colors}],
                 "pitch": PITCH, "w": 260, "h": 420, "ppu": 64, "shadow": False, "center": [0, 2.6, 0]})
json.dump(jobs, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "jobs.json"), "w"), indent=1)
print(len(jobs), "jobs")
