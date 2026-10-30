#!/usr/bin/env python3
"""Writes jobs.json: the knight, the orc and the dungeon props of
Pinwork, rendered side on with ../../tools/render3d (sh fetch.sh
mini-dungeon first). The knight faces right, the orc left."""
import json
import os

DUNGEON = "models/mini-dungeon/Models/GLB format/"
PITCH = 10
PPU = 160


def person(name, parts, turn, poses):
    return [{"name": f"{name}-{pose}", "parts": [dict(parts[0], anim=anim, t=t)] + parts[1:], "turn": turn, "pitch": PITCH,
             "w": 220, "h": 220, "ppu": PPU, "center": [0, 0.45, 0], "shadowOpacity": 0.28}
            for pose, anim, t in poses]


jobs = []
knight = [{"url": DUNGEON + "character-human.glb"},
          {"url": DUNGEON + "weapon-sword.glb", "hold": [0, "arm-right"], "pos": [0, -0.22, 0.05], "rot": [1.57, 0, 0], "scale": 1.5}]
jobs += person("knight", knight, 50, [("idle", "idle", 0), ("worried", "emote-no", 0.3), ("cheer", "emote-yes", 0.3), ("down", "die", 0.33)])
jobs += person("orc", [{"url": DUNGEON + "character-orc.glb"}], -50, [("idle", "idle", 0), ("swing", "attack-melee-right", 0.2), ("down", "die", 0.33)])
for name, scale, turn in (("chest", 1.2, -25), ("barrel", 1.0, -20), ("banner", 1.0, 0), ("pot", 1.2, 0)):
    jobs.append({"name": name, "parts": [{"url": DUNGEON + name + ".glb", "scale": scale}], "turn": turn, "pitch": PITCH,
                 "w": 220, "h": 240, "ppu": PPU, "center": [0, 0.5, 0], "shadowOpacity": 0.28})
json.dump(jobs, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "jobs.json"), "w"), indent=1)
print(len(jobs), "jobs")
