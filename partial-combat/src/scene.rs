//! What the page draws in 3D each frame (the layout is in the site's
//! src/scripts/engines/scene3d/abi.ts; keep the two in step): every model
//! where it stands, sprites, the numbers over things, and the effects started
//! since the last frame. Built from the game as it stands when the page asks
//! (wg3d_scene), so none of it is saved.
use crate::*;

// effects (abi.ts FX_*)
pub const FX_MUZZLE: u32 = 1;
pub const FX_HIT: u32 = 2;
pub const FX_BLAST: u32 = 3;
pub const FX_SPLASH: u32 = 4;
pub const FX_PUFF: u32 = 5;
pub const FX_GATE: u32 = 6;
pub const FX_PICKUP: u32 = 7;
pub const FX_SMOKE: u32 = 8;
pub const FX_STOMP: u32 = 9;
pub const FX_CONFETTI: u32 = 10;
pub const FX_CANNON: u32 = 11;

// sprites and labels
const SPRITE_BILLBOARD: u8 = 0;
const SPRITE_GROUND: u8 = 1;
const SPRITE_PANEL: u8 = 2;
const SPRITE_STREAK: u8 = 3;
const PICTURE_BOX: u8 = 255;
const PIC_FLARE: u8 = 7;
const PIC_RING: u8 = 14;
const PIC_GLOW: u8 = 15;
const PIC_LIGHT: u8 = 16;
const LABEL_PILL: u8 = 0;
const LABEL_GATE: u8 = 1;
const LABEL_BAR: u8 = 3;
const INST_LOOP: u8 = 1;
const INST_NO_SHADOW: u8 = 2;
const NO_HOLDER: u16 = 0xffff;

/// colours as the page reads them: red in the low byte, alpha in the high one
pub const fn rgba(hex: u32, a: u8) -> u32 {
    ((hex >> 16) & 255) | (hex & 0xff00) | ((hex & 255) << 16) | ((a as u32) << 24)
}
pub const GREEN: u32 = rgba(0x96c172, 240);
pub const GREEN_LIGHT: u32 = rgba(0x8fdc5a, 255);
pub const TAN: u32 = rgba(0xd8b47a, 240);
const RUNNER: u32 = rgba(0xc77d4f, 235);
const RIFLE: u32 = rgba(0xb0925e, 240);
const BRUTE: u32 = rgba(0xa39c90, 225);
const BARREL_BLUE: u32 = rgba(0x83baeb, 190);
const TNT_RED: u32 = rgba(0xe9826e, 210);
const GOOD: u32 = rgba(0x2f8fff, 255);
const BAD: u32 = rgba(0xff3b3b, 255);
/// the bosses are tan plastic like the army they lead, each a shade of its own
const BOSS_TINTS: [u32; 4] = [rgba(0x9a7a4e, 225), rgba(0x86684a, 225), rgba(0xa88a5c, 225), rgba(0x7c6444, 225)];
/// a mech's size as a boss (about 3.5 m tall at 1)
const BOSS_SCALE: f32 = 1.7;
/// the pier's planks, toned down from the kit's orange
const PLANKS: u32 = rgba(0x8f7556, 150);

