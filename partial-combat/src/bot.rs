//! Bots that play every level and the endless pier, for balancing:
//!   cargo test --release -- --nocapture
//! A sharp player picks the better gate, shoots barrels and crates open and
//! keeps out of shell fire; a good one now and then misses a gate
//! or a shell; a casual one gets gates wrong more often, doesn't steer at
//! barrels and crates and dodges half the shells. The asserts hold the
//! balance aimed for: every level can be lost, a sharp player clears each
//! nearly always, a good one most, a casual one most of the early ones and
//! under half of the last.
use crate::*;

/// one step of the rules with nothing pressed
const STEP: Input = Input {
    held: 0,
    pressed: 0,
    dt: 20,
};

struct Skill {
    name: &'static str,
    /// how often the better gate is picked
    gates: f32,
    /// steers at barrels and crates so the squad's fire turns on them
    aim: bool,
    /// how often a shell is seen in time
    dodge: f32,
    /// ms between looks at the pier
    look: i32,
}

const SHARP: Skill = Skill {
    name: "sharp",
    gates: 1.0,
    aim: true,
    dodge: 1.0,
    look: 120,
};
/// the endless runs, each its own luck
const SEEDS: [u32; 3] = [1, 2, 5];
/// the runs at each level: enough that how often it is won isn't left to chance
const LEVEL_SEEDS: [u32; 24] = [
    1, 2, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71, 73, 79, 83, 89,
];

/// one who plays well but not flawlessly
const GOOD: Skill = Skill {
    name: "good",
    gates: 0.95,
    aim: true,
    dodge: 0.7,
    look: 180,
};

const CASUAL: Skill = Skill {
    name: "casual",
    gates: 0.8,
    aim: false,
    dodge: 0.5,
    look: 250,
};

#[derive(Default, Debug)]
struct Run {
    won: bool,
    /// metres reached, and soldiers at the boss and at the end
    d: f32,
    at_boss: i32,
    peak: i32,
    end: i32,
    boss_s: f32,
    time_s: f32,
    score: i32,
    bosses: i32,
    /// what knocked the squad out
    died: &'static str,
}

