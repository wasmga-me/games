// Builds the models and effect pictures Partial Combat loads
// (public/games/partial-combat/*.glb and fx.png, or $COMBAT_OUT) from the CC0
// packs they come from, only the models, clips and pictures the game uses:
//   npm install && node pack.mjs [figures giants mechs guns pier boats effects]
// The packs are downloaded once into $COMBAT_CACHE (a temp folder by
// default). Quaternius' packs come as FBX, turned into glTF by FBX2glTF.
// Every model keeps its pack's own name, under a prefix (fig., giant., mech.,
// gun., pier., boat.), and the game asks for them by those names.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Document, NodeIO } from "@gltf-transform/core";
import { ALL_EXTENSIONS } from "@gltf-transform/extensions";
import { dedup, meshopt, normals, prune, quantize, resample, simplify, weld } from "@gltf-transform/functions";
import { MeshoptDecoder, MeshoptEncoder, MeshoptSimplifier } from "meshoptimizer";
import sharp from "sharp";

const here = dirname(fileURLToPath(import.meta.url));
const OUT = process.env.COMBAT_OUT || join(here, "../../../public/games/partial-combat");
const CACHE = process.env.COMBAT_CACHE || join(tmpdir(), "partial-combat-assets");
const FBX2GLTF = join(here, "node_modules/fbx2gltf/bin", process.platform === "darwin" ? "Darwin" : "Linux", "FBX2glTF");

/** The packs, as their authors publish them (all CC0). */
const PACKS = {
  characters: "https://opengameart.org/sites/default/files/ultimate_animated_character_pack_by_quaternius.zip",
  mechs: "https://opengameart.org/sites/default/files/animated_mech_pack_-_march_2021.zip",
  guns: "https://opengameart.org/sites/default/files/ultimate_gun_pack_by_quaternius.zip",
  pirate: "https://kenney.nl/media/pages/assets/pirate-kit/e6d4bb1525-1771333093/kenney_pirate-kit.zip",
  watercraft: "https://kenney.nl/media/pages/assets/watercraft-kit/a335cfed49-1713519620/kenney_watercraft-pack.zip",
  particles: "https://kenney.nl/media/pages/assets/particle-pack/f8fe0f8cb8-1677578741/kenney_particle-pack.zip",
};

await Promise.all([MeshoptEncoder.ready, MeshoptDecoder.ready, MeshoptSimplifier.ready]);
const io = new NodeIO().registerExtensions(ALL_EXTENSIONS).registerDependencies({ "meshopt.encoder": MeshoptEncoder, "meshopt.decoder": MeshoptDecoder });

/** The pack's folder, downloaded and unzipped on first use. */
function pack(name) {
  const dir = join(CACHE, name);
  if (existsSync(join(dir, ".done"))) return dir;
  mkdirSync(dir, { recursive: true });
  const zip = join(CACHE, `${name}.zip`);
  if (!existsSync(zip)) run("curl", ["-sSfL", "-o", zip, PACKS[name]]);
  run("unzip", ["-q", "-o", zip, "-d", dir]);
  writeFileSync(join(dir, ".done"), "");
  return dir;
}

function run(cmd, args) {
  const r = spawnSync(cmd, args, { stdio: ["ignore", "ignore", "inherit"] });
  if (r.status !== 0) throw new Error(`${cmd} ${args.join(" ")} failed`);
}

/** The first file under `dir` whose path ends with `tail`. */
function find(dir, tail) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name);
    if (e.isDirectory()) {
      const f = find(p, tail);
      if (f) return f;
    } else if (p.endsWith(tail)) return p;
  }
  return null;
}

/** An FBX as a glTF document (converted once, into the cache). */
async function readFbx(file) {
  const glb = join(CACHE, "glb", basename(dirname(dirname(file))).replace(/\W+/g, "_"), basename(file, ".fbx"));
  if (!existsSync(`${glb}.glb`)) {
    mkdirSync(dirname(glb), { recursive: true });
    run(FBX2GLTF, ["--binary", "--input", file, "--output", glb]);
  }
  return io.read(`${glb}.glb`);
}

/** "CharacterArmature|Shoot_OneHanded" reads "shoot_onehanded". */
const clipName = (n) => n.replace(/^.*\|/, "").toLowerCase();

/** Copies an accessor's data into another document. */
function copyAccessor(doc, a, buffer, array = a.getArray()) {
  return doc.createAccessor(a.getName()).setType(a.getType()).setArray(array.slice()).setNormalized(a.getNormalized()).setBuffer(buffer);
}