/// The models the game names, by index, as the page finds them in its files.
pub const MODELS: [&str; 41] = [
    "fig.soldier", "fig.beret", "fig.viking", "fig.ninja",
    "gun.pistol", "gun.smg", "gun.rifle", "gun.shotgun", "gun.sniper",
    "mech.george", "mech.leela", "mech.mike", "mech.stan",
    "pier.structure-platform", "pier.structure-platform-dock", "pier.barrel", "pier.crate", "pier.cannon-ball",
    "pier.flag-pirate-high", "pier.palm-detailed-bend", "pier.palm-detailed-straight", "pier.rocks-a", "pier.rocks-b",
    "pier.rocks-c", "pier.rocks-sand-a", "pier.patch-sand-foliage", "pier.patch-grass-foliage", "pier.ship-pirate-large",
    "pier.ship-pirate-medium", "pier.ship-wreck", "pier.tower-complete-large", "pier.castle-wall", "pier.tower-watch",
    "boat.gate", "boat.gate-finish", "boat.buoy-flag", "boat.boat-speed-a", "boat.boat-sail-a",
    "boat.boat-fishing-small", "boat.boat-tug-a", "boat.ship-cargo-a",
];
const M_SOLDIER: u16 = 0;
const M_BERET: u16 = 1;
const M_VIKING: u16 = 2;
const M_NINJA: u16 = 3;
const M_GUN: u16 = 4;
const M_MECH: u16 = 9;
const M_DECK: u16 = 13;
const M_DOCK: u16 = 14;
const M_BARREL: u16 = 15;
const M_CRATE: u16 = 16;
const M_BALL: u16 = 17;
const M_FLAG: u16 = 18;
const M_PALM: u16 = 19;
const M_ROCKS: u16 = 21;
const M_SAND: u16 = 25;
const M_GRASS: u16 = 26;
const M_SHIP: u16 = 27;
const M_SHIP_MEDIUM: u16 = 28;
const M_TOWER: u16 = 30;
const M_WALL: u16 = 31;
const M_WATCH: u16 = 32;
const M_ARCH: u16 = 33;
const M_FINISH: u16 = 34;
const M_BUOY: u16 = 35;
const M_BOAT: u16 = 36;
const M_CARGO: u16 = 40;

pub const CLIPS: [&str; 10] = ["idle", "walk", "shoot_onehanded", "recievehit", "defeat", "victory", "punch", "shoot", "hitrecieve_1", "death"];
const C_IDLE: u16 = 0;
const C_WALK: u16 = 1;
const C_AIM: u16 = 2;
const C_FALL: u16 = 3;
const C_DOWN: u16 = 4;
const C_CHEER: u16 = 5;
const C_PUNCH: u16 = 6;
const C_SHOOT: u16 = 7;
const C_DEATH: u16 = 9;

