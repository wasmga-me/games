//! The rooms: walls, ledges and steel plates, where everyone stands, how a
//! shot bounces through it all, and the levels, dealt so each needs a
//! ricochet.

use wg::Rng;

pub const LEVELS: i32 = 30;
/// the room, plan units: the walls bullets bounce off
pub const LEFT: f32 = 0.0;
pub const RIGHT: f32 = 300.0;
pub const TOP: f32 = 48.0;
pub const FLOOR: f32 = 352.0;
/// a person's half width and height, plan units
pub const HALF_W: f32 = 8.0;
pub const TALL: f32 = 36.0;
/// a plate's half thickness
pub const PLATE_R: f32 = 3.0;
/// where a shot leaves the agent, above the feet
pub const GUN_UP: f32 = 21.0;
/// how far a shot goes, and how many times it may bounce
const REACH: f32 = 1600.0;
pub const BOUNCES: usize = 6;
/// the solver's aim steps, and how many steps in a row must agree for a
/// shot to count (so no level hangs on a hair)
pub const ANGLES: usize = 1440;
const WINDOW: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
    fn grow(&self, d: f32) -> Rect {
        Rect { x: self.x - d, y: self.y - d, w: self.w + 2.0 * d, h: self.h + 2.0 * d }
    }
    /// Where the segment a to b first enters this box, as a share of the
    /// way, if it does.
    fn enter(&self, a: (f32, f32), b: (f32, f32)) -> Option<f32> {
        let (mut t0, mut t1) = (0.0f32, 1.0f32);
        for (p, d, lo, hi) in [(a.0, b.0 - a.0, self.x, self.x + self.w), (a.1, b.1 - a.1, self.y, self.y + self.h)] {
            if d.abs() < 1e-6 {
                if p < lo || p > hi {
                    return None;
                }
                continue;
            }
            let (mut u, mut v) = ((lo - p) / d, (hi - p) / d);
            if u > v {
                std::mem::swap(&mut u, &mut v);
            }
            t0 = t0.max(u);
            t1 = t1.min(v);
            if t0 > t1 {
                return None;
            }
        }
        Some(t0)
    }
}

/// A steel plate: a thin bar from a to b.
#[derive(Clone, Copy, Debug)]
pub struct Plate {
    pub a: (f32, f32),
    pub b: (f32, f32),
}

/// A gunman or a hostage: where the feet are, and the look.
#[derive(Clone, Copy, Debug)]
pub struct Person {
    pub x: f32,
    pub feet: f32,
    pub look: u8,
}

impl Person {
    pub fn rect(&self) -> Rect {
        Rect { x: self.x - HALF_W, y: self.feet - TALL, w: 2.0 * HALF_W, h: TALL }
    }
}

#[derive(Clone)]
pub struct Room {
    /// walls and ledges
    pub blocks: Vec<Rect>,
    pub plates: Vec<Plate>,
    /// the agent's feet
    pub agent: (f32, f32),
    pub foes: Vec<Person>,
    pub hostages: Vec<Person>,
    /// bullets for the level
    pub shots: i32,
}

/// Who on our side a shot can hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Hostage(usize),
    /// the shot came back into the agent
    Agent,
}

/// A shot's way through the room: the points it turns at, and who it hits
/// how far along. It ends at a hostage, back at the agent, or when it runs
/// out.
#[derive(Clone, Default)]
pub struct Path {
    pub pts: Vec<(f32, f32)>,
    /// each foe hit and the distance there
    pub foes: Vec<(usize, f32)>,
    /// someone on our side hit, if any: the path ends there
    pub hit: Option<Hit>,
    pub len: f32,
}

