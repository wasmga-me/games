// Builds the models and effect pictures Partial Combat loads
// (public/games/partial-combat/*.glb and fx.png, or $COMBAT_OUT) from the CC0
// packs they come from, only the models, clips and pictures the game uses:
//   npm install && node pack.mjs [soldiers giants mechs guns pier boats snow nature effects]
// The packs are downloaded once into $COMBAT_CACHE (a temp folder by
// default). Quaternius' character, mech and gun packs come as FBX, turned
// into glTF by FBX2glTF. Every model is named under a prefix (man., giant.,
// mech., gun., pier., boat., snow., nature.), after its pack's own name but for
// the soldiers, which are kinds of one mannequin; the game asks for them by
// those names. The gates' poles are made here (gate.pole).
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Document, NodeIO } from "@gltf-transform/core";
import { ALL_EXTENSIONS } from "@gltf-transform/extensions";
import { dedup, joinPrimitives, meshopt, normals, prune, quantize, resample, simplify, simplifyPrimitive, weld, weldPrimitive } from "@gltf-transform/functions";
import { MeshoptDecoder, MeshoptEncoder, MeshoptSimplifier } from "meshoptimizer";
import sharp from "sharp";

const here = dirname(fileURLToPath(import.meta.url));
const OUT = process.env.COMBAT_OUT || join(here, "../../../public/games/partial-combat");
const CACHE = process.env.COMBAT_CACHE || join(tmpdir(), "partial-combat-assets");
const FBX2GLTF = join(here, "node_modules/fbx2gltf/bin", process.platform === "darwin" ? "Darwin" : "Linux", "FBX2glTF");

