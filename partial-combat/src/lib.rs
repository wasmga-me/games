//! Partial Combat: Clickbait Tactics - an original wasmga.me game. MIT licensed.
//! A squad of green plastic soldiers runs down a pier over the sea, firing
//! ahead on its own. Steer it through the gate that grows it most, shoot
//! barrels for more soldiers and crates for better guns, keep out of holes
//! and shell fire, outnumber the tan army, and beat the boss at the end.
//! Twenty levels, or one endless pier.
//!
//! Runs on the wg engine, drawn in 3D: every frame the page reads the scene
//! from wg3d_scene (scene.rs) and draws it, and lays this module's own
//! picture (hud.rs, the wg screen) over it. The page draws the setup, pause
//! and game over screens.
mod glyphs;
mod hud;
mod scene;
mod track;

use wg::{Game, Input, Pointer, Rng, Screen};

/// 1: the first; 3: a pier twice as wide, each gun its own shots, bosses that walk, jump and bring an army;
/// 4: lanes and holes laid in 5 m tiles, bigger crates
const VERSION: u32 = 4;

/// options, in catalog order: mode, level to start at
pub const O_MODE: usize = 0;
pub const O_LEVEL: usize = 1;
pub const ENDLESS: i32 = 1;
pub const LEVELS: i32 = 20;

/// The pier: lanes of TILE metres, HALF either side of the middle, its deck DECK above the sea.
pub const LANES: i32 = 4;
pub const TILE: f32 = 5.0;
pub const HALF: f32 = 10.0;
pub const DECK: f32 = 0.93;

/// soldiers shown and moved one by one; a bigger squad shows its number
pub const MAXV: usize = 200;
pub const MAXE: usize = 320;
pub const MAXO: usize = 48;
pub const MAXB: usize = 192;
pub const MAXSH: usize = 16;
/// the most a squad can number, only so its sums stay in range
pub const MAX_COUNT: i32 = 9_999_999;
/// barrels' and crates' sizes
pub const BARREL: f32 = 1.0;
pub const CRATE: f32 = 2.6;
/// room each soldier takes in the squad
const SPACING: f32 = 0.34;
/// bullets of the squad's in the air at most; past that each carries the
/// damage of more than one shot
const VOLLEY: f32 = MAXB as f32 * 0.75;
/// what fired a bullet besides the squad's guns (WEAPONS): a tan rifleman, the boss
pub const G_RIFLE: i32 = WEAPONS.len() as i32;
pub const G_BOSS: i32 = G_RIFLE + 1;
/// how near an enemy has to be to fight hand to hand
const REACH: f32 = 0.5;
/// the longest step the rules take, so nothing jumps through a gate
const STEP_MS: i32 = 20;

// phases
pub const P_RUN: i32 = 0;
pub const P_BOSS: i32 = 1;
pub const P_CLEAR: i32 = 2;
pub const P_DEAD: i32 = 3;
pub const P_WON: i32 = 4;
/// how long a cleared level shows before the next, ms
const CLEAR_MS: i32 = 3200;

// soldiers
pub const S_OK: i32 = 0;
pub const S_FALL: i32 = 1;
pub const S_DOWN: i32 = 2;

// enemies
pub const E_GRUNT: i32 = 0;
pub const E_RUNNER: i32 = 1;
pub const E_RIFLE: i32 = 2;
pub const E_BRUTE: i32 = 3;
pub const ES_WAIT: i32 = 0;
pub const ES_CHARGE: i32 = 1;
pub const ES_DOWN: i32 = 2;

// things on the pier
pub const K_NONE: i32 = 0;
pub const K_GATE: i32 = 1;
pub const K_BARREL: i32 = 2;
pub const K_TNT: i32 = 3;
pub const K_CRATE: i32 = 4;
pub const K_HOLE: i32 = 5;
pub const K_SHIP: i32 = 6;
pub const K_GROUP: i32 = 7;
pub const K_ARENA: i32 = 8;

// gate sums
pub const OP_ADD: i32 = 0;
pub const OP_SUB: i32 = 1;
pub const OP_MUL: i32 = 2;
pub const OP_DIV: i32 = 3;
/// damage that counts a gate one point, in a save from before gates were laid with their own
const GATE_DMG: i32 = 3;
/// how far ahead of the squad TNT has to be for its fire to turn on it, so it doesn't go off in its face
const TNT_NEAR: f32 = 8.0;

// banners over the scene
pub const B_NONE: i32 = 0;
pub const B_LEVEL: i32 = 1;
pub const B_BOSS: i32 = 2;
pub const B_CLEAR: i32 = 3;
pub const B_GUN: i32 = 4;

