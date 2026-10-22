//! What the page draws in 3D each frame (the layout is in the site's
//! src/scripts/engines/scene3d/abi.ts; keep the two in step): every model
//! where it stands, sprites, the numbers over things, and the effects started
//! since the last frame. Built from the game as it stands when the page asks
//! (wg3d_scene), so none of it is saved.
use crate::*;

/// the layout's version (abi.ts VERSION)
const SCENE_VERSION: u32 = 2;

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
pub const FX_SHOT: u32 = 12;
pub const FX_INCOMING: u32 = 13;
pub const FX_STEP: u32 = 14;
pub const FX_CLANG: u32 = 15;

// sprites and labels
const SPRITE_BILLBOARD: u8 = 0;
const SPRITE_GROUND: u8 = 1;
const SPRITE_PANEL: u8 = 2;
const SPRITE_STREAK: u8 = 3;
const SPRITE_OVER: u8 = 1;
const PICTURE_BOX: u8 = 255;
// pictures in fx.png (fx.ts PIC)
const PIC_FLARE: u8 = 4;
const PIC_RING: u8 = 6;
const PIC_GLOW: u8 = 7;
const PIC_LIGHT: u8 = 8;
const PIC_CIRCLE: u8 = 11;
const LABEL_PILL: u8 = 0;
const LABEL_GATE: u8 = 1;
const LABEL_PLAIN: u8 = 2;
const LABEL_BAR: u8 = 3;
const INST_LOOP: u8 = 1;
const INST_NO_SHADOW: u8 = 2;
const NO_HOLDER: u16 = 0xffff;

/// colours as the page reads them: red in the low byte, alpha in the high one
pub const fn rgba(hex: u32, a: u8) -> u32 {
    wg::hex(hex) & 0x00ff_ffff | (a as u32) << 24
}
pub const GREEN: u32 = rgba(0x4f8a2b, 240);
pub const GREEN_LIGHT: u32 = rgba(0x8fdc5a, 255);
pub const TAN: u32 = rgba(0xb17732, 240);
const RUNNER: u32 = rgba(0x943513, 235);
const RIFLE: u32 = rgba(0x714b1c, 240);
const BRUTE: u32 = rgba(0x5f5749, 225);
const BARREL_BLUE: u32 = rgba(0x3b7fd5, 190);
const TNT_RED: u32 = rgba(0xd13a28, 210);
const GOOD: u32 = rgba(0x0647ff, 255);
const BAD: u32 = rgba(0xff0a0a, 255);

/// The models the game names, by index, as the page finds them in its files.
pub const MODELS: [&str; 72] = [
    "fig.soldier",
    "fig.beret",
    "fig.viking",
    "fig.ninja",
    "gun.pistol",
    "gun.smg",
    "gun.rifle",
    "gun.shotgun",
    "gun.sniper",
    "mech.george",
    "mech.leela",
    "mech.mike",
    "mech.stan",
    "giant.knight",
    "giant.pirate",
    "giant.goblin",
    "giant.zombie",
    "pier.structure-platform",
    "pier.structure-platform-dock",
    "pier.barrel",
    "pier.crate",
    "pier.cannon-ball",
    "pier.flag-pirate-high",
    "pier.palm-detailed-bend",
    "pier.palm-detailed-straight",
    "pier.rocks-a",
    "pier.rocks-b",
    "pier.rocks-c",
    "pier.rocks-sand-a",
    "pier.patch-sand-foliage",
    "pier.patch-grass-foliage",
    "pier.ship-pirate-large",
    "pier.ship-pirate-medium",
    "pier.ship-wreck",
    "pier.tower-complete-large",
    "pier.castle-wall",
    "pier.tower-watch",
    "boat.gate",
    "boat.gate-finish",
    "boat.buoy-flag",
    "boat.boat-speed-a",
    "boat.boat-sail-a",
    "boat.boat-fishing-small",
    "boat.boat-tug-a",
    "boat.ship-cargo-a",
    "snow.tree-snow-a",
    "snow.tree-snow-b",
    "snow.tree-snow-c",
    "snow.snowman-hat",
    "snow.snow-flat-large",
    "snow.rocks-large",
    "snow.rocks-medium",
    "snow.reindeer",
    "snow.sled",
    "nature.cactus_tall",
    "nature.cactus_short",
    "nature.rock_tallA",
    "nature.rock_tallB",
    "nature.rock_tallC",
    "nature.rock_tallD",
    "nature.statue_obelisk",
    "nature.statue_head",
    "nature.statue_columnDamaged",
    "nature.tent_detailedOpen",
    "nature.log_stack",
    "nature.canoe",
    "nature.rock_largeA",
    "nature.rock_largeC",
    "nature.tree_oak_fall",
    "nature.tree_detailed_fall",
    "nature.tree_default_fall",
    "nature.tree_fat_fall",
];
/// A model's place in MODELS, looked up as the game is built.
pub const fn model(name: &str) -> u16 {
    let want = name.as_bytes();
    let mut i = 0;
    while i < MODELS.len() {
        let have = MODELS[i].as_bytes();
        let mut k = 0;
        while k < have.len() && k < want.len() && have[k] == want[k] {
            k += 1;
        }
        if k == have.len() && k == want.len() {
            return i as u16;
        }
        i += 1;
    }
    panic!("not in MODELS");
}
const M_SOLDIER: u16 = model("fig.soldier");
const M_BERET: u16 = model("fig.beret");
const M_VIKING: u16 = model("fig.viking");
const M_NINJA: u16 = model("fig.ninja");
const M_GUN: u16 = model("gun.pistol");
const M_DECK: u16 = model("pier.structure-platform");
const M_DOCK: u16 = model("pier.structure-platform-dock");
const M_BARREL: u16 = model("pier.barrel");
const M_CRATE: u16 = model("pier.crate");
const M_BALL: u16 = model("pier.cannon-ball");
const M_FLAG: u16 = model("pier.flag-pirate-high");
const M_SHIP_MEDIUM: u16 = model("pier.ship-pirate-medium");
const M_TOWER: u16 = model("pier.tower-complete-large");
const M_WALL: u16 = model("pier.castle-wall");
const M_ARCH: u16 = model("boat.gate");
const M_FINISH: u16 = model("boat.gate-finish");
const M_BUOY: u16 = model("boat.buoy-flag");

