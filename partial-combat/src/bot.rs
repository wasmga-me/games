//! Bots that play every level and the endless pier, for balancing:
//!   cargo test --release -- --nocapture
//! A sharp player picks the better gate, shoots barrels and crates open and
//! keeps out of holes and shell fire; a casual one gets gates wrong now and
//! then, doesn't line up shots and dodges half the shells. The asserts hold
//! the balance aimed for: a sharp player clears every level, a casual one
//! the early ones, and the last levels are hard for both.
use crate::*;

struct Skill {
    name: &'static str,
    /// how often the better gate is picked
    gates: f32,
    /// lines up on barrels and crates
    aim: bool,
    /// how often a shell is seen in time
    dodge: f32,
    /// ms between looks at the pier
    look: i32,
}

const SHARP: Skill = Skill { name: "sharp", gates: 1.0, aim: true, dodge: 1.0, look: 120 };
/// runs per level, each its own luck
const SEEDS: [u32; 3] = [1, 2, 5];

const CASUAL: Skill = Skill { name: "casual", gates: 0.8, aim: false, dodge: 0.5, look: 250 };

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
    let h = (d.to_bits() ^ salt).wrapping_mul(0x9e37_79b9).rotate_left(13).wrapping_mul(0x85eb_ca6b);
    (h >> 8) as f32 / (1u32 << 24) as f32
}

fn result(op: i32, v: i32, n: i32) -> i32 {
    match op {
        OP_ADD => n + v,
        OP_SUB => n - v,
        OP_MUL => n * v,
        _ => (n + v - 1) / v.max(1),
    }
}

/// Where to steer, as a player who sees the pier would.
fn aim(p: &Combat, sk: &Skill, salt: u32) -> f32 {
    let s = &p.s;
    let r = radius(s.count.min(MAXV as i32));
    let mut want = s.tx;
    // the next gate pair
    let gate = s.things().iter().enumerate().filter(|(_, t)| t.kind == K_GATE && t.state == 0 && t.d > s.d - 0.2).min_by(|a, b| a.1.d.total_cmp(&b.1.d));
    let mut busy = false;
    if let Some((k, g)) = gate {
        if g.d - s.d < 30.0 {
            let o = &s.thing[g.b as usize];
            let (mine, other) = (result(g.a, g.value, s.count), if g.b as usize != k { result(o.a, o.value, s.count) } else { i32::MIN });
            let best = if mine >= other { g.x } else { o.x };
            let worst = if mine >= other { o.x } else { g.x };
            want = if hash01(g.d, salt) < sk.gates { best } else { worst };
            busy = true;
        }
    }
    // a barrel or crate in range and nearer than any gate
    if sk.aim {
        let gd = gate.map_or(f32::MAX, |(_, g)| g.d);
        if let Some(t) = s.things().iter().filter(|t| matches!(t.kind, K_BARREL | K_CRATE) && t.hp > 0 && t.d > s.d + 1.0 && t.d - s.d < RANGE - 4.0 && t.d < gd).min_by(|a, b| a.d.total_cmp(&b.d)) {
            want = if t.d - s.d < 3.5 { t.x + if t.x > 0.0 { -1.8 } else { 1.8 } } else { t.x };
            busy = true;
        }
    }
    // TNT: shot from afar, given a wide berth near
    if let Some(t) = s.things().iter().filter(|t| t.kind == K_TNT && t.hp > 0 && t.d > s.d - 2.0 && t.d - s.d < 20.0).min_by(|a, b| a.d.total_cmp(&b.d)) {
        if t.d - s.d > 9.0 && sk.aim {
            want = t.x;
        } else if (want - t.x).abs() < r + 1.0 {
            let side = if t.x > 0.0 { -1.0 } else { 1.0 };
            want = t.x + side * (r + 1.5);
        }
        busy = true;
    }
    if !busy {
        want = s.enemies().iter().find(|e| e.state == ES_WAIT && e.d - s.d < 25.0).map_or(0.0, |e| e.x.clamp(-2.0, 2.0));
    }
    want = want.clamp(-HALF + 0.8, HALF - 0.8);
    // keep to the deck: how far ahead x stays clear, and whether the squad gets there without crossing a hole
    let w = r.min(1.2);
    let ahead = |x: f32| (0..12).take_while(|&i| {
        let d = s.d + i as f32;
        s.deck(x, d) && s.deck(x - w, d) && s.deck(x + w, d)
    }).count();
    // on the way across the squad runs on about a metre for every two it moves, front to back
    let reach = |x: f32| (0..=20).all(|i| {
        let f = i as f32 / 20.0;
        let x = s.cx + (x - s.cx) * f;
        let d = s.d + (x - s.cx).abs() * f * 0.5;
        (-2..=2).all(|k| s.deck(x, d + k as f32 * r * 0.5 + 0.5))
    });
    let clear = |x: f32| ahead(x) >= 12 && reach(x);
    if !clear(want) {
        // the most deck ahead, near where it wanted to go; crossing a hole's edge only when staying is worse
        let score = |x: f32| ahead(x) as f32 * 100.0 - (x - want).abs() * 4.0 - if reach(x) { 0.0 } else { 400.0 };
        if let Some(x) = (-16..=16).map(|i| i as f32 * 0.25).max_by(|a, b| score(*a).total_cmp(&score(*b))) {
            want = x;
        }
    }
    // shells about to land: where they catch the fewest
    let hit = |x: f32| s.shell[..s.nshell as usize].iter().filter(|h| h.t_max - h.t < 1.4).map(|h| (h.r + r - (x - h.x).hypot(s.d - h.d)).max(0.0)).sum::<f32>();
    if s.nshell > 0 && hit(want) > 0.0 && hash01(s.shell[0].x, salt ^ 7) < sk.dodge {
        let score = |x: f32| hit(x) * 10.0 + (x - s.cx).abs();
        if let Some(x) = (-16..=16).map(|i| i as f32 * 0.25).filter(|&x| clear(x)).min_by(|a, b| score(*a).total_cmp(&score(*b))) {
            want = x;
        }
    }
    want
}