pub struct Weapon {
    /// in wg::LANGS order
    pub names: [&'static str; 10],
    /// shots a second for each shooter
    pub rate: f32,
    pub dmg: i32,
    /// pellets a shot
    pub pellets: i32,
    /// sideways spread, m/s
    pub spread: f32,
    /// targets a bullet goes through
    pub pierce: i32,
    /// how fast its bullets go (m/s) and how far
    pub speed: f32,
    pub range: f32,
    /// the gun's size in a soldier's hand
    pub scale: f32,
}
pub const WEAPONS: [Weapon; 5] = [
    Weapon {
        names: [
            "Pistol",
            "手枪",
            "ピストル",
            "권총",
            "Pistola",
            "Pistola",
            "Pistolet",
            "Pistole",
            "Пистолет",
            "手槍",
        ],
        rate: 1.6,
        dmg: 1,
        pellets: 1,
        spread: 0.5,
        pierce: 0,
        speed: 34.0,
        range: 28.0,
        scale: 0.5,
    },
    Weapon {
        names: [
            "SMG",
            "冲锋枪",
            "サブマシンガン",
            "기관단총",
            "Subfusil",
            "Submetralhadora",
            "Mitraillette",
            "MP",
            "Пистолет-пулемёт",
            "衝鋒槍",
        ],
        rate: 3.6,
        dmg: 1,
        pellets: 1,
        spread: 1.0,
        pierce: 0,
        speed: 38.0,
        range: 28.0,
        scale: 0.42,
    },
    Weapon {
        names: [
            "Rifle",
            "步枪",
            "ライフル",
            "소총",
            "Fusil",
            "Fuzil",
            "Fusil",
            "Gewehr",
            "Винтовка",
            "步槍",
        ],
        rate: 2.4,
        dmg: 3,
        pellets: 1,
        spread: 0.35,
        pierce: 1,
        speed: 46.0,
        range: 34.0,
        scale: 0.38,
    },
    Weapon {
        names: [
            "Shotgun",
            "霰弹枪",
            "ショットガン",
            "샷건",
            "Escopeta",
            "Espingarda",
            "Fusil à pompe",
            "Schrotflinte",
            "Дробовик",
            "霰彈槍",
        ],
        rate: 1.0,
        dmg: 2,
        pellets: 6,
        spread: 3.0,
        pierce: 0,
        speed: 32.0,
        range: 24.0,
        scale: 0.31,
    },
    Weapon {
        names: [
            "Sniper rifle",
            "狙击枪",
            "スナイパーライフル",
            "저격총",
            "Rifle de francotirador",
            "Rifle de precisão",
            "Fusil de précision",
            "Scharfschützengewehr",
            "Снайперская винтовка",
            "狙擊槍",
        ],
        rate: 1.1,
        dmg: 18,
        pellets: 1,
        spread: 0.15,
        pierce: 5,
        speed: 70.0,
        range: 40.0,
        scale: 0.24,
    },
];
/// damage a second one soldier puts out with gun w into what it aims at;
/// each gun puts out at least half as much again as the last
pub fn gun_dps(w: i32) -> f32 {
    let g = &WEAPONS[w.clamp(0, 4) as usize];
    g.rate * (g.dmg * g.pellets) as f32
}

/// one soldier shown
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Sol {
    pub x: f32,
    pub d: f32,
    pub y: f32,
    pub vy: f32,
    pub state: i32,
    /// seconds into its animation, so the squad doesn't move as one
    pub t: f32,
    pub flash: f32,
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Enemy {
    pub kind: i32,
    pub state: i32,
    pub x: f32,
    pub d: f32,
    pub hp: i32,
    /// its group's thing, for the number over the group
    pub group: i32,
    pub t: f32,
    /// till its next shot, or its next blow in a fight
    pub cool: f32,
    pub flash: f32,
    pub yaw: f32,
}

/// A thing on the pier. Gates: value, op in a, the other gate of the pair
/// in b, the damage a point in hp, points left to count in hp_max, damage
/// towards the next in c, state 1 taken and 2 passed by. Barrels and
/// crates: hp of hp_max, soldiers or the gun inside in value. Holes: x..x+w
/// by d..d+len. Ships: time to the next shell in cool. Groups: how many it
/// had in value. The arena: where the boss stands.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Thing {
    pub kind: i32,
    pub state: i32,
    pub x: f32,
    pub d: f32,
    pub w: f32,
    pub len: f32,
    pub value: i32,
    pub hp: i32,
    pub hp_max: i32,
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub flash: f32,
    pub cool: f32,
}

impl Thing {
    /// A barrel, crate or TNT barrel not yet shot open.
    pub fn breakable(&self) -> bool {
        matches!(self.kind, K_BARREL | K_TNT | K_CRATE) && self.hp > 0
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Bullet {
    pub x: f32,
    pub d: f32,
    pub y: f32,
    pub vx: f32,
    pub vd: f32,
    /// falling, m/s (the boss's rounds come down from its gun hand)
    pub vy: f32,
    /// seconds left
    pub life: f32,
    pub dmg: i32,
    pub pierce: i32,
    /// the gun that fired it: one of the squad's (WEAPONS), or G_RIFLE or G_BOSS (those knock out a soldier)
    pub gun: i32,
}
impl Bullet {
    pub fn ours(&self) -> bool {
        self.gun < G_RIFLE
    }
}

/// A shell lobbed from ox, oy, od: lands at t_max on everything within r of x, d.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Shell {
    pub x: f32,
    pub d: f32,
    pub ox: f32,
    pub oy: f32,
    pub od: f32,
    pub r: f32,
    pub t: f32,
    pub t_max: f32,
    /// 0 a ship's cannonball, 1 a boss's shell
    pub kind: i32,
}
impl Shell {
    /// Where it is on its way, x y d (high up the arc, so it is seen coming).
    pub fn at(&self) -> (f32, f32, f32) {
        let u = (self.t / self.t_max).clamp(0.0, 1.0);
        let far = (self.x - self.ox).hypot(self.d - self.od);
        let high = 6.0 + far * 0.3;
        (
            self.ox + (self.x - self.ox) * u,
            self.oy + (DECK - self.oy) * u + high * 4.0 * u * (1.0 - u),
            self.od + (self.d - self.od) * u,
        )
    }
}

// what a boss is doing (Boss::atk)
pub const A_NONE: i32 = 0;
/// walking to tx, td
pub const A_MOVE: i32 = 1;
/// a burst of rounds at the squad
pub const A_VOLLEY: i32 = 2;
/// shells lobbed round the squad
pub const A_SHELLS: i32 = 3;
/// waving its troops on
pub const A_TROOPS: i32 = 4;
/// walking up to the squad and jumping on it, at tx, td
pub const A_JUMP: i32 = 5;
/// how far short of the squad it stops to jump
pub const JUMP_REACH: f32 = 4.5;
/// the longest leap it takes
pub const JUMP_FAR: f32 = 8.0;
/// how far round where it lands soldiers are knocked out
pub const JUMP_R: f32 = 3.8;
/// Where across the pier it jumps at a squad at cx, kept in from the edges.
fn jump_x(cx: f32) -> f32 {
    cx.clamp(-HALF + JUMP_R * 0.6, HALF - JUMP_R * 0.6)
}

/// A boss's body: its model (scene.rs MODELS) and tint; as its clips have
/// it, the walk's loop (s), how far a foot goes back a second at the clip's
/// pace (m at scale 1), the pace it walks at as a boss, when in the jump the
/// feet leave the deck and come down (s), and how long its wave lasts (s);
/// then the size it is drawn at, and at scale 1 how tall it stands, how far
/// its sides and each foot are from its middle and where its gun fires from
/// (x to its left, up, ahead). Last, the gun it holds (scene.rs WEAPONS; -1,
/// its own built in) and whether it is armoured: rounds ping off a mech and
/// it goes down in blasts, where a giant chips like its troops and tips over
/// stiff as a toy.
pub struct Body {
    pub model: u16,
    pub tint: u32,
    pub walk: f32,
    pub stride: f32,
    pub pace: f32,
    pub lift: f32,
    pub land: f32,
    pub hello: f32,
    pub scale: f32,
    pub height: f32,
    pub width: f32,
    pub foot: f32,
    pub muzzle: [f32; 3],
    pub gun: i32,
    pub armoured: bool,
}
/// how big a mech boss is drawn
pub const BOSS_SCALE: f32 = 1.9;
/// the first kinds are mechs, the rest giants: people from the soldiers' pack (pack.mjs makes their jump and fall)
const MECHS: i32 = 4;
/// the bosses are tan plastic like the army they lead, each a shade of its own
const fn tint(rgb: u32) -> u32 {
    scene::rgba(rgb, 225)
}
pub const BODIES: [Body; 8] = {
    use scene::model;
    // slow, heavy steps
    const GEORGE: Body = Body {
        model: model("mech.george"),
        tint: tint(0x543213),
        walk: 0.833,
        stride: 8.6,
        pace: 0.34,
        lift: 0.23,
        land: 0.5,
        hello: 1.5,
        scale: BOSS_SCALE,
        height: 6.0,
        width: 2.0,
        foot: 1.0,
        muzzle: [-1.0, 4.7, 2.0],
        gun: -1,
        armoured: true,
    };
    // Leela, Mike and Stan share their clips
    const MECH: Body = Body {
        walk: 1.033,
        stride: 6.9,
        lift: 0.3,
        land: 0.63,
        hello: 1.87,
        height: 5.5,
        width: 2.7,
        ..GEORGE
    };
    const PERSON: Body = Body {
        model: model("giant.pirate"),
        tint: tint(0x4a2c12),
        walk: 1.25,
        stride: 1.85,
        pace: 0.71,
        lift: 0.3,
        land: 0.63,
        hello: 1.88,
        scale: 3.4,
        height: 3.1,
        width: 0.7,
        foot: 0.26,
        muzzle: [-0.34, 1.9, 1.5],
        gun: 0,
        armoured: false,
    };
    [
        GEORGE,
        Body {
            model: model("mech.leela"),
            tint: tint(0x3e2311),
            ..MECH
        },
        Body {
            model: model("mech.mike"),
            tint: tint(0x66421b),
            ..MECH
        },
        Body {
            model: model("mech.stan"),
            tint: tint(0x34210e),
            ..MECH
        },
        Body {
            model: model("giant.knight"),
            tint: tint(0x6b4520),
            gun: 1,
            ..PERSON
        },
        PERSON,
        Body {
            model: model("giant.goblin"),
            tint: tint(0x5c3a17),
            gun: 3,
            ..PERSON
        },
        Body {
            model: model("giant.zombie"),
            tint: tint(0x3a2510),
            gun: 2,
            ..PERSON
        },
    ]
};
/// A level's boss: a mech and a giant by turns.
pub fn boss_kind(level: i32) -> i32 {
    let giants = BODIES.len() as i32 - MECHS;
    if level % 2 == 0 {
        level / 2 % MECHS
    } else {
        MECHS + level / 2 % giants
    }
}
/// when a giant going down starts to tip over, and when it hits the deck (s)
pub const TOPPLE: [f32; 2] = [0.25, 1.15];
/// How far short of the arena the boss fight starts.
pub const BOSS_LEAD: f32 = 12.0;
/// where the boss stands between attacks, past the arena's start
pub const BOSS_HOME: f32 = 15.0;
/// the ping of a round off the boss's armour
const CLANG: u32 = 0xff_70d0ff;
/// blast(): a TNT barrel's, meant for the enemy; the boss's, which spares its troops
const BLAST_TNT: u32 = 1;
const BLAST_FOE: u32 = 2;
/// A boss's flash runs from 1 down to this before it can blink again: a tenth of a second lit, then a gap.
const BLINK_GAP: f32 = -3.0;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Boss {
    /// 0 none, 1 waiting, 2 fighting, 3 going down
    pub state: i32,
    /// which body (BODIES)
    pub kind: i32,
    pub x: f32,
    pub d: f32,
    pub hp: i32,
    pub hp_max: i32,
    pub t: f32,
    /// till the next attack
    pub cool: f32,
    /// what it is doing (A_*), the step of it, and how long since that began
    pub atk: i32,
    pub stage: i32,
    pub atk_t: f32,
    /// lit when above 0, see BLINK_GAP
    pub flash: f32,
    /// where it is going
    pub tx: f32,
    pub td: f32,
    /// which way it faces (0 the squad); where a jump started
    pub yaw: f32,
    pub from: [f32; 2],
    /// clip seconds into its walk's loop (walking backwards it runs down)
    pub walk: f32,
    /// a flinch under way (s), and the hp below which it flinches next
    pub hurt: f32,
    pub flinch_at: i32,
}
impl Boss {
    /// where it will come down, once it has picked the spot (it stays put)
    pub fn landing(&self) -> Option<(f32, f32)> {
        (self.state == 2 && self.atk == A_JUMP && self.stage >= 1).then_some((self.tx, self.td))
    }
    pub fn body(&self) -> &'static Body {
        &BODIES[self.kind as usize]
    }
    /// how tall it stands (m)
    pub fn tall(&self) -> f32 {
        let m = self.body();
        m.height * m.scale
    }
    /// how fast it walks, m/s
    pub fn speed(&self) -> f32 {
        let m = self.body();
        m.stride * m.scale * m.pace
    }
    /// where its gun fires from, x y d
    pub fn muzzle(&self) -> (f32, f32, f32) {
        let m = self.body();
        let [ox, oy, oz] = m.muzzle.map(|v| v * m.scale);
        let (s, c) = self.yaw.sin_cos();
        (
            self.x + ox * c + oz * s,
            DECK + oy,
            self.d - (-ox * s + oz * c),
        )
    }
    /// which way a giant tips over, to its left (1) or right (-1):
    /// the side nearer the pier's middle, and a little away from the camera
    pub fn fall_side(&self) -> f32 {
        let (s, c) = self.yaw.sin_cos();
        if c * -self.x.signum() + s * 0.5 >= 0.0 {
            1.0
        } else {
            -1.0
        }
    }
}

/// everything a save keeps; animations and the camera live in Combat
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Save {
    pub version: u32,
    pub opt: [i32; 2],
    pub rng: Rng,
    /// the pier's own, so a level is laid out the same every time
    pub track_rng: Rng,
    pub phase: i32,
    pub phase_ms: i32,
    /// the level, or in endless the bosses beaten so far
    pub level: i32,
    pub time_ms: i32,
    pub score: i32,
    /// what this level has scored, and the last level cleared with its score
    pub level_score: i32,
    pub cleared: i32,
    pub cleared_score: i32,
    pub kills: i32,
    pub bosses: i32,
    pub biggest: i32,
    pub count: i32,
    pub weapon: i32,
    /// the squad's middle, where it steers to, and how far down the pier it is
    pub cx: f32,
    pub tx: f32,
    pub d: f32,
    pub speed: f32,
    pub fire_acc: f32,
    /// a drag under way: 1 if so, where it started, and the squad's aim then
    pub drag: [f32; 3],
    /// the pier is laid out up to here; chunks laid so far; the squad's size the layout expects
    pub next_d: f32,
    pub chunk: i32,
    pub expect: f32,
    pub expect_gun: i32,
    /// where this level's boss fight starts (0 till it is laid)
    pub arena_d: f32,
    pub banner: i32,
    pub banner_ms: i32,
    pub banner_v: i32,
    pub shake: f32,
    pub nsol: i32,
    pub sol: [Sol; MAXV],
    pub nen: i32,
    pub en: [Enemy; MAXE],
    pub nthing: i32,
    pub thing: [Thing; MAXO],
    pub nbul: i32,
    pub bul: [Bullet; MAXB],
    pub nshell: i32,
    pub shell: [Shell; MAXSH],
    pub boss: Boss,
}
unsafe impl wg::Plain for Sol {}
unsafe impl wg::Plain for Enemy {}
unsafe impl wg::Plain for Thing {}
unsafe impl wg::Plain for Bullet {}
unsafe impl wg::Plain for Shell {}
unsafe impl wg::Plain for Boss {}
unsafe impl wg::Plain for Save {}

const NO_SOL: Sol = Sol {
    x: 0.0,
    d: 0.0,
    y: DECK,
    vy: 0.0,
    state: S_OK,
    t: 0.0,
    flash: 0.0,
};
const NO_ENEMY: Enemy = Enemy {
    kind: 0,
    state: ES_DOWN,
    x: 0.0,
    d: 0.0,
    hp: 0,
    group: -1,
    t: 0.0,
    cool: 0.0,
    flash: 0.0,
    yaw: 0.0,
};
pub const NO_THING: Thing = Thing {
    kind: K_NONE,
    state: 0,
    x: 0.0,
    d: 0.0,
    w: 0.0,
    len: 0.0,
    value: 0,
    hp: 0,
    hp_max: 0,
    a: 0,
    b: 0,
    c: 0,
    flash: 0.0,
    cool: 0.0,
};
const NO_BULLET: Bullet = Bullet {
    x: 0.0,
    d: 0.0,
    y: 0.0,
    vx: 0.0,
    vd: 0.0,
    vy: 0.0,
    life: 0.0,
    dmg: 0,
    pierce: 0,
    gun: 0,
};
const NO_SHELL: Shell = Shell {
    x: 0.0,
    d: 0.0,
    ox: 0.0,
    oy: 0.0,
    od: 0.0,
    r: 0.0,
    t: 0.0,
    t_max: 1.0,
    kind: 0,
};
pub const NO_BOSS: Boss = Boss {
    state: 0,
    kind: 0,
    x: 0.0,
    d: 0.0,
    hp: 0,
    hp_max: 1,
    t: 0.0,
    cool: 0.0,
    atk: A_NONE,
    stage: 0,
    atk_t: 0.0,
    flash: 0.0,
    tx: 0.0,
    td: 0.0,
    yaw: 0.0,
    from: [0.0; 2],
    walk: 0.0,
    hurt: 0.0,
    flinch_at: 0,
};

impl Save {
    fn new() -> Self {
        Save {
            version: VERSION,
            opt: [0, 0],
            rng: Rng::new(1),
            track_rng: Rng::new(1),
            phase: P_RUN,
            phase_ms: 0,
            level: 0,
            time_ms: 0,
            score: 0,
            level_score: 0,
            cleared: -1,
            cleared_score: 0,
            kills: 0,
            bosses: 0,
            biggest: 0,
            count: 0,
            weapon: 0,
            cx: 0.0,
            tx: 0.0,
            d: 0.0,
            speed: 6.5,
            fire_acc: 0.0,
            drag: [0.0; 3],
            next_d: 0.0,
            chunk: 0,
            expect: 0.0,
            expect_gun: 0,
            arena_d: 0.0,
            banner: B_NONE,
            banner_ms: 0,
            banner_v: 0,
            shake: 0.0,
            nsol: 0,
            sol: [NO_SOL; MAXV],
            nen: 0,
            en: [NO_ENEMY; MAXE],
            nthing: 0,
            thing: [NO_THING; MAXO],
            nbul: 0,
            bul: [NO_BULLET; MAXB],
            nshell: 0,
            shell: [NO_SHELL; MAXSH],
            boss: NO_BOSS,
        }
    }
    pub fn endless(&self) -> bool {
        self.opt[O_MODE] == ENDLESS
    }
    pub fn things(&self) -> &[Thing] {
        &self.thing[..self.nthing as usize]
    }
    pub fn arenas(&self) -> impl Iterator<Item = &Thing> {
        self.things().iter().filter(|t| t.kind == K_ARENA)
    }
    pub fn enemies(&self) -> &[Enemy] {
        &self.en[..self.nen as usize]
    }
    pub fn soldiers(&self) -> &[Sol] {
        &self.sol[..self.nsol as usize]
    }
    fn frand(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.rng.float()
    }
    /// How hard the pier is where the squad is.
    pub fn hardness(&self) -> f32 {
        self.hardness_at(self.d)
    }
    /// How hard the pier is at d: the level, or in endless also how far it has come.
    pub fn hardness_at(&self, d: f32) -> f32 {
        if self.endless() {
            d / 160.0 + self.level as f32
        } else {
            self.level as f32
        }
    }
    /// How many soldiers are still standing.
    pub fn alive(&self) -> usize {
        self.soldiers().iter().filter(|q| q.state == S_OK).count()
    }
    /// The pier is there to stand on at x, d.
    pub fn deck(&self, x: f32, d: f32) -> bool {
        if x.abs() > HALF - 0.05 {
            return false;
        }
        !self
            .things()
            .iter()
            .any(|t| t.kind == K_HOLE && x >= t.x && x < t.x + t.w && d >= t.d && d < t.d + t.len)
    }
    /// How wide the pier is at d, as the lane edges either side of x.
    pub fn deck_span(&self, x: f32, d: f32) -> (f32, f32) {
        let mut lo = -HALF;
        let mut hi = HALF;
        for t in self.things() {
            if t.kind != K_HOLE || d < t.d || d >= t.d + t.len {
                continue;
            }
            if t.x + t.w <= x {
                lo = lo.max(t.x + t.w);
            } else if t.x >= x {
                hi = hi.min(t.x);
            }
        }
        (lo, hi)
    }
}

/// How many of a squad of n fire: every soldier it shows.
pub fn shooters(n: f32) -> f32 {
    n.clamp(0.0, MAXV as f32)
}

/// Damage a second a squad of n puts into what it aims at with gun w.
pub fn squad_dps(n: f32, w: i32) -> f32 {
    shooters(n) * gun_dps(w)
}

/// The damage a tan soldier of kind takes before it falls, where the pier is that hard.
pub fn enemy_hp(kind: i32, hard: f32) -> i32 {
    match kind {
        E_BRUTE => 6 + (hard * 0.6) as i32,
        E_RIFLE => 2 + (hard / 6.0) as i32,
        _ => 1 + (hard / 10.0) as i32,
    }
}

/// A squad of n through a gate of op and value; dividing rounds up.
pub fn gate_sum(op: i32, v: i32, n: i32) -> i32 {
    match op {
        OP_ADD => n + v,
        OP_SUB => n - v,
        OP_MUL => n * v,
        _ => (n + v - 1) / v.max(1),
    }
}

/// the squad's radius for n soldiers
pub fn radius(n: i32) -> f32 {
    SPACING * (n.clamp(1, MAXV as i32) as f32 + 0.5).sqrt() + 0.3
}

/// soldier i's place in the squad, from its middle (a sunflower, densest in the middle)
pub fn slot(i: usize) -> (f32, f32) {
    let r = SPACING * (i as f32 + 0.5).sqrt();
    let a = i as f32 * 2.399_963;
    (r * a.cos(), r * a.sin())
}

pub struct Combat {
    pub s: Save,
    /// what happened this frame, for the page's effects and sounds (scene.rs)
    pub fx: Vec<scene::Fx>,
    pub scene: scene::Scene,
    /// the camera (eye, then what it looks at), eased after the squad
    pub cam: [f32; 6],
    pub cam_set: bool,
    /// seconds shown, for the sea and things bobbing on it; the screen's shape
    pub clock: f32,
    pub aspect: f32,
    /// the boss stepped this frame: its walk plays
    pub boss_walked: bool,
}

impl Combat {
    pub fn fx(&mut self, kind: u32, x: f32, y: f32, d: f32, a: f32, b: f32, c: f32, rgba: u32) {
        if self.fx.len() < 256 {
            self.fx.push(scene::Fx {
                kind,
                pos: [x, y, -d],
                a,
                b,
                c,
                rgba,
            });
        }
    }