/// Models to pick from, each with the size it is drawn at, and their tint.
/// One listed twice comes up twice as often.
struct Set {
    models: &'static [(u16, f32)],
    tint: u32,
}
const fn set(models: &'static [(u16, f32)]) -> Set {
    Set { models, tint: 0 }
}
const fn tinted(models: &'static [(u16, f32)], tint: u32) -> Set {
    Set { models, tint }
}
impl Set {
    fn pick(&self, h: u32) -> (u16, f32) {
        self.models[h as usize % self.models.len()]
    }
}

/// A shore the pier runs past: its looks one time round after another
/// (view.ts THEMES), its planks' tint, and what stands in the water.
struct Place {
    looks: &'static [u32],
    planks: u32,
    /// rocks near the pier
    rocks: Set,
    /// islands: their ground, and the trees on them
    grounds: Set,
    trees: Set,
    boats: Set,
    /// the big things further out, how far (m) their middles stand off a
    /// 5 m pier's edge, and whether they float
    far: Set,
    far_x: f32,
    afloat: bool,
    /// now and then one more thing near the shore, on the first ground at
    /// this size, or in the water
    sights: Set,
    sight_ground: Option<f32>,
}
const TROPICS: Place = Place {
    looks: &[0, 2, 1, 3],
    // toned down from the kit's orange
    planks: rgba(0x472e17, 150),
    rocks: set(&[
        (model("pier.rocks-a"), 1.0),
        (model("pier.rocks-b"), 1.0),
        (model("pier.rocks-c"), 1.0),
        (model("pier.rocks-sand-a"), 1.0),
    ]),
    grounds: set(&[
        (model("pier.patch-sand-foliage"), 1.4),
        (model("pier.patch-grass-foliage"), 1.4),
    ]),
    trees: set(&[
        (model("pier.palm-detailed-bend"), 1.2),
        (model("pier.palm-detailed-straight"), 1.2),
    ]),
    boats: set(&[
        (model("boat.boat-speed-a"), 1.2),
        (model("boat.boat-sail-a"), 1.2),
        (model("boat.boat-fishing-small"), 1.2),
        (model("boat.boat-tug-a"), 1.2),
    ]),
    far: set(&[
        (model("pier.ship-pirate-large"), 1.6),
        (model("pier.ship-pirate-medium"), 1.6),
        (model("boat.ship-cargo-a"), 1.6),
        (model("pier.ship-wreck"), 1.6),
    ]),
    far_x: 26.0,
    afloat: true,
    sights: set(&[(model("pier.tower-watch"), 1.0)]),
    sight_ground: None,
};
const SNOW: Place = Place {
    looks: &[4, 5],
    planks: rgba(0x9fa7b0, 170),
    rocks: set(&[
        (model("snow.rocks-medium"), 2.4),
        (model("snow.rocks-large"), 1.3),
        (model("snow.rocks-medium"), 1.8),
        (model("snow.rocks-large"), 1.0),
    ]),
    grounds: set(&[(model("snow.snow-flat-large"), 3.5)]),
    trees: set(&[
        (model("snow.tree-snow-a"), 2.7),
        (model("snow.tree-snow-b"), 2.7),
        (model("snow.tree-snow-c"), 2.7),
    ]),
    boats: set(&[
        (model("boat.boat-fishing-small"), 1.2),
        (model("boat.boat-tug-a"), 1.2),
        (model("boat.boat-sail-a"), 1.2),
        (model("boat.boat-fishing-small"), 1.2),
    ]),
    // ice: the kit's snowy rocks, big and white
    far: tinted(
        &[
            (model("snow.rocks-large"), 4.5),
            (model("snow.rocks-medium"), 6.5),
            (model("snow.rocks-large"), 5.5),
            (model("snow.rocks-medium"), 8.0),
        ],
        rgba(0xe6f2fa, 210),
    ),
    far_x: 32.0,
    afloat: false,
    sights: set(&[
        (model("snow.snowman-hat"), 3.2),
        (model("snow.reindeer"), 3.0),
        (model("snow.sled"), 3.0),
    ]),
    sight_ground: Some(1.8),
};
const LAKE_BOATS: Set = set(&[
    (model("nature.canoe"), 3.5),
    (model("boat.boat-sail-a"), 1.2),
    (model("boat.boat-fishing-small"), 1.2),
    (model("nature.canoe"), 3.5),
]);
const SANDSTONE: u32 = rgba(0xc0703f, 150);
const DESERT: Place = Place {
    looks: &[6, 7],
    planks: rgba(0x8f7f6a, 180),
    rocks: tinted(
        &[
            (model("nature.rock_tallA"), 7.0),
            (model("nature.rock_tallB"), 7.0),
            (model("nature.rock_tallC"), 8.0),
            (model("nature.rock_tallD"), 8.0),
        ],
        SANDSTONE,
    ),
    grounds: set(&[(model("pier.patch-sand-foliage"), 1.6)]),
    trees: tinted(
        &[
            (model("nature.cactus_tall"), 7.5),
            (model("nature.cactus_short"), 7.5),
        ],
        rgba(0x5d7a3a, 130),
    ),
    boats: LAKE_BOATS,
    // the far bank's mesas
    far: tinted(
        &[
            (model("nature.rock_tallA"), 22.0),
            (model("nature.rock_tallB"), 24.0),
            (model("nature.rock_tallA"), 18.0),
            (model("nature.rock_tallB"), 20.0),
        ],
        SANDSTONE,
    ),
    far_x: 35.0,
    afloat: false,
    sights: tinted(
        &[
            (model("nature.statue_head"), 6.0),
            (model("nature.statue_obelisk"), 8.0),
            (model("nature.statue_columnDamaged"), 8.0),
        ],
        rgba(0xd9a066, 140),
    ),
    sight_ground: Some(0.9),
};
const AUTUMN: Place = Place {
    looks: &[8, 9],
    planks: rgba(0x4a4038, 200),
    rocks: set(&[
        (model("nature.rock_largeA"), 7.0),
        (model("nature.rock_largeC"), 7.0),
        (model("nature.rock_largeA"), 5.0),
        (model("nature.rock_largeC"), 5.0),
    ]),
    // grass gone over
    grounds: tinted(
        &[(model("pier.patch-grass-foliage"), 1.7)],
        rgba(0x8a7a3c, 150),
    ),
    trees: set(&[
        (model("nature.tree_oak_fall"), 5.5),
        (model("nature.tree_detailed_fall"), 5.5),
        (model("nature.tree_default_fall"), 4.5),
        (model("nature.tree_fat_fall"), 6.0),
    ]),
    boats: LAKE_BOATS,
    // boulders out in the lake
    far: set(&[
        (model("nature.rock_largeC"), 16.0),
        (model("nature.rock_largeA"), 16.0),
        (model("nature.rock_largeA"), 12.0),
        (model("nature.rock_largeC"), 12.0),
    ]),
    far_x: 32.0,
    afloat: false,
    sights: set(&[
        (model("nature.tent_detailedOpen"), 6.0),
        (model("nature.log_stack"), 6.0),
    ]),
    sight_ground: Some(1.0),
};
const PLACES: [Place; 4] = [TROPICS, SNOW, DESERT, AUTUMN];

