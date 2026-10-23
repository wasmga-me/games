//! Partial Combat: Clickbait Tactics - an original wasmga.me game. MIT licensed.
//! A squad of green plastic soldiers runs down a pier over the sea, firing
//! ahead on its own. Steer it through the gate that grows it most, shoot
//! barrels for more soldiers and crates for better guns, keep out of shell
//! fire, outnumber the tan army, and beat the boss marching in among the last
//! of it. Twenty levels, or one endless pier.
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
/// 4: lanes and holes laid in 5 m tiles, bigger crates;
/// 5: heavies and grenadiers, each tan soldier with its own hp_max and size;
/// 6: no holes or arenas, nobody falling;
/// 7: armies of hundreds, and a boss that marches in with its own rather than stopping the squad
const VERSION: u32 = 7;

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
pub const MAXE: usize = 400;
pub const MAXO: usize = 48;
pub const MAXB: usize = 192;
pub const MAXSH: usize = 16;
/// the most a squad can number, only so its sums stay in range
pub const MAX_COUNT: i32 = 9_999_999;
/// barrels' and crates' sizes
pub const BARREL: f32 = 1.0;
pub const CRATE: f32 = 2.6;
/// how far from a ship's middle, towards the pier, its shells leave it
const SHIP_GUN: f32 = 1.7 * scene::SHIP_SCALE;
/// soldiers' size: the mannequin is about 1.83 m tall
pub const FIG: f32 = 0.95;
/// how high a soldier's gun fires from
const MUZZLE: f32 = DECK + 0.87 * FIG;
/// room each soldier takes in the squad
const SPACING: f32 = 0.44;
/// how far aside a soldier turns its fire, for each metre ahead
const TURN: f32 = 1.5;
/// bullets of the squad's in the air at most; past that each carries the
/// damage of more than one shot
const VOLLEY: f32 = MAXB as f32 * 0.75;
/// what fired a bullet besides the squad's guns (WEAPONS): a tan rifleman
pub const G_RIFLE: i32 = WEAPONS.len() as i32;
/// how near an enemy has to be to fight hand to hand
const REACH: f32 = 0.8 * FIG;
/// the longest step the rules take, so nothing jumps through a gate
const STEP_MS: i32 = 20;

// phases
pub const P_RUN: i32 = 0;
pub const P_CLEAR: i32 = 1;
pub const P_DEAD: i32 = 2;
pub const P_WON: i32 = 3;
/// chunks laid ahead whose plan is kept: more than fit in AHEAD (track.rs)
pub const PLANS: usize = 8;
/// how long a cleared level shows before the next, ms
const CLEAR_MS: i32 = 3200;

// soldiers; each keeps its place in the squad, slot(its index)
pub const S_OK: i32 = 0;
pub const S_DOWN: i32 = 1;
/// seconds a knocked out soldier's place stays empty before one from further out steps in
const STEP_UP: f32 = 0.35;

// enemies
pub const E_GRUNT: i32 = 0;
pub const E_RUNNER: i32 = 1;
pub const E_RIFLE: i32 = 2;
pub const E_BRUTE: i32 = 3;
pub const E_HEAVY: i32 = 4;
pub const E_GRENADIER: i32 = 5;
pub const ES_WAIT: i32 = 0;
pub const ES_CHARGE: i32 = 1;
pub const ES_DOWN: i32 = 2;
/// thinned out for a squad weaker than planned: it fades away where it stands
pub const ES_GONE: i32 = 3;
/// how near the squad a tan soldier is sized to its fire (Enemy.meant), and
/// how long one thinned out takes to fade (s)
const THIN: f32 = 55.0;
pub const FADE: f32 = 0.6;
/// how near the squad riflemen and grenadiers start firing
pub const STAND_RANGE: f32 = 26.0;
/// how near the squad the rest of the tan army, and its boss, march on it
const MARCH: f32 = 34.0;

// things on the pier
pub const K_NONE: i32 = 0;
pub const K_GATE: i32 = 1;
pub const K_BARREL: i32 = 2;
pub const K_TNT: i32 = 3;
pub const K_CRATE: i32 = 4;
pub const K_SHIP: i32 = 6;

// gate sums
pub const OP_ADD: i32 = 0;
pub const OP_SUB: i32 = 1;
pub const OP_MUL: i32 = 2;
pub const OP_DIV: i32 = 3;
/// how far ahead of the squad TNT has to be for its fire to turn on it, so it doesn't go off in its face
const TNT_NEAR: f32 = 8.0;

// banners over the scene
pub const B_NONE: i32 = 0;
pub const B_LEVEL: i32 = 1;
pub const B_CLEAR: i32 = 2;
pub const B_GUN: i32 = 3;

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
    pub hp_max: i32,
    pub t: f32,
    /// till its next shot, or its next blow in a fight
    pub cool: f32,
    pub flash: f32,
    pub yaw: f32,
    /// how big it stands, against a grunt: its kind's size, a little more the tougher it is
    pub size: f32,
    /// the squad's fire (damage a second) the plan has THIN short of it; a
    /// squad that comes there firing less thins out its army to match, and
    /// this is then 0
    pub meant: f32,
    /// how far it has walked: its walk plays by it
    pub walk: f32,
}