    /// A new level (or the endless pier): the squad at the start, the pier laid afresh.
    fn begin_level(&mut self) {
        let s = &mut self.s;
        s.phase = P_RUN;
        s.phase_ms = 0;
        s.level_score = 0;
        s.count = if s.endless() { 5 } else { 3 };
        s.weapon = 0;
        s.cx = 0.0;
        s.tx = 0.0;
        s.d = 0.0;
        s.speed = 6.5 + (s.level as f32 * 0.08).min(1.4);
        s.fire_acc = 0.0;
        s.next_d = 12.0;
        s.chunk = 0;
        s.expect = s.count as f32;
        s.expect_gun = 0;
        s.arena_d = 0.0;
        s.nsol = 0;
        s.nen = 0;
        s.nthing = 0;
        s.nbul = 0;
        s.nshell = 0;
        s.boss = NO_BOSS;
        s.track_rng = if s.endless() {
            Rng::new(s.rng.next())
        } else {
            Rng::new(0x51ed_270b ^ (s.level as u32 + 1).wrapping_mul(0x9e37_79b9))
        };
        s.banner = if s.endless() { B_NONE } else { B_LEVEL };
        s.banner_v = s.level + 1;
        s.banner_ms = 0;
        self.fill_squad();
        let s = &mut self.s;
        for i in 0..s.nsol as usize {
            let (ox, oz) = slot(i);
            s.sol[i].x = s.cx + ox;
            s.sol[i].d = s.d + oz;
        }
        self.cam_set = false;
        self.lay_track();
    }

    /// Shows as many soldiers as the squad has, up to MAXV: new ones join at the back.
    fn fill_squad(&mut self) {
        let s = &mut self.s;
        let want = shooters(s.count as f32) as usize;
        while (s.nsol as usize) < want {
            let i = s.nsol as usize;
            let (ox, oz) = slot(i);
            let t = s.rng.float() * 3.0;
            s.sol[i] = Sol {
                x: s.cx + ox * 0.4,
                d: s.d + oz * 0.4 - 0.6,
                y: DECK,
                vy: 0.0,
                state: S_OK,
                t,
                flash: 0.6,
            };
            s.nsol += 1;
        }
        // fewer: the last ones go
        let mut alive = s.alive();
        let mut i = s.nsol as usize;
        while alive > want && i > 0 {
            i -= 1;
            if s.sol[i].state == S_OK {
                s.sol[i].state = S_DOWN;
                s.sol[i].vy = 0.0;
                s.sol[i].t = 0.0;
                alive -= 1;
            }
        }
        s.biggest = s.biggest.max(s.count);
    }

    /// A soldier is knocked out (or falls in): the squad is that much smaller.
    fn lose(&mut self, i: usize, fall: bool) {
        let s = &mut self.s;
        if s.sol[i].state != S_OK {
            return;
        }
        let alive = s.alive().max(1) as i32;
        // each one shown stands for a share of a big squad
        let share = ((s.count + alive - 1) / alive).max(1);
        s.count = (s.count - share).max(0);
        let q = &mut s.sol[i];
        q.state = if fall { S_FALL } else { S_DOWN };
        q.t = 0.0;
        q.vy = if fall { 1.0 } else { 0.0 };
        let (x, d) = (q.x, q.d);
        if !fall {
            self.fx(scene::FX_PUFF, x, DECK, d, 0.0, 0.0, 0.0, scene::GREEN);
        }
        // a big squad's reserves step in
        self.fill_squad();
    }

    fn enemy_down(&mut self, k: usize) {
        let e = &mut self.s.en[k];
        if e.state == ES_DOWN {
            return;
        }
        e.state = ES_DOWN;
        e.t = 0.0;
        let (x, d, kind) = (e.x, e.d, e.kind);
        let points = [10, 15, 15, 40][kind as usize];
        self.s.kills += 1;
        self.add_score(points);
        self.fx(scene::FX_PUFF, x, DECK, d, 0.0, 0.0, 0.0, scene::TAN);
    }