/// A number from 0 to 1 that stays the same for one thing on the pier.
fn hash01(d: f32, salt: u32) -> f32 {
    let h = (d.to_bits() ^ salt)
        .wrapping_mul(0x9e37_79b9)
        .rotate_left(13)
        .wrapping_mul(0x85eb_ca6b);
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Where to steer, as a player who sees the pier would.
fn aim(p: &Combat, sk: &Skill, salt: u32) -> f32 {
    let s = &p.s;
    let r = radius(s.count.min(MAXV as i32));
    let mut want = s.tx;
    // the next gate pair
    let gate = s
        .things()
        .iter()
        .enumerate()
        .filter(|(_, t)| t.kind == K_GATE && t.state == 0 && t.d > s.d - 0.2)
        .min_by(|a, b| a.1.d.total_cmp(&b.1.d));
    let mut busy = false;
    if let Some((k, g)) = gate {
        if g.d - s.d < 30.0 {
            let o = &s.thing[g.b as usize];
            let (mine, other) = (
                gate_sum(g.a, g.value, s.count),
                if g.b as usize != k {
                    gate_sum(o.a, o.value, s.count)
                } else {
                    i32::MIN
                },
            );
            let best = if mine >= other { g.x } else { o.x };
            let worst = if mine >= other { o.x } else { g.x };
            want = if hash01(g.d, salt) < sk.gates {
                best
            } else {
                worst
            };
            busy = true;
        }
    }
    // a barrel or crate in range and nearer than any gate
    if sk.aim {
        let gd = gate.map_or(f32::MAX, |(_, g)| g.d);
        if let Some(t) = s
            .things()
            .iter()
            .filter(|t| {
                matches!(t.kind, K_BARREL | K_CRATE)
                    && t.hp > 0
                    && t.d > s.d + 1.0
                    && t.d - s.d < WEAPONS[s.weapon as usize].range - 4.0
                    && t.d < gd
            })
            .min_by(|a, b| a.d.total_cmp(&b.d))
        {
            // close, it steps the squad round it
            let clear = half_size(t.kind) + radius(s.count) + 0.4;
            want = if t.d - s.d < clear + 1.0 {
                t.x + clear * if t.x > 0.0 { -1.0 } else { 1.0 }
            } else {
                t.x
            };
            busy = true;
        }
    }
    // TNT: shot from afar, given a wide berth near
    if let Some(t) = s
        .things()
        .iter()
        .filter(|t| t.kind == K_TNT && t.hp > 0 && t.d > s.d - 2.0 && t.d - s.d < 20.0)
        .min_by(|a, b| a.d.total_cmp(&b.d))
    {
        if t.d - s.d > 9.0 && sk.aim {
            want = t.x;
        } else if (want - t.x).abs() < r + 1.0 {
            let side = if t.x > 0.0 { -1.0 } else { 1.0 };
            want = t.x + side * (r + 1.5);
        }
        busy = true;
    }
    if !busy {
        want = s
            .enemies()
            .iter()
            .find(|e| e.state == ES_WAIT && e.d - s.d < 25.0)
            .map_or(0.0, |e| e.x.clamp(-4.0, 4.0));
    }
    want = want.clamp(-HALF + 0.8, HALF - 0.8);
    // shells about to land: where they catch the fewest
    let hit = |x: f32| {
        s.shell[..s.nshell as usize]
            .iter()
            .filter(|h| h.t_max - h.t < 1.4)
            .map(|h| (h.r + r - (x - h.x).hypot(s.d - h.d)).max(0.0))
            .sum::<f32>()
    };
    // one look per threat: the first shell
    let seen = s.nshell > 0 && hash01(s.shell[0].x, salt ^ 7) < sk.dodge;
    if seen && hit(want) > 0.0 {
        let score = |x: f32| hit(x) * 10.0 + (x - s.cx).abs();
        // weighing places across the pier a quarter metre apart
        if let Some((x, _)) = (-36..=36)
            .map(|i| i as f32 * 0.25)
            .map(|x| (x, score(x)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
        {
            want = x;
        }
    }
    want
}

/// The boss marching on the squad and nearly there.
fn near_boss(s: &Save) -> bool {
    s.boss.state == 2 && s.boss.d - s.d < 15.0
}

fn play(level: i32, endless: bool, sk: &Skill, seed: u32, secs: f32) -> Run {
    let mut p = Combat::new();
    p.s.opt = [endless as i32, level];
    p.start(seed);
    let mut run = Run::default();
    let mut t = 0;
    let mut boss_t = 0;
    let mut cause = "";
    while (t as f32) < secs * 1000.0 {
        if t % sk.look < 20 {
            p.s.tx = aim(&p, sk, seed);
        }
        let (before, boss) = (p.s.count, near_boss(&p.s));
        let ev = p.update(&STEP);
        t += 20;
        if p.s.count < before {
            // what was near when soldiers were lost
            let s = &p.s;
            cause = if boss {
                "boss"
            } else if s
                .enemies()
                .iter()
                .any(|e| e.state == ES_CHARGE && (e.d - s.d).abs() < 3.0)
            {
                "enemies"
            } else {
                "other"
            };
        }
        // what the page would draw, and the save it would keep, hold up
        if t % 500 == 0 {
            p.build(1280, 720);
            let mut kept = Combat::new();
            kept.s = p.s;
            assert!(kept.loaded(), "a save made in play fails loaded()");
        }
        run.peak = run.peak.max(p.s.count);
        if near_boss(&p.s) {
            if boss_t == 0 {
                run.at_boss = p.s.count;
            }
            boss_t += 20;
        }
        if ev & wg::EV_LEVEL != 0 {
            run.boss_s = boss_t as f32 / 1000.0;
            boss_t = 0;
            if !endless {
                run.won = true;
                break;
            }
        }
        if ev & wg::EV_GAMEOVER != 0 {
            run.died = cause;
            break;
        }
    }
    run.d = p.s.d;
    run.end = p.s.count;
    run.time_s = t as f32 / 1000.0;
    run.score = p.s.score;
    run.bosses = p.s.bosses;
    run
}

#[test]
fn levels() {
    let mut cleared = [[0; LEVELS as usize]; 3];
    for (n, sk) in [SHARP, GOOD, CASUAL].iter().enumerate() {
        println!("{}", sk.name);
        for level in 0..LEVELS {
            // each run its own game, so a level's runs play side by side
            let runs = std::thread::scope(|sc| {
                LEVEL_SEEDS
                    .map(|seed| sc.spawn(move || play(level, false, sk, seed, 400.0)))
                    .map(|h| h.join().unwrap())
            });
            let won: Vec<_> = runs.iter().filter(|r| r.won).collect();
            let lost: Vec<_> = runs
                .iter()
                .filter(|r| !r.won)
                .map(|r| format!("{:.0} m ({})", r.d, r.died))
                .collect();
            cleared[n][level as usize] = won.len() as i32;
            let per = won.len().max(1) as i32;
            println!(
                "  level {:2}: won {:2} of {}, {:4} at the boss, {:4.1}s on it, {:4} left{}",
                level + 1,
                won.len(),
                runs.len(),
                won.iter().map(|r| r.at_boss).sum::<i32>() / per,
                won.iter().map(|r| r.boss_s).sum::<f32>() / per as f32,
                won.iter().map(|r| r.end).sum::<i32>() / per,
                if lost.is_empty() {
                    String::new()
                } else {
                    format!("; lost at {}", lost.join(", "))
                },
            );
        }
    }
    // a sharp player clears every level nearly always (5 runs in 6); a good
    // one 4 runs in 5 or more, but not 97 in 100; a casual one most of the
    // first five (11 in 15), loses level 1 one run in 8 or more, and clears
    // at most half of the last five
    let runs = LEVEL_SEEDS.len() as i32;
    let sum = |c: &[i32]| c.iter().sum::<i32>();
    assert!(
        cleared[0].iter().all(|&c| c * 6 >= runs * 5),
        "sharp: {:?}",
        cleared[0]
    );
    let good = sum(&cleared[1]) * 100;
    assert!(
        good >= runs * LEVELS * 80 && good <= runs * LEVELS * 97,
        "good: {:?}",
        cleared[1]
    );
    assert!(
        sum(&cleared[2][..5]) * 15 >= runs * 5 * 11 && cleared[2][0] * 8 <= runs * 7,
        "casual: {:?}",
        cleared[2]
    );
    assert!(
        sum(&cleared[2][15..]) * 2 <= runs * 5,
        "casual: {:?}",
        cleared[2]
    );
}

#[test]
fn endless() {
    for sk in [SHARP, CASUAL] {
        let mut line = format!("{}:", sk.name);
        let (mut far, mut ended) = (vec![], vec![]);
        for seed in SEEDS {
            let r = play(0, true, &sk, seed, 3600.0);
            ended.push(r.time_s < 3600.0);
            line += &format!(
                "  {:5.0} m {} bosses ({}, peak {})",
                r.d, r.bosses, r.died, r.peak
            );
            far.push(r.d);
        }
        println!("{line}");
        // it ends: no run lasts an hour (a flawless one can last most of it), and a sharp player gets past a few bosses
        assert!(ended.iter().all(|&e| e), "{line}");
        if sk.name == "sharp" {
            assert!(far.iter().all(|&d| d > 1000.0), "{line}");
        }
    }
}

/// One run told as it goes: BOT_LEVEL=4 BOT_SEED=1 cargo test --release trace -- --nocapture --ignored
#[test]
#[ignore]
fn trace() {
    let level: i32 = std::env::var("BOT_LEVEL").map_or(1, |v| v.parse().unwrap()) - 1;
    let seed: u32 = std::env::var("BOT_SEED").map_or(1, |v| v.parse().unwrap());
    let endless = level < 0;
    let sk = match std::env::var("BOT_SKILL").as_deref() {
        Ok("casual") => CASUAL,
        Ok("good") => GOOD,
        _ => SHARP,
    };
    let mut p = Combat::new();
    p.s.opt = [endless as i32, level.max(0)];
    p.start(seed);
    for t in 0..75_000 {
        if (t * 20) % sk.look < 20 {
            p.s.tx = aim(&p, &sk, seed);
        }
        let before = (p.s.count, p.s.phase);
        let ev = p.update(&STEP);
        let s = &p.s;
        if t % 25 == 0 || s.count < before.0 - 3 || s.phase != before.1 {
            let near: Vec<String> = s
                .things()
                .iter()
                .filter(|t| t.d - s.d > -3.0 && t.d - s.d < 20.0)
                .map(|t| {
                    format!(
                        "{}@{:.0}x{:.1}v{}h{}",
                        t.kind,
                        t.d - s.d,
                        t.x,
                        t.value,
                        t.hp
                    )
                })
                .collect();
            let en = s.enemies().iter().filter(|e| e.up()).count();
            println!("{:5.1}s d {:5.1} ph {} n {:3} w {} cx {:5.2} tx {:5.2} en {:2} shells {} boss {}/{} plan {:.0}/{} fire {:.2} | {}", t as f32 * 0.02, s.d, s.phase, s.count, s.weapon, s.cx, s.tx, en, s.nshell, s.boss.hp, s.boss.hp_max, s.expect, s.expect_gun, s.fire() / s.planned_fire(s.d), near.join(" "));
        }
        if ev & wg::EV_GAMEOVER != 0 || (ev & wg::EV_LEVEL != 0 && !endless) {
            println!("ev {ev}");
            break;
        }
    }
}

/// Damage a second a squad of n with gun puts into a barrel or crate across
/// its way, as a share of squad_dps().
fn aimed(n: i32, gun: i32, kind: i32) -> f32 {
    const HP: i32 = 1_000_000;
    let mut p = Combat::new();
    p.start(1);
    p.s.count = n;
    p.s.weapon = gun;
    p.fill_squad();
    p.s.nen = 0;
    p.s.nthing = 1;
    // near the squad's edge, so only its fire turned that way reaches it,
    // and the squad standing so nobody runs into it
    p.s.thing[0] = Thing {
        kind,
        x: p.s.cx + radius(n) * 0.8,
        d: p.s.d + 10.0,
        hp: HP,
        hp_max: HP,
        ..NO_THING
    };
    p.s.speed = 0.0;
    // two seconds, the first bullets' flight in them
    for _ in 0..100 {
        p.update(&STEP);
    }
    (HP - p.s.thing[0].hp) as f32 / (2.0 * squad_dps(n as f32, gun))
}

/// However big the squad and whatever its gun, a barrel or crate in its way
/// takes about all of its fire.
#[test]
fn aimed_fire() {
    for kind in [K_BARREL, K_CRATE] {
        for gun in 0..WEAPONS.len() as i32 {
            for n in [10, 30, 100, 200, 1000] {
                let r = aimed(n, gun, kind);
                assert!(
                    (0.75..1.2).contains(&r),
                    "{n} soldiers with gun {gun} put {r:.2} of the damage planned into a {}",
                    if kind == K_CRATE { "crate" } else { "barrel" }
                );
            }
        }
    }
}

/// Each gun a crate hands out beats the one before at a barrel, crate, gate
/// or boss, and at a crowd of grunts.
#[test]
fn guns_climb() {
    let crowd = |w: usize| {
        let g = &WEAPONS[w];
        g.rate * (g.pellets * (g.pierce + 1)) as f32
    };
    for w in 1..WEAPONS.len() {
        assert!(gun_dps(w as i32) >= gun_dps(w as i32 - 1) * 1.5, "gun {w}");
        assert!(crowd(w) > crowd(w - 1), "gun {w}");
    }
}