impl Enemy {
    /// Still in the fight: waiting or charging.
    pub fn up(&self) -> bool {
        self.state == ES_WAIT || self.state == ES_CHARGE
    }
    /// What a blow takes off it: all of it, or a share for each soldier it is worth.
    pub fn blow(&self) -> i32 {
        blow(self.hp_max, worth(self.kind))
    }
}

/// A thing on the pier. Gates: value, op in a, the other gate of the pair
/// in b, the damage a point in hp, points left to count in hp_max, damage
/// towards the next in c, state 1 taken and 2 passed by. Barrels and
/// crates: hp of hp_max, soldiers or the gun inside in value. Ships: time to
/// the next shell in cool.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Thing {
    pub kind: i32,
    pub state: i32,
    pub x: f32,
    pub d: f32,
    pub w: f32,
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

/// A round in the air, at gun height (MUZZLE).
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Bullet {
    pub x: f32,
    pub d: f32,
    pub vx: f32,
    pub vd: f32,
    /// seconds left
    pub life: f32,
    pub dmg: i32,
    pub pierce: i32,
    /// the gun that fired it: one of the squad's (WEAPONS), or G_RIFLE (that knocks out a soldier)
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
    /// SH_*; only a cannonball hurts the tan army too
    pub kind: i32,
}
/// a ship's cannonball, a grenadier's grenade
pub const SH_BALL: i32 = 0;
pub const SH_GRENADE: i32 = 1;
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

/// how far round where a boss stamps soldiers are knocked out
const STAMP_R: f32 = 3.4;
/// how many times the fire it was sized for a squad meeting a boss can have and still find it as tough
const BOSS_UP: f32 = 4.0;