impl Path {
    /// the point `d` along the way
    pub fn at(&self, d: f32) -> (f32, f32) {
        let mut left = d;
        for w in self.pts.windows(2) {
            let l = dist(w[0], w[1]);
            if left <= l {
                let t = if l > 0.0 { left / l } else { 0.0 };
                return (w[0].0 + (w[1].0 - w[0].0) * t, w[0].1 + (w[1].1 - w[0].1) * t);
            }
            left -= l;
        }
        *self.pts.last().unwrap_or(&(0.0, 0.0))
    }
    pub fn bounces(&self) -> usize {
        self.pts.len().saturating_sub(2)
    }
    /// the foes it hits, as bits
    pub fn mask(&self) -> u32 {
        self.foes.iter().fold(0, |m, &(i, _)| m | 1 << i)
    }
    /// Ends the way `len` along, before anyone it would hit after that.
    pub fn cut(&mut self, len: f32) {
        let end = self.at(len);
        let mut gone = 0.0;
        let keep = self.pts.windows(2).take_while(|w| {
            gone += dist(w[0], w[1]);
            gone < len
        });
        let mut pts = vec![self.pts[0]];
        pts.extend(keep.map(|w| w[1]));
        pts.push(end);
        self.pts = pts;
        self.foes.retain(|&(_, d)| d <= len);
        self.hit = None;
        self.len = len;
    }
}

pub fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

impl Room {
    /// a room with nobody in it, until a level is dealt
    pub fn empty() -> Room {
        Room { blocks: Vec::new(), plates: Vec::new(), agent: (RIGHT / 2.0, FLOOR), foes: Vec::new(), hostages: Vec::new(), shots: 0 }
    }
    /// every foe, as bits
    pub fn all_foes(&self) -> u32 {
        (1u32 << self.foes.len()) - 1
    }
    /// where a shot starts
    pub fn gun(&self) -> (f32, f32) {
        (self.agent.0, self.agent.1 - GUN_UP)
    }

    /// Every surface a shot bounces off, as segments.
    fn walls(&self) -> Vec<((f32, f32), (f32, f32))> {
        let mut out = vec![((LEFT, TOP), (RIGHT, TOP)), ((RIGHT, TOP), (RIGHT, FLOOR)), ((RIGHT, FLOOR), (LEFT, FLOOR)), ((LEFT, FLOOR), (LEFT, TOP))];
        for b in &self.blocks {
            let (x0, y0, x1, y1) = (b.x, b.y, b.x + b.w, b.y + b.h);
            out.extend([((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))]);
        }
        for p in &self.plates {
            // the plate's two faces and its ends
            let l = dist(p.a, p.b).max(1e-3);
            let n = (-(p.b.1 - p.a.1) / l * PLATE_R, (p.b.0 - p.a.0) / l * PLATE_R);
            let c = [(p.a.0 + n.0, p.a.1 + n.1), (p.b.0 + n.0, p.b.1 + n.1), (p.b.0 - n.0, p.b.1 - n.1), (p.a.0 - n.0, p.a.1 - n.1)];
            out.extend([(c[0], c[1]), (c[1], c[2]), (c[2], c[3]), (c[3], c[0])]);
        }
        out
    }

    /// The way a shot at `angle` (radians, 0 to the right, down positive)
    /// goes.
    pub fn trace(&self, angle: f32) -> Path {
        self.trace_with(&self.walls(), angle)
    }

