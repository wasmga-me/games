//! A bot that plays the course, for the tests (every hole can be made in
//! par) and the card picture. It tries a fan of putts from where the ball
//! lies and keeps the one that sinks, or else ends nearest the cup by the
//! way round the rails.

use crate::golf::*;
use crate::holes::Hole;

const CELL: f32 = 4.0;
const GW: usize = (PW / CELL) as usize;
const GH: usize = (PH / CELL) as usize;

/// How far each spot is from the cup, going round rails and water.
pub struct Field {
    d: Vec<f32>,
}

fn in_shape(pts: &[(f32, f32)], x: f32, y: f32) -> bool {
    let n = pts.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        if (a.1 <= y) != (b.1 <= y) && x < a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            inside = !inside;
        }
    }
    inside
}

fn seg_dist(px: f32, py: f32, (ax, ay, bx, by): (f32, f32, f32, f32)) -> f32 {
    let (qx, qy) = closest(px, py, ax, ay, bx, by);
    ((qx - px).powi(2) + (qy - py).powi(2)).sqrt()
}

/// A putt as a human plays it: off by `e` and `p` (-1..1) of the error `slop` allows.
fn jitter(aim: f32, power: f32, slop: f32, e: f32, p: f32) -> (f32, f32) {
    (aim + e * slop * 0.05, (power * (1.0 + p * slop * 0.12)).clamp(0.02, 1.0))
}

impl Field {
    pub fn new(h: &Hole) -> Field {
        let rails: Vec<_> = h.rails().collect();
        let open = |i: usize, j: usize| {
            let (x, y) = ((i as f32 + 0.5) * CELL, (j as f32 + 0.5) * CELL);
            in_shape(h.shape, x, y)
                && rails.iter().all(|&r| seg_dist(x, y, r) > RAIL + BALL_R - 1.0)
                && !h.water.iter().any(|a| inside(a, x, y))
                && h.bumpers.iter().all(|&(bx, by, r)| ((x - bx).powi(2) + (y - by).powi(2)).sqrt() > r + BALL_R - 1.0)
        };
        let mut ok = vec![false; GW * GH];
        for j in 0..GH {
            for i in 0..GW {
                ok[j * GW + i] = open(i, j);
            }
        }
        let mut d = vec![f32::INFINITY; GW * GH];
        let (ci, cj) = ((h.cup.0 / CELL) as usize, (h.cup.1 / CELL) as usize);
        d[cj * GW + ci] = 0.0;
        // Dijkstra on the grid, eight ways
        let mut heap = std::collections::BinaryHeap::new();
        heap.push((std::cmp::Reverse(0u32), ci, cj));
        while let Some((std::cmp::Reverse(k), i, j)) = heap.pop() {
            let here = k as f32 / 100.0;
            if here > d[j * GW + i] {
                continue;
            }
            for (di, dj, c) in [(1i32, 0i32, 1.0f32), (-1, 0, 1.0), (0, 1, 1.0), (0, -1, 1.0), (1, 1, 1.414), (1, -1, 1.414), (-1, 1, 1.414), (-1, -1, 1.414)] {
                let (ni, nj) = (i as i32 + di, j as i32 + dj);
                if ni < 0 || nj < 0 || ni >= GW as i32 || nj >= GH as i32 {
                    continue;
                }
                let at = nj as usize * GW + ni as usize;
                let nd = here + c * CELL;
                if ok[at] && nd < d[at] {
                    d[at] = nd;
                    heap.push((std::cmp::Reverse((nd * 100.0) as u32), ni as usize, nj as usize));
                }
            }
        }
        Field { d }
    }

    pub fn at(&self, x: f32, y: f32) -> f32 {
        let (i, j) = ((x / CELL) as usize, (y / CELL) as usize);
        // a ball resting by a rail can sit on a closed cell: take the best next to it
        let mut best = f32::INFINITY;
        for dj in -2i32..=2 {
            for di in -2i32..=2 {
                let (ni, nj) = (i as i32 + di, j as i32 + dj);
                if ni >= 0 && nj >= 0 && (ni as usize) < GW && (nj as usize) < GH {
                    best = best.min(self.d[nj as usize * GW + ni as usize] + (di.abs() + dj.abs()) as f32 * CELL);
                }
            }
        }
        best
    }
}