/// The shore of a stretch of the run, and its light: one look after another each time round.
fn shore(stage: i32) -> (&'static Place, u32) {
    let (k, n) = (stage as usize, PLACES.len());
    let place = &PLACES[k % n];
    (place, place.looks[k / n % place.looks.len()])
}

pub const CLIPS: [&str; 12] = [
    "idle",
    "walk",
    "shoot_onehanded",
    "recievehit",
    "defeat",
    "victory",
    "punch",
    "shoot",
    "death",
    "jump",
    "hello",
    "hitrecieve_1",
];
const C_IDLE: u16 = 0;
const C_WALK: u16 = 1;
const C_AIM: u16 = 2;
const C_FALL: u16 = 3;
const C_DOWN: u16 = 4;
const C_CHEER: u16 = 5;
const C_PUNCH: u16 = 6;
const C_SHOOT: u16 = 7;
const C_DEATH: u16 = 8;
const C_JUMP: u16 = 9;
const C_WAVE: u16 = 10;
const C_REEL: u16 = 11;

/// soldiers' size: the models are about 1.9 m tall
const FIG: f32 = 0.55;
/// a gate's arch's size, before it is widened to the gate, and the model's width
const GATE_ARCH: f32 = 0.95;
const ARCH_W: f32 = 6.2;
/// the deck model is 2.5 m square and DECK high: scaled to a tile
const DECK_SCALE: f32 = TILE / 2.5;
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
    roll: f32,
    scale: f32,
    tint: u32,
    fade: f32,
    /// its own x scaled by this too, for a model stretched across
    wide: f32,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Sprite {
    kind: u8,
    picture: u8,
    add: u8,
    flags: u8,
    pos: [f32; 3],
    w: f32,
    h: f32,
    a: f32,
    b: f32,
    rgba: u32,
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
    /// how many rows from the top of the screen the HUD draws in (hud.rs)
    pub hud_rows: u32,
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
    pub fn new() -> Self {
        Scene {
            names: names(&MODELS),
            clips: names(&CLIPS),
            ..Default::default()
        }
    }
    fn put(
        &mut self,
        model: u16,
        clip: u16,
        t: f32,
        x: f32,
        y: f32,
        d: f32,
        yaw: f32,
        scale: f32,
        tint: u32,
    ) -> u16 {
        self.inst.push(Inst {
            model,
            clip,
            holder: NO_HOLDER,
            flash: 0,
            flags: INST_LOOP,
            t,
            pos: [x, y, -d],
            yaw,
            roll: 0.0,
            scale,
            tint,
            fade: 1.0,
            wide: 1.0,
        });
        (self.inst.len() - 1) as u16
    }
    fn last(&mut self) -> &mut Inst {
        self.inst.last_mut().unwrap()
    }
    /// A gun in the hand of the figure just put.
    fn gun(&mut self, gun: i32, holder: u16, fade: f32) {
        let scale = WEAPONS[gun.clamp(0, 4) as usize].scale;
        self.inst.push(Inst {
            model: M_GUN + gun as u16,
            clip: 0,
            holder,
            flash: 0,
            flags: INST_NO_SHADOW,
            t: 0.0,
            pos: [0.0; 3],
            yaw: -PI / 2.0,
            roll: 0.0,
            scale,
            tint: 0,
            fade,
            wide: 1.0,
        });
    }
    fn sprite(
        &mut self,
        kind: u8,
        picture: u8,
        add: bool,
        x: f32,
        y: f32,
        d: f32,
        w: f32,
        h: f32,
        a: f32,
        b: f32,
        rgba: u32,
    ) {
        self.sprites.push(Sprite {
            kind,
            picture,
            add: add as u8,
            flags: 0,
            pos: [x, y, -d],
            w,
            h,
            a,
            b,
            rgba,
        });
    }
    fn label(
        &mut self,
        kind: u8,
        sign: u8,
        x: f32,
        y: f32,
        d: f32,
        size: f32,
        value: i32,
        rgba: u32,
        opacity: f32,
    ) {
        self.labels.push(Label {
            kind,
            sign,
            _pad: 0,
            pos: [x, y, -d],
            size,
            value,
            rgba,
            opacity,
        });
    }
}