    fn trace_with(&self, walls: &[((f32, f32), (f32, f32))], angle: f32) -> Path {
        let mut p = self.gun();
        let mut d = (angle.cos(), angle.sin());
        let mut path = Path { pts: vec![p], ..Path::default() };
        let mut left = REACH;
        let mut hit_foes = 0u32;
        for bounce in 0..=BOUNCES {
            // the nearest surface ahead
            let mut best: Option<(f32, (f32, f32))> = None;
            for &(a, b) in walls {
                let e = (b.0 - a.0, b.1 - a.1);
                let den = d.0 * e.1 - d.1 * e.0;
                if den.abs() < 1e-6 {
                    continue;
                }
                let w = (a.0 - p.0, a.1 - p.1);
                let t = (w.0 * e.1 - w.1 * e.0) / den;
                let u = (w.0 * d.1 - w.1 * d.0) / den;
                if t > 1e-3 && (-1e-4..=1.0 + 1e-4).contains(&u) && best.is_none_or(|bb| t < bb.0) {
                    let l = (e.0 * e.0 + e.1 * e.1).sqrt();
                    best = Some((t, (-e.1 / l, e.0 / l)));
                }
            }
            let (t, n) = best.unwrap_or((left, (0.0, 0.0)));
            let t = t.min(left);
            let q = (p.0 + d.0 * t, p.1 + d.1 * t);
            // who stands on the way: a hostage, or the agent once the shot
            // has bounced (it leaves the agent's own gun first)
            let mut stop: Option<(Hit, f32)> = None;
            let agent = (bounce > 0).then(|| (Hit::Agent, agent_rect(self.agent)));
            for (who, r) in self.hostages.iter().enumerate().map(|(i, h)| (Hit::Hostage(i), h.rect())).chain(agent) {
                if let Some(s) = r.enter(p, q) {
                    if stop.is_none_or(|st| s < st.1) {
                        stop = Some((who, s));
                    }
                }
            }
            let reach = stop.map_or(1.0, |s| s.1);
            for (i, f) in self.foes.iter().enumerate() {
                if hit_foes & 1 << i != 0 {
                    continue;
                }
                if let Some(s) = f.rect().enter(p, q) {
                    if s <= reach {
                        hit_foes |= 1 << i;
                        path.foes.push((i, path.len + s * t));
                    }
                }
            }
            if let Some((who, s)) = stop {
                let end = (p.0 + d.0 * t * s, p.1 + d.1 * t * s);
                path.len += t * s;
                path.hit = Some(who);
                path.pts.push(end);
                break;
            }
            path.len += t;
            path.pts.push(q);
            left -= t;
            if left <= 1e-3 || best.is_none() || bounce == BOUNCES {
                break;
            }
            // off the surface, mirror-like
            let k = d.0 * n.0 + d.1 * n.1;
            d = (d.0 - 2.0 * k * n.0, d.1 - 2.0 * k * n.1);
            p = q;
        }
        path.foes.sort_by(|a, b| a.1.total_cmp(&b.1));
        path
    }

    /// The angle of aim step k.
    pub fn step_angle(k: usize) -> f32 {
        std::f32::consts::TAU * k as f32 / ANGLES as f32
    }

    /// Each shot that hits someone, by aim step: the foes it hits as
    /// bits, and whether it hurts a hostage or the agent; and which foes a straight shot
    /// reaches.
    fn survey(&self) -> (Vec<(u32, bool)>, u32) {
        let walls = self.walls();
        let mut direct = 0;
        let shots = (0..ANGLES)
            .map(|k| {
                let path = self.trace_with(&walls, Room::step_angle(k));
                let first = dist(path.pts[0], path.pts[1]);
                for &(i, d) in &path.foes {
                    if d <= first + 0.01 {
                        direct |= 1 << i;
                    }
                }
                (path.mask(), path.hit.is_some())
            })
            .collect();
        (shots, direct)
    }

    /// The foes the shot at aim step k hits, as bits, if it hits some,
    /// spares the hostages and the agent and hits the same for WINDOW aim steps in a row.
    fn steady(survey: &[(u32, bool)], k: usize) -> Option<u32> {
        let n = survey.len();
        let m = survey[k].0;
        (m != 0 && (0..WINDOW).all(|j| survey[(k + j) % n] == (m, false))).then_some(m)
    }

    /// The steady shots: their foe bits.
    fn good_shots(survey: &[(u32, bool)]) -> Vec<u32> {
        let mut out = Vec::new();
        for m in (0..survey.len()).filter_map(|k| Room::steady(survey, k)) {
            if !out.contains(&m) {
                out.push(m);
            }
        }
        out
    }

    /// An aim on a way to down every foe in the fewest shots, safely and
    /// within a window of aim steps, as the dealer counted.
    #[cfg(test)]
    pub fn best_shot(&self, down: u32) -> Option<f32> {
        let (survey, _) = self.survey();
        let good = Room::good_shots(&survey);
        (0..survey.len())
            .filter_map(|k| Room::steady(&survey, k).filter(|m| m & !down != 0).map(|m| (m, k)))
            .filter_map(|(m, k)| Room::shots_needed(&good, self.all_foes(), down | m).map(|left| (left, k)))
            .min()
            .map(|(_, k)| Room::step_angle(k + WINDOW / 2))
    }