    fn add_score(&mut self, n: i32) {
        self.s.score += n;
        self.s.level_score += n;
    }

    fn banner(&mut self, b: i32, v: i32) {
        self.s.banner = b;
        self.s.banner_v = v;
        self.s.banner_ms = 0;
    }

    /// One step of the rules, dt seconds long.
    fn step(&mut self, input: &Input, dt: f32) -> u32 {
        let mut ev = 0;
        self.boss_walked = false;
        self.s.phase_ms += (dt * 1000.0) as i32;
        self.s.banner_ms += (dt * 1000.0) as i32;
        self.s.shake = (self.s.shake - dt * 2.5).max(0.0);
        match self.s.phase {
            P_DEAD | P_WON => {
                self.move_soldiers(dt);
                self.move_bullets(dt);
                return 0;
            }
            P_CLEAR => {
                self.move_soldiers(dt);
                self.s.boss.atk_t += dt;
                if self.s.phase_ms > 600 && self.s.phase_ms - (dt * 1000.0) as i32 <= 600 {
                    let (x, d) = (self.s.cx, self.s.d);
                    self.fx(scene::FX_CONFETTI, x, DECK, d - 2.0, 0.0, 0.0, 0.0, 0);
                }
                if self.s.phase_ms >= CLEAR_MS {
                    if self.s.endless() {
                        // on down the pier
                        self.s.phase = P_RUN;
                        self.s.phase_ms = 0;
                        self.s.arena_d = 0.0;
                        self.s.boss = NO_BOSS;
                        self.s.level += 1;
                    } else {
                        self.s.level += 1;
                        self.begin_level();
                    }
                }
                return 0;
            }
            _ => {}
        }
        self.s.time_ms += (dt * 1000.0) as i32;
        self.steer(input, dt);
        if self.s.phase == P_RUN || self.s.phase == P_BOSS {
            self.s.d += self.s.speed * dt;
            if self.s.arena_d > 0.0 {
                self.s.d = self.s.d.min(self.s.arena_d);
            }
            // the fight starts as the boss's army comes on, and the squad walks on to the arena
            if self.s.phase == P_RUN
                && self.s.arena_d > 0.0
                && self.s.d >= self.s.arena_d - BOSS_LEAD
            {
                self.s.phase = P_BOSS;
                self.s.phase_ms = 0;
                self.banner(B_BOSS, 0);
                self.s.boss.state = 2;
                self.s.boss.cool = 1.2;
                // its army charges as one; riflemen hold and fire
                let n = self.s.nen as usize;
                for e in self.s.en[..n]
                    .iter_mut()
                    .filter(|e| e.state == ES_WAIT && e.kind != E_RIFLE)
                {
                    e.state = ES_CHARGE;
                }
            }
        }
        self.lay_track();
        self.move_soldiers(dt);
        self.fire(dt);
        self.move_bullets(dt);
        self.move_enemies(dt);
        self.pass_things(dt);
        self.move_shells(dt);
        if self.s.phase == P_BOSS {
            ev |= self.boss_step(dt);
        }
        self.drop_behind();
        if self.s.count <= 0 && self.s.phase != P_CLEAR {
            self.s.count = 0;
            for i in 0..self.s.nsol as usize {
                if self.s.sol[i].state == S_OK {
                    self.s.sol[i].state = S_DOWN;
                    self.s.sol[i].t = 0.0;
                }
            }
            self.s.phase = P_DEAD;
            self.s.phase_ms = 0;
            ev |= wg::EV_LOSE | wg::EV_GAMEOVER;
        }
        ev
    }

    fn steer(&mut self, input: &Input, dt: f32) {
        let s = &mut self.s;
        let dir = (input.held & wg::RIGHT != 0) as i32 - (input.held & wg::LEFT != 0) as i32;
        if dir != 0 {
            s.drag[0] = 0.0;
            s.tx += dir as f32 * HALF * 2.2 * dt;
        }
        // a big squad steered to the side squeezes up against the edge
        s.tx = s.tx.clamp(-HALF + 0.8, HALF - 0.8);
        let step = 16.0 * dt;
        let want = s.cx + (s.tx - s.cx) * (1.0 - (-14.0 * dt).exp());
        s.cx = want.clamp(s.cx - step, s.cx + step);
    }

    /// Soldiers keep their places in the squad, squeezed to fit the pier; off its edge they fall.
    fn move_soldiers(&mut self, dt: f32) {
        let ease = 1.0 - (-9.0 * dt).exp();
        let n = self.s.nsol as usize;
        let alive = self.s.alive();
        // the deck under the squad, front to back; it squeezes in to fit
        let r = radius(alive as i32);
        let (mut lo, mut hi) = (-HALF, HALF);
        for k in -2..=2 {
            let (l, h) = self.s.deck_span(self.s.cx, self.s.d + k as f32 * r * 0.5);
            lo = lo.max(l);
            hi = hi.min(h);
        }
        const EDGE: f32 = 0.3;
        let spread = (r - 0.3).max(0.01);
        let room = ((hi - lo) * 0.5 - EDGE).max(0.2);
        // steered against an edge it squeezes up, to half its width
        let off = (self.s.cx - lo).min(hi - self.s.cx) - EDGE;
        let squeeze = (room / spread).min((off / spread).max(0.5)).min(1.0);
        let used = spread * squeeze;
        let mid = self
            .s
            .cx
            .clamp(lo + EDGE + used, (hi - EDGE - used).max(lo + EDGE + used));
        let cheer = self.s.phase == P_CLEAR || self.s.phase == P_WON;
        let mut k = 0;
        for i in 0..n {
            let q = &mut self.s.sol[i];
            q.t += dt;
            q.flash = (q.flash - dt * 3.0).max(0.0);
            match q.state {
                S_OK => {
                    let (ox, oz) = slot(k);
                    k += 1;
                    let gx = if cheer { q.x } else { mid + ox * squeeze };
                    let gd = self.s.d + oz;
                    q.x += (gx - q.x) * ease;
                    q.d += (gd - q.d) * ease;
                }
                S_FALL => {
                    q.vy -= 18.0 * dt;
                    q.y += q.vy * dt;
                }
                _ => {}
            }
        }
        // off the deck they fall
        for i in 0..n {
            let q = self.s.sol[i];
            if !cheer && q.state == S_OK && !self.s.deck(q.x, q.d) {
                self.lose(i, true);
            }
        }
        // splashes, and the gone dropped
        let mut w = 0;
        for i in 0..self.s.nsol as usize {
            let q = self.s.sol[i];
            if q.state == S_FALL && q.y < 0.2 && q.y - q.vy * dt >= 0.2 {
                self.fx(scene::FX_SPLASH, q.x, 0.0, q.d, 0.6, 0.0, 0.0, 0);
            }
            let gone = (q.state == S_FALL && q.y < -2.0) || (q.state == S_DOWN && q.t > 1.6);
            if !gone {
                self.s.sol[w] = q;
                w += 1;
            }
        }
        self.s.nsol = w as i32;
    }

    /// The squad fires ahead, each of its shooters() at its gun's rate. Past
    /// VOLLEY bullets in the air, each bullet carries the damage of several shots.
    fn fire(&mut self, dt: f32) {
        if self.s.alive() == 0 || self.s.phase == P_CLEAR {
            return;
        }
        let g = &WEAPONS[self.s.weapon as usize];
        let want = shooters(self.s.count as f32) * g.rate;
        let room = VOLLEY * g.speed / (g.range * g.pellets as f32);
        let weight = (want / room).max(1.0);
        self.s.fire_acc += dt * want.min(room);
        if self.s.fire_acc < 1.0 {
            return;
        }
        // the tan army close by first; in a boss fight, then the boss; else the
        // nearest barrel, crate or TNT across the squad's way, so however wide
        // it has spread, all of it shoots at what it is running into
        let sd = self.s.d;
        let close = |e: &&Enemy| e.state != ES_DOWN && e.d > sd && e.d < sd + 14.0;
        let aim = if self.s.boss.state == 2 {
            let near = self
                .s
                .enemies()
                .iter()
                .filter(close)
                .min_by(|a, b| a.d.total_cmp(&b.d));
            Some(near.map_or((self.s.boss.x, self.s.boss.d), |e| (e.x, e.d)))
        } else if self.s.enemies().iter().any(|e| close(&e)) {
            // a group is wider than the squad: straight ahead, every shot finds one
            None
        } else {
            let (lo, hi) = self
                .s
                .soldiers()
                .iter()
                .filter(|q| q.state == S_OK)
                .fold((f32::MAX, f32::MIN), |(lo, hi), q| {
                    (lo.min(q.x), hi.max(q.x))
                });
            self.s
                .things()
                .iter()
                .filter(|t| {
                    t.breakable()
                        && t.d > sd + if t.kind == K_TNT { TNT_NEAR } else { 0.0 }
                        && t.d < sd + g.range
                        && t.x + half_size(t.kind) > lo
                        && t.x - half_size(t.kind) < hi
                })
                .min_by(|a, b| a.d.total_cmp(&b.d))
                .map(|t| (t.x, t.d))
        };
        let mut guard = 0;
        while self.s.fire_acc >= 1.0 && guard < 12 {
            self.s.fire_acc -= 1.0;
            guard += 1;
            let pick = self.s.rng.below(self.s.nsol as u32) as usize;
            let q = self.s.sol[pick];
            if q.state != S_OK {
                continue;
            }
            for p in 0..g.pellets {
                if self.s.nbul as usize >= MAXB {
                    break;
                }
                let (mut vx, vd) = if g.pellets > 1 {
                    // a fan of pellets, some faster than others
                    let fan = (p as f32 / (g.pellets - 1) as f32 - 0.5) * g.spread;
                    (
                        fan + self.s.frand(-0.3, 0.3),
                        g.speed * self.s.frand(0.9, 1.1),
                    )
                } else {
                    (self.s.frand(-g.spread, g.spread) * 0.5, g.speed)
                };
                if let Some((ax, ad)) = aim {
                    // turned towards what is ahead of it, as far as a soldier turns
                    let ahead = ad - q.d;
                    if ahead > 1.0 {
                        vx += ((ax - q.x) / ahead).clamp(-1.0, 1.0) * vd;
                    }
                }
                let b = Bullet {
                    x: q.x - 0.12,
                    d: q.d + 0.35,
                    y: DECK + 0.52,
                    vx,
                    vd,
                    vy: 0.0,
                    life: g.range / vd,
                    // a share of a shot's damage over a whole one is a chance of one more
                    dmg: (g.dmg as f32 * weight + self.s.rng.float()) as i32,
                    pierce: g.pierce,
                    gun: self.s.weapon,
                };
                self.s.bul[self.s.nbul as usize] = b;
                self.s.nbul += 1;
            }
            let gun = self.s.weapon as f32;
            self.fx(
                scene::FX_SHOT,
                q.x - 0.12,
                DECK + 0.52,
                q.d + 0.4,
                gun,
                0.0,
                0.0,
                0,
            );
        }
        self.s.fire_acc = self.s.fire_acc.min(2.0);
    }