/** Removes an animation with its samplers, whose keyframes would otherwise stay in the file. */
function drop(anim) {
  for (const c of anim.listChannels()) c.dispose();
  for (const s of anim.listSamplers()) s.dispose();
  anim.dispose();
}

/** Drops animations FBX2glTF wrote twice (once as the action, once with the armature's name). */
function oneOfEachClip(doc) {
  const seen = new Set();
  for (const a of doc.getRoot().listAnimations()) {
    const n = clipName(a.getName());
    if (seen.has(n) || /\.\d+$/.test(n)) drop(a);
    else {
      seen.add(n);
      a.setName(n);
    }
  }
}

/** Where a node rests: its translation, rotation or scale (null for another path). */
function restOf(node, path) {
  return path === "translation" ? node.getTranslation() : path === "rotation" ? node.getRotation() : path === "scale" ? node.getScale() : null;
}

/** Drops animation channels that hold their node where it rests the whole clip. */
function dropStillChannels(doc) {
  for (const anim of doc.getRoot().listAnimations()) {
    for (const ch of anim.listChannels()) {
      const node = ch.getTargetNode(), path = ch.getTargetPath();
      const rest = restOf(node, path);
      const out = ch.getSampler().getOutput().getArray();
      if (!rest || !out || out.length % rest.length) continue;
      let still = true;
      for (let i = 0; i < out.length && still; i++) still = Math.abs(out[i] - rest[i % rest.length]) < 1e-4;
      if (still) {
        const sampler = ch.getSampler();
        ch.dispose();
        sampler.dispose();
      }
    }
  }
}

/**
 * Writes the document out small. `simplify` cuts its meshes to that share of
 * their triangles where that moves them less than `error` (of their size):
 * the flat-shaded packs repeat each corner per face, so their corners are
 * joined first and the faces' normals made again after.
 */
async function finish(doc, file, { simplify: ratio, error = 0.02 } = {}) {
  dropStillChannels(doc);
  if (ratio) for (const p of doc.getRoot().listMeshes().flatMap((m) => m.listPrimitives())) p.getAttribute("NORMAL")?.dispose();
  await doc.transform(
    dedup(),
    weld(),
    ...(ratio ? [simplify({ simplifier: MeshoptSimplifier, ratio, error, lockBorder: true }), normals(), weld()] : []),
    resample({ tolerance: 1e-4 }),
    prune({ keepLeaves: true }),
    quantize({ quantizePosition: 14, quantizeNormal: 10, quantizeTexcoord: 12, quantizeWeight: 8 }),
    meshopt({ encoder: MeshoptEncoder, level: "medium" }),
  );
  const bytes = await io.writeBinary(doc);
  writeFileSync(join(OUT, file), bytes);
  console.log(`${file}: ${(bytes.length / 1024).toFixed(0)} KB`);
}

// ---------- toy figures and giants: Quaternius' Ultimate Animated Character Pack ----------
// Every character there shares one rig, so a file holds one skeleton and its
// animations, and each character's mesh is skinned to it.
const FIGURES = { Soldier_Male: "soldier", BlueSoldier_Male: "beret", Ninja_Male: "ninja", Viking_Male: "viking" };
const FIGURE_CLIPS = ["idle", "walk", "shoot_onehanded", "recievehit", "defeat", "victory", "punch"];

/** The characters (pack name: our name) in one document, under `prefix`, with their clips. */
async function characters(names, prefix) {
  const dir = join(pack("characters"), "Ultimate Animated Character Pack - Nov 2019", "FBX");
  const [first, ...rest] = Object.keys(names);
  const doc = await readFbx(join(dir, `${first}.fbx`));
  const root = doc.getRoot();
  const buffer = root.listBuffers()[0];
  const skin = root.listSkins()[0];
  const meshNode = root.listNodes().find((n) => n.getMesh());
  meshNode.setName(`${prefix}.${names[first]}`);
  const parent = meshNode.getParentNode();
  const scene = root.listScenes()[0];
  for (const name of rest) {
    const other = await readFbx(join(dir, `${name}.fbx`));
    const src = other.getRoot().listNodes().find((n) => n.getMesh());
    const order = other.getRoot().listSkins()[0].listJoints().map((j) => j.getName());
    if (order.join() !== skin.listJoints().map((j) => j.getName()).join()) throw new Error(`${name} has another rig`);
    const mesh = doc.createMesh(name);
    for (const p of src.getMesh().listPrimitives()) {
      const q = doc.createPrimitive().setMode(p.getMode());
      for (const sem of p.listSemantics()) q.setAttribute(sem, copyAccessor(doc, p.getAttribute(sem), buffer));
      q.setIndices(copyAccessor(doc, p.getIndices(), buffer));
      const m = p.getMaterial();
      q.setMaterial(doc.createMaterial(m.getName()).setBaseColorFactor(m.getBaseColorFactor()));
      mesh.addPrimitive(q);
    }
    const node = doc.createNode(`${prefix}.${names[name]}`).setMesh(mesh).setSkin(skin);
    if (parent) parent.addChild(node);
    else scene.addChild(node);
  }
  for (const m of root.listMaterials()) m.setMetallicFactor(0).setRoughnessFactor(0.6);
  oneOfEachClip(doc);
  return doc;
}