    /// How few shots clear the room from foes `down` (as bits), using
    /// these shots; None if some foe cannot be hit.
    fn shots_needed(good: &[u32], all: u32, down: u32) -> Option<i32> {
        if down == all {
            return Some(0);
        }
        good.iter().filter(|&&m| m & !down != 0).filter_map(|&m| Room::shots_needed(good, all, down | m)).min().map(|n| n + 1)
    }
}

/// a level's foes and hostages
fn counts(level: i32) -> (usize, usize) {
    let foes = (1 + level / 7) as usize;
    let hostages = if level >= 4 { 1 + (level >= 16) as usize } else { 0 };
    (foes.min(4), hostages)
}

/// Deals a level: the same level always deals the same room.
pub fn level(level: i32) -> Room {
    let mut rng = Rng::new(0xBA45 + level as u32 * 7919);
    let (foes, hostages) = counts(level);
    // the share of aim steps that hit a foe safely: more at first, fewer
    // at the end
    let want = 0.15 - 0.12 * level as f32 / (LEVELS - 1) as f32;
    let mut best: Option<(f32, Room)> = None;
    let (mut tries, mut dealt) = (0, 0);
    while tries < 2000 && dealt < 16 {
        tries += 1;
        let Some(mut room) = build(&mut rng, foes, hostages) else { continue };
        let (survey, direct) = room.survey();
        let all = room.all_foes();
        // at least one foe out of a straight shot's reach, two from the
        // middle levels on
        let hidden = (all & !direct).count_ones();
        if hidden < 1 + (level >= 10 && foes >= 2) as u32 {
            continue;
        }
        let good = Room::good_shots(&survey);
        let Some(need) = Room::shots_needed(&good, all, 0) else { continue };
        room.shots = need + (level < 15) as i32;
        dealt += 1;
        let safe = survey.iter().filter(|&&(m, h)| m != 0 && !h).count() as f32 / survey.len() as f32;
        let off = (safe - want).abs();
        if best.as_ref().is_none_or(|b| off < b.0) {
            best = Some((off, room));
        }
    }
    best.expect("no room dealt").1
}