    fn move_bullets(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.s.nbul as usize {
            let mut b = self.s.bul[i];
            let d0 = b.d;
            b.x += b.vx * dt;
            b.d += b.vd * dt;
            b.y += b.vy * dt;
            b.life -= dt;
            let mut spent = b.life <= 0.0 || b.y < DECK;
            if !spent {
                spent = if b.ours() {
                    self.hit_enemy_side(&mut b, d0)
                } else {
                    self.hit_squad(&b)
                };
            }
            if spent {
                self.s.nbul -= 1;
                self.s.bul[i] = self.s.bul[self.s.nbul as usize];
            } else {
                self.s.bul[i] = b;
                i += 1;
            }
        }
    }

    /// Our bullet from d0 to b.d: what it hits first. True when it is spent.
    fn hit_enemy_side(&mut self, b: &mut Bullet, d0: f32) -> bool {
        let (lo, hi) = (d0.min(b.d), d0.max(b.d));
        // enemies
        let mut best: Option<(usize, f32)> = None;
        for (k, e) in self.s.enemies().iter().enumerate() {
            if e.state == ES_DOWN {
                continue;
            }
            let r = if e.kind == E_BRUTE { 0.6 } else { 0.38 };
            if (e.x - b.x).abs() < r
                && e.d >= lo - r
                && e.d <= hi + r
                && best.map_or(true, |(_, bd)| e.d < bd)
            {
                best = Some((k, e.d));
            }
        }
        // things in the way
        let mut thing: Option<(usize, f32)> = None;
        for (k, t) in self.s.things().iter().enumerate() {
            let hit = match t.kind {
                K_GATE => {
                    t.state == 0 && (b.x - t.x).abs() < t.w * 0.5 && t.d >= lo && t.d <= hi + 0.2
                }
                _ if t.breakable() => {
                    (b.x - t.x).abs() < half_size(t.kind)
                        && t.d >= lo - half_size(t.kind)
                        && t.d <= hi + half_size(t.kind)
                }
                _ => false,
            };
            if hit && thing.map_or(true, |(_, td)| t.d < td) {
                thing = Some((k, t.d));
            }
        }
        if let (Some((_, ed)), Some((_, td))) = (best, thing) {
            if td < ed {
                best = None;
            } else {
                thing = None;
            }
        }
        if let Some((k, _)) = best {
            let (ex, ed) = (self.s.en[k].x, self.s.en[k].d);
            self.fx(scene::FX_HIT, ex, DECK + 0.6, ed, 0.0, 0.0, 0.0, scene::TAN);
            let e = &mut self.s.en[k];
            // a bullet carrying several shots goes on into the next with the
            // ones this one didn't take
            let took = e.hp.max(WEAPONS[b.gun as usize].dmg);
            e.hp -= b.dmg;
            e.flash = 1.0;
            if e.hp <= 0 {
                self.enemy_down(k);
            }
            if b.dmg > took {
                b.dmg -= took;
                return self.hit_enemy_side(b, d0);
            }
            b.pierce -= 1;
            return b.pierce < 0;
        }
        if let Some((k, _)) = thing {
            self.hit_thing(k, b.dmg);
            return true;
        }
        // the boss, its legs and body: the same box whichever it is, so each is as easy to hit
        let boss = self.s.boss;
        let (wide, deep) = (0.85 * BOSS_SCALE, 0.7 * BOSS_SCALE);
        if boss.state == 2
            && (b.x - boss.x).abs() < wide
            && boss.d >= lo - deep
            && boss.d <= hi + deep
        {
            let y = DECK + self.s.frand(0.18, 0.6) * boss.tall();
            if boss.body().armoured {
                self.fx(
                    scene::FX_CLANG,
                    b.x,
                    y,
                    boss.d - deep,
                    b.dmg as f32,
                    0.0,
                    0.0,
                    CLANG,
                );
            } else {
                self.fx(
                    scene::FX_HIT,
                    b.x,
                    y,
                    boss.d - deep,
                    0.0,
                    0.0,
                    0.0,
                    scene::TAN,
                );
            }
            let hurt = b.dmg;
            let boss = &mut self.s.boss;
            boss.hp = (boss.hp - hurt).max(0);
            // under steady fire it blinks now and then rather than stays white
            if boss.flash <= BLINK_GAP {
                boss.flash = 1.0;
            }
            let dead = boss.hp == 0;
            self.add_score(hurt);
            if dead {
                self.boss_down();
            }
            return true;
        }
        false
    }

    /// An enemy bullet: knocks out the first soldier it reaches; the boss's
    /// rounds burst there, and knock out all round it too.
    fn hit_squad(&mut self, b: &Bullet) -> bool {
        let n = self.s.nsol as usize;
        let hit = (0..n).find(|&i| {
            let q = self.s.sol[i];
            q.state == S_OK
                && b.y < DECK + 1.2
                && (q.x - b.x).abs() < 0.35
                && (q.d - b.d).abs() < 0.5
        });
        let Some(i) = hit else {
            return false;
        };
        if b.gun == G_BOSS {
            let (x, d) = (self.s.sol[i].x, self.s.sol[i].d);
            self.blast(x, d, 0.9, BLAST_FOE);
        } else {
            self.lose(i, false);
        }
        true
    }

    fn hit_thing(&mut self, k: usize, dmg: i32) {
        let t = &mut self.s.thing[k];
        if t.flash < 0.4 {
            t.flash = 1.0;
        }
        match t.kind {
            K_GATE => {
                // shooting a gate counts it up, a point for every hp of damage
                // and at most hp_max points; a minus one counts down and at
                // nought turns plus
                if t.a == OP_ADD || t.a == OP_SUB {
                    t.c += dmg;
                    let n = (t.c / t.hp).min(t.hp_max);
                    t.c %= t.hp;
                    t.hp_max -= n;
                    if t.a == OP_ADD {
                        t.value = (t.value + n).min(MAX_COUNT);
                    } else if t.value > n {
                        t.value -= n;
                    } else {
                        t.a = OP_ADD;
                        t.value = n - t.value;
                    }
                }
                let (x, d) = (t.x, t.d);
                self.fx(scene::FX_HIT, x, DECK + 1.6, d, 0.0, 0.0, 0.0, 0xff_ffe0a0);
            }
            K_BARREL | K_TNT | K_CRATE => {
                t.hp = (t.hp - dmg).max(0);
                let (x, d, kind) = (t.x, t.d, t.kind);
                if t.hp == 0 {
                    self.break_thing(k);
                } else {
                    self.fx(
                        scene::FX_HIT,
                        x,
                        DECK + 0.7,
                        d,
                        0.0,
                        0.0,
                        0.0,
                        if kind == K_TNT {
                            0xff_3030e0
                        } else {
                            0xff_40a0e0
                        },
                    );
                }
            }
            _ => {}
        }
    }

    /// A barrel, crate or TNT barrel shot open.
    fn break_thing(&mut self, k: usize) {
        let t = self.s.thing[k];
        self.s.thing[k].hp = 0;
        self.s.thing[k].state = 1;
        match t.kind {
            K_BARREL => {
                self.s.count = (self.s.count + t.value).min(MAX_COUNT);
                self.fill_squad();
                self.add_score(20);
                self.fx(
                    scene::FX_PICKUP,
                    t.x,
                    DECK,
                    t.d,
                    0.0,
                    0.0,
                    0.0,
                    scene::GREEN_LIGHT,
                );
            }
            K_CRATE => {
                if t.value > self.s.weapon {
                    self.s.weapon = t.value;
                    self.banner(B_GUN, t.value);
                }
                self.add_score(30);
                self.fx(scene::FX_PICKUP, t.x, DECK, t.d, 0.0, 0.0, 0.0, 0xff_40d0ff);
            }
            K_TNT => self.blast(t.x, t.d, 3.4, BLAST_TNT),
            _ => {}
        }
    }

    /// An explosion: knocks out every figure within r, and sets off TNT nearby.
    /// An explosion of radius r: `how` is 0 or BLAST_* flags.
    fn blast(&mut self, x: f32, d: f32, r: f32, how: u32) {
        let big = how & BLAST_TNT != 0;
        self.fx(scene::FX_BLAST, x, DECK, d, r / 3.0, 0.0, 0.0, 0);
        self.s.shake = self.s.shake.max(if big { 0.35 } else { 0.2 });
        for k in 0..self.s.nen as usize {
            let e = self.s.en[k];
            if how & BLAST_FOE == 0 && e.state != ES_DOWN && (e.x - x).hypot(e.d - d) < r {
                self.s.en[k].hp -= 6;
                if self.s.en[k].hp <= 0 {
                    self.enemy_down(k);
                }
            }
        }
        // a TNT barrel's blast is meant for the enemy: it reaches less far into the squad
        let reach = r * if big { 0.65 } else { 0.85 };
        for i in 0..self.s.nsol as usize {
            let q = self.s.sol[i];
            if q.state == S_OK && (q.x - x).hypot(q.d - d) < reach {
                self.lose(i, false);
            }
        }
        for k in 0..self.s.nthing as usize {
            let t = self.s.thing[k];
            if t.kind == K_TNT && t.hp > 0 && (t.x - x).hypot(t.d - d) < r {
                self.break_thing(k);
            }
        }
    }