async function figures() {
  const doc = await characters(FIGURES, "fig");
  for (const a of doc.getRoot().listAnimations()) if (!FIGURE_CLIPS.includes(a.getName())) drop(a);
  // a squad is hundreds of them: cut to about a third of their triangles, they look the same from the camera
  await finish(doc, "figures.glb", { simplify: 0.25 });
}

// The bosses that are people: four more of the pack's characters, made giant.
// Their clips go by the mechs' names, so a boss asks for the same ones
// whichever it is. The pack has no leap or fall, so those are made of poses
// from its other clips: [s, clip, s into that clip], eased from one to the next.
const GIANTS = { Knight_Golden_Male: "knight", Pirate_Male: "pirate", Goblin_Male: "goblin", Zombie_Male: "zombie" };
const GIANT_CLIPS = { idle: "idle", walk: "walk", shoot_onehanded: "shoot", victory: "hello", recievehit: "hitrecieve_1" };
// a crouch, up with its arms raised, down onto its hands, and back up
const LEAP = [[0, "idle", 0], [0.2, "pickup", 0.3], [0.4, "victory", 0.85], [0.62, "pickup", 0.6], [0.95, "pickup", 0.9], [1.3, "idle", 0]];
// its feet leave the deck and land (s; PERSON's lift and land in lib.rs) on an arc this high, 3.5 m at
// PERSON's scale of 3.4: the rig's root bone goes up, as the feet hang from it
const LEAP_UP = { bone: "Bone", from: 0.3, to: 0.63, height: 3.5 / 3.4 };
// caught by a hit and held stiff (the last frame stays), as the game tips it over
const FALL = [[0, "recievehit", 0], [0.2, "recievehit", 0.2]];

async function giants() {
  const doc = await characters(GIANTS, "giant");
  const clips = new Map(doc.getRoot().listAnimations().map((a) => [a.getName(), a]));
  posed(doc, clips, "jump", LEAP, LEAP_UP);
  posed(doc, clips, "death", FALL);
  for (const [n, a] of clips) {
    if (GIANT_CLIPS[n]) a.setName(GIANT_CLIPS[n]);
    else drop(a);
  }
  // a boss is one big figure close up: it keeps most of its triangles
  await finish(doc, "giants.glb", { simplify: 0.6 });
}

/** Where a clip has a node's translation, rotation or scale at t (s), or its rest. */
function sampleAt(anim, node, path, t) {
  const ch = anim.listChannels().find((c) => c.getTargetNode() === node && c.getTargetPath() === path);
  const rest = restOf(node, path);
  if (!ch) return rest.slice();
  const times = ch.getSampler().getInput().getArray(), out = ch.getSampler().getOutput().getArray(), n = rest.length;
  let i = 0;
  while (i < times.length - 2 && times[i + 1] <= t) i++;
  const u = Math.min(1, Math.max(0, (t - times[i]) / (times[i + 1] - times[i] || 1)));
  return mix(path, Array.from(out.subarray(i * n, i * n + n)), Array.from(out.subarray((i + 1) * n, (i + 1) * n + n)), u);
}

/** a to b by u, along the short way round for a rotation */
function mix(path, a, b, u) {
  if (path !== "rotation") return a.map((x, k) => x + (b[k] - x) * u);
  const flip = a.reduce((s, x, k) => s + x * b[k], 0) < 0 ? -1 : 1;
  const q = a.map((x, k) => x + (b[k] * flip - x) * u);
  const len = Math.hypot(...q);
  return q.map((x) => x / len);
}

/** keys a posed clip has from one pose to the next */
const POSE_STEPS = 6;

/**
 * Makes clip `name` through `poses`, eased in and out of each. A `lift`
 * raises its bone along the world's up from one time to another, on an arc.
 */
