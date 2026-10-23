//! A bot that climbs, for the tests (the way up never closes) and the card
//! picture: it steers under the best ledge it can reach.

use crate::play::*;

/// The x to steer to this frame: the highest ledge the puff can still land
/// on, from where it will be when it comes down to it.
pub fn steer(s: &State) -> f32 {
    let mut best: Option<(f32, f32)> = None;
    for l in s.ledges.iter().filter(|l| l.w > 0.0 && l.crumble_ms == 0) {
        // when the puff falls past the ledge's top, if it ever does
        let rise = l.alt + R - s.alt;
        let disc = s.vy * s.vy - 2.0 * GRAVITY * rise;
        if disc < 0.0 || (s.vy <= 0.0 && rise > 0.0) || s.gust_ms > 0 {
            continue;
        }
        let t = (s.vy + disc.sqrt()) / GRAVITY;
        let mid = (l.x + l.w / 2.0 + l.vx * t).clamp(l.w / 2.0, PW - l.w / 2.0);
        let mid = if l.kind == GROUND { s.x } else { mid };
        let reach = MAX_VX * 0.93 * t + l.w / 2.0 - 8.0;
        if wrap_dx(s.x, mid).abs() > reach {
            continue;
        }
        // a crumbling ledge only when nothing as high will do
        let score = l.alt - if l.kind == CRUMBLE { 20.0 } else { 0.0 };
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, mid));
        }
    }
    let x = best.map_or(s.x, |b| b.1);
    // look ahead: the first way that meets no bird
    if s.birds.iter().any(|b| b.alive == 1 && (b.alt - s.alt).abs() < 250.0) {
        for dx in [0.0, 50.0, -50.0, 100.0, -100.0, 160.0, -160.0] {
            let to = (x + dx).rem_euclid(PW);
            let mut t = *s;
            let mut ok = true;
            for _ in 0..45 {
                t.frame(0, Some(to), 16, &mut NoFx);
                if t.phase != PLAY {
                    ok = false;
                    break;
                }
            }
            if ok {
                return to;
            }
        }
    }
    x
}

/// Climbs with the bot until it falls or `ms` passes; returns where it got.
pub fn climb(pace: i32, seed: u32, ms: i32) -> State {
    let mut s = State::zero();
    s.opt[O_PACE] = pace;
    s.start(seed);
    let mut t = 0;
    while !s.over() && t < ms {
        let to = steer(&s);
        s.frame(0, Some(to), 16, &mut NoFx);
        t += 16;
    }
    s
}

#[cfg(test)]
mod tests {
    /// the way up never closes: the bot climbs on for minutes on every pace
    #[test]
    fn the_way_up_stays_open() {
        for pace in 0..3 {
            for seed in [7919, 15838, 23757] {
                let s = super::climb(pace, seed, 180_000);
                assert!(s.height() >= 2500, "pace {pace} seed {seed}: {} m", s.height());
            }
        }
    }

    /// back down on the ground after the first ledge, the run is over
    #[test]
    fn the_ground_holds_only_at_the_start() {
        use crate::play::*;
        for pace in 0..3 {
            let mut s = State::zero();
            s.opt[O_PACE] = pace;
            s.start(7);
            let mut t = 0;
            while s.ledges.iter().any(|l| l.kind == GROUND && l.crumble_ms == 0) {
                s.frame(0, Some(super::steer(&s)), 16, &mut NoFx);
                t += 16;
                assert!(t < 10_000, "pace {pace}: never left the ground");
            }
            // dropped back to just above the ground
            s.alt = R + 20.0;
            s.vy = -200.0;
            for _ in 0..30 {
                s.frame(0, None, 16, &mut NoFx);
            }
            assert!(s.over(), "pace {pace}: still bouncing at {} m", s.height());
        }
    }
}