/// Plays one putt to its end with the clocks running; returns the state after.
pub fn play(s: &State, aim: f32, power: f32) -> State {
    let mut t = *s;
    t.aim = aim;
    t.power = power;
    t.putt();
    let mut ms = 0;
    while t.phase == ROLL && ms < 30_000 {
        t.frame(0, 0, 16, false, &mut 0, &mut NoFx);
        ms += 16;
    }
    t
}

/// How good a putt's end is: lower is better. A splash costs the putt
/// again from where it was hit, and some.
fn cost(t: &State, f: &Field, hole: i32) -> f32 {
    if t.hole != hole || t.phase == SUNK || t.phase == OVER {
        return -40.0;
    }
    if t.phase == SPLASH {
        return f.at(t.from_x, t.from_y) + 150.0;
    }
    f.at(t.x, t.y)
}

/// The best putt from here: (aim, power). With `slop` above 0 it plays safe:
/// the most promising putts are tried again with a human's errors, and the
/// one that does best on average wins.
pub fn best(s: &State, f: &Field, slop: f32) -> (f32, f32) {
    let mut all = Vec::with_capacity(240 * 25);
    for ai in 0..240 {
        let aim = ai as f32 * core::f32::consts::TAU / 240.0;
        for pi in 1..=25 {
            let power = pi as f32 / 25.0;
            let c = cost(&play(s, aim, power), f, s.hole);
            if c < 0.0 && slop == 0.0 {
                return (aim, power);
            }
            all.push((c, aim, power));
        }
    }
    all.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    if slop == 0.0 {
        return (all[0].1, all[0].2);
    }
    let mut best = (f32::INFINITY, all[0].1, all[0].2);
    for &(_, aim, power) in all.iter().take(16) {
        let mut sum = 0.0;
        for k in 0..6 {
            let e = [-1.0, -0.5, 0.0, 0.5, 1.0, 0.0][k];
            let p = [0.0, 1.0, -1.0, 0.5, -0.5, 0.0][k];
            let (aim, power) = jitter(aim, power, slop, e, p);
            sum += cost(&play(s, aim, power), f, s.hole);
        }
        if sum < best.0 {
            best = (sum, aim, power);
        }
    }
    (best.1, best.2)
}

/// Plays hole i to its end; `slop` makes the bot human (an aim and power
/// error drawn from `seed`). Returns the strokes it took.
pub fn hole(i: i32, slop: f32, seed: u32) -> i32 {
    let mut s = State::zero();
    s.opt[O_HOLE] = i;
    s.start();
    let f = Field::new(crate::golf::hole(i));
    let mut rng = wg::Rng::new(seed);
    while s.hole == i && s.phase == AIM {
        // a human waits a moment before each putt
        let wait = rng.range(0, 1500);
        let mut w = 0;
        while w < wait && s.phase == AIM {
            s.frame(0, 0, 16, false, &mut 0, &mut NoFx);
            w += 16;
        }
        // a bar or gate may knock the ball on while it waits
        while s.phase == ROLL || s.phase == SPLASH {
            s.frame(0, 0, 16, false, &mut 0, &mut NoFx);
        }
        if s.phase != AIM {
            break;
        }
        let (aim, power) = best(&s, &f, slop);
        let (aim, power) = jitter(aim, power, slop, rng.float() * 2.0 - 1.0, rng.float() * 2.0 - 1.0);
        let strokes = s.strokes;
        s.aim = aim;
        s.power = power;
        s.putt();
        while s.phase == ROLL || s.phase == SPLASH {
            s.frame(0, 0, 16, false, &mut 0, &mut NoFx);
        }
        assert!(s.phase != AIM || s.strokes > strokes);
    }
    s.last_strokes
}

#[cfg(test)]
mod tests {
    /// a sharp player makes every hole in par or better
    #[test]
    fn every_hole_in_par() {
        for i in 0..crate::holes::COUNT as i32 {
            let strokes = super::hole(i, 0.0, 1);
            assert!(strokes <= crate::golf::hole(i).par, "hole {} took {strokes}", i + 1);
        }
    }
}