fn sign_of(op: i32) -> u8 {
    [b'+', b'-', b'x', b'/'][op.clamp(0, 3) as usize]
}

impl Combat {
    /// How much taller than wide the screen is, 0 to 0.6: the camera stands back and widens for it.
    fn tall(&self) -> f32 {
        (1.0 - self.aspect).clamp(0.0, 0.6)
    }

    /// Moves the camera on after the squad (called each update, dt seconds):
    /// high over the pier and far enough back to see across it.
    pub fn follow(&mut self, dt: f32) {
        let s = &self.s;
        let tall = self.tall();
        let (cx, sd) = (s.cx, s.d);
        let boss = s.phase == P_BOSS || (s.phase == P_CLEAR && s.arena_d > 0.0);
        // as far back as shows `half` either side, tall screens a little less
        let half = HALF + 1.0 - tall * 5.0;
        let fov = self.fov().to_radians();
        let far = (half / ((fov * 0.5).tan() * self.aspect.max(0.3))).clamp(15.0, 34.0);
        let (pitch, ahead, look_y, far, lx) = if boss {
            // back and up while the squad walks up to the arena, so its army and the boss fit
            let gap = (s.arena_d - sd).max(0.0);
            (
                0.58f32,
                8.0 + gap * 0.5,
                DECK + 3.2,
                far * 1.25 + gap * 0.6,
                s.boss.x * 0.3 + cx * 0.2,
            )
        } else {
            (0.84, far * 0.3, DECK, far, cx * 0.55)
        };
        let (sp, cp) = pitch.sin_cos();
        let want = [
            cx * 0.45,
            look_y + far * sp,
            -(sd + ahead - far * cp),
            lx,
            look_y,
            -(sd + ahead),
        ];
        let k = if self.cam_set {
            1.0 - (-4.0 * dt).exp()
        } else {
            1.0
        };
        for i in 0..6 {
            // the camera keeps up with the run, and eases only the rest
            let e = if i == 2 || i == 5 { k.max(0.25) } else { k };
            self.cam[i] += (want[i] - self.cam[i]) * e;
        }
        self.cam_set = true;
        self.clock += dt;
    }

    /// The camera's field of view, degrees up and down: wider on tall screens.
    fn fov(&self) -> f32 {
        52.0 + self.tall() * 22.0
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
        if !self.cam_set {
            self.follow(0.0);
        }
        self.put_pier(&mut sc);
        self.put_decor(&mut sc);
        self.put_things(&mut sc);
        self.put_squad(&mut sc);
        self.put_enemies(&mut sc);
        self.put_boss(&mut sc);
        self.put_shots(&mut sc);
        let fov = self.fov();
        // the light of the shore the squad is on
        let (_, theme) = shore(self.stage_at(self.s.d));
        let f = |v: f32| v.to_bits();
        let c = self.cam;
        sc.header = [
            SCENE_VERSION,
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
            sc.hud_rows,
        ];
        self.scene = sc;
        self.scene.header.as_ptr()
    }