/// soldiers' size: the models are about 1.9 m tall
const FIG: f32 = 0.55;
/// how fast the walk plays, for a squad that runs
const STRIDE: f32 = 2.3;
const PI: f32 = core::f32::consts::PI;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Inst {
    model: u16,
    clip: u16,
    holder: u16,
    flash: u8,
    flags: u8,
    t: f32,
    pos: [f32; 3],
    yaw: f32,
    pitch: f32,
    roll: f32,
    scale: f32,
    tint: u32,
    fade: f32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Sprite {
    kind: u8,
    picture: u8,
    add: u8,
    _pad: u8,
    pos: [f32; 3],
    w: f32,
    h: f32,
    a: f32,
    b: f32,
    rgba: u32,
    _zero: u32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Label {
    kind: u8,
    sign: u8,
    _pad: u16,
    pos: [f32; 3],
    size: f32,
    value: i32,
    rgba: u32,
    opacity: f32,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Fx {
    pub kind: u32,
    pub pos: [f32; 3],
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub rgba: u32,
}

/// The lists the header points at, kept between frames so they aren't made anew.
#[derive(Default)]
pub struct Scene {
    header: [u32; 20],
    inst: Vec<Inst>,
    sprites: Vec<Sprite>,
    labels: Vec<Label>,
    fx: Vec<Fx>,
    names: Vec<u8>,
    clips: Vec<u8>,
}

fn names(list: &[&str]) -> Vec<u8> {
    let mut v = Vec::new();
    for n in list {
        v.extend_from_slice(n.as_bytes());
        v.push(0);
    }
    v.push(0);
    v
}

/// A number from a place, the same every frame: decor is placed by it.
fn hash2(a: i32, b: i32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9e37_79b1) ^ (b as u32).wrapping_mul(0x85eb_ca77);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h
}
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

impl Scene {
    fn put(&mut self, model: u16, clip: u16, t: f32, x: f32, y: f32, d: f32, yaw: f32, scale: f32, tint: u32) -> u16 {
        self.inst.push(Inst { model, clip, holder: NO_HOLDER, flash: 0, flags: INST_LOOP, t, pos: [x, y, -d], yaw, pitch: 0.0, roll: 0.0, scale, tint, fade: 1.0 });
        (self.inst.len() - 1) as u16
    }
    fn last(&mut self) -> &mut Inst {
        self.inst.last_mut().unwrap()
    }
    /// A gun in the hand of the figure just put.
    fn gun(&mut self, gun: i32, holder: u16, fade: f32) {
        let scale = WEAPONS[gun.clamp(0, 4) as usize].scale;
        self.inst.push(Inst { model: M_GUN + gun as u16, clip: 0, holder, flash: 0, flags: INST_NO_SHADOW, t: 0.0, pos: [0.0; 3], yaw: -PI / 2.0, pitch: 0.0, roll: 0.0, scale, tint: 0, fade });
    }
    fn sprite(&mut self, kind: u8, picture: u8, add: bool, x: f32, y: f32, d: f32, w: f32, h: f32, a: f32, b: f32, rgba: u32) {
        self.sprites.push(Sprite { kind, picture, add: add as u8, _pad: 0, pos: [x, y, -d], w, h, a, b, rgba, _zero: 0 });
    }
    fn label(&mut self, kind: u8, sign: u8, x: f32, y: f32, d: f32, size: f32, value: i32, rgba: u32, opacity: f32) {
        self.labels.push(Label { kind, sign, _pad: 0, pos: [x, y, -d], size, value, rgba, opacity });
    }
}

fn sign_of(op: i32) -> u8 {
    [b'+', b'-', b'x', b'/'][op.clamp(0, 3) as usize]
}

impl Combat {
    /// Moves the camera on after the squad (called each update, dt seconds).
    pub fn follow(&mut self, dt: f32, aspect: f32) {
        let s = &self.s;
        let tall = (1.0 - aspect).clamp(0.0, 0.6);
        let (cx, sd) = (s.cx, s.d);
        let boss = s.phase == P_BOSS || (s.phase == P_CLEAR && s.arena_d > 0.0);
        let want = if boss {
            [cx * 0.4, DECK + 11.0 + tall * 5.0, -(sd - 12.5 - tall * 5.0), s.boss.x * 0.3, DECK + 2.6, -(sd + 10.0)]
        } else {
            [cx * 0.55, DECK + 9.0 + tall * 4.0, -(sd - 11.0 - tall * 4.0), cx * 0.65, DECK + 0.3, -(sd + 9.0)]
        };
        let k = if self.cam_set { 1.0 - (-4.0 * dt).exp() } else { 1.0 };
        for i in 0..6 {
            // the camera keeps up with the run, and eases only the rest
            let lag = if i == 2 || i == 5 { (want[i] - self.cam[i]) * k.max(if self.cam_set { 0.25 } else { 1.0 }) } else { (want[i] - self.cam[i]) * k };
            self.cam[i] += lag;
        }
        self.cam_set = true;
        self.clock += dt;
    }

    /// The scene as it stands; returns the header's address.
    pub fn build(&mut self, w: i32, h: i32) -> *const u32 {
        self.aspect = w as f32 / h.max(1) as f32;
        let mut sc = core::mem::take(&mut self.scene);
        sc.inst.clear();
        sc.sprites.clear();
        sc.labels.clear();
        sc.fx.clear();
        sc.fx.append(&mut self.fx);
        if sc.names.is_empty() {
            sc.names = names(&MODELS);
            sc.clips = names(&CLIPS);
        }
        if !self.cam_set {
            self.follow(0.0, self.aspect);
        }
        self.put_pier(&mut sc);
        self.put_decor(&mut sc);
        self.put_things(&mut sc);
        self.put_squad(&mut sc);
        self.put_enemies(&mut sc);
        self.put_boss(&mut sc);
        self.put_shots(&mut sc);
        let fov = 52.0 + (1.0 - self.aspect).clamp(0.0, 0.6) * 22.0;
        let theme = [0, 2, 1, 3][(self.s.level.max(0) % 4) as usize];
        let f = |v: f32| v.to_bits();
        let c = self.cam;
        sc.header = [
            1,
            sc.inst.as_ptr() as u32,
            sc.inst.len() as u32,
            sc.sprites.as_ptr() as u32,
            sc.sprites.len() as u32,
            sc.labels.as_ptr() as u32,
            sc.labels.len() as u32,
            sc.fx.as_ptr() as u32,
            sc.fx.len() as u32,
            f(c[0]),
            f(c[1]),
            f(c[2]),
            f(c[3]),
            f(c[4]),
            f(c[5]),
            f(fov),
            f(self.clock),
            theme,
            f(self.s.shake),
            0,
        ];
        self.scene = sc;
        self.scene.header.as_ptr()
    }

    /// The pier's planks from behind the squad to as far as can be seen, with the holes left out.
    fn put_pier(&self, sc: &mut Scene) {
        let s = &self.s;
        let first = ((s.d - 16.0) / TILE).floor().max(-6.0) as i32;
        let mut last = ((s.d + 80.0) / TILE).ceil() as i32;
        let arena = s.things().iter().find(|t| t.kind == K_ARENA).copied();
        if let Some(a) = arena {
            if !s.endless() {
                last = last.min(((a.d + a.len) / TILE).ceil() as i32);
            }
        }
        for row in first..last {
            let d = (row as f32 + 0.5) * TILE;
            for lane in 0..LANES {
                let x = -HALF + (lane as f32 + 0.5) * TILE;
                if !s.deck(x, d) {
                    continue;
                }
                let h = hash2(row, lane);
                let model = if h % 11 == 0 { M_DOCK } else { M_DECK };
                let yaw = if h & 16 != 0 { PI } else { 0.0 };
                sc.put(model, 0, 0.0, x, 0.0, d, yaw, 1.0, PLANKS);
            }
        }
        if let Some(a) = arena {
            // flags at the arena's corners, the finish arch behind the boss
            for side in [-1.0, 1.0] {
                sc.put(M_FLAG, 0, 0.0, side * (HALF - 0.5), DECK, a.d + 1.0, if side < 0.0 { 0.0 } else { PI }, 0.8, 0);
            }
            sc.put(M_FINISH, 0, 0.0, 0.0, DECK, a.d + a.len - 2.0, 0.0, 1.62, 0);
            if !s.endless() {
                // a fort where the pier ends
                let end = a.d + a.len + 4.0;
                for side in [-1.0f32, 1.0] {
                    sc.put(M_TOWER, 0, 0.0, side * 7.5, 0.0, end, 0.0, 1.1, 0);
                    for k in 0..3 {
                        sc.put(M_WALL, 0, 0.0, side * (10.5 + k as f32 * 2.2), 0.0, end + 0.5, PI / 2.0, 1.1, 0);
                    }
                }
            }
        }
    }

    /// The sea round the pier: buoys, rocks, islands with palms, boats and ships, the same each time past.
    fn put_decor(&self, sc: &mut Scene) {
        let s = &self.s;
        let seed = if s.endless() { 7 } else { s.level * 31 + 3 };
        let first = ((s.d - 20.0) / 12.0).floor() as i32;
        let last = ((s.d + 130.0) / 12.0).ceil() as i32;
        let t = self.clock;
        for seg in first..last {
            for side in [-1.0f32, 1.0] {
                let h = hash2(seg * 2 + (side > 0.0) as i32, seed);
                let d = seg as f32 * 12.0 + unit(h >> 3) * 10.0;
                if seg % 2 == 0 {
                    // buoys along the pier
                    let bob = (t * 1.6 + seg as f32).sin() * 0.08;
                    sc.put(M_BUOY, 0, 0.0, side * 7.2, bob - 0.15, seg as f32 * 12.0, 0.0, 0.9, 0);
                    sc.last().flags |= INST_NO_SHADOW;
                }
                let pick = h % 100;
                let far = unit(hash2(seg, seed + 1 + (side > 0.0) as i32));
                let bob = (t * 1.1 + seg as f32 * 0.7).sin();
                match pick {
                    0..=17 => {
                        let x = side * (11.0 + far * 7.0);
                        sc.put(M_ROCKS + (h % 4) as u16, 0, 0.0, x, -0.3, d, unit(h) * 6.3, 1.0 + far * 0.6, 0);
                    }
                    18..=31 => {
                        // an island with a palm or two
                        let x = side * (16.0 + far * 8.0);
                        sc.put(if h & 1 == 0 { M_SAND } else { M_GRASS }, 0, 0.0, x, -0.12, d, unit(h) * 6.3, 1.4, 0);
                        sc.put(M_PALM + (h >> 4 & 1) as u16, 0, 0.0, x - side * 1.2, 0.0, d, unit(h >> 5) * 6.3, 1.2, 0);
                        if h & 64 != 0 {
                            sc.put(M_PALM, 0, 0.0, x + side * 1.4, 0.0, d + 1.2, unit(h >> 7) * 6.3, 1.0, 0);
                        }
                    }
                    32..=43 => {
                        let x = side * (10.0 + far * 9.0);
                        sc.put(M_BOAT + (h >> 6 & 3) as u16, 0, 0.0, x, bob * 0.1 - 0.05, d, if side < 0.0 { 0.3 } else { PI - 0.3 }, 1.2, 0);
                        sc.last().roll = bob * 0.05;
                    }
                    44..=51 => {
                        let x = side * (26.0 + far * 14.0);
                        let ship = [M_SHIP, M_SHIP_MEDIUM, M_CARGO, M_SHIP + 2][(h >> 6 & 3) as usize];
                        sc.put(ship, 0, 0.0, x, bob * 0.15 - 0.1, d, if side < 0.0 { 0.2 } else { PI + 0.2 }, 1.6, 0);
                        sc.last().roll = bob * 0.03;
                    }
                    52..=55 => {
                        sc.put(M_WATCH, 0, 0.0, side * (13.0 + far * 4.0), -0.2, d, unit(h) * 6.3, 1.0, 0);
                    }
                    _ => {}
                }
            }
        }
    }

    fn put_things(&self, sc: &mut Scene) {
        let s = &self.s;
        let t = self.clock;
        for th in s.things() {
            if th.d > s.d + 85.0 {
                continue;
            }
            match th.kind {
                K_GATE => {
                    // gates the squad has passed fade
                    let fade = if th.state == 0 { 1.0 } else { (1.0 - (s.d - th.d) / 2.5).clamp(0.0, 1.0) };
                    if fade <= 0.0 {
                        continue;
                    }
                    let good = match th.a {
                        OP_ADD | OP_MUL => true,
                        _ => false,
                    } && !(th.a == OP_ADD && th.value == 0);
                    let col = if good { GOOD } else { BAD };
                    sc.put(M_ARCH, 0, 0.0, th.x, DECK, th.d, 0.0, th.w / 6.2, col & 0x00ff_ffff | 0xd800_0000);
                    sc.last().fade = fade;
                    sc.last().flash = (th.flash * 120.0) as u8;
                    let panel = if good { rgba(0x3a9bff, 95) } else { rgba(0xff4646, 95) };
                    sc.sprite(SPRITE_PANEL, PICTURE_BOX, false, th.x, DECK, th.d + 0.05, th.w - 0.7, 3.0, 0.0, 0.0, panel & 0x00ff_ffff | (((panel >> 24) as f32 * fade) as u32) << 24);
                    let pulse = 1.0 + th.flash * 0.12;
                    sc.label(LABEL_GATE, sign_of(th.a), th.x, DECK + 1.75, th.d, 1.25 * pulse, th.value, col, fade);
                }
                K_BARREL | K_TNT | K_CRATE if th.hp > 0 => {
                    let flash = (th.flash * 160.0) as u8;
                    match th.kind {
                        K_BARREL => {
                            sc.put(M_BARREL, 0, 0.0, th.x, DECK, th.d, th.d * 0.7, 0.85, BARREL_BLUE);
                            sc.last().flash = flash;
                            sc.label(LABEL_PILL, b'+', th.x, DECK + 1.75, th.d, 0.62, th.value, rgba(0x2f7fe0, 255), 1.0);
                        }
                        K_TNT => {
                            sc.put(M_BARREL, 0, 0.0, th.x, DECK, th.d, th.d, 0.85, TNT_RED);
                            sc.last().flash = flash;
                            // its fuse
                            let blink = 0.6 + 0.4 * (t * 14.0 + th.d).sin();
                            sc.sprite(SPRITE_BILLBOARD, PIC_FLARE, true, th.x, DECK + 1.18, th.d, 0.9 * blink, 0.9 * blink, t * 3.0, 0.0, rgba(0xffa040, 255));
                        }
                        _ => {
                            sc.put(M_CRATE, 0, 0.0, th.x, DECK, th.d, 0.0, 1.15, 0);
                            sc.last().flash = flash;
                            // the gun inside, turning over it
                            let gun = th.value.clamp(0, 4);
                            sc.put(M_GUN + gun as u16, 0, 0.0, th.x, DECK + 1.5 + (t * 2.0).sin() * 0.1, th.d, t * 1.6, WEAPONS[gun as usize].scale * 1.6, 0);
                            sc.sprite(SPRITE_BILLBOARD, PIC_GLOW, true, th.x, DECK + 1.5, th.d, 1.6, 1.6, 0.0, 0.0, rgba(0x60c0ff, 150));
                        }
                    }
                    if th.kind != K_TNT || th.hp < th.hp_max {
                        sc.label(LABEL_BAR, 5, th.x, DECK + 1.12, th.d, 0.16, th.hp * 1000 / th.hp_max.max(1), if th.kind == K_CRATE { rgba(0x40d0ff, 255) } else { rgba(0x5ad05a, 255) }, 1.0);
                    }
                }
                K_SHIP => {
                    let bob = (t * 1.2 + th.d).sin();
                    sc.put(M_SHIP_MEDIUM, 0, 0.0, th.x, bob * 0.12 - 0.1, th.d, if th.x < 0.0 { PI / 2.0 } else { -PI / 2.0 }, 1.1, 0);
                    sc.last().roll = bob * 0.03;
                }
                K_GROUP => {
                    let (mut n, mut x, mut d) = (0, 0.0, 0.0);
                    for e in s.enemies() {
                        if e.group >= 0 && s.thing[e.group as usize].d == th.d && e.state != ES_DOWN {
                            n += 1;
                            x += e.x;
                            d += e.d;
                        }
                    }
                    if n > 0 {
                        sc.label(LABEL_PILL, 0, x / n as f32, DECK + 2.3, d / n as f32 + 0.6, 0.85, n, rgba(0xc0392b, 255), 1.0);
                    }
                }
                _ => {}
            }
        }
    }

    fn put_squad(&self, sc: &mut Scene) {
        let s = &self.s;
        let cheer = s.phase == P_CLEAR || s.phase == P_WON;
        for q in s.soldiers() {
            let (clip, t, fade, y) = match q.state {
                S_OK if cheer => (C_CHEER, q.t, 1.0, DECK),
                S_OK if s.phase == P_BOSS => (C_AIM, q.t * 1.3, 1.0, DECK),
                S_OK => (C_WALK, q.t * STRIDE, 1.0, DECK),
                S_FALL => (C_FALL, q.t, 1.0, q.y),
                _ => (C_DOWN, q.t * 1.6, (1.6 - q.t).clamp(0.0, 1.0), DECK),
            };
            let at = sc.put(M_SOLDIER, clip, t, q.x, y, q.d, PI, FIG, GREEN);
            let i = sc.last();
            i.fade = fade;
            i.flash = (q.flash * 200.0) as u8;
            if q.state != S_OK {
                i.flags = 0;
            }
            if q.state != S_FALL || q.y > 0.0 {
                sc.gun(s.weapon, at, fade);
            }
        }
        if s.count > 0 && s.phase != P_DEAD {
            let n = s.soldiers().iter().filter(|q| q.state == S_OK).count().max(1);
            let (x, d) = s.soldiers().iter().filter(|q| q.state == S_OK).fold((0.0, 0.0), |a, q| (a.0 + q.x, a.1 + q.d));
            sc.label(LABEL_PILL, 0, x / n as f32, DECK + 2.0 + radius(s.count) * 0.2, d / n as f32 - 0.4, 0.95, s.count, rgba(0x3f8f2a, 255), 1.0);
        }
    }

    fn put_enemies(&self, sc: &mut Scene) {
        let s = &self.s;
        for e in s.enemies() {
            if e.d > s.d + 85.0 {
                continue;
            }
            let (model, tint, scale, gun) = match e.kind {
                E_RUNNER => (M_NINJA, RUNNER, FIG, -1),
                E_RIFLE => (M_BERET, RIFLE, FIG, 2),
                E_BRUTE => (M_VIKING, BRUTE, FIG * 1.65, -1),
                _ => (M_SOLDIER, TAN, FIG, 0),
            };
            let (clip, t, fade, flags) = match e.state {
                ES_WAIT if e.kind == E_RIFLE => (C_AIM, e.t, 1.0, INST_LOOP),
                ES_WAIT => (C_IDLE, e.t, 1.0, INST_LOOP),
                ES_CHARGE => (if e.kind == E_BRUTE { C_PUNCH } else { C_WALK }, e.t * if e.kind == E_RUNNER { 3.2 } else { 2.4 }, 1.0, INST_LOOP),
                _ => (C_DOWN, e.t * 1.5, (1.8 - e.t).clamp(0.0, 1.0), 0),
            };
            let at = sc.put(model, clip, t, e.x, DECK, e.d, e.yaw, scale, tint);
            let i = sc.last();
            i.fade = fade;
            i.flags = flags;
            i.flash = (e.flash * 220.0) as u8;
            if gun >= 0 {
                sc.gun(gun, at, fade);
            }
            if e.kind == E_BRUTE && e.state != ES_DOWN {
                sc.label(LABEL_BAR, 4, e.x, DECK + 2.1, e.d, 0.16, (e.hp * 1000 / (6 + (s.hardness() * 0.3) as i32).max(1)).min(1000), rgba(0xe04030, 255), 1.0);
            }
        }
    }

    fn put_boss(&self, sc: &mut Scene) {
        let b = &self.s.boss;
        if b.state == 0 || b.d > self.s.d + 85.0 {
            return;
        }
        let (clip, t, flags) = match (b.state, b.atk) {
            (3, _) => (C_DEATH, b.atk_t, 0),
            (1, _) => (C_IDLE, b.t, INST_LOOP),
            (_, 1) | (_, 3) => (C_SHOOT, b.atk_t * 1.4, INST_LOOP),
            (_, 2) | (_, 4) => (C_PUNCH, b.atk_t, 0),
            _ => (C_WALK, b.t * 1.2, INST_LOOP),
        };
        let fade = if self.s.phase == P_CLEAR || self.s.phase == P_WON { (1.0 - self.s.phase_ms as f32 / 1500.0).clamp(0.0, 1.0) } else { 1.0 };
        sc.put(M_MECH + b.kind as u16, clip, t, b.x, DECK, b.d, 0.0, BOSS_SCALE, BOSS_TINTS[b.kind.clamp(0, 3) as usize]);
        let i = sc.last();
        i.flags = flags;
        i.flash = (b.flash * 70.0) as u8;
        i.fade = fade;
    }

    fn put_shots(&self, sc: &mut Scene) {
        let s = &self.s;
        for b in &s.bul[..s.nbul as usize] {
            let col = if b.side == 0 { rgba(0xffd25a, 255) } else { rgba(0xff5a3a, 255) };
            sc.sprite(SPRITE_STREAK, PIC_LIGHT, true, b.x, b.y, b.d, 0.16, 1.1, b.vx, -b.vd, col);
        }
        for sh in &s.shell[..s.nshell as usize] {
            let u = (sh.t / sh.t_max).clamp(0.0, 1.0);
            if u <= 0.0 {
                continue;
            }
            let red = rgba(0xff3020, (120.0 + 120.0 * u) as u8);
            sc.sprite(SPRITE_GROUND, PIC_RING, false, sh.x, DECK + 0.05, sh.d, sh.r * 2.2, sh.r * 2.2, 0.0, 0.0, red);
            sc.sprite(SPRITE_GROUND, PIC_GLOW, true, sh.x, DECK + 0.06, sh.d, sh.r * 2.4 * u, sh.r * 2.4 * u, 0.0, 0.0, rgba(0xff5030, 200));
            if sh.kind == 0 {
                sc.put(M_BALL, 0, 0.0, sh.x, DECK + (1.0 - u) * 14.0, sh.d, 0.0, 1.6, 0);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn wg3d_scene() -> *const u32 {
    let rt = crate::WG_GAME.get();
    let (w, h) = (rt.screen.sw, rt.screen.sh);
    rt.game.build(w, h)
}

#[unsafe(no_mangle)]
pub extern "C" fn wg3d_models() -> *const u8 {
    let g = &mut crate::WG_GAME.get().game;
    if g.scene.names.is_empty() {
        g.scene.names = names(&MODELS);
    }
    g.scene.names.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn wg3d_clips() -> *const u8 {
    let g = &mut crate::WG_GAME.get().game;
    if g.scene.clips.is_empty() {
        g.scene.clips = names(&CLIPS);
    }
    g.scene.clips.as_ptr()
}