    fn move_enemies(&mut self, dt: f32) {
        let (cx, sd) = (self.s.cx, self.s.d);
        let hard = self.s.hardness();
        let edge = radius(self.s.alive() as i32) + 2.0;
        let mut w = 0;
        for k in 0..self.s.nen as usize {
            let mut e = self.s.en[k];
            e.t += dt;
            e.flash = (e.flash - dt * 4.0).max(0.0);
            e.cool -= dt;
            match e.state {
                ES_WAIT => {
                    let near = e.d - sd;
                    if near < if e.kind == E_RIFLE { 26.0 } else { 20.0 } {
                        if e.kind == E_RIFLE {
                            // stands and fires
                            if e.cool <= 0.0 && near > 2.0 && self.s.nbul < MAXB as i32 {
                                e.cool = (2.2 - hard * 0.04).max(1.1) + self.s.rng.float();
                                let dx = cx - e.x;
                                let speed = 18.0;
                                let time = near / speed;
                                self.s.bul[self.s.nbul as usize] = Bullet {
                                    x: e.x,
                                    d: e.d - 0.4,
                                    y: DECK + 0.52,
                                    vx: dx / time.max(0.3),
                                    vd: -speed,
                                    vy: 0.0,
                                    life: 2.0,
                                    dmg: 1,
                                    pierce: 0,
                                    gun: G_RIFLE,
                                };
                                self.s.nbul += 1;
                                self.fx(
                                    scene::FX_MUZZLE,
                                    e.x,
                                    DECK + 0.52,
                                    e.d - 0.5,
                                    0.0,
                                    1.0,
                                    0.7,
                                    0,
                                );
                            }
                            if near < 6.0 {
                                e.state = ES_CHARGE;
                            }
                        } else {
                            e.state = ES_CHARGE;
                        }
                    }
                    e.yaw = 0.0;
                }
                ES_CHARGE => {
                    // at the squad, and once at its edge, at the nearest soldier
                    let mut best = (f32::MAX, cx, sd);
                    if (e.x - cx).hypot(e.d - sd) < edge {
                        for q in self.s.soldiers().iter().filter(|q| q.state == S_OK) {
                            let dd = (q.x - e.x).powi(2) + (q.d - e.d).powi(2);
                            if dd < best.0 {
                                best = (dd, q.x, q.d);
                            }
                        }
                    }
                    let (dx, dd) = (best.1 - e.x, best.2 - e.d);
                    let len = (dx * dx + dd * dd).sqrt().max(0.001);
                    let speed = match e.kind {
                        E_RUNNER => 6.5,
                        E_BRUTE => 2.6,
                        _ => 3.6,
                    } + (hard * 0.05).min(1.0);
                    if len > REACH * 0.8 {
                        e.x += dx / len * speed * dt;
                        e.d += dd / len * speed * dt;
                    }
                    e.x = e.x.clamp(-HALF + 0.3, HALF - 0.3);
                    e.yaw = dx.atan2(-dd);
                }
                _ => {}
            }
            let gone = e.state == ES_DOWN && e.t > 1.8;
            if !gone {
                self.s.en[w] = e;
                w += 1;
            }
        }
        self.s.nen = w as i32;
        // hand to hand: an enemy and a soldier in reach knock each other out (a brute takes several)
        for k in 0..self.s.nen as usize {
            let e = self.s.en[k];
            if e.state == ES_DOWN || (e.cool > 0.0 && e.kind == E_BRUTE) {
                continue;
            }
            if (e.d - sd).abs() > 8.0 {
                continue;
            }
            for i in 0..self.s.nsol as usize {
                let q = self.s.sol[i];
                if q.state != S_OK || (q.x - e.x).abs() > REACH || (q.d - e.d).abs() > REACH {
                    continue;
                }
                self.lose(i, false);
                let en = &mut self.s.en[k];
                en.hp = if en.kind == E_BRUTE { en.hp - 1 } else { 0 };
                en.cool = 0.18;
                if en.hp <= 0 {
                    self.enemy_down(k);
                }
                break;
            }
        }
        // an enemy that falls behind the squad or off the pier is gone
        for k in 0..self.s.nen as usize {
            let e = self.s.en[k];
            if e.state != ES_DOWN && (e.d < sd - 6.0 || !self.s.deck(e.x, e.d)) {
                self.s.en[k].state = ES_DOWN;
                self.s.en[k].t = 0.0;
            }
        }
    }