/// One room: the agent at one side, walls between, perhaps ledges and
/// plates, and the foes and hostages standing about. None if it did not
/// fit together.
fn build(rng: &mut Rng, foes: usize, hostages: usize) -> Option<Room> {
    let left = rng.below(2) == 0;
    let ax = if left { 26.0 } else { RIGHT - 26.0 };
    let mut room = Room { blocks: Vec::new(), plates: Vec::new(), agent: (ax, FLOOR), foes: Vec::new(), hostages: Vec::new(), shots: 0 };
    // the walls between: up from the floor or down from the ceiling
    let walls = 1 + rng.below(2) as usize;
    for i in 0..walls {
        let w = 14.0 + rng.below(3) as f32 * 4.0;
        let x = 92.0 + rng.float() * 116.0 - w / 2.0;
        let r = if i == 0 || rng.below(2) == 0 {
            let top = 140.0 + rng.float() * 110.0;
            Rect { x, y: top, w, h: FLOOR - top }
        } else {
            let bottom = 170.0 + rng.float() * 110.0;
            Rect { x, y: TOP, w, h: bottom - TOP }
        };
        if room.blocks.iter().any(|b| b.grow(26.0).overlaps(&r)) {
            return None;
        }
        room.blocks.push(r);
    }
    // ledges to stand on
    for _ in 0..rng.below(3) {
        let w = 56.0 + rng.float() * 50.0;
        let x = if rng.below(2) == 0 { LEFT } else { RIGHT - w };
        let x = if rng.below(3) == 0 { 60.0 + rng.float() * (180.0 - w) } else { x };
        let r = Rect { x, y: 150.0 + rng.float() * 130.0, w, h: 10.0 };
        if room.blocks.iter().any(|b| b.grow(2.0).overlaps(&r)) || r.grow(TALL + 6.0).overlaps(&agent_rect((ax, FLOOR))) {
            return None;
        }
        room.blocks.push(r);
    }
    // steel plates in the air
    for _ in 0..rng.below(3) {
        let len = 34.0 + rng.float() * 24.0;
        let a = [-60.0f32, -45.0, -30.0, 0.0, 30.0, 45.0, 60.0, 90.0][rng.below(8) as usize].to_radians();
        let c = (40.0 + rng.float() * 220.0, TOP + 30.0 + rng.float() * 170.0);
        let (dx, dy) = (a.cos() * len / 2.0, a.sin() * len / 2.0);
        let p = Plate { a: (c.0 - dx, c.1 - dy), b: (c.0 + dx, c.1 + dy) };
        let bound = Rect { x: c.0 - dx.abs() - PLATE_R, y: c.1 - dy.abs() - PLATE_R, w: 2.0 * (dx.abs() + PLATE_R), h: 2.0 * (dy.abs() + PLATE_R) };
        if room.blocks.iter().any(|b| b.grow(10.0).overlaps(&bound)) || bound.grow(12.0).overlaps(&agent_rect((ax, FLOOR))) {
            return None;
        }
        room.plates.push(p);
    }
    // where people can stand: the floor and the tops of walls and ledges
    let mut tops: Vec<(f32, f32, f32)> = vec![(LEFT, RIGHT, FLOOR)];
    for b in &room.blocks {
        if b.y > TOP + TALL + 4.0 {
            tops.push((b.x, b.x + b.w, b.y));
        }
    }
    let stands = |room: &Room, p: &Person| {
        let r = p.rect();
        r.x >= LEFT + 2.0
            && r.x + r.w <= RIGHT - 2.0
            && r.y > TOP + 4.0
            && !room.blocks.iter().any(|b| b.overlaps(&r))
            && !room.plates.iter().any(|pl| plate_box(pl).overlaps(&r.grow(4.0)))
            && !agent_rect((ax, FLOOR)).grow(50.0).overlaps(&r)
            && !room.foes.iter().chain(&room.hostages).any(|o| (o.x - p.x).abs() < 2.0 * HALF_W + 18.0 && (o.feet - p.feet).abs() < TALL)
    };
    for i in 0..foes {
        let mut placed = false;
        for _ in 0..30 {
            let &(x0, x1, y) = &tops[rng.below(tops.len() as u32) as usize];
            if x1 - x0 < 2.0 * HALF_W + 4.0 {
                continue;
            }
            let p = Person { x: x0 + HALF_W + 2.0 + rng.float() * (x1 - x0 - 2.0 * HALF_W - 4.0), feet: y, look: (i % 3) as u8 };
            if stands(&room, &p) {
                room.foes.push(p);
                placed = true;
                break;
            }
        }
        if !placed {
            return None;
        }
    }
    // hostages next to a foe
    for _ in 0..hostages {
        let mut placed = false;
        for _ in 0..20 {
            let f = room.foes[rng.below(room.foes.len() as u32) as usize];
            let side = if rng.below(2) == 0 { -1.0 } else { 1.0 };
            let p = Person { x: f.x + side * (2.0 * HALF_W + 18.0 + rng.float() * 8.0), feet: f.feet, look: 0 };
            // on the same ground
            let ground = tops.iter().any(|&(x0, x1, y)| y == p.feet && p.x - HALF_W >= x0 && p.x + HALF_W <= x1);
            if ground && stands(&room, &p) {
                room.hostages.push(p);
                placed = true;
                break;
            }
        }
        if !placed {
            return None;
        }
    }
    Some(room)
}

fn agent_rect((x, feet): (f32, f32)) -> Rect {
    Rect { x: x - HALF_W, y: feet - TALL, w: 2.0 * HALF_W, h: TALL }
}