function posed(doc, clips, name, poses, lift) {
  const buffer = doc.getRoot().listBuffers()[0];
  const anim = doc.createAnimation(name);
  const targets = new Map();
  for (const [, c] of poses) for (const ch of clips.get(c).listChannels()) targets.set(`${ch.getTargetNode().getName()} ${ch.getTargetPath()}`, [ch.getTargetNode(), ch.getTargetPath()]);
  const bone = lift && doc.getRoot().listNodes().find((n) => n.getName() === lift.bone);
  if (lift && !bone) throw new Error(`${name}: no ${lift.bone} to lift`);
  if (bone) targets.set(`${lift.bone} translation`, [bone, "translation"]);
  const times = new Set(lift ? [lift.from, lift.to] : []);
  for (let k = 0; k + 1 < poses.length; k++) for (let j = 0; j < POSE_STEPS; j++) times.add(poses[k][0] + ((poses[k + 1][0] - poses[k][0]) * j) / POSE_STEPS);
  times.add(poses.at(-1)[0]);
  // each key: the pose it is after, the eased share of the way on to the next, and how high the lift has it
  const keys = [...times].sort((a, b) => a - b).map((t) => {
    let k = 0;
    while (k + 2 < poses.length && poses[k + 1][0] <= t) k++;
    const u = Math.min(1, (t - poses[k][0]) / (poses[k + 1][0] - poses[k][0]));
    const v = lift ? Math.min(1, Math.max(0, (t - lift.from) / (lift.to - lift.from))) : 0;
    return { t, k, u: u * u * (3 - 2 * u), up: lift ? lift.height * 4 * v * (1 - v) : 0 };
  });
  const input = doc.createAccessor().setType("SCALAR").setArray(new Float32Array(keys.map((key) => key.t))).setBuffer(buffer);
  const up = bone && upIn(bone);
  for (const [node, path] of targets.values()) {
    const at = poses.map(([, c, t]) => sampleAt(clips.get(c), node, path, t));
    const raised = node === bone && path === "translation";
    const values = keys.flatMap(({ k, u, up: h }) => {
      const v = mix(path, at[k], at[k + 1], u);
      return raised ? v.map((x, i) => x + up[i] * h) : v;
    });
    const output = doc.createAccessor().setType(path === "rotation" ? "VEC4" : "VEC3").setArray(new Float32Array(values)).setBuffer(buffer);
    const sampler = doc.createAnimationSampler().setInput(input).setOutput(output).setInterpolation("LINEAR");
    anim.addSampler(sampler).addChannel(doc.createAnimationChannel().setTargetNode(node).setTargetPath(path).setSampler(sampler));
  }
}