fn play(level: i32, endless: bool, sk: &Skill, seed: u32, secs: f32) -> Run {
    let mut p = Combat::new();
    p.s.opt = [endless as i32, level];
    p.start(seed);
    let mut run = Run::default();
    let input = Input { held: 0, pressed: 0, dt: 20 };
    let mut t = 0;
    let mut boss_t = 0;
    let mut cause = "";
    while (t as f32) < secs * 1000.0 {
        if t % sk.look < 20 {
            p.s.tx = aim(&p, sk, seed);
        }
        let (before, phase) = (p.s.count, p.s.phase);
        let ev = p.update(&input);
        t += 20;
        if p.s.count < before {
            // what was near when soldiers were lost
            let s = &p.s;
            cause = if phase == P_BOSS {
                "boss"
            } else if s.soldiers().iter().any(|q| q.state == S_FALL) {
                "fell"
            } else if s.enemies().iter().any(|e| e.state == ES_CHARGE && (e.d - s.d).abs() < 3.0) {
                "enemies"
            } else {
                "other"
            };
        }
        run.peak = run.peak.max(p.s.count);
        if p.s.phase == P_BOSS {
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
    let mut cleared = [[0; LEVELS as usize]; 2];
    for (n, sk) in [SHARP, CASUAL].iter().enumerate() {
        println!("{}", sk.name);
        for level in 0..LEVELS {
            let mut line = format!("  level {:2}:", level + 1);
            for seed in SEEDS {
                let r = play(level, false, sk, seed, 400.0);
                cleared[n][level as usize] += r.won as i32;
                line += &if r.won {
                    format!("  won {:3}s peak {:3} boss {:3} {:4.1}s left {:3}", r.time_s as i32, r.peak, r.at_boss, r.boss_s, r.end)
                } else {
                    format!("  lost {:3}s at {:3.0} m peak {:3} ({})", r.time_s as i32, r.d, r.peak, r.died)
                };
            }
            println!("{line}");
        }
    }
    // a sharp player clears every level; a casual one most of the first five, and few of the last five
    let sum = |c: &[i32]| c.iter().sum::<i32>();
    assert!(cleared[0].iter().all(|&c| c >= 2), "sharp: {:?}", cleared[0]);
    assert!(sum(&cleared[1][..5]) >= 11, "casual: {:?}", cleared[1]);
    assert!(sum(&cleared[1][15..]) <= 8, "casual: {:?}", cleared[1]);
}

#[test]
fn endless() {
    for sk in [SHARP, CASUAL] {
        let mut line = format!("{}:", sk.name);
        let mut far = vec![];
        for seed in SEEDS {
            let r = play(0, true, &sk, seed, 1500.0);
            line += &format!("  {:5.0} m {} bosses ({}, peak {})", r.d, r.bosses, r.died, r.peak);
            far.push(r.d);
        }
        println!("{line}");
        // it ends: no run lasts the 25 minutes, and a sharp player gets past a few bosses
        assert!(far.iter().all(|&d| d < 6000.0), "{line}");
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
    let sk = if std::env::var("BOT_SKILL").is_ok_and(|v| v == "casual") { CASUAL } else { SHARP };
    let mut p = Combat::new();
    p.s.opt = [endless as i32, level.max(0)];
    p.start(seed);
    let input = Input { held: 0, pressed: 0, dt: 20 };
    for t in 0..30_000 {
        if (t * 20) % sk.look < 20 {
            p.s.tx = aim(&p, &sk, seed);
        }
        let before = (p.s.count, p.s.phase);
        let ev = p.update(&input);
        let s = &p.s;
        if t % 25 == 0 || s.count < before.0 - 3 || s.phase != before.1 {
            let near: Vec<String> = s.things().iter().filter(|t| t.d - s.d > -3.0 && t.d - s.d < 20.0).map(|t| if t.kind == K_HOLE { format!("hole@{:.0}..{:.0}x{:.1}..{:.1}", t.d - s.d, t.d + t.len - s.d, t.x, t.x + t.w) } else { format!("{}@{:.0}x{:.1}v{}h{}", t.kind, t.d - s.d, t.x, t.value, t.hp) }).collect();
            let en = s.enemies().iter().filter(|e| e.state != ES_DOWN).count();
            println!("{:5.1}s d {:5.1} ph {} n {:3} w {} cx {:5.2} tx {:5.2} en {:2} shells {} boss {}/{} | {}", t as f32 * 0.02, s.d, s.phase, s.count, s.weapon, s.cx, s.tx, en, s.nshell, s.boss.hp, s.boss.hp_max, near.join(" "));
        }
        if ev & (wg::EV_GAMEOVER | wg::EV_LEVEL) != 0 {
            println!("ev {ev}");
            break;
        }
    }
}