fn plate_box(p: &Plate) -> Rect {
    let (x0, x1) = (p.a.0.min(p.b.0), p.a.0.max(p.b.0));
    let (y0, y1) = (p.a.1.min(p.b.1), p.a.1.max(p.b.1));
    Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }.grow(PLATE_R)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_level_can_be_cleared_with_its_shots() {
        for l in 0..LEVELS {
            let room = level(l);
            let (survey, direct) = room.survey();
            let all = room.all_foes();
            let need = Room::shots_needed(&Room::good_shots(&survey), all, 0).expect("unclearable");
            assert!(need <= room.shots, "level {}", l + 1);
            assert!(all & !direct != 0, "level {}: every foe in plain sight", l + 1);
            let safe = survey.iter().filter(|&&(m, h)| m != 0 && !h).count();
            println!(
                "level {:2}: foes {} hostages {} blocks {} plates {} shots {} hidden {} safe {:.3}",
                l + 1,
                room.foes.len(),
                room.hostages.len(),
                room.blocks.len(),
                room.plates.len(),
                room.shots,
                (all & !direct).count_ones(),
                safe as f32 / survey.len() as f32
            );
        }
    }

    #[test]
    fn a_shot_bounces_like_a_mirror() {
        let room = Room { agent: (30.0, FLOOR), shots: 1, ..Room::empty() };
        // straight up: off the ceiling and back down
        let p = room.trace(-std::f32::consts::FRAC_PI_2);
        assert!((p.pts[1].1 - TOP).abs() < 0.01 && (p.pts[1].0 - 30.0).abs() < 0.01);
        assert!(p.pts[2].1 > p.pts[1].1);
        // 45 degrees up to the right: off the right wall, still 45 degrees
        let p = room.trace(-std::f32::consts::FRAC_PI_4);
        let (b, c) = (p.pts[1], p.pts[2]);
        assert!((b.0 - RIGHT).abs() < 0.01);
        assert!(((c.0 - b.0) - (c.1 - b.1)).abs() < 0.1 && c.0 < b.0);
    }

    #[test]
    fn a_shot_coming_back_hits_the_agent() {
        let room = Room { agent: (30.0, FLOOR), shots: 1, ..Room::empty() };
        // straight up and back down onto the agent's head
        let p = room.trace(-std::f32::consts::FRAC_PI_2);
        assert_eq!(p.hit, Some(Hit::Agent));
        assert!((p.pts[2].1 - (FLOOR - TALL)).abs() < 0.01);
        // off the floor at the agent's feet, back up through the agent
        let p = room.trace(std::f32::consts::FRAC_PI_2 * 0.98);
        assert!(p.hit == Some(Hit::Agent) && p.pts.len() == 3);
        // level, off the far wall, back the same way
        assert_eq!(room.trace(0.0).hit, Some(Hit::Agent));
        // the shot leaving the gun does not count: it goes on to the far
        // wall and bounces
        let p = room.trace(-std::f32::consts::FRAC_PI_4);
        assert!((p.pts[1].0 - RIGHT).abs() < 0.01 && p.bounces() > 0);
    }

    #[test]
    fn a_cut_shot_ends_there() {
        let foe = Person { x: 200.0, feet: FLOOR, look: 0 };
        let room = Room { agent: (30.0, FLOOR), foes: vec![foe], shots: 1, ..Room::empty() };
        // level: through the foe, off the far wall and back into the agent
        let mut p = room.trace(0.0);
        assert_eq!((p.mask(), p.hit), (1, Some(Hit::Agent)));
        p.cut(170.0);
        assert_eq!((p.mask(), p.hit, p.pts.len()), (1, None, 2));
        assert!((p.pts[1].0 - 200.0).abs() < 0.01);
    }

    #[test]
    fn a_hostage_stops_the_shot() {
        let foe = Person { x: 200.0, feet: FLOOR, look: 0 };
        let hostage = Person { x: 150.0, feet: FLOOR, look: 0 };
        let room = Room { agent: (30.0, FLOOR), foes: vec![foe], hostages: vec![hostage], shots: 1, ..Room::empty() };
        let p = room.trace(0.0);
        assert_eq!(p.hit, Some(Hit::Hostage(0)));
        assert!(p.foes.is_empty());
        assert!((p.len - (150.0 - HALF_W - 30.0)).abs() < 0.1);
    }
}