/** The packs, as their authors publish them (all CC0). */
const PACKS = {
  characters: "https://opengameart.org/sites/default/files/ultimate_animated_character_pack_by_quaternius.zip",
  animations: "https://opengameart.org/sites/default/files/universal_animation_librarystandard.zip",
  mechs: "https://opengameart.org/sites/default/files/animated_mech_pack_-_march_2021.zip",
  guns: "https://opengameart.org/sites/default/files/ultimate_gun_pack_by_quaternius.zip",
  pirate: "https://kenney.nl/media/pages/assets/pirate-kit/e6d4bb1525-1771333093/kenney_pirate-kit.zip",
  watercraft: "https://kenney.nl/media/pages/assets/watercraft-kit/a335cfed49-1713519620/kenney_watercraft-pack.zip",
  particles: "https://kenney.nl/media/pages/assets/particle-pack/f8fe0f8cb8-1677578741/kenney_particle-pack.zip",
  holiday: "https://kenney.nl/media/pages/assets/holiday-kit/3976a6496a-1733923970/kenney_holiday-kit.zip",
  nature: "https://kenney.nl/media/pages/assets/nature-kit/37ac38a37b-1677698939/kenney_nature-kit.zip",
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
 * joined first and the faces' normals made again after. `volume` "scene"
 * quantizes every mesh in one space, so meshes that share parts share them
 * in the file too.
 */
async function finish(doc, file, { simplify: ratio, error = 0.02, volume = "mesh" } = {}) {
  dropStillChannels(doc);
  if (ratio) for (const p of doc.getRoot().listMeshes().flatMap((m) => m.listPrimitives())) p.getAttribute("NORMAL")?.dispose();
  await doc.transform(
    dedup(),
    weld(),
    ...(ratio ? [simplify({ simplifier: MeshoptSimplifier, ratio, error, lockBorder: true }), normals(), weld()] : []),
    resample({ tolerance: 1e-4 }),
    prune({ keepLeaves: true }),
    quantize({ quantizePosition: 14, quantizeNormal: 10, quantizeTexcoord: 12, quantizeWeight: 8, quantizationVolume: volume }),
    meshopt({ encoder: MeshoptEncoder, level: "medium", quantizationVolume: volume }),
  );
  const bytes = await io.writeBinary(doc);
  writeFileSync(join(OUT, file), bytes);
  console.log(`${file}: ${(bytes.length / 1024).toFixed(0)} KB`);
}

// ---------- toy soldiers: the mannequin of Quaternius' Universal Animation Library ----------
// One smooth figure on a full rig: in the game's tint it reads as a moulded
// army man. Each kind gets headgear made here and skinned to the head, and
// the clips the game plays go by its names for them.
const SOLDIERS = { soldier: "helmet", runner: "band", rifle: "beret" };
const SOLDIER_CLIPS = {
  Idle_Loop: "idle",
  Pistol_Idle_Loop: "hold",
  Sprint_Loop: "walk",
  Pistol_Shoot: "shoot_onehanded",
  Death01: "defeat",
  Dance_Loop: "victory",
  Punch_Jab: "punch",
};
/** Columns round a piece of headgear. */
const AROUND = 14;

async function soldiers() {
  const doc = await io.read(find(pack("animations"), "/AnimationLibrary_Godot_Standard.glb"));
  const root = doc.getRoot();
  for (const a of root.listAnimations()) {
    const name = SOLDIER_CLIPS[a.getName()];
    if (name) a.setName(name);
    else drop(a);
  }
  const node = root.listNodes().find((n) => n.getMesh());
  const skin = node.getSkin();
  const joints = skin.listJoints();
  // the fingers too small to see move: held in the run's fist, with no keys of their own
  const walk = root.listAnimations().find((a) => a.getName() === "walk");
  const fingers = new Set(joints.filter((j) => /^DEF-(f_|thumb)/.test(j.getName())));
  for (const f of fingers) f.setRotation(sampleAt(walk, f, "rotation", 0));
  for (const a of root.listAnimations()) {
    for (const ch of a.listChannels()) {
      if (fingers.has(ch.getTargetNode())) {
        ch.getSampler().dispose();
        ch.dispose();
      }
    }
  }
  // a held gun hangs from the joint the view knows by the other rig's name
  joints.find((j) => j.getName() === "DEF-hand.R").setName("Fist.R");
  const plastic = doc.createMaterial("plastic").setBaseColorFactor([0.8, 0.8, 0.8, 1]).setMetallicFactor(0).setRoughnessFactor(0.6);
  // the body in one part of one plastic, without the texture coordinates
  // nothing reads, its corners joined by where they are and how they move
  const parts = node.getMesh().listPrimitives();
  for (const p of parts) {
    for (const a of ["TEXCOORD_0", "TEXCOORD_1", "NORMAL"]) p.setAttribute(a, null);
    p.setMaterial(plastic);
  }
  const body = joinPrimitives(parts);
  weldPrimitive(body);
  // a squad is hundreds of them: about a thousand triangles each
  simplifyPrimitive(body, { simplifier: MeshoptSimplifier, ratio: 0.07, error: 0.05 });
  smoothNormals(doc, body);
  const j = joints.findIndex((n) => n.getName() === "DEF-head");
  const head = extentOf(body, j);
  for (const [kind, hat] of Object.entries(SOLDIERS)) {
    const mesh = doc.createMesh(kind).addPrimitive(body.clone()).addPrimitive(headgear(doc, hat, head, j, plastic));
    node.getParentNode().addChild(doc.createNode(`man.${kind}`).setMesh(mesh).setSkin(skin));
  }
  node.dispose();
  await finish(doc, "soldiers.glb", { volume: "scene" });
}

/** The normal of the triangle at corners a, b, c of `pos` (x, y, z after each other), as long as twice its size. */
function faceNormal(pos, a, b, c) {
  const u = [0, 1, 2].map((e) => pos[b * 3 + e] - pos[a * 3 + e]), v = [0, 1, 2].map((e) => pos[c * 3 + e] - pos[a * 3 + e]);
  return [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
}

/** Normals for a smooth surface: at each corner, its faces' normals weighted by their size. */
function smoothNormals(doc, prim) {
  const pos = prim.getAttribute("POSITION").getArray(), ix = prim.getIndices().getArray();
  const n = new Float32Array(pos.length);
  for (let i = 0; i < ix.length; i += 3) {
    const f = faceNormal(pos, ix[i], ix[i + 1], ix[i + 2]);
    for (const k of ix.subarray(i, i + 3)) for (let e = 0; e < 3; e++) n[k * 3 + e] += f[e];
  }
  for (let i = 0; i < n.length; i += 3) {
    const l = Math.hypot(n[i], n[i + 1], n[i + 2]) || 1;
    for (let e = 0; e < 3; e++) n[i + e] /= l;
  }
  prim.setAttribute("NORMAL", accessor(doc, "VEC3", n));
}

/** The middle and half sizes of the part of `prim` that moves with joint `j` most. */
function extentOf(prim, j) {
  const pos = prim.getAttribute("POSITION").getArray();
  const jt = prim.getAttribute("JOINTS_0").getArray(), wt = prim.getAttribute("WEIGHTS_0").getArray();
  const lo = [Infinity, Infinity, Infinity], hi = [-Infinity, -Infinity, -Infinity];
  for (let v = 0; v < pos.length / 3; v++) {
    let k = 0;
    for (let e = 1; e < 4; e++) if (wt[v * 4 + e] > wt[v * 4 + k]) k = e;
    if (jt[v * 4 + k] !== j) continue;
    for (let e = 0; e < 3; e++) {
      lo[e] = Math.min(lo[e], pos[v * 3 + e]);
      hi[e] = Math.max(hi[e], pos[v * 3 + e]);
    }
  }
  if (lo[0] === Infinity) throw new Error(`no part moves with joint ${j}`);
  return { c: lo.map((l, e) => (l + hi[e]) / 2), h: lo.map((l, e) => (hi[e] - l) / 2) };
}

/**
 * A helmet, a beret or a headband for a head (its middle c and half sizes h,
 * the face towards +z), all of it moving with the head's joint.
 */
function headgear(doc, kind, { c, h }, j, material) {
  const pos = [], nor = [], ix = [];
  // a point on the ellipsoid round o with radii r, at unit direction u, turned by tilt about z
  const put = (o, r, u, tilt = 0) => {
    const [cs, sn] = [Math.cos(tilt), Math.sin(tilt)];
    const p = [r[0] * u[0], r[1] * u[1], r[2] * u[2]];
    const m = [u[0] / r[0], u[1] / r[1], u[2] / r[2]];
    const l = Math.hypot(...m) || 1;
    pos.push(o[0] + p[0] * cs - p[1] * sn, o[1] + p[0] * sn + p[1] * cs, o[2] + p[2]);
    nor.push((m[0] * cs - m[1] * sn) / l, (m[0] * sn + m[1] * cs) / l, m[2] / l);
    return pos.length / 3 - 1;
  };
  // a triangle turned to face the way its corners' normals point
  const tri = (a, b, d) => {
    const f = faceNormal(pos, a, b, d);
    const want = [0, 1, 2].reduce((s, e) => s + f[e] * (nor[a * 3 + e] + nor[b * 3 + e] + nor[d * 3 + e]), 0);
    ix.push(...(want >= 0 ? [a, b, d] : [a, d, b]));
  };
  const ring = (o, r, lat, tilt = 0) => {
    const first = pos.length / 3;
    for (let k = 0; k < AROUND; k++) {
      const b = (k / AROUND) * 2 * Math.PI;
      put(o, r, [Math.cos(lat) * Math.sin(b), Math.sin(lat), Math.cos(lat) * Math.cos(b)], tilt);
    }
    return first;
  };
  const band = (a, b) => {
    for (let k = 0; k < AROUND; k++) {
      const k1 = (k + 1) % AROUND;
      tri(a + k, a + k1, b + k1);
      tri(a + k, b + k1, b + k);
    }
  };
  // an ellipsoid's top (y 1) or bottom (y -1) from latitude lat0 to its pole
  const dome = (o, r, lat0, rings, tilt = 0, y = 1) => {
    let last = ring(o, r, y * lat0, tilt);
    for (let i = 1; i < rings; i++) {
      const next = ring(o, r, y * (lat0 + ((Math.PI / 2 - lat0) * i) / rings), tilt);
      band(last, next);
      last = next;
    }
    const pole = put(o, r, [0, y, 0], tilt);
    for (let k = 0; k < AROUND; k++) tri(last + k, last + ((k + 1) % AROUND), pole);
  };
  const at = (x, y, z) => [c[0] + x * h[0], c[1] + y * h[1], c[2] + z * h[2]];
  const size = (x, y, z) => [x * h[0], y * h[1], z * h[2]];
  if (kind === "helmet") {
    // the dome down to the brow and the ears, its rim flared out a little
    // (one sheet: the view lights a face's back too)
    const o = at(0, 0.12, -0.06), r = size(1.22, 0.95, 1.2), brow = -0.12;
    dome(o, r, brow, 4);
    const first = pos.length / 3;
    for (const [w, lat] of [[1, brow], [1.08, -0.3]]) {
      for (let k = 0; k < AROUND; k++) {
        const b = (k / AROUND) * 2 * Math.PI;
        pos.push(o[0] + r[0] * w * Math.cos(brow) * Math.sin(b), o[1] + r[1] * Math.sin(lat), o[2] + r[2] * w * Math.cos(brow) * Math.cos(b));
        nor.push(Math.sin(b) * 0.7, 0.7, Math.cos(b) * 0.7);
      }
    }
    band(first, first + AROUND);
  } else if (kind === "beret") {
    // a soft flat disc pulled down to one side
    const o = at(0.18, 0.72, -0.1), r = size(1.26, 0.27, 1.2);
    dome(o, r, 0, 3, -0.32);
    dome(o, r, 0, 2, -0.32, -1);
  } else {
    // a band round the brow
    const o = at(0, 0.32, -0.04), r = size(1.09, 1, 1.07);
    band(ring(o, r, -0.12), ring(o, r, 0.12));
  }
  const n = pos.length / 3;
  return doc
    .createPrimitive()
    .setAttribute("POSITION", accessor(doc, "VEC3", new Float32Array(pos)))
    .setAttribute("NORMAL", accessor(doc, "VEC3", new Float32Array(nor)))
    .setAttribute("JOINTS_0", accessor(doc, "VEC4", new Uint16Array(n * 4).map((_, i) => (i % 4 ? 0 : j))))
    .setAttribute("WEIGHTS_0", accessor(doc, "VEC4", new Float32Array(n * 4).map((_, i) => (i % 4 ? 0 : 1))))
    .setIndices(accessor(doc, "SCALAR", new Uint16Array(ix)))
    .setMaterial(material);
}

/** An accessor of `array` in the document's one buffer. */
function accessor(doc, type, array) {
  return doc.createAccessor().setType(type).setArray(array).setBuffer(doc.getRoot().listBuffers()[0]);
}

// ---------- giants: Quaternius' Ultimate Animated Character Pack ----------
// Every character there shares one rig, so a file holds one skeleton and its
// animations, and each character's mesh is skinned to it.

/** The characters (pack name: our name) in one document, as giant.<name>, with their clips. */
async function characters(names) {
  const dir = join(pack("characters"), "Ultimate Animated Character Pack - Nov 2019", "FBX");
  const [first, ...rest] = Object.keys(names);
  const doc = await readFbx(join(dir, `${first}.fbx`));
  const root = doc.getRoot();
  const buffer = root.listBuffers()[0];
  const skin = root.listSkins()[0];
  const meshNode = root.listNodes().find((n) => n.getMesh());
  meshNode.setName(`giant.${names[first]}`);
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
    const node = doc.createNode(`giant.${names[name]}`).setMesh(mesh).setSkin(skin);
    if (parent) parent.addChild(node);
    else scene.addChild(node);
  }
  for (const m of root.listMaterials()) m.setMetallicFactor(0).setRoughnessFactor(0.6);
  oneOfEachClip(doc);
  return doc;
}

// The bosses that are people: four of the pack's characters, made giant.
// Their clips go by the mechs' names, so a boss asks for the same ones
// whichever it is. The pack has no leap or fall, so those are made of poses
// from its other clips: [s, clip, s into that clip], eased from one to the next.
const GIANTS = { Knight_Golden_Male: "knight", Pirate_Male: "pirate", Goblin_Male: "goblin", Zombie_Male: "zombie" };
const GIANT_CLIPS = { idle: "idle", walk: "walk", recievehit: "hitrecieve_1" };
// a crouch, up with its arms raised, down onto its hands, and back up
const LEAP = [[0, "idle", 0], [0.2, "pickup", 0.3], [0.4, "victory", 0.85], [0.62, "pickup", 0.6], [0.95, "pickup", 0.9], [1.3, "idle", 0]];
// its feet leave the deck and land (s; the second is PERSON's land in lib.rs) on an arc this high, 3.5 m at
// PERSON's scale of 3.4: the rig's root bone goes up, as the feet hang from it
const LEAP_UP = { bone: "Bone", from: 0.3, to: 0.63, height: 3.5 / 3.4 };
// caught by a hit and held stiff (the last frame stays), as the game tips it over
const FALL = [[0, "recievehit", 0], [0.2, "recievehit", 0.2]];

async function giants() {
  const doc = await characters(GIANTS);
  const clips = new Map(doc.getRoot().listAnimations().map((a) => [a.getName(), a]));
  posed(doc, clips, "jump", LEAP, LEAP_UP);
  posed(doc, clips, "death", FALL);
  for (const [n, a] of clips) {
    if (GIANT_CLIPS[n]) a.setName(GIANT_CLIPS[n]);
    else drop(a);
  }
  // a boss is one big figure close up: it keeps most of its triangles
  await finish(doc, "giants.glb", { simplify: 0.6, volume: "scene" });
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
const MECH_CLIPS = ["idle", "walk", "jump", "hitrecieve_1", "death"];
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
  "structure-platform", "structure-platform-dock", "barrel", "crate", "cannon-ball",
  "palm-detailed-bend", "palm-detailed-straight", "rocks-a", "rocks-b", "rocks-c", "rocks-sand-a",
  "patch-sand-foliage", "patch-grass-foliage", "ship-pirate-large", "ship-pirate-medium", "ship-wreck",
  "tower-watch",
];
const WATERCRAFT = ["buoy-flag", "boat-speed-a", "boat-sail-a", "boat-fishing-small", "boat-tug-a", "ship-cargo-a"];
// the other shores: snow from Kenney's Holiday Kit; desert and autumn lake from Kenney's Nature Kit
const SNOW = ["tree-snow-a", "tree-snow-b", "tree-snow-c", "snowman-hat", "snow-flat-large", "rocks-large", "rocks-medium", "reindeer", "sled"];
const NATURE = [
  "cactus_tall", "cactus_short", "rock_tallA", "rock_tallB", "rock_tallC", "rock_tallD", "statue_obelisk", "statue_head",
  "statue_columnDamaged", "tent_detailedOpen", "log_stack", "canoe", "rock_largeA", "rock_largeC",
  "tree_oak_fall", "tree_detailed_fall", "tree_default_fall", "tree_fat_fall",
];

async function kit(packName, names, prefix, file, made) {
  // each kit keeps its .glb files in one of its Models folders
  const dir = join(pack(packName), "Models");
  const doc = new Document();
  doc.createBuffer();
  doc.createScene();
  for (const name of names) {
    const glb = find(dir, `/${name}.glb`);
    if (!glb) throw new Error(`${packName}: no ${name}.glb`);
    const k = await io.read(glb);
    const roots = k.getRoot().listScenes()[0].listChildren();
    if (roots.length !== 1) throw new Error(`${name} has ${roots.length} roots`);
    roots[0].setName(`${prefix}.${name}`);
    await mergeInto(doc, k);
  }
  made?.(doc);
  await finish(doc, file);
}

/**
 * A pole a gate's light curtain shines across to the next (gate.pole), 3 m
 * tall on its foot: a chamfered post with a lamp and a cap on top, in one
 * plain material the game tints the gate's colour.
 */
function pole(doc) {
  const pos = [], ix = [];
  // a flat face, its corners turned to face away from `mid`
  const face = (pts, mid) => {
    const first = pos.length / 3;
    for (const p of pts) pos.push(...p);
    const f = faceNormal(pos, first, first + 1, first + 2);
    const out = [0, 1, 2].reduce((s, e) => s + f[e] * (pts[0][e] - mid[e]), 0) >= 0;
    for (let k = 1; k + 1 < pts.length; k++) ix.push(...(out ? [first, first + k, first + k + 1] : [first, first + k + 1, first + k]));
  };
  // upright square prisms, half w across with their corners cut by c, from
  // height y0 to y1: the foot, the post, the lamp and the cap
  for (const [w, c, y0, y1] of [[0.2, 0.05, 0, 0.06], [0.09, 0.03, 0.06, 2.9], [0.105, 0.035, 2.9, 3.02], [0.12, 0.035, 3.02, 3.08]]) {
    const ring = [[w, w - c], [w - c, w], [c - w, w], [-w, w - c], [-w, c - w], [c - w, -w], [w - c, -w], [w, c - w]];
    const mid = [0, (y0 + y1) / 2, 0];
    ring.forEach(([a, b], k) => {
      const [e, f] = ring[(k + 1) % ring.length];
      face([[a, y0, b], [e, y0, f], [e, y1, f], [a, y1, b]], mid);
    });
    face(ring.map(([a, b]) => [a, y1, b]), mid);
    face(ring.map(([a, b]) => [a, y0, b]), mid);
  }
  const material = doc.createMaterial("pole").setBaseColorFactor([0.7, 0.7, 0.7, 1]).setMetallicFactor(0).setRoughnessFactor(0.5);
  const prim = doc
    .createPrimitive()
    .setAttribute("POSITION", accessor(doc, "VEC3", new Float32Array(pos)))
    .setIndices(accessor(doc, "SCALAR", new Uint16Array(ix)))
    .setMaterial(material);
  // each face has corners of its own, so this gives it its flat normal
  smoothNormals(doc, prim);
  doc.getRoot().listScenes()[0].addChild(doc.createNode("gate.pole").setMesh(doc.createMesh("pole").addPrimitive(prim)));
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
const jobs = {
  soldiers,
  giants,
  mechs,
  guns,
  pier: () => kit("pirate", PIRATE, "pier", "pier.glb"),
  boats: () => kit("watercraft", WATERCRAFT, "boat", "boats.glb", pole),
  snow: () => kit("holiday", SNOW, "snow", "snow.glb"),
  nature: () => kit("nature", NATURE, "nature", "nature.glb"),
  effects,
};
for (const [name, job] of Object.entries(jobs)) if (!only.length || only.includes(name)) await job();
// served next to what it builds, for the Sources panel and the credits to link
copyFileSync(fileURLToPath(import.meta.url), join(OUT, "pack.mjs"));
