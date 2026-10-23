//! Bots that play every level and the endless pier, for balancing:
//!   cargo test --release -- --nocapture
//! A sharp player picks the best lane at each row of gates, runs into buffs
//! and keeps out of shell fire; a good one now and then misses a gate
//! or a shell; a casual one gets gates wrong more often, doesn't steer at
//! buffs and dodges half the shells. The asserts hold the
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
    /// steers into buffs
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
    /// metres reached, soldiers on meeting the first mech or giant and at
    /// the end, and seconds spent near them
    d: f32,
    at_big: i32,
    peak: i32,
    end: i32,
    big_s: f32,
    time_s: f32,
    score: i32,
    bigs: i32,
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
    // the middle lane, unless there is a reason to be elsewhere
    let mut want = 0.0;
    // a lane to keep to, with a gate close
    let mut keep = None;
    // the next row of gates: the lane that grows the squad most, its gate
    // or none, or now and then the lane that grows it least
    let gate = s
        .things()
        .iter()
        .filter(|t| t.kind == K_GATE && t.state == 0 && t.d > s.d - 0.2)
        .min_by(|a, b| a.d.total_cmp(&b.d));
    if let Some(g) = gate.filter(|g| g.d - s.d < 30.0) {
        let after = |lane: i32| {
            s.things()
                .iter()
                .find(|t| t.kind == K_GATE && t.state == 0 && t.b == g.b && lane_of(t.x) == lane)
                .map_or(s.count, |t| gate_sum(t.a, t.value, s.count))
        };
        // ties go to the lane nearer the squad
        let by = |l: &i32| (after(*l), -(lane_x(*l) - s.cx).abs().round() as i32);
        let best = (0..3).max_by_key(by).unwrap();
        let worst = (0..3).min_by_key(|l| after(*l)).unwrap();
        let lane = if hash01(g.d, salt) < sk.gates {
            best
        } else {
            worst
        };
        want = lane_x(lane);
        keep = (g.d - s.d < 12.0).then_some(lane);
    }
    // a buff in reach, before any gate
    if sk.aim {
        let gd = gate.map_or(f32::MAX, |g| g.d);
        if let Some(t) = s
            .things()
            .iter()
            .filter(|t| {
                t.kind == K_BUFF && t.state == 0 && t.d > s.d - 1.0 && t.d - s.d < 25.0 && t.d < gd
            })
            .min_by(|a, b| a.d.total_cmp(&b.d))
        {
            want = t.x;
        }
    }
    // shells about to land, and mechs and giants about to stamp: where they catch the fewest
    let stamps = s.big.iter().filter_map(|b| b.landing());
    let hit = |x: f32| {
        s.shell[..s.nshell as usize]
            .iter()
            .filter(|h| h.t_max - h.t < 1.4)
            .map(|h| (h.x, h.d, h.r))
            .chain(stamps.clone().map(|(x, d)| (x, d, STAMP_R)))
            .map(|(hx, hd, hr)| (hr + r - (x - hx).hypot(s.d - hd)).max(0.0))
            .sum::<f32>()
    };
    // one look per threat: the first shell, or where the first stamp comes down
    let threat = if s.nshell > 0 {
        Some(s.shell[0].x)
    } else {
        stamps.clone().next().map(|(x, _)| (x * 4.0).round())
    };
    let seen = threat.is_some_and(|t| hash01(t, salt ^ 7) < sk.dodge);
    if seen && hit(want) > 0.0 {
        let score = |x: f32| hit(x) * 10.0 + (x - s.cx).abs();
        // weighing places across the pier a quarter metre apart, in the
        // lane picked when a gate is close
        if let Some((x, _)) = (-36..=36)
            .map(|i| i as f32 * 0.25)
            .filter(|&x| keep.is_none_or(|l| lane_of(x) == l))
            .map(|x| (x, score(x)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
        {
            want = x;
        }
    }
    want
}

/// The nearest mech or giant up, if any.
fn nearest_big(s: &Save) -> Option<&Big> {
    s.big
        .iter()
        .filter(|b| b.up())
        .min_by(|a, b| a.d.total_cmp(&b.d))
}

/// A mech or giant marching on the squad and nearly there.
fn near_big(s: &Save) -> bool {
    nearest_big(s).is_some_and(|b| b.d - s.d < 15.0)
}

fn play(level: i32, endless: bool, sk: &Skill, seed: u32, secs: f32) -> Run {
    let mut p = Combat::new();
    p.s.opt = [endless as i32, level];
    p.start(seed);
    let mut run = Run::default();
    let mut t = 0;
    let mut big_t = 0;
    let mut cause = "";
    while (t as f32) < secs * 1000.0 {
        if t % sk.look < 20 {
            p.s.tx = aim(&p, sk, seed);
        }
        let (before, big) = (p.s.count, near_big(&p.s));
        let ev = p.update(&STEP);
        t += 20;
        if p.s.count < before {
            // what was near when soldiers were lost
            let s = &p.s;
            cause = if big {
                "big"
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
        if near_big(&p.s) {
            if big_t == 0 {
                run.at_big = p.s.count;
            }
            big_t += 20;
        }
        if ev & wg::EV_LEVEL != 0 {
            run.big_s = big_t as f32 / 1000.0;
            run.won = true;
            break;
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
    run.bigs = p.s.bigs;
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
                "  level {:2}: won {:2} of {}, {:4} at the first big one, {:4.1}s near them, {:4} left{}",
                level + 1,
                won.len(),
                runs.len(),
                won.iter().map(|r| r.at_big).sum::<i32>() / per,
                won.iter().map(|r| r.big_s).sum::<f32>() / per as f32,
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
                "  {:5.0} m {} big ones ({}, peak {})",
                r.d, r.bigs, r.died, r.peak
            );
            far.push(r.d);
        }
        println!("{line}");
        // it ends: no run lasts an hour (a flawless one can last most of it), and a sharp player gets past a kilometre
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
            let big = nearest_big(s).unwrap_or(&NO_BIG);
            println!("{:5.1}s d {:5.1} ph {} n {:3} w {} cx {:5.2} tx {:5.2} en {:2} shells {} big {}/{} plan {:.0}/{} fire {:.2} | {}", t as f32 * 0.02, s.d, s.phase, s.count, s.weapon, s.cx, s.tx, en, s.nshell, big.hp, big.hp_max, s.expect, s.expect_gun, s.fire() / s.planned_fire(s.d), near.join(" "));
        }
        if ev & wg::EV_GAMEOVER != 0 || (ev & wg::EV_LEVEL != 0 && !endless) {
            println!("ev {ev}");
            break;
        }
    }
}

/// Each gun a buff hands out beats the one before at a gate or mech, and at
/// a crowd of grunts.
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

/// The squad takes the gate in its lane and passes the rest of the row by,
/// and takes a buff it runs over but not one in another lane.
#[test]
fn lanes() {
    let mut p = Combat::new();
    p.s.opt = [0, 0];
    p.start(1);
    let d = p.s.d;
    (p.s.nthing, p.s.nen) = (0, 0);
    let gate = Thing {
        kind: K_GATE,
        d: d + 15.0,
        w: LANE_W - 0.5,
        b: 1,
        hp: 1,
        ..NO_THING
    };
    let buff = Thing {
        kind: K_BUFF,
        d: d + 30.0,
        a: BUFF_MEN,
        value: 7,
        ..NO_THING
    };
    let things = [
        Thing {
            x: lane_x(0),
            a: OP_ADD,
            value: 5,
            ..gate
        },
        Thing {
            x: lane_x(1),
            a: OP_MUL,
            value: 3,
            ..gate
        },
        Thing {
            x: lane_x(2),
            a: OP_DIV,
            value: 2,
            ..gate
        },
        Thing {
            x: lane_x(0),
            ..buff
        },
        Thing {
            x: lane_x(2),
            ..buff
        },
    ];
    p.s.thing[..5].copy_from_slice(&things);
    p.s.nthing = 5;
    p.s.count = 20;
    p.fill_squad();
    let run_to = |p: &mut Combat, to: f32| {
        while p.s.d < to {
            p.s.tx = lane_x(2);
            p.update(&STEP);
        }
    };
    let states = |p: &Combat, kind: i32| -> Vec<i32> {
        p.s.things()
            .iter()
            .filter(|t| t.kind == kind)
            .map(|t| t.state)
            .collect()
    };
    run_to(&mut p, d + 17.0);
    assert_eq!(p.s.count, 10);
    assert_eq!(states(&p, K_GATE), [2, 2, 1]);
    run_to(&mut p, d + 36.0);
    assert_eq!(p.s.count, 17);
    assert_eq!(states(&p, K_BUFF), [2, 1]);
}