/// A boss's body: its model (scene.rs MODELS) and tint; as its clips have
/// it, the walk's loop (s), how far a foot goes back a second at the clip's
/// pace (m at scale 1), and when in its stamp (the jump clip) the feet come
/// down and how long it lasts (s); then the size it is drawn at, and at scale 1
/// how tall it stands and how far its sides and each foot are from its
/// middle. Last, the gun it holds (scene.rs WEAPONS; -1, its own built in)
/// and whether it is armoured: rounds ping off a mech and it goes down in
/// blasts, where a giant chips like its troops and tips over stiff as a toy.
pub struct Body {
    pub model: u16,
    pub tint: u32,
    pub walk: f32,
    pub stride: f32,
    pub land: f32,
    pub leap: f32,
    pub scale: f32,
    pub height: f32,
    pub width: f32,
    pub foot: f32,
    pub gun: i32,
    pub armoured: bool,
}
impl Body {
    /// how far its sides are from its middle, as drawn (m)
    pub fn half(&self) -> f32 {
        self.width * self.scale * 0.5
    }
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
        land: 0.5,
        leap: 0.833,
        scale: BOSS_SCALE,
        height: 6.0,
        width: 2.0,
        foot: 1.0,
        gun: -1,
        armoured: true,
    };
    // Leela, Mike and Stan share their clips
    const MECH: Body = Body {
        walk: 1.033,
        stride: 6.9,
        land: 0.63,
        leap: 1.083,
        height: 5.5,
        width: 2.7,
        ..GEORGE
    };
    const PERSON: Body = Body {
        model: model("giant.pirate"),
        tint: tint(0x4a2c12),
        walk: 1.25,
        stride: 1.85,
        land: 0.63,
        leap: 1.3,
        scale: 3.4,
        height: 3.1,
        width: 0.7,
        foot: 0.26,
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
/// the ping of a round off the boss's armour
const CLANG: u32 = 0xff_70d0ff;
/// blast(): a TNT barrel's, meant for the enemy; a grenade's, which spares the tan army
const BLAST_TNT: u32 = 1;
const BLAST_FOE: u32 = 2;
/// A boss's flash runs from 1 down to this before it can blink again: a tenth of a second lit, then a gap.
const BLINK_GAP: f32 = -3.0;

/// The tan army's biggest: it marches in among a crowd of its soldiers and
/// fights the squad hand to hand, stamping on the front of it.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct Boss {
    /// 0 none, 1 waiting, 2 marching on the squad, 3 going down
    pub state: i32,
    /// which body (BODIES)
    pub kind: i32,
    pub x: f32,
    pub d: f32,
    pub hp: i32,
    pub hp_max: i32,
    /// how many soldiers it knocks out before it falls: each costs it hp_max / worth, as Enemy::blow()
    pub worth: i32,
    /// seconds in its state
    pub t: f32,
    /// till it can stamp again, and how far into a stamp it is (below 0, not stamping)
    pub cool: f32,
    pub stamp: f32,
    /// lit when above 0, see BLINK_GAP
    pub flash: f32,
    /// which way it faces (0 the squad)
    pub yaw: f32,
    /// clip seconds into its walk's loop (walking backwards it runs down)
    pub walk: f32,
    /// a flinch under way (s), and the hp below which it flinches next
    pub hurt: f32,
    pub flinch_at: i32,
    /// the squad's fire (damage a second) its hp was set for
    pub meant: f32,
}
impl Boss {
    pub fn body(&self) -> &'static Body {
        &BODIES[self.kind as usize]
    }
    /// how tall it stands (m)
    pub fn tall(&self) -> f32 {
        let m = self.body();
        m.height * m.scale
    }
    /// How far in front of the squad's front its middle keeps, its body clear of the soldiers.
    pub fn gap(&self) -> f32 {
        self.body().half() + 0.6
    }
    /// Where its front is, which it stamps on and the squad aims at.
    pub fn front(&self) -> f32 {
        self.d - self.gap()
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
    /// where each of the last chunks laid ends (0 for none), and the size
    /// and gun expected past it
    pub plan_at: [[f32; 3]; PLANS],
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
    hp_max: 0,
    t: 0.0,
    cool: 0.0,
    flash: 0.0,
    yaw: 0.0,
    size: 1.0,
    meant: 0.0,
    walk: 0.0,
};
pub const NO_THING: Thing = Thing {
    kind: K_NONE,
    state: 0,
    x: 0.0,
    d: 0.0,
    w: 0.0,
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
    vx: 0.0,
    vd: 0.0,
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
    worth: 1,
    t: 0.0,
    cool: 0.0,
    stamp: -1.0,
    flash: 0.0,
    yaw: 0.0,
    walk: 0.0,
    hurt: 0.0,
    flinch_at: 0,
    meant: 0.0,
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
            plan_at: [[0.0; 3]; PLANS],
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
    pub fn enemies(&self) -> &[Enemy] {
        &self.en[..self.nen as usize]
    }
    pub fn soldiers(&self) -> &[Sol] {
        &self.sol[..self.nsol as usize]
    }
    fn frand(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.rng.float()
    }
    /// Damage a second the squad puts into what it aims at, as it is now.
    pub fn fire(&self) -> f32 {
        squad_dps(self.count as f32, self.weapon)
    }
    /// The squad a level or the endless pier starts with.
    pub fn start_count(&self) -> i32 {
        if self.endless() {
            5
        } else {
            3
        }
    }
    /// The plan for the last chunk that ends by d, if any.
    pub fn plan_by(&self, d: f32) -> Option<[f32; 3]> {
        self.plan_at
            .iter()
            .filter(|p| p[0] > 0.0 && p[0] <= d)
            .max_by(|a, b| a[0].total_cmp(&b[0]))
            .copied()
    }
    /// The squad's fire the layout plans for at d: as it sets out, till
    /// the first chunk ends.
    pub fn planned_fire(&self, d: f32) -> f32 {
        self.plan_by(d)
            .map_or(squad_dps(self.start_count() as f32, 0), |p| {
                squad_dps(p[1].max(1.0), p[2] as i32)
            })
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
    /// How fast the squad goes on (m/s): its speed, easing to a stop as a boss goes down in front of it.
    pub fn pace(&self) -> f32 {
        if self.boss.state == 3 {
            self.speed * (1.0 - self.boss.t / 0.6).max(0.0)
        } else {
            self.speed
        }
    }
    /// How many soldiers are still standing.
    pub fn alive(&self) -> usize {
        self.soldiers().iter().filter(|q| q.state == S_OK).count()
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
        E_HEAVY => 3 + (hard / 4.0) as i32,
        E_RIFLE | E_GRENADIER => 2 + (hard / 6.0) as i32,
        _ => 1 + (hard / 10.0) as i32,
    }
}

/// How fast a tan soldier of kind marches on the squad where the pier is that hard (m/s); the boss keeps a grunt's pace.
pub fn march_speed(kind: i32, hard: f32) -> f32 {
    let base = match kind {
        E_RUNNER => 6.5,
        E_BRUTE => 2.6,
        E_HEAVY => 3.0,
        _ => 3.6,
    };
    base + (hard * 0.05).min(1.0)
}

/// How big a tan soldier of kind stands, against a grunt: a runner small, a heavy big, a brute bigger.
pub fn kind_size(kind: i32) -> f32 {
    match kind {
        E_RUNNER => 0.82,
        E_HEAVY => 1.3,
        E_BRUTE => 1.65,
        _ => 1.0,
    }
}

/// How much bigger than its kind a tan soldier tough times its kind's hp stands.
fn bulk(tough: f32) -> f32 {
    (1.0 + tough.log2() * 0.06).clamp(1.0, 1.3)
}

/// Whether a tan soldier of kind stands back and fires rather than charging at once.
fn stands(kind: i32) -> bool {
    kind == E_RIFLE || kind == E_GRENADIER
}

/// Of foes at (x, d), the nearest one a soldier at x, d turns to fire at.
fn nearest(foes: &[(f32, f32)], x: f32, d: f32) -> Option<(f32, f32)> {
    let far = |p: &(f32, f32)| (p.0 - x).powi(2) + (p.1 - d).powi(2);
    foes.iter()
        .filter(|f| f.1 > d + 1.0 && (f.0 - x).abs() <= TURN * (f.1 - d))
        .min_by(|a, b| far(a).total_cmp(&far(b)))
        .copied()
}

/// What each soldier it knocks out costs a foe of hp_max that knocks out
/// worth of them before it falls: an even share, rounded up.
fn blow(hp_max: i32, worth: i32) -> i32 {
    (hp_max + worth - 1) / worth.max(1)
}

/// How many soldiers a tan soldier of kind knocks out hand to hand before it falls.
pub fn worth(kind: i32) -> i32 {
    match kind {
        E_BRUTE => 6,
        E_HEAVY => 3,
        _ => 1,
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
    /// the camera (eye, then what it looks at), eased after the squad; along
    /// the pier as far ahead of it (z + d), as it keeps up with the run
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
        s.count = s.start_count();
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
        s.plan_at = [[0.0; 3]; PLANS];
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

    /// Shows as many soldiers as the squad has, up to MAXV: new ones join at the edge.
    fn fill_squad(&mut self) {
        let s = &mut self.s;
        let want = shooters(s.count as f32) as usize;
        let mut alive = s.alive();
        while alive < want && (s.nsol as usize) < MAXV {
            let i = s.nsol as usize;
            s.nsol += 1;
            alive += 1;
            let (ox, oz) = slot(i);
            let t = s.rng.float() * 3.0;
            s.sol[i] = Sol {
                x: s.cx + ox * 0.4,
                d: s.d + oz * 0.4 - 0.6,
                state: S_OK,
                t,
                flash: 0.6,
            };
        }
        // fewer: the last ones go
        let mut i = s.nsol as usize;
        while alive > want && i > 0 {
            i -= 1;
            if s.sol[i].state == S_OK {
                s.sol[i].state = S_DOWN;
                s.sol[i].t = 0.0;
                alive -= 1;
            }
        }
        s.biggest = s.biggest.max(s.count);
    }

    /// A soldier is knocked out: the squad is that much smaller.
    fn lose(&mut self, i: usize) {
        let s = &mut self.s;
        if s.sol[i].state != S_OK {
            return;
        }
        let alive = s.alive().max(1) as i32;
        // each one shown stands for a share of a big squad
        let share = ((s.count + alive - 1) / alive).max(1);
        s.count = (s.count - share).max(0);
        let q = &mut s.sol[i];
        q.state = S_DOWN;
        q.t = 0.0;
        let (x, d) = (q.x, q.d);
        self.fx(scene::FX_PUFF, x, DECK, d, 0.0, 0.0, 0.0, scene::GREEN);
        // a big squad's reserves step in
        self.fill_squad();
    }

    fn enemy_down(&mut self, k: usize) {
        let e = &mut self.s.en[k];
        if !e.up() {
            return;
        }
        e.state = ES_DOWN;
        e.t = 0.0;
        let (x, d, kind) = (e.x, e.d, e.kind);
        let points = [10, 15, 15, 40, 25, 20][kind as usize];
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
                self.s.boss.t += dt;
                if self.s.phase_ms > 600 && self.s.phase_ms - (dt * 1000.0) as i32 <= 600 {
                    let (x, d) = (self.s.cx, self.s.d);
                    self.fx(scene::FX_CONFETTI, x, DECK, d - 2.0, 0.0, 0.0, 0.0, 0);
                }
                if self.s.phase_ms >= CLEAR_MS {
                    if self.s.endless() {
                        // on down the pier
                        self.s.phase = P_RUN;
                        self.s.phase_ms = 0;
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
        self.s.d += self.s.pace() * dt;
        self.lay_track();
        self.move_soldiers(dt);
        self.fire(dt);
        self.move_bullets(dt);
        self.move_enemies(dt);
        self.pass_things(dt);
        self.move_shells(dt);
        ev |= self.boss_step(dt);
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

    /// Soldiers keep their places in the squad, squeezed to fit the pier.
    fn move_soldiers(&mut self, dt: f32) {
        let ease = 1.0 - (-9.0 * dt).exp();
        self.step_up();
        let n = self.s.nsol as usize;
        // as wide as the places held, squeezed in to fit the deck
        let held = self.s.soldiers().iter().rposition(|q| q.state == S_OK);
        let r = radius(held.map_or(0, |i| i as i32 + 1));
        const EDGE: f32 = 0.3;
        let spread = (r - 0.3).max(0.01);
        let room = HALF - EDGE;
        // steered against an edge it squeezes up, to half its width
        let off = room - self.s.cx.abs();
        let squeeze = (room / spread).min((off / spread).max(0.5)).min(1.0);
        let used = (spread * squeeze).min(room);
        let mid = self.s.cx.clamp(used - room, room - used);
        let cheer = self.s.phase == P_CLEAR || self.s.phase == P_WON;
        for i in 0..n {
            let q = &mut self.s.sol[i];
            q.t += dt;
            q.flash = (q.flash - dt * 3.0).max(0.0);
            if q.state == S_OK {
                let (ox, oz) = slot(i);
                let gx = if cheer { q.x } else { mid + ox * squeeze };
                let gd = self.s.d + oz;
                q.x += (gx - q.x) * ease;
                q.d += (gd - q.d) * ease;
            }
        }
        // the gone are dropped: step_up has moved them out past everyone
        // standing by then, so nobody's place changes
        let mut w = 0;
        for i in 0..self.s.nsol as usize {
            let q = self.s.sol[i];
            if !(q.state == S_DOWN && q.t > 1.6) {
                self.s.sol[w] = q;
                w += 1;
            }
        }
        if w != self.s.nsol as usize {
            self.s.nsol = w as i32;
            // a big squad's reserves take the places freed
            self.fill_squad();
        }
    }

    /// A moment after a soldier is knocked out, the one nearest its place from
    /// further out steps into it, and one into that one's, out to the edge: so
    /// the gap shows where the hit was and the rest of the squad stays put.
    fn step_up(&mut self) {
        let s = &mut self.s;
        let n = s.nsol as usize;
        let open = |q: &Sol| q.state != S_OK && q.t >= STEP_UP;
        // only a gap with someone standing further out is filled
        let Some(last) = s.sol[..n].iter().rposition(|q| q.state == S_OK) else {
            return;
        };
        if !s.sol[..last].iter().any(open) {
            return;
        }
        let mut at = [(0.0, 0.0); MAXV];
        for (i, a) in at[..n].iter_mut().enumerate() {
            *a = slot(i);
        }
        for f in 0..last {
            if !open(&s.sol[f]) {
                continue;
            }
            let (fx, fz) = at[f];
            let far = |m: usize| (at[m].0 - fx).powi(2) + (at[m].1 - fz).powi(2);
            let near = (f + 1..n)
                .filter(|&m| s.sol[m].state == S_OK)
                .min_by(|&a, &b| far(a).total_cmp(&far(b)));
            if let Some(m) = near {
                s.sol.swap(f, m);
            }
        }
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
        // the tan army and its boss in range first, each shot at the one
        // nearest the soldier firing it; else the nearest barrel, crate or TNT
        // across the squad's way, so however wide the squad has spread, all
        // of it shoots at what it is running into
        let sd = self.s.d;
        let reach = sd + g.range * 0.8;
        let mut foes = [(0.0, 0.0); MAXE + 1];
        let mut nfoe = 0;
        let b = &self.s.boss;
        let boss = (b.state == 2).then(|| (b.x, b.d));
        for (x, d) in self
            .s
            .enemies()
            .iter()
            .filter(|e| e.up())
            .map(|e| (e.x, e.d))
            .chain(boss)
        {
            if d > sd && d < reach {
                foes[nfoe] = (x, d);
                nfoe += 1;
            }
        }
        let foes = &foes[..nfoe];
        let aim = if !foes.is_empty() {
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
            // the muzzle, at the soldier's right hand
            let (mx, md) = (q.x - 0.2 * FIG, q.d + 0.6 * FIG);
            let target = if foes.is_empty() {
                aim
            } else {
                nearest(foes, mx, md)
            };
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
                if let Some((ax, ad)) = target {
                    // turned towards what is ahead of it, as far as a soldier turns
                    let ahead = ad - md;
                    if ahead > 1.0 {
                        vx += ((ax - mx) / ahead).clamp(-TURN, TURN) * vd;
                    }
                }
                let b = Bullet {
                    x: mx,
                    d: md,
                    vx,
                    vd,
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
                mx,
                MUZZLE,
                md + 0.05 * FIG,
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
            b.life -= dt;
            let mut spent = b.life <= 0.0;
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
            if !e.up() {
                continue;
            }
            let r = 0.63 * FIG * e.size;
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
            self.fx(scene::FX_HIT, ex, DECK + FIG, ed, 0.0, 0.0, 0.0, scene::TAN);
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
                    // a shot's damage, not all the bullet carries
                    WEAPONS[b.gun as usize].dmg as f32,
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

    /// An enemy bullet: knocks out the first soldier it reaches.
    fn hit_squad(&mut self, b: &Bullet) -> bool {
        let n = self.s.nsol as usize;
        let hit = (0..n).find(|&i| {
            let q = self.s.sol[i];
            q.state == S_OK && (q.x - b.x).abs() < 0.58 * FIG && (q.d - b.d).abs() < 0.83 * FIG
        });
        let Some(i) = hit else {
            return false;
        };
        self.lose(i);
        true
    }

    fn hit_thing(&mut self, k: usize, dmg: i32) {
        let t = &mut self.s.thing[k];
        if t.flash < 0.4 {
            t.flash = 1.0;
        }
        match t.kind {
            K_GATE => {
                // shooting a gate counts it a point for every hp of damage: a
                // minus one down to nought, where it turns plus, and a plus
                // one up, at most hp_max points
                if t.a == OP_ADD || t.a == OP_SUB {
                    t.c += dmg;
                    let mut n = t.c / t.hp;
                    t.c %= t.hp;
                    if t.a == OP_SUB {
                        let down = n.min(t.value);
                        (t.value, n) = (t.value - down, n - down);
                        if t.value == 0 {
                            t.a = OP_ADD;
                        }
                    }
                    if t.a == OP_ADD {
                        let up = n.min(t.hp_max);
                        t.hp_max -= up;
                        t.value = (t.value + up).min(MAX_COUNT);
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
            if how & BLAST_FOE == 0 && e.up() && (e.x - x).hypot(e.d - d) < r {
                // it knocks out a grunt, but a heavy or a brute only takes a blow or so
                self.s.en[k].hp -= e.blow().max(6);
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
                self.lose(i);
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
        let fire = self.s.fire();
        let mut w = 0;
        for k in 0..self.s.nen as usize {
            let mut e = self.s.en[k];
            let (x0, d0) = (e.x, e.d);
            e.t += dt;
            e.flash = (e.flash - dt * 4.0).max(0.0);
            e.cool -= dt;
            match e.state {
                ES_WAIT => {
                    let near = e.d - sd;
                    if e.meant > 0.0 && near < THIN {
                        // a squad firing less than planned where it is isn't
                        // swept away: fewer come at it, a little fewer than its
                        // fire falls short, less so on later levels
                        let keep = (fire / e.meant).min(1.0).powf((1.5 - hard * 0.05).max(0.8));
                        e.meant = 0.0;
                        if self.s.rng.float() >= keep {
                            (e.state, e.t) = (ES_GONE, 0.0);
                        }
                    }
                    if e.state == ES_WAIT && near < if stands(e.kind) { STAND_RANGE } else { MARCH }
                    {
                        if e.kind == E_GRENADIER {
                            // lobs a grenade where the squad will be
                            if e.cool <= 0.0 && near > 4.0 {
                                e.cool = (3.4 - hard * 0.07).max(1.8) + self.s.rng.float();
                                e.t = 0.0;
                                let flight = 1.3;
                                let (x, d) = self.ahead(flight, 1.2, 0.5);
                                self.lob(e.x, DECK + 2.6 * FIG, e.d, x, d, 1.4, flight, SH_GRENADE);
                            }
                            if near < 6.0 {
                                e.state = ES_CHARGE;
                            }
                        } else if e.kind == E_RIFLE {
                            // stands and fires
                            if e.cool <= 0.0 && near > 2.0 && self.s.nbul < MAXB as i32 {
                                e.cool = (2.2 - hard * 0.04).max(1.1) + self.s.rng.float();
                                let dx = cx - e.x;
                                let speed = 18.0;
                                let time = near / speed;
                                self.s.bul[self.s.nbul as usize] = Bullet {
                                    x: e.x,
                                    d: e.d - 0.7 * FIG,
                                    vx: dx / time.max(0.3),
                                    vd: -speed,
                                    life: 2.0,
                                    dmg: 1,
                                    pierce: 0,
                                    gun: G_RIFLE,
                                };
                                self.s.nbul += 1;
                                self.fx(
                                    scene::FX_MUZZLE,
                                    e.x,
                                    MUZZLE,
                                    e.d - 0.8 * FIG,
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
                    let speed = march_speed(e.kind, hard);
                    if len > REACH * 0.8 {
                        e.x += dx / len * speed * dt;
                        e.d += dd / len * speed * dt;
                    }
                    e.x = e.x.clamp(-HALF + 0.3, HALF - 0.3);
                    e.yaw = dx.atan2(-dd);
                }
                _ => {}
            }
            e.walk += (e.x - x0).hypot(e.d - d0);
            let gone = (e.state == ES_DOWN && e.t > 1.8) || (e.state == ES_GONE && e.t > FADE);
            if !gone {
                self.s.en[w] = e;
                w += 1;
            }
        }
        self.s.nen = w as i32;
        // hand to hand: an enemy and a soldier in reach knock each other out;
        // a heavy or a brute takes worth() soldiers, a blow at a time
        for k in 0..self.s.nen as usize {
            let e = self.s.en[k];
            if !e.up() || (e.cool > 0.0 && worth(e.kind) > 1) {
                continue;
            }
            if (e.d - sd).abs() > edge {
                continue;
            }
            for i in 0..self.s.nsol as usize {
                let q = self.s.sol[i];
                if q.state != S_OK || (q.x - e.x).abs() > REACH || (q.d - e.d).abs() > REACH {
                    continue;
                }
                self.lose(i);
                let en = &mut self.s.en[k];
                en.hp -= en.blow();
                en.cool = 0.18;
                if en.hp <= 0 {
                    self.enemy_down(k);
                }
                break;
            }
        }
        // an enemy that falls behind the squad is gone
        for k in 0..self.s.nen as usize {
            let e = self.s.en[k];
            if e.up() && e.d < sd - edge {
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
                    // (all but one of a squad smaller than that) break it
                    // unshot, so it is no gain, and a crate takes more on
                    // later levels
                    let ram = if t.kind == K_BARREL {
                        t.value.min(self.s.alive() as i32 - 1).max(1)
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
                        self.lose(i);
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
                            let (x, d) = self.ahead(flight, 1.5, 1.0);
                            let dir = if t.x < 0.0 { 1.0 } else { -1.0 };
                            let gx = t.x + dir * SHIP_GUN;
                            self.lob(gx, 2.2, t.d, x, d, 1.8, flight, SH_BALL);
                            self.fx(scene::FX_CANNON, gx, 2.2, t.d, dir, 0.0, 0.0, 0);
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

    /// Where the squad will be in `flight` seconds, give or take wide across and long along.
    fn ahead(&mut self, flight: f32, wide: f32, long: f32) -> (f32, f32) {
        let x = (self.s.cx + self.s.frand(-wide, wide)).clamp(-HALF + 0.8, HALF - 0.8);
        let d = self.s.d + self.s.pace() * flight + self.s.frand(-long, long);
        (x, d)
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
                self.blast(
                    sh.x,
                    sh.d,
                    sh.r,
                    if sh.kind == SH_BALL { 0 } else { BLAST_FOE },
                );
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

    /// The boss waits among its crowd and marches on the squad with it; once
    /// at the squad's front it keeps there, pushed back as the squad comes
    /// on, and stamps on the soldiers in front of it now and then.
    fn boss_step(&mut self, dt: f32) -> u32 {
        let (hard, sd) = (self.s.hardness(), self.s.d);
        let b = &mut self.s.boss;
        b.t += dt;
        b.flash = (b.flash - dt * 10.0).max(BLINK_GAP);
        match b.state {
            // the squad near, and past the gates before its army, where
            // lay_boss() left the pier laid to
            1 if b.d - sd < MARCH && sd > self.s.next_d => {
                self.boss_wake();
                return 0;
            }
            2 => {}
            3 => return self.boss_falling(dt),
            _ => return 0,
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
        // it reels as each quarter of its hp goes, unless in the middle of a stamp
        let b = &mut self.s.boss;
        if b.hp < b.flinch_at && b.stamp < 0.0 {
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
        self.s.boss.hurt = (self.s.boss.hurt - dt).max(0.0);
        self.boss_move(dt, hard);
        self.boss_stamp(dt, hard, enraged);
        0
    }

    /// The boss sets off with its army: a squad that comes firing less than
    /// planned meets a weaker boss, but not by as much, and one firing more
    /// a tougher one.
    fn boss_wake(&mut self) {
        let fire = self.s.fire();
        let b = &mut self.s.boss;
        let k = fire / b.meant;
        let k = if k < 1.0 { k.sqrt() } else { k.min(BOSS_UP) };
        b.hp = ((b.hp as f32 * k).round() as i32).max(30);
        (b.hp_max, b.flinch_at) = (b.hp, b.hp * 3 / 4);
        (b.state, b.t) = (2, 0.0);
    }

    /// Once at the squad the boss stamps on the soldiers in front of it as
    /// often as it can, each it knocks out costing it a blow, as a brute's do.
    fn boss_stamp(&mut self, dt: f32, hard: f32, enraged: bool) {
        let b = &mut self.s.boss;
        let m = b.body();
        if b.stamp < 0.0 {
            b.cool -= dt;
            if b.cool <= 0.0 && b.hurt == 0.0 && self.boss_close() {
                self.s.boss.stamp = 0.0;
            }
            return;
        }
        b.stamp += dt;
        if crossed(b.stamp, dt, m.land) {
            let (x, d, cost) = (b.x, b.front(), blow(b.hp_max, b.worth));
            self.fx(scene::FX_STOMP, x, DECK, d, STAMP_R, 0.0, 0.0, 0);
            self.s.shake = self.s.shake.max(0.6);
            for k in 0..self.s.nsol as usize {
                let q = self.s.sol[k];
                if q.state == S_OK && (q.x - x).hypot(q.d - d) < STAMP_R {
                    self.lose(k);
                    self.s.boss.hp -= cost;
                    if self.s.boss.hp <= 0 {
                        break;
                    }
                }
            }
            if self.s.boss.hp <= 0 {
                self.s.boss.hp = 0;
                self.boss_down();
                return;
            }
        }
        let b = &mut self.s.boss;
        if b.stamp > m.leap {
            b.stamp = -1.0;
            b.cool = (0.8 - hard * 0.015).max(0.4) * if enraged { 0.7 } else { 1.0 };
        }
    }

    /// A boss going down, tipping over or in blasts; then the level is won.
    fn boss_falling(&mut self, dt: f32) -> u32 {
        let b = &self.s.boss;
        let (t, x, d, tall) = (b.t, b.x, b.d, b.tall());
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
                    STAMP_R,
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
        0
    }

    /// The front of the squad, where the boss stands to stamp on it.
    fn squad_front(&self) -> f32 {
        self.s.d + radius(self.s.alive() as i32) * 0.9
    }

    /// Whether the boss stands at the squad's front with soldiers under its stamp.
    fn boss_close(&self) -> bool {
        let b = &self.s.boss;
        let (x, d) = (b.x, b.front());
        d <= self.squad_front() + 0.3
            && self
                .s
                .soldiers()
                .iter()
                .any(|q| q.state == S_OK && (q.x - x).hypot(q.d - d) < STAMP_R)
    }

    /// The boss walks at its crowd's pace towards the squad's front and across
    /// to its middle, facing it (not while it reels or stamps), and is pushed
    /// back once there as the squad comes on: its walk plays as far as it
    /// goes, backwards when pushed.
    fn boss_move(&mut self, dt: f32, hard: f32) {
        let (cx, sd, front) = (self.s.cx, self.s.d, self.squad_front());
        let b = &mut self.s.boss;
        let (x0, d0) = (b.x, b.d);
        let at = front + b.gap();
        if b.hurt == 0.0 && b.stamp < 0.0 {
            let step = march_speed(E_GRUNT, hard) * dt;
            b.d -= step;
            b.x += (cx - b.x).clamp(-step, step) * 0.6;
        }
        let side = HALF - b.body().half();
        b.x = b.x.clamp(-side, side);
        b.d = b.d.max(at);
        turn(&mut b.yaw, (cx - b.x).atan2(b.d - sd), 1.8 * dt);
        let along = d0 - b.d;
        let went = along.abs().max((b.x - x0).abs());
        if went < 1e-4 {
            return;
        }
        let m = b.body();
        let before = b.walk;
        let clip = went / (m.stride * m.scale);
        b.walk = (b.walk + if along < 0.0 { -clip } else { clip }).rem_euclid(m.walk);
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
    }

    /// A tan soldier of kind, with its kind's hp (toughen() can raise it).
    fn spawn_enemy(&mut self, kind: i32, x: f32, d: f32) {
        if self.s.nen as usize >= MAXE {
            return;
        }
        let hp = enemy_hp(kind, self.s.hardness_at(d));
        let t = self.s.rng.float() * 2.0;
        let size = kind_size(kind) * self.s.frand(0.93, 1.07);
        self.s.en[self.s.nen as usize] = Enemy {
            kind,
            state: ES_WAIT,
            x,
            d,
            hp,
            hp_max: hp,
            t,
            cool: self.s.rng.float() * 1.5,
            flash: 0.0,
            yaw: 0.0,
            size,
            meant: 0.0,
            walk: t * 4.0,
        };
        self.s.nen += 1;
    }

    /// Makes the tan soldiers from first on tough enough that it takes
    /// `damage` to cut them all down, and a little bigger for it.
    fn toughen(&mut self, first: usize, damage: f32) {
        let army = &mut self.s.en[first..self.s.nen as usize];
        let hp = army.iter().map(|e| e.hp).sum::<i32>().max(1);
        let tough = (damage / hp as f32).max(1.0);
        for e in army {
            e.hp = ((e.hp as f32 * tough).round() as i32).max(1);
            e.hp_max = e.hp;
            e.size *= bulk(tough);
        }
    }

    fn boss_down(&mut self) {
        let b = &mut self.s.boss;
        (b.state, b.t, b.stamp, b.hurt) = (3, 0.0, -1.0, 0.0);
        self.hush_shells();
        // its troops fall with it, worth nothing more
        for k in 0..self.s.nen as usize {
            let e = &mut self.s.en[k];
            if e.up() {
                (e.state, e.t) = (ES_DOWN, 0.0);
                let (x, d) = (e.x, e.d);
                self.fx(scene::FX_PUFF, x, DECK, d, 0.0, 0.0, 0.0, scene::TAN);
            }
        }
        // the enemy's shots in the air fizzle
        for b in &mut self.s.bul[..self.s.nbul as usize] {
            if !b.ours() {
                b.life = 0.0;
            }
        }
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
        for k in 0..self.s.nthing as usize {
            let t = self.s.thing[k];
            // ships stay while their hulls show
            let end = t.d + if t.kind == K_SHIP { 12.0 } else { 0.0 };
            if end > sd - 16.0 {
                self.s.thing[w] = t;
                w += 1;
            }
        }
        if w != self.s.nthing as usize {
            self.s.nthing = w as i32;
            self.relink_gates();
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

/// Eases u (0 to 1) in and out.
fn smooth(u: f32) -> f32 {
    u * u * (3.0 - 2.0 * u)
}

/// True when a clock at t, dt on from the last step, has just passed `at`.
fn crossed(t: f32, dt: f32, at: f32) -> bool {
    t >= at && t - dt < at
}

/// True when a clock at t, dt on from the last step, has just passed one of `rate` marks a second.
fn ticks(t: f32, dt: f32, rate: f32) -> bool {
    (t * rate) as i32 != ((t - dt) * rate) as i32
}

/// Turns `yaw` towards `want`, `step` at most.
fn turn(yaw: &mut f32, want: f32, step: f32) {
    use core::f32::consts::{PI, TAU};
    let off = (want - *yaw + PI).rem_euclid(TAU) - PI;
    *yaw += off.clamp(-step, step);
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
                (t.kind != K_GATE || ((0..s.nthing).contains(&t.b) && t.hp > 0))
                    && (t.kind != K_CRATE || (0..=4).contains(&t.value))
            })
            && s.enemies().iter().all(|e| {
                (E_GRUNT..=E_GRENADIER).contains(&e.kind)
                    && e.hp_max > 0
                    && e.size > 0.0
                    && e.size < 4.0
                    && e.meant.is_finite()
            })
            && (0..BODIES.len() as i32).contains(&s.boss.kind)
            && s.plan_at.iter().flatten().all(|v| v.is_finite())
            && s.boss.meant.is_finite()
            && s.d.is_finite()
            && s.cx.is_finite();
        self.fx.clear();
        self.cam_set = false;
        ok
    }
}

wg::game!(Combat);

#[cfg(test)]
mod bot;
