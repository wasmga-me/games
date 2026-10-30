#!/usr/bin/env python3
"""Writes jobs.json: the hero, monsters, items and trees of Higher Floor,
rendered side on with ../../tools/render3d (sh fetch.sh mini-dungeon
graveyard-kit nature-kit first). The hero faces right, the monsters left."""
import json
import os

DUNGEON = "models/mini-dungeon/Models/GLB format/"
GRAVE = "models/graveyard-kit/Models/GLB format/"
NATURE = "models/nature-kit/Models/GLTF format/"
PITCH = 10
PPU = 160


def person(name, parts, turn, poses):
    return [{"name": f"{name}-{pose}", "parts": [dict(parts[0], anim=anim, t=t)] + parts[1:], "turn": turn, "pitch": PITCH,
             "w": 220, "h": 220, "ppu": PPU, "center": [0, 0.45, 0], "shadowOpacity": 0.28}
            for pose, anim, t in poses]


jobs = []
hero = [{"url": DUNGEON + "character-human.glb"},
        {"url": DUNGEON + "weapon-sword.glb", "hold": [0, "arm-right"], "pos": [0, -0.22, 0.05], "rot": [1.57, 0, 0], "scale": 1.5},
        {"url": DUNGEON + "shield-round.glb", "hold": [0, "arm-left"], "pos": [0.08, -0.18, 0], "rot": [0, 1.57, 0], "scale": 0.9}]
jobs += person("hero", hero, 55, [("idle", "idle", 0), ("swing-a", "attack-melee-right", 0.12), ("swing-b", "attack-melee-right", 0.25),
                                  ("jump", "jump", 0.2), ("down", "die", 0.33)])
# weakest first
for name, url in (("skeleton", GRAVE + "character-skeleton.glb"), ("zombie", GRAVE + "character-zombie.glb"),
                  ("ghost", GRAVE + "character-ghost.glb"), ("orc", DUNGEON + "character-orc.glb"),
                  ("vampire", GRAVE + "character-vampire.glb")):
    jobs += person(name, [{"url": url}], -55, [("idle", "idle", 0), ("swing", "attack-melee-right", 0.2), ("down", "die", 0.33)])
for name, scale, turn in (("potion", 2.2, -20), ("chest", 1.2, -25), ("trap", 1.1, 0), ("banner", 1.0, 0), ("barrel", 1.0, -20)):
    jobs.append({"name": name, "parts": [{"url": DUNGEON + name + ".glb", "scale": scale}], "turn": turn, "pitch": PITCH,
                 "w": 220, "h": 240, "ppu": PPU, "center": [0, 0.5, 0], "shadowOpacity": 0.28})
for name, scale in (("tree_default", 3.0), ("tree_oak", 3.2), ("tree_cone", 3.0), ("plant_bushLarge", 4.0)):
    colors = {"leafsGreen": "#7fd05c", "grass": "#86d460", "woodBark": "#a8724c"}
    jobs.append({"name": name.replace("plant_", "").lower(), "parts": [{"url": NATURE + name + ".glb", "scale": scale, "colors": colors}],
                 "pitch": PITCH, "w": 260, "h": 420, "ppu": 64, "shadow": False, "center": [0, 2.6, 0]})
json.dump(jobs, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "jobs.json"), "w"), indent=1)
print(len(jobs), "jobs")