    /// The pier's planks from behind the squad to as far as can be seen, with the holes left out.
    fn put_pier(&self, sc: &mut Scene) {
        let s = &self.s;
        let first = ((s.d - 16.0) / TILE).floor().max(-15.0 / TILE) as i32;
        let mut last = ((s.d + 80.0) / TILE).ceil() as i32;
        if let (false, Some(a)) = (s.endless(), s.arenas().next()) {
            last = last.min(((a.d + a.len) / TILE).ceil() as i32);
        }
        for row in first..last {
            let d = (row as f32 + 0.5) * TILE;
            let planks = self.place_at(d).planks;
            for lane in 0..LANES {
                let x = -HALF + (lane as f32 + 0.5) * TILE;
                if !s.deck(x, d) {
                    continue;
                }
                let h = hash2(row, lane);
                let model = if h % 11 == 0 { M_DOCK } else { M_DECK };
                let yaw = if h & 16 != 0 { PI } else { 0.0 };
                // sunk to keep its top at DECK
                sc.put(
                    model,
                    0,
                    0.0,
                    x,
                    DECK * (1.0 - DECK_SCALE),
                    d,
                    yaw,
                    DECK_SCALE,
                    planks,
                );
            }
        }
        for a in s.arenas() {
            // flags at the arena's corners, the finish arch behind the boss
            for side in [-1.0, 1.0] {
                sc.put(
                    M_FLAG,
                    0,
                    0.0,
                    side * (HALF - 0.5),
                    DECK,
                    a.d + 1.0,
                    if side < 0.0 { 0.0 } else { PI },
                    0.8,
                    0,
                );
            }
            sc.put(
                M_FINISH,
                0,
                0.0,
                0.0,
                DECK,
                a.d + a.len - 2.0,
                0.0,
                HALF * 0.28,
                0,
            );
            if !s.endless() {
                // a fort where the pier ends
                let end = a.d + a.len + 4.0;
                for side in [-1.0f32, 1.0] {
                    sc.put(M_TOWER, 0, 0.0, side * (HALF + 2.5), 0.0, end, 0.0, 1.3, 0);
                    for k in 0..3 {
                        sc.put(
                            M_WALL,
                            0,
                            0.0,
                            side * (HALF + 6.0 + k as f32 * 2.6),
                            0.0,
                            end + 0.5,
                            PI / 2.0,
                            1.3,
                            0,
                        );
                    }
                }
            }
        }
    }

    /// Which stretch of the run d is in: the level, or on the endless pier
    /// how many bosses are beaten before it (one stretch ends with each arena).
    fn stage_at(&self, d: f32) -> i32 {
        let s = &self.s;
        let mut k = s.level.max(0);
        if s.endless() {
            // level already counts the beaten arenas (those arena_d no longer marks)
            for a in s.arenas() {
                k += (a.d + a.len <= d) as i32 - (a.d != s.arena_d) as i32;
            }
        }
        k.max(0)
    }