    /// Gates the squad goes through, barrels and crates it runs into, ships firing on it.
    fn pass_things(&mut self, dt: f32) {
        let (cx, sd) = (self.s.cx, self.s.d);
        for k in 0..self.s.nthing as usize {
            let t = self.s.thing[k];
            self.s.thing[k].flash = (t.flash - dt * 4.0).max(0.0);
            match t.kind {
                K_GATE if t.state == 0 && sd >= t.d => {
                    // the gate the squad's middle is in, or the nearer
                    let other = self.s.thing[t.b as usize];
                    let mine = (cx - t.x).abs() <= (cx - other.x).abs() || other.kind != K_GATE;
                    if !mine {
                        continue;
                    }
                    self.s.thing[k].state = 1;
                    if other.kind == K_GATE && other.state == 0 {
                        self.s.thing[t.b as usize].state = 2;
                    }
                    let before = self.s.count;
                    let t = self.s.thing[k];
                    let after = gate_sum(t.a, t.value, before).clamp(0, MAX_COUNT);
                    self.s.count = after;
                    let good = after >= before;
                    self.fx(
                        scene::FX_GATE,
                        t.x,
                        DECK,
                        t.d,
                        t.w,
                        good as i32 as f32,
                        0.0,
                        if good { 0xff_60d0ff } else { 0xff_5050ff },
                    );
                    // the ones lost go from the back
                    self.fill_squad();
                }
                _ if t.breakable() && (t.d - sd).abs() < 4.0 => {
                    // soldiers running into it: as many as a barrel holds
                    // break it unshot, so it is no gain, and a crate takes
                    // more on later levels
                    let ram = if t.kind == K_BARREL {
                        t.value.max(1)
                    } else {
                        4 + (self.s.hardness() / 4.0) as i32
                    };
                    for i in 0..self.s.nsol as usize {
                        let q = self.s.sol[i];
                        if q.state != S_OK
                            || (q.x - t.x).abs() > half_size(t.kind) + 0.1
                            || (q.d - t.d).abs() > half_size(t.kind)
                        {
                            continue;
                        }
                        if t.kind == K_TNT {
                            self.break_thing(k);
                            break;
                        }
                        self.lose(i, false);
                        // what is left of it takes its share of ram soldiers
                        let th = &mut self.s.thing[k];
                        let full = th.hp_max.max(th.hp);
                        let left = (th.hp * ram + full - 1) / full;
                        th.hp = full * (left - 1) / ram;
                        th.flash = 1.0;
                        if th.hp == 0 {
                            self.break_thing(k);
                            break;
                        }
                    }
                }
                K_SHIP => {
                    // fires a shell at where the squad will be
                    let near = t.d - sd;
                    if near < 26.0 && near > -8.0 {
                        self.s.thing[k].cool -= dt;
                        if self.s.thing[k].cool <= 0.0 && self.s.nshell < MAXSH as i32 {
                            let hard = self.s.hardness();
                            self.s.thing[k].cool = (2.6 - hard * 0.05).max(1.5);
                            let flight = 1.9;
                            let lead = self.s.speed * flight;
                            let x = (cx + self.s.frand(-1.5, 1.5)).clamp(-HALF + 0.8, HALF - 0.8);
                            let dir = if t.x < 0.0 { 1.0 } else { -1.0 };
                            let d = sd + lead + self.s.frand(-1.0, 1.0);
                            self.lob(t.x + dir * 2.2, 2.2, t.d, x, d, 1.8, flight, 0);
                            self.fx(
                                scene::FX_CANNON,
                                t.x + dir * 2.2,
                                2.2,
                                t.d,
                                dir,
                                0.0,
                                0.0,
                                0,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// A shell from ox, oy, od to land on x, d in `flight` seconds, knocking out all within r.
    /// The shells in the air are gone: the page stops their whistles (FX_INCOMING with a 0).
    fn hush_shells(&mut self) {
        if self.s.nshell > 0 {
            self.s.nshell = 0;
            self.fx(scene::FX_INCOMING, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0);
        }
    }

    pub fn lob(
        &mut self,
        ox: f32,
        oy: f32,
        od: f32,
        x: f32,
        d: f32,
        r: f32,
        flight: f32,
        kind: i32,
    ) {
        if self.s.nshell as usize >= MAXSH {
            return;
        }
        self.s.shell[self.s.nshell as usize] = Shell {
            x,
            d,
            ox,
            oy,
            od,
            r,
            t: 0.0,
            t_max: flight,
            kind,
        };
        self.s.nshell += 1;
        // the whistle as it comes down
        self.fx(scene::FX_INCOMING, x, DECK, d, flight, 0.0, 0.0, 0);
    }

    fn move_shells(&mut self, dt: f32) {
        let mut i = 0;
        while i < self.s.nshell as usize {
            // shells fall with the squad still running at them
            let sh = &mut self.s.shell[i];
            let before = sh.t;
            sh.t += dt;
            let sh = *sh;
            if sh.t >= sh.t_max {
                self.s.nshell -= 1;
                self.s.shell[i] = self.s.shell[self.s.nshell as usize];
                self.blast(sh.x, sh.d, sh.r, if sh.kind == 1 { BLAST_FOE } else { 0 });
                continue;
            }
            // a trail of smoke behind it
            if ticks(sh.t, sh.t - before, 14.0) {
                let (x, y, d) = sh.at();
                self.fx(
                    scene::FX_SMOKE,
                    x,
                    y,
                    d,
                    0.35,
                    0.0,
                    0.0,
                    scene::rgba(0x8a8580, 170),
                );
            }
            i += 1;
        }
    }

    /// The boss stands its ground facing the squad, walks to another spot now
    /// and then, and takes turns at a volley, shells lobbed round the squad,
    /// waving troops on, and walking up to jump on the squad.
    fn boss_step(&mut self, dt: f32) -> u32 {
        let hard = self.s.hardness();
        let b = &mut self.s.boss;
        b.t += dt;
        b.flash = (b.flash - dt * 10.0).max(BLINK_GAP);
        if b.state == 3 {
            // going down, tipping over or in blasts; then the level is won
            b.atk_t += dt;
            let t = b.atk_t;
            let (x, d, tall) = (b.x, b.d, b.tall());
            let m = b.body();
            if !m.armoured {
                if crossed(t, dt, TOPPLE[1]) {
                    // tipped over its side, its middle hits the deck half its height further out
                    let (s, c) = b.yaw.sin_cos();
                    let out = b.fall_side() * (m.width * m.scale + tall * 0.5);
                    self.fx(
                        scene::FX_STOMP,
                        x + out * c,
                        DECK,
                        d + out * s,
                        JUMP_R,
                        0.0,
                        0.0,
                        0,
                    );
                    self.s.shake = self.s.shake.max(0.8);
                }
            } else if ticks(t, dt, 4.0) && t < 1.6 {
                let ox = self.s.frand(-1.5, 1.5);
                let od = self.s.frand(-1.0, 1.0);
                let oy = self.s.frand(0.1, 0.55) * tall;
                self.fx(scene::FX_BLAST, x + ox, DECK + oy, d + od, 0.8, 0.0, 0.0, 0);
                self.s.shake = self.s.shake.max(0.3);
            }
            if t >= 2.4 {
                return self.level_cleared();
            }
            return 0;
        }
        if b.state != 2 {
            return 0;
        }
        let enraged = b.hp * 2 < b.hp_max;
        if enraged && ticks(b.t, dt, 5.0) {
            let (x, d, tall) = (b.x, b.d, b.tall());
            let ox = self.s.frand(-0.8, 0.8);
            self.fx(
                scene::FX_SMOKE,
                x + ox,
                DECK + 0.8 * tall,
                d + 0.5,
                1.3,
                0.0,
                0.0,
                scene::rgba(0x4a4540, 255),
            );
        }
        // it reels as each quarter of its hp goes; in the air, once it is down
        let b = &mut self.s.boss;
        if b.hp < b.flinch_at && !(b.atk == A_JUMP && b.stage == 2) {
            while b.hp < b.flinch_at {
                b.flinch_at -= (b.hp_max / 4).max(1);
            }
            b.hurt = 0.55;
            let (x, d, tall) = (b.x, b.d, b.tall());
            if b.body().armoured {
                self.fx(
                    scene::FX_CLANG,
                    x,
                    DECK + 0.65 * tall,
                    d - 0.6,
                    12.0,
                    0.0,
                    0.0,
                    CLANG,
                );
            }
            self.fx(scene::FX_BLAST, x, DECK + 0.6 * tall, d, 0.5, 0.0, 0.0, 0);
            self.s.shake = self.s.shake.max(0.2);
        }
        let b = &mut self.s.boss;
        if b.hurt > 0.0 {
            b.hurt -= dt;
            return 0;
        }
        b.atk_t += dt;
        let (cx, sd) = (self.s.cx, self.s.d);
        let (atk, stage, t) = (b.atk, b.stage, b.atk_t);
        // a volley or shells: it turns to the squad first
        if matches!(atk, A_VOLLEY | A_SHELLS) && stage == 0 {
            if self.boss_face(dt) || t > 0.8 {
                self.boss_stage(1);
            }
            return 0;
        }
        match atk {
            A_MOVE => {
                let (tx, td) = (b.tx, b.td);
                if self.boss_walk(tx, td, dt) {
                    self.boss_rest(hard, enraged);
                }
            }
            A_VOLLEY => {
                self.boss_face(dt);
                let shots = 5 + (hard / 6.0) as i32 + 2 * enraged as i32;
                let rate = 5.0;
                if ticks(t, dt, rate) && t * rate <= shots as f32 + 1.0 && self.s.nbul < MAXB as i32
                {
                    let (x, y, d) = self.s.boss.muzzle();
                    let tx = cx + self.s.frand(-1.6, 1.6);
                    let speed = 17.0;
                    // into the squad's front rows
                    let front = sd + radius(self.s.alive() as i32) * 0.7;
                    let time = ((d - front) / speed).max(0.3);
                    self.s.bul[self.s.nbul as usize] = Bullet {
                        x,
                        d,
                        y,
                        vx: (tx - x) / time,
                        vd: -speed,
                        // down from its gun hand
                        vy: (DECK + 0.5 - y) / time,
                        life: 3.0,
                        dmg: 1,
                        pierce: 0,
                        gun: G_BOSS,
                    };
                    self.s.nbul += 1;
                    self.fx(scene::FX_MUZZLE, x, y, d - 0.3, 0.0, 1.0, 2.2, 0);
                }
                if t > shots as f32 / rate + 0.4 {
                    self.boss_rest(hard, enraged);
                }
            }
            A_SHELLS => {
                let at = 0.2;
                if crossed(t, dt, at) {
                    let n = (3 + (hard / 5.0) as i32 + enraged as i32).min(6);
                    let flight = (2.2 - hard * 0.025).max(1.5);
                    // early on a shell takes a smaller bite out of a small squad
                    let blast = (1.3 + hard * 0.04).min(1.8);
                    let r = radius(self.s.alive() as i32);
                    let (x, y, d) = self.s.boss.muzzle();
                    for k in 0..n {
                        // the first at the squad's middle, the rest round it
                        let (tx, td) = if k == 0 {
                            (cx, sd)
                        } else {
                            let a = self.s.frand(0.0, 6.28);
                            let far = self.s.frand(0.5, 1.0) * (r + 2.0);
                            (cx + a.cos() * far, sd + a.sin() * far)
                        };
                        let tx = tx.clamp(-HALF + 1.0, HALF - 1.0);
                        self.lob(x, y, d, tx, td, blast, flight + k as f32 * 0.18, 1);
                    }
                    self.fx(scene::FX_CANNON, x, y + 0.5, d - 0.5, 0.0, 1.0, 1.6, 0);
                }
                if t > 0.8 {
                    self.boss_rest(hard, enraged);
                }
            }
            A_TROOPS => {
                self.boss_face(dt);
                let at = 0.55;
                if crossed(t, dt, at) {
                    let n = (6 + (hard * 0.8) as i32).min(24);
                    let (bx, bd) = (self.s.boss.x, self.s.boss.d);
                    for k in 0..n {
                        let (ox, oz) = slot(k as usize);
                        let kind = if hard >= 4.0 && k % 4 == 3 {
                            E_RUNNER
                        } else {
                            E_GRUNT
                        };
                        self.spawn_enemy(
                            kind,
                            (bx + ox * 1.8).clamp(-HALF + 0.4, HALF - 0.4),
                            bd + 1.5 + oz,
                            -1,
                            ES_CHARGE,
                        );
                    }
                }
                if t > self.s.boss.body().hello {
                    self.boss_rest(hard, enraged);
                }
            }
            A_JUMP => return self.boss_jump(dt, hard, enraged),
            _ => {
                self.boss_face(dt);
                self.s.boss.cool -= dt;
                if self.s.boss.cool <= 0.0 {
                    self.boss_next(hard, enraged);
                }
            }
        }
        0
    }

    /// Walking up to the squad (in line with tx), then jumping on it (A_JUMP). It picks where it
    /// will land (tx, td) as it stops and holds a moment, so a squad that moves off is missed.
    fn boss_jump(&mut self, dt: f32, hard: f32, enraged: bool) -> u32 {
        let b = &mut self.s.boss;
        let (stage, t) = (b.stage, b.atk_t);
        let m = b.body();
        let (tx, td) = (b.tx, b.td);
        match stage {
            0 => {
                // straight up to the squad's front, keeping pace as it walks on to the arena
                let front = self.s.d + radius(self.s.alive() as i32) * 0.6;
                if self.boss_walk(tx, front + JUMP_REACH, dt) || t > 6.0 {
                    // where the squad is now, as far as one leap goes
                    let b = &mut self.s.boss;
                    let (dx, dd) = (jump_x(self.s.cx) - b.x, front - b.d);
                    let k = (JUMP_FAR / dx.hypot(dd).max(0.1)).min(1.0);
                    (b.tx, b.td) = (b.x + dx * k, b.d + dd * k);
                    self.boss_stage(1);
                }
            }
            1 => {
                let b = &mut self.s.boss;
                let want = (tx - b.x).atan2(b.d - td);
                let hold = (1.0 - hard * 0.03).max(0.4) * if enraged { 0.8 } else { 1.0 };
                if turn(&mut b.yaw, want, 2.5 * dt) && t > hold {
                    b.from = [b.x, b.d];
                    self.boss_stage(2);
                }
            }
            2 => {
                // in the air between lift and land, onto where it aimed
                let u = ((t - m.lift) / (m.land - m.lift)).clamp(0.0, 1.0);
                let u = u * u * (3.0 - 2.0 * u);
                b.x = b.from[0] + (tx - b.from[0]) * u;
                b.d = b.from[1] + (td + 0.8 - b.from[1]) * u;
                if crossed(t, dt, m.land) {
                    self.fx(scene::FX_STOMP, tx, DECK, td, JUMP_R, 0.0, 0.0, 0);
                    self.s.shake = self.s.shake.max(0.7);
                    for k in 0..self.s.nsol as usize {
                        let q = self.s.sol[k];
                        if q.state == S_OK && (q.x - tx).hypot(q.d - td) < JUMP_R {
                            self.lose(k, false);
                        }
                    }
                }
                if t > m.land + 0.6 {
                    // and back to its spot, facing the squad
                    let back = (self.s.boss.x, self.s.arena_d + BOSS_HOME);
                    self.boss_start(A_MOVE);
                    (self.s.boss.tx, self.s.boss.td) = back;
                }
            }
            _ => self.boss_rest(hard, enraged),
        }
        0
    }

    /// The boss's next move: another spot, or one of its attacks.
    fn boss_next(&mut self, hard: f32, enraged: bool) {
        let roll = self.s.rng.below(100) as i32;
        let jump = if enraged { 34 } else { 26 };
        let troops = if hard >= 1.0 { 16 } else { 0 };
        let atk = if roll < 22 {
            A_MOVE
        } else if roll < 22 + jump {
            A_JUMP
        } else if roll < 22 + jump + troops {
            A_TROOPS
        } else if roll % 2 == 0 {
            A_SHELLS
        } else {
            A_VOLLEY
        };
        self.boss_start(atk);
        if atk == A_MOVE {
            // another spot, well along the arena
            let x = self.s.boss.x;
            let mut tx = self.s.frand(-HALF + 3.0, HALF - 3.0);
            if (tx - x).abs() < 3.0 {
                tx = if x > 0.0 { x - 4.0 } else { x + 4.0 };
            }
            let td = self.s.arena_d + BOSS_HOME + self.s.frand(-1.5, 1.5);
            (self.s.boss.tx, self.s.boss.td) = (tx, td);
        } else if atk == A_JUMP {
            // it walks straight up to the squad, in line with where it is now (boss_jump)
            self.s.boss.tx = jump_x(self.s.cx);
        }
    }

    /// The boss starts on an attack (A_*), or A_NONE to stand.
    fn boss_start(&mut self, atk: i32) {
        self.s.boss.atk = atk;
        self.boss_stage(0);
    }

    fn boss_stage(&mut self, stage: i32) {
        self.s.boss.stage = stage;
        self.s.boss.atk_t = 0.0;
    }

    /// An attack done: the boss stands a while.
    fn boss_rest(&mut self, hard: f32, enraged: bool) {
        self.boss_start(A_NONE);
        self.s.boss.cool = (1.4 - hard * 0.03).max(0.6) * if enraged { 0.7 } else { 1.0 };
    }

    /// The boss turns to face the squad; true once it does.
    fn boss_face(&mut self, dt: f32) -> bool {
        let (cx, sd) = (self.s.cx, self.s.d);
        let b = &mut self.s.boss;
        let want = (cx - b.x).atan2(b.d - sd);
        turn(&mut b.yaw, want, 1.8 * dt)
    }

    /// The boss walks a step towards tx, td: turned the way it goes, or backing
    /// away still facing the squad, its walk as fast as its feet; true once there.
    fn boss_walk(&mut self, tx: f32, td: f32, dt: f32) -> bool {
        let (cx, sd) = (self.s.cx, self.s.d);
        let b = &mut self.s.boss;
        let (dx, dd) = (tx - b.x, td - b.d);
        let far = dx.hypot(dd);
        if far < 0.05 {
            return true;
        }
        let back = dd > dx.abs();
        let want = if back {
            (cx - b.x).atan2(b.d - sd)
        } else {
            dx.atan2(-dd)
        };
        if !turn(&mut b.yaw, want, 1.8 * dt) {
            return false;
        }
        let step = (b.speed() * dt).min(far);
        b.x += dx / far * step;
        b.d += dd / far * step;
        let m = b.body();
        let before = b.walk;
        let clip = step / (m.stride * m.scale);
        b.walk = (b.walk + if back { -clip } else { clip }).rem_euclid(m.walk);
        // a footfall each half of the walk's loop
        let half = m.walk * 0.5;
        let (k0, k1) = ((before / half).floor(), (b.walk / half).floor());
        if k0 != k1 {
            let side = if k1 as i32 % 2 == 0 { 1.0 } else { -1.0 };
            let (s, c) = b.yaw.sin_cos();
            let foot = side * m.foot * m.scale;
            let (x, d) = (b.x + foot * c, b.d + foot * s);
            self.fx(scene::FX_STEP, x, DECK, d, m.foot * m.scale, 0.0, 0.0, 0);
            self.s.shake = self.s.shake.max(0.12);
        }
        self.boss_walked = true;
        far - step < 0.05
    }

    pub fn spawn_enemy(&mut self, kind: i32, x: f32, d: f32, group: i32, state: i32) {
        if self.s.nen as usize >= MAXE {
            return;
        }
        let hp = enemy_hp(kind, self.s.hardness_at(d));
        let t = self.s.rng.float() * 2.0;
        self.s.en[self.s.nen as usize] = Enemy {
            kind,
            state,
            x,
            d,
            hp,
            group,
            t,
            cool: self.s.rng.float() * 1.5,
            flash: 0.0,
            yaw: 0.0,
        };
        self.s.nen += 1;
    }

    fn boss_down(&mut self) {
        let b = &mut self.s.boss;
        b.state = 3;
        b.atk_t = 0.0;
        b.t = 0.0;
        self.hush_shells();
        // its troops fall with it, worth nothing more
        for k in 0..self.s.nen as usize {
            let e = &mut self.s.en[k];
            if e.state != ES_DOWN {
                (e.state, e.t) = (ES_DOWN, 0.0);
                let (x, d) = (e.x, e.d);
                self.fx(scene::FX_PUFF, x, DECK, d, 0.0, 0.0, 0.0, scene::TAN);
            }
        }
        // the enemy's shots in the air fizzle
        let mut w = 0;
        for i in 0..self.s.nbul as usize {
            if self.s.bul[i].ours() {
                self.s.bul[w] = self.s.bul[i];
                w += 1;
            }
        }
        self.s.nbul = w as i32;
        self.s.bosses += 1;
        let bonus = 500 + 100 * self.s.level;
        self.add_score(bonus);
    }

    fn level_cleared(&mut self) -> u32 {
        let bonus = self.s.count * 5;
        self.add_score(bonus);
        self.s.phase = P_CLEAR;
        self.s.phase_ms = 0;
        self.banner(B_CLEAR, 0);
        if self.s.endless() {
            return wg::EV_LEVEL;
        }
        self.s.cleared = self.s.level;
        self.s.cleared_score = self.s.level_score;
        if self.s.level + 1 >= LEVELS {
            self.s.phase = P_WON;
            return wg::EV_LEVEL | wg::EV_WIN | wg::EV_GAMEOVER;
        }
        wg::EV_LEVEL
    }

    /// Drops what the squad has left behind.
    fn drop_behind(&mut self) {
        let sd = self.s.d;
        let mut w = 0;
        // where each thing kept went, for the enemies' groups
        let mut moved = [-1; MAXO];
        for k in 0..self.s.nthing as usize {
            let t = self.s.thing[k];
            // ships stay while their hulls show, arenas while the shore
            // behind them is drawn (scene.rs put_decor)
            let end = t.d
                + t.len.max(0.0)
                + match t.kind {
                    K_SHIP => 12.0,
                    K_ARENA => 20.0,
                    _ => 0.0,
                };
            if end > sd - 16.0 {
                self.s.thing[w] = t;
                moved[k] = w as i32;
                w += 1;
            }
        }
        if w != self.s.nthing as usize {
            self.s.nthing = w as i32;
            self.relink_gates();
            for k in 0..self.s.nen as usize {
                let g = &mut self.s.en[k].group;
                if *g >= 0 {
                    *g = moved.get(*g as usize).copied().unwrap_or(-1);
                }
            }
        }
    }

    /// A gate's pair is the other gate at its distance (itself when alone), found again as things move.
    pub fn relink_gates(&mut self) {
        let n = self.s.nthing as usize;
        for k in 0..n {
            if self.s.thing[k].kind != K_GATE {
                continue;
            }
            let (d, x) = (self.s.thing[k].d, self.s.thing[k].x);
            self.s.thing[k].b = (0..n)
                .find(|&j| {
                    j != k
                        && self.s.thing[j].kind == K_GATE
                        && self.s.thing[j].d == d
                        && self.s.thing[j].x != x
                })
                .map_or(k as i32, |j| j as i32);
        }
    }
}

/// Half the size of a barrel or crate, across and along the pier, for what hits it.
pub fn half_size(kind: i32) -> f32 {
    if kind == K_CRATE {
        CRATE * 0.5
    } else {
        BARREL * 0.6
    }
}

/// True when a clock at t, dt on from the last step, has just passed `at`.
fn crossed(t: f32, dt: f32, at: f32) -> bool {
    t >= at && t - dt < at
}

/// True when a clock at t, dt on from the last step, has just passed one of `rate` marks a second.
fn ticks(t: f32, dt: f32, rate: f32) -> bool {
    (t * rate) as i32 != ((t - dt) * rate) as i32
}

/// Turns `yaw` towards `want`, `step` at most; true once it faces near enough that way.
fn turn(yaw: &mut f32, want: f32, step: f32) -> bool {
    use core::f32::consts::{PI, TAU};
    let off = (want - *yaw + PI).rem_euclid(TAU) - PI;
    *yaw += off.clamp(-step, step);
    off.abs() < 0.4
}

impl Game for Combat {
    type Save = Save;
    const CHOICES: &'static [i32] = &[2, LEVELS];

    fn new() -> Self {
        wg::add_glyphs(&glyphs::GLYPHS);
        Combat {
            s: Save::new(),
            fx: Vec::new(),
            scene: scene::Scene::new(),
            cam: [0.0; 6],
            cam_set: false,
            clock: 0.0,
            aspect: 1.0,
            boss_walked: false,
        }
    }
    fn save(&mut self) -> &mut Save {
        &mut self.s
    }
    fn options(&mut self) -> &mut [i32] {
        &mut self.s.opt
    }
    fn plan(&self, w: i32, h: i32) -> (f32, f32) {
        // as wide as the screen, 400 units across
        (400.0, 400.0 * h.max(1) as f32 / w.max(1) as f32)
    }
    fn start(&mut self, seed: u32) {
        self.fx.clear();
        self.hush_shells();
        self.s = Save {
            rng: Rng::new(seed),
            opt: self.s.opt,
            ..Save::new()
        };
        self.s.level = if self.s.endless() {
            0
        } else {
            self.s.opt[O_LEVEL]
        };
        self.begin_level();
    }
    fn update(&mut self, input: &Input) -> u32 {
        let mut ev = 0;
        let mut left = input.dt;
        while left > 0 {
            let step = left.min(STEP_MS);
            left -= step;
            ev |= self.step(input, step as f32 / 1000.0);
        }
        self.follow(input.dt as f32 / 1000.0);
        ev
    }
    fn pointer(&mut self, p: &Pointer) -> u32 {
        let s = &mut self.s;
        if p.pressed & wg::M_LEFT != 0 {
            s.drag = [1.0, p.x, s.tx];
        }
        if p.held & wg::M_LEFT == 0 || !p.inside {
            s.drag[0] = 0.0;
        }
        if s.drag[0] > 0.0 {
            // a drag across the screen is a bit more than the pier
            s.tx = s.drag[2] + (p.x - s.drag[1]) * (2.6 * HALF / 400.0);
        }
        0
    }
    fn picture(&mut self) -> u32 {
        hud::picture(self)
    }
    fn draw(&mut self, s: &mut Screen) {
        hud::draw(self, s);
    }
    fn score(&self) -> u32 {
        self.s.score.max(0) as u32
    }
    fn over(&self) -> bool {
        self.s.phase == P_DEAD || self.s.phase == P_WON
    }
    fn reached(&self) -> i32 {
        if self.s.endless() {
            -1
        } else {
            self.s.level
        }
    }
    fn cleared(&self) -> Option<(i32, u32)> {
        (self.s.cleared >= 0).then(|| (self.s.cleared, self.s.cleared_score.max(0) as u32))
    }
    fn stat(&self, i: usize) -> i32 {
        let s = &self.s;
        match i {
            0 if !s.endless() => s.level + 1,
            1 if s.endless() => s.d as i32,
            2 => s.kills,
            3 => s.bosses,
            4 => s.biggest,
            5 => s.time_ms,
            _ => -1,
        }
    }
    fn loaded(&mut self) -> bool {
        let s = &self.s;
        let ok = s.version == VERSION
            && s.level >= 0
            && (P_RUN..=P_WON).contains(&s.phase)
            && (0..=MAXV as i32).contains(&s.nsol)
            && (0..=MAXE as i32).contains(&s.nen)
            && (0..=MAXO as i32).contains(&s.nthing)
            && (0..=MAXB as i32).contains(&s.nbul)
            && (0..=MAXSH as i32).contains(&s.nshell)
            && (0..=4).contains(&s.weapon)
            && (0..=MAX_COUNT).contains(&s.count)
            && s.things().iter().all(|t| {
                (t.kind != K_GATE || (0..s.nthing).contains(&t.b))
                    && (t.kind != K_CRATE || (0..=4).contains(&t.value))
            })
            && s.enemies().iter().all(|e| {
                (E_GRUNT..=E_BRUTE).contains(&e.kind) && (-1..s.nthing).contains(&e.group)
            })
            && (0..BODIES.len() as i32).contains(&s.boss.kind)
            && s.d.is_finite()
            && s.cx.is_finite();
        if ok {
            // a gate from before gates counted damage, with every hit so far
            // in c: GATE_DMG a point, up to about twice what it shows
            let n = self.s.nthing as usize;
            for t in &mut self.s.thing[..n] {
                if t.kind == K_GATE && t.hp <= 0 {
                    (t.hp, t.hp_max, t.c) = (GATE_DMG, t.value.max(5) * 2, t.c % GATE_DMG);
                }
            }
        }
        self.fx.clear();
        self.cam_set = false;
        ok
    }
}

wg::game!(Combat);

#[cfg(test)]
mod bot;