/** The world's up, one long, in the space a node's translation is in (its parent's, turned and scaled the same each way). */
function upIn(node) {
  const m = node.getParentNode()?.getWorldMatrix() ?? [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
  // for a turn and an even scale, the inverse is the transpose over the scale squared
  const s2 = m[0] ** 2 + m[1] ** 2 + m[2] ** 2;
  return [m[1] / s2, m[5] / s2, m[9] / s2];
}

// ---------- bosses: Quaternius' Animated Mech Pack ----------
const MECHS = { George: "george", Leela: "leela", Mike: "mike", Stan: "stan" };
const MECH_CLIPS = ["idle", "walk", "shoot", "jump", "hello", "hitrecieve_1", "death"];
const TEXTURE = 512;

/** Moves every root node and animation of `from` into `doc`. */
async function mergeInto(doc, from) {
  const { mergeDocuments } = await import("@gltf-transform/functions");
  const map = mergeDocuments(doc, from);
  const scene = doc.getRoot().listScenes()[0];
  for (const s of from.getRoot().listScenes()) {
    const copy = map.get(s);
    for (const n of copy.listChildren()) scene.addChild(n);
    copy.dispose();
  }
  // one buffer for the file
  const [keep, ...others] = doc.getRoot().listBuffers();
  for (const b of others) {
    for (const a of doc.getRoot().listAccessors()) if (a.getBuffer() === b) a.setBuffer(keep);
    b.dispose();
  }
}

async function mechs() {
  const dir = join(pack("mechs"), "Animated Mech Pack - March 2021");
  const doc = new Document();
  doc.createBuffer();
  doc.createScene();
  for (const [name, id] of Object.entries(MECHS)) {
    const m = await readFbx(join(dir, "FBX", `${name}.fbx`));
    oneOfEachClip(m);
    for (const a of m.getRoot().listAnimations()) if (!MECH_CLIPS.includes(a.getName())) drop(a);
    m.getRoot().listNodes().find((n) => n.getMesh()).setName(`mech.${id}`);
    const jpg = await sharp(join(dir, "Textures", `${name}_Texture.png`)).resize(TEXTURE, TEXTURE).jpeg({ quality: 85 }).toBuffer();
    const tex = m.createTexture(`${id}`).setImage(jpg).setMimeType("image/jpeg");
    for (const mat of m.getRoot().listMaterials()) mat.setBaseColorTexture(tex).setBaseColorFactor([1, 1, 1, 1]).setMetallicFactor(0).setRoughnessFactor(0.5);
    await mergeInto(doc, m);
  }
  await finish(doc, "mechs.glb");
}

// ---------- weapons: Quaternius' Ultimate Gun Pack ----------
const GUNS = { Pistol_1: "pistol", SubmachineGun_1: "smg", AssaultRifle_1: "rifle", Shotgun_1: "shotgun", SniperRifle_1: "sniper" };

async function guns() {
  const dir = join(pack("guns"), "Ultimate Gun Pack - July 2019", "FBX");
  const doc = new Document();
  doc.createBuffer();
  doc.createScene();
  for (const [name, id] of Object.entries(GUNS)) {
    const g = await readFbx(join(dir, `${name}.fbx`));
    g.getRoot().listNodes().find((n) => n.getMesh()).setName(`gun.${id}`);
    for (const m of g.getRoot().listMaterials()) m.setMetallicFactor(0).setRoughnessFactor(0.4);
    await mergeInto(doc, g);
  }
  // each soldier holds one
  await finish(doc, "guns.glb", { simplify: 0.2 });
}

// ---------- pier and shore: Kenney's Pirate Kit, boats: Kenney's Watercraft Kit ----------
const PIRATE = [
  "structure-platform", "structure-platform-dock", "barrel", "crate", "cannon-ball", "flag-pirate-high",
  "palm-detailed-bend", "palm-detailed-straight", "rocks-a", "rocks-b", "rocks-c", "rocks-sand-a",
  "patch-sand-foliage", "patch-grass-foliage", "ship-pirate-large", "ship-pirate-medium", "ship-wreck",
  "tower-watch", "tower-complete-large", "castle-wall",
];
const WATERCRAFT = ["gate", "gate-finish", "buoy-flag", "boat-speed-a", "boat-sail-a", "boat-fishing-small", "boat-tug-a", "ship-cargo-a"];

async function kit(packName, names, prefix, file) {
  const dir = join(pack(packName), "Models", "GLB format");
  const doc = new Document();
  doc.createBuffer();
  doc.createScene();
  for (const name of names) {
    const k = await io.read(join(dir, `${name}.glb`));
    const roots = k.getRoot().listScenes()[0].listChildren();
    if (roots.length !== 1) throw new Error(`${name} has ${roots.length} roots`);
    roots[0].setName(`${prefix}.${name}`);
    await mergeInto(doc, k);
  }
  await finish(doc, file);
}

// ---------- effects: Kenney's Particle Pack, as one picture ----------
/** Sprites in the order the game names them (fx.ts), 128 px each, 8 to a row. */
const FX = [
  "smoke_01", "smoke_04", "smoke_07", "smoke_10", "flare_01", "star_04", "circle_02", "circle_05",
  "light_01", "scorch_02", "dirt_02", "circle_01", "light_03",
];
const CELL = 128;

async function effects() {
  const dir = join(pack("particles"), "PNG (Transparent)");
  const cols = 8, rows = Math.ceil(FX.length / cols);
  const parts = await Promise.all(
    FX.map(async (n, i) => ({
      // white sprites: the alpha is the picture
      input: await sharp(join(dir, `${n}.png`)).ensureAlpha().resize(CELL, CELL).toBuffer(),
      left: (i % cols) * CELL,
      top: Math.floor(i / cols) * CELL,
    })),
  );
  const png = await sharp({ create: { width: cols * CELL, height: rows * CELL, channels: 4, background: { r: 0, g: 0, b: 0, alpha: 0 } } })
    .composite(parts)
    .png({ compressionLevel: 9 })
    .toBuffer();
  writeFileSync(join(OUT, "fx.png"), png);
  console.log(`fx.png: ${(png.length / 1024).toFixed(0)} KB`);
}

mkdirSync(OUT, { recursive: true });
const only = process.argv.slice(2);
const jobs = { figures, giants, mechs, guns, pier: () => kit("pirate", PIRATE, "pier", "pier.glb"), boats: () => kit("watercraft", WATERCRAFT, "boat", "boats.glb"), effects };
for (const [name, job] of Object.entries(jobs)) if (!only.length || only.includes(name)) await job();
// served next to what it builds, for the Sources panel and the credits to link
copyFileSync(fileURLToPath(import.meta.url), join(OUT, "pack.mjs"));