    /// The shore along d.
    fn place_at(&self, d: f32) -> &'static Place {
        shore(self.stage_at(d)).0
    }

    /// The sea round the pier: buoys, and the shore's rocks, islands, boats and sights, the same each time past.
    fn put_decor(&self, sc: &mut Scene) {
        let s = &self.s;
        let seed = if s.endless() { 7 } else { s.level * 31 + 3 };
        let first = ((s.d - 20.0) / 12.0).floor() as i32;
        // no farther than where an arena ends when it is laid (track.rs AHEAD
        // + 36), so a shore never changes in view
        let ahead = s.d + 125.0;
        let last = (ahead / 12.0).ceil() as i32;
        let t = self.clock;
        // the sea starts this much further out than on a pier 5 m to a side
        let out = HALF - 5.0;
        // nothing stands in the water where a ship fires from
        let clear = |side: f32, d: f32| {
            !s.things()
                .iter()
                .any(|th| th.kind == K_SHIP && th.x * side > 0.0 && (th.d - d).abs() < 9.0)
        };
        for seg in first..last {
            for side in [-1.0f32, 1.0] {
                let h = hash2(seg * 2 + (side > 0.0) as i32, seed);
                let d = seg as f32 * 12.0 + unit(h >> 3) * 10.0;
                if seg % 2 == 0 && clear(side, seg as f32 * 12.0) {
                    // buoys along the pier
                    let bob = (t * 1.6 + seg as f32).sin() * 0.08;
                    sc.put(
                        M_BUOY,
                        0,
                        0.0,
                        side * (7.2 + out),
                        bob - 0.15,
                        seg as f32 * 12.0,
                        0.0,
                        0.9,
                        0,
                    );
                    sc.last().flags |= INST_NO_SHADOW;
                }
                if d > ahead || !clear(side, d) {
                    continue;
                }
                let place = self.place_at(d);
                let far = unit(hash2(seg, seed + 1 + (side > 0.0) as i32));
                let bob = (t * 1.1 + seg as f32 * 0.7).sin();
                match h % 100 {
                    0..=17 => {
                        let x = side * (11.0 + out + far * 7.0);
                        let (m, size) = place.rocks.pick(h);
                        let (yaw, size) = (unit(h) * 6.3, size * (1.0 + far * 0.6));
                        sc.put(m, 0, 0.0, x, -0.3, d, yaw, size, place.rocks.tint);
                    }
                    18..=31 => {
                        // an island with a tree or two
                        let x = side * (16.0 + out + far * 8.0);
                        let (ground, trees) = (&place.grounds, &place.trees);
                        let (m, size) = ground.pick(h);
                        sc.put(m, 0, 0.0, x, -0.12, d, unit(h) * 6.3, size, ground.tint);
                        let (m, size) = trees.pick(h >> 4);
                        let yaw = unit(h >> 5) * 6.3;
                        sc.put(m, 0, 0.0, x - side * 1.2, 0.0, d, yaw, size, trees.tint);
                        if h & 64 != 0 {
                            let (m, size) = trees.models[0];
                            let (x, yaw) = (x + side * 1.4, unit(h >> 7) * 6.3);
                            let size = size * 5.0 / 6.0;
                            sc.put(m, 0, 0.0, x, 0.0, d + 1.2, yaw, size, trees.tint);
                        }
                    }
                    32..=43 => {
                        let x = side * (10.0 + out + far * 9.0);
                        let (m, size) = place.boats.pick(h >> 6);
                        let yaw = if side < 0.0 { 0.3 } else { PI - 0.3 };
                        sc.put(
                            m,
                            0,
                            0.0,
                            x,
                            bob * 0.1 - 0.05,
                            d,
                            yaw,
                            size,
                            place.boats.tint,
                        );
                        sc.last().roll = bob * 0.05;
                    }
                    44..=51 => {
                        let x = side * (place.far_x + out + far * 14.0);
                        let (m, size) = place.far.pick(h >> 6);
                        let tint = place.far.tint;
                        if place.afloat {
                            let yaw = if side < 0.0 { 0.2 } else { PI + 0.2 };
                            sc.put(m, 0, 0.0, x, bob * 0.15 - 0.1, d, yaw, size, tint);
                            sc.last().roll = bob * 0.03;
                        } else {
                            sc.put(m, 0, 0.0, x, -0.4, d, unit(h) * 6.3, size, tint);
                        }
                    }
                    52..=55 => {
                        let x = side * (13.0 + out + far * 4.0);
                        let yaw = unit(h) * 6.3;
                        let (m, size) = place.sights.pick(h >> 6);
                        let y = if let Some(size) = place.sight_ground {
                            let (g, tint) = (place.grounds.models[0].0, place.grounds.tint);
                            sc.put(g, 0, 0.0, x, -0.12, d, yaw, size, tint);
                            0.0
                        } else {
                            -0.2
                        };
                        sc.put(m, 0, 0.0, x, y, d, yaw, size, place.sights.tint);
                    }
                    _ => {}
                }
            }
        }
    }

    fn put_things(&self, sc: &mut Scene) {
        let s = &self.s;
        let t = self.clock;
        for (k, th) in s.things().iter().enumerate() {
            if th.d > s.d + 85.0 {
                continue;
            }
            match th.kind {
                K_GATE => {
                    // gates the squad has passed fade
                    let fade = if th.state == 0 {
                        1.0
                    } else {
                        (1.0 - (s.d - th.d) / 2.5).clamp(0.0, 1.0)
                    };
                    if fade <= 0.0 {
                        continue;
                    }
                    let good = match th.a {
                        OP_ADD | OP_MUL => true,
                        _ => false,
                    } && !(th.a == OP_ADD && th.value == 0);
                    let col = if good { GOOD } else { BAD };
                    // one arch across the gate, widened to fit, with a glass panel in it
                    let wide = th.w / (ARCH_W * GATE_ARCH);
                    sc.put(
                        M_ARCH,
                        0,
                        0.0,
                        th.x,
                        DECK,
                        th.d,
                        0.0,
                        GATE_ARCH,
                        col & 0x00ff_ffff | 0xd800_0000,
                    );
                    let arch = sc.last();
                    arch.wide = wide;
                    arch.fade = fade;
                    arch.flash = (th.flash * 120.0) as u8;
                    let panel = if good {
                        rgba(0x3a9bff, 95)
                    } else {
                        rgba(0xff4646, 95)
                    };
                    sc.sprite(
                        SPRITE_PANEL,
                        PICTURE_BOX,
                        false,
                        th.x,
                        DECK,
                        th.d + 0.05,
                        th.w - 0.9 * GATE_ARCH * wide,
                        3.95 * GATE_ARCH,
                        0.0,
                        0.0,
                        panel & 0x00ff_ffff | (((panel >> 24) as f32 * fade) as u32) << 24,
                    );
                    let pulse = 1.0 + th.flash * 0.12;
                    sc.label(
                        LABEL_GATE,
                        sign_of(th.a),
                        th.x,
                        DECK + 2.3 * GATE_ARCH,
                        th.d,
                        1.6 * pulse,
                        th.value,
                        col,
                        fade,
                    );
                }
                _ if th.breakable() => {
                    let flash = (th.flash * 160.0) as u8;
                    match th.kind {
                        K_BARREL => {
                            sc.put(
                                M_BARREL,
                                0,
                                0.0,
                                th.x,
                                DECK,
                                th.d,
                                th.d * 0.7,
                                BARREL,
                                BARREL_BLUE,
                            );
                            sc.last().flash = flash;
                            sc.label(
                                LABEL_PILL,
                                b'+',
                                th.x,
                                DECK + 2.0,
                                th.d,
                                0.8,
                                th.value,
                                rgba(0x2f7fe0, 255),
                                1.0,
                            );
                        }
                        K_TNT => {
                            sc.put(M_BARREL, 0, 0.0, th.x, DECK, th.d, th.d, BARREL, TNT_RED);
                            sc.last().flash = flash;
                            // its fuse
                            let blink = 0.6 + 0.4 * (t * 14.0 + th.d).sin();
                            sc.sprite(
                                SPRITE_BILLBOARD,
                                PIC_FLARE,
                                true,
                                th.x,
                                DECK + 1.39,
                                th.d,
                                1.05 * blink,
                                1.05 * blink,
                                t * 3.0,
                                0.0,
                                rgba(0xffa040, 255),
                            );
                        }
                        _ => {
                            sc.put(M_CRATE, 0, 0.0, th.x, DECK, th.d, 0.0, CRATE, 0);
                            sc.last().flash = flash;
                            // the gun inside, turning over it
                            let gun = th.value.clamp(0, 4);
                            let y = DECK + CRATE * 1.1 + (t * 2.0).sin() * 0.1;
                            sc.put(
                                M_GUN + gun as u16,
                                0,
                                0.0,
                                th.x,
                                y,
                                th.d,
                                t * 1.6,
                                WEAPONS[gun as usize].scale * 2.5,
                                0,
                            );
                            sc.sprite(
                                SPRITE_BILLBOARD,
                                PIC_GLOW,
                                true,
                                th.x,
                                y,
                                th.d,
                                CRATE * 1.3,
                                CRATE * 1.3,
                                0.0,
                                0.0,
                                rgba(0x60c0ff, 150),
                            );
                            // what it takes to shoot open, on its front
                            let pulse = 1.0 + th.flash * 0.15;
                            sc.label(
                                LABEL_PLAIN,
                                0,
                                th.x,
                                DECK + CRATE * 0.41,
                                th.d - CRATE * 0.67,
                                1.4 * pulse,
                                th.hp,
                                rgba(0xffffff, 255),
                                1.0,
                            );
                        }
                    }
                    if th.kind == K_BARREL || (th.kind == K_TNT && th.hp < th.hp_max) {
                        sc.label(
                            LABEL_BAR,
                            5,
                            th.x,
                            DECK + 1.35,
                            th.d,
                            0.2,
                            th.hp * 1000 / th.hp_max.max(1),
                            rgba(0x5ad05a, 255),
                            1.0,
                        );
                    }
                }
                K_SHIP => {
                    let bob = (t * 1.2 + th.d).sin();
                    sc.put(
                        M_SHIP_MEDIUM,
                        0,
                        0.0,
                        th.x,
                        bob * 0.12 - 0.1,
                        th.d,
                        if th.x < 0.0 { PI / 2.0 } else { -PI / 2.0 },
                        1.3,
                        0,
                    );
                    sc.last().roll = bob * 0.03;
                }
                K_GROUP => {
                    let (mut n, mut x, mut d) = (0, 0.0, 0.0);
                    for e in s.enemies() {
                        if e.group == k as i32 && e.state != ES_DOWN {
                            n += 1;
                            x += e.x;
                            d += e.d;
                        }
                    }
                    if n > 0 {
                        sc.label(
                            LABEL_PILL,
                            0,
                            x / n as f32,
                            DECK + 2.3,
                            d / n as f32 + 0.6,
                            1.05,
                            n,
                            rgba(0xc0392b, 255),
                            1.0,
                        );
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
                // halted at the arena, they stand and fire
                S_OK if s.phase == P_BOSS && s.d >= s.arena_d => (C_AIM, q.t * 1.3, 1.0, DECK),
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
            let (n, x, d) = s
                .soldiers()
                .iter()
                .filter(|q| q.state == S_OK)
                .fold((0, 0.0, 0.0), |a, q| (a.0 + 1, a.1 + q.x, a.2 + q.d));
            let n = n.max(1) as f32;
            sc.label(
                LABEL_PILL,
                0,
                x / n,
                DECK + 2.0 + radius(s.count) * 0.2,
                d / n - 0.4,
                1.15,
                s.count,
                rgba(0x3f8f2a, 255),
                1.0,
            );
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
                ES_CHARGE => (
                    if e.kind == E_BRUTE { C_PUNCH } else { C_WALK },
                    e.t * if e.kind == E_RUNNER { 3.2 } else { 2.4 },
                    1.0,
                    INST_LOOP,
                ),
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
                sc.label(
                    LABEL_BAR,
                    4,
                    e.x,
                    DECK + 2.1,
                    e.d,
                    0.16,
                    (e.hp * 1000 / enemy_hp(E_BRUTE, s.hardness()).max(1)).min(1000),
                    rgba(0xe04030, 255),
                    1.0,
                );
            }
        }
    }

    fn put_boss(&self, sc: &mut Scene) {
        let b = &self.s.boss;
        if b.state == 0 || b.d > self.s.d + 85.0 {
            return;
        }
        let m = b.body();
        let (clip, t, flags) = if b.state == 3 {
            (C_DEATH, b.atk_t, 0)
        } else if b.hurt > 0.0 {
            (C_REEL, 0.55 - b.hurt, 0)
        } else if self.boss_walked {
            (C_WALK, b.walk, INST_LOOP)
        } else {
            match (b.atk, b.stage) {
                (A_VOLLEY, 1) => (C_SHOOT, b.atk_t * 1.2, INST_LOOP),
                (A_SHELLS, 1) => (C_SHOOT, b.atk_t, 0),
                (A_TROOPS, _) => (C_WAVE, b.atk_t, 0),
                (A_JUMP, 2) => (C_JUMP, b.atk_t, 0),
                _ => (C_IDLE, b.t, INST_LOOP),
            }
        };
        let fade = if self.s.phase == P_CLEAR || self.s.phase == P_WON {
            (1.0 - self.s.phase_ms as f32 / 1500.0).clamp(0.0, 1.0)
        } else {
            1.0
        };
        // a giant going down tips over its side, stiff as a toy, falling faster as it goes
        let (mut x, mut y, mut d, mut roll) = (b.x, DECK, b.d, 0.0);
        if !m.armoured && b.state == 3 {
            let u = ((b.atk_t - TOPPLE[0]) / (TOPPLE[1] - TOPPLE[0])).clamp(0.0, 1.0);
            let a = PI / 2.0 * u * u;
            let side = b.fall_side();
            let (s, c) = b.yaw.sin_cos();
            let w = m.width * m.scale;
            // turned about the edge it falls over, so it lies on the deck rather than through it
            let out = side * w * (1.0 - a.cos());
            (x, y, d, roll) = (x + out * c, y + w * a.sin(), d + out * s, -side * a);
        }
        let at = sc.put(m.model, clip, t, x, y, d, b.yaw, m.scale, m.tint);
        let i = sc.last();
        i.roll = roll;
        i.flags = flags;
        // the shader whitens in linear light, so a little reads as a lot on a big dark mech
        i.flash = (b.flash.max(0.0) * 25.0) as u8;
        i.fade = fade;
        if m.gun >= 0 {
            sc.gun(m.gun, at, fade);
        }
        let down = b.stage == 2 && b.atk_t >= m.land;
        if let (P_BOSS, Some((x, d)), false) = (self.s.phase, b.landing(), down) {
            // where it will land: a ring that fills as it comes down, gone once it is down
            let u = if b.stage == 2 {
                (b.atk_t / m.land).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let blink = 0.75 + 0.25 * (self.clock * 9.0).sin();
            warning(sc, x, d, JUMP_R, u, blink);
        }
    }

    fn put_shots(&self, sc: &mut Scene) {
        let s = &self.s;
        for b in &s.bul[..s.nbul as usize] {
            // width, length, colour, and a glow round the head
            let (w, len, col, glow) = match b.gun {
                0 => (0.2, 1.4, rgba(0xffd25a, 255), 0.0),
                1 => (0.15, 1.0, rgba(0xff9a3a, 255), 0.0),
                2 => (0.22, 2.8, rgba(0xfff2c0, 255), 0.0),
                3 => (0.3, 0.6, rgba(0xffc070, 255), 0.65),
                4 => (0.34, 8.0, rgba(0x9ff0ff, 255), 0.9),
                G_BOSS => (0.45, 2.4, rgba(0xff4020, 255), 1.3),
                _ => (0.2, 1.5, rgba(0xff5a3a, 255), 0.0),
            };
            sc.sprite(
                SPRITE_STREAK,
                PIC_LIGHT,
                true,
                b.x,
                b.y,
                b.d,
                w,
                len,
                b.vx,
                -b.vd,
                col,
            );
            if glow > 0.0 {
                sc.sprite(
                    SPRITE_BILLBOARD,
                    PIC_GLOW,
                    true,
                    b.x,
                    b.y,
                    b.d,
                    glow,
                    glow,
                    0.0,
                    0.0,
                    col & 0x00ff_ffff | 0xb000_0000,
                );
            }
        }
        for sh in &s.shell[..s.nshell as usize] {
            let u = (sh.t / sh.t_max).clamp(0.0, 1.0);
            // where it lands, from the moment it is fired
            let blink = 0.7 + 0.3 * (self.clock * (8.0 + 10.0 * u)).sin();
            warning(sc, sh.x, sh.d, sh.r, u, blink);
            let (x, y, d) = sh.at();
            if sh.kind == 0 {
                sc.put(M_BALL, 0, 0.0, x, y, d, 0.0, 1.8, 0);
                sc.last().flags |= INST_NO_SHADOW;
            } else {
                sc.sprite(
                    SPRITE_BILLBOARD,
                    PIC_GLOW,
                    true,
                    x,
                    y,
                    d,
                    1.3,
                    1.3,
                    0.0,
                    0.0,
                    rgba(0xff7030, 230),
                );
                sc.sprite(
                    SPRITE_BILLBOARD,
                    PIC_FLARE,
                    true,
                    x,
                    y,
                    d,
                    0.9,
                    0.9,
                    self.clock * 6.0,
                    0.0,
                    rgba(0xffd080, 255),
                );
            }
        }
    }
}

/// A red mark on the deck where something will come down, r round, `u` of
/// the way there: its ring blinks and the middle fills as it nears.
fn warning(sc: &mut Scene, x: f32, d: f32, r: f32, u: f32, blink: f32) {
    let from = sc.sprites.len();
    sc.sprite(
        SPRITE_GROUND,
        PIC_CIRCLE,
        false,
        x,
        DECK + 0.04,
        d,
        r * 2.0,
        r * 2.0,
        0.0,
        0.0,
        rgba(0xff2010, (70.0 + 60.0 * u) as u8),
    );
    sc.sprite(
        SPRITE_GROUND,
        PIC_RING,
        true,
        x,
        DECK + 0.05,
        d,
        r * 2.2,
        r * 2.2,
        0.0,
        0.0,
        rgba(0xff2a18, (255.0 * blink) as u8),
    );
    if u > 0.0 {
        sc.sprite(
            SPRITE_GROUND,
            PIC_GLOW,
            true,
            x,
            DECK + 0.06,
            d,
            r * 2.3 * u,
            r * 2.3 * u,
            0.0,
            0.0,
            rgba(0xff5030, 220),
        );
    }
    // over the squad standing on it, so it shows through
    for s in &mut sc.sprites[from..] {
        s.flags |= SPRITE_OVER;
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
    crate::WG_GAME.get().game.scene.names.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn wg3d_clips() -> *const u8 {
    crate::WG_GAME.get().game.scene.clips.as_ptr()
}
