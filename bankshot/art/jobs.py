#!/usr/bin/env python3
"""Writes jobs.json: the agent, the gunmen, the hostage and the crates of
Bankshot, rendered side on with ../../tools/render3d (sh fetch.sh
mini-characters blaster-kit first). Everyone faces right."""
import json
import os

PEOPLE = "models/mini-characters/Models/GLB format/"
BLASTER = "models/blaster-kit/Models/GLB format/"
PITCH = 8
PPU = 200


def person(name, body, gun, poses):
    out = []
    for pose, anim, t, armed in poses:
        parts = [{"url": PEOPLE + "character-" + body + ".glb", "anim": anim, "t": t}]
        if armed:
            # in the right hand, the barrel ahead
            parts.append({"url": BLASTER + gun + ".glb", "pos": [-0.1, 0.29, 0.27], "rot": [0, 3.14, 0], "scale": 0.8})
        out.append({"name": f"{name}-{pose}", "parts": parts, "turn": 70, "pitch": PITCH, "w": 240, "h": 220, "ppu": PPU,
                    "center": [0, 0.4, 0], "shadowOpacity": 0.3})
    return out


jobs = []
jobs += person("agent", "male-c", "blaster-b", [("aim", "holding-right-shoot", 0, True), ("fire", "holding-right-shoot", 0.1, True),
                                                ("cheer", "emote-yes", 0.3, False), ("sad", "emote-no", 0.3, False)])
for name, body, gun in (("suit", "male-d", "blaster-h"), ("beard", "male-b", "blaster-d"), ("green", "male-f", "blaster-l")):
    jobs += person(name, body, gun, [("aim", "holding-right", 0, True), ("down", "die", 0.33, False)])
jobs += person("hostage", "female-b", "", [("scared", "emote-no", 0.25, False), ("down", "die", 0.33, False),
                                          ("cheer", "emote-yes", 0.3, False)])
for name, turn in (("crate-wide", 20), ("crate-medium", -25)):
    jobs.append({"name": name, "parts": [{"url": BLASTER + name + ".glb"}], "turn": turn, "pitch": PITCH, "w": 260, "h": 220,
                 "ppu": PPU, "center": [0, 0.3, 0], "shadowOpacity": 0.3})
json.dump(jobs, open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "jobs.json"), "w"), indent=1)
print(len(jobs), "jobs")
