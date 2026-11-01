//! The rooms: walls, ledges and steel plates, where everyone stands, how a
//! shot bounces through it all, and a fixed campaign teaching one idea at a time.

pub const LEVELS: i32 = 100;
include!("campaign.rs");
#[cfg(test)]
#[path = "curation.rs"]
mod curation;
/// the room, plan units: the walls bullets bounce off
pub const LEFT: f32 = 0.0;
pub const RIGHT: f32 = 300.0;
pub const TOP: f32 = 48.0;
pub const FLOOR: f32 = 352.0;
/// a person's half width and height, plan units
pub const HALF_W: f32 = 10.0;
pub const TALL: f32 = 36.0;
/// a plate's half thickness
pub const PLATE_R: f32 = 3.0;
/// where a shot leaves the agent, above the feet
pub const GUN_UP: f32 = 21.0;
/// how far a shot goes, and how many times it may bounce
const REACH: f32 = 1600.0;
pub const BOUNCES: usize = 6;
/// the solver's aim steps
#[cfg(test)]
pub const ANGLES: usize = 1440;

/// A forgiving three-degree solution window, including on the last level.
/// Extra targets and cover add difficulty; smaller aim tolerances do not.
#[cfg(test)]
pub fn window(_level: i32) -> usize {
    13
}

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
    pub crates: Vec<Rect>,
    /// the agent's feet
    pub agent: (f32, f32),
    pub foes: Vec<Person>,
    pub hostages: Vec<Person>,
    /// bullets for the level
    pub shots: i32,
    /// Shots for three stars; missed shots cost stars, not the whole attempt.
    pub par: i32,
}

/// Objects that absorb a shot.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Agent,
    Hostage(usize),
    Crate(usize),
}

/// A shot's way through the room: the points it turns at, and who it hits
/// how far along. It ends at the returning shooter, a hostage, a crate, or its range limit.
#[derive(Clone, Default)]
pub struct Path {
    pub pts: Vec<(f32, f32)>,
    /// each foe hit and the distance there
    pub foes: Vec<(usize, f32)>,
    /// A shooter, hostage or crate hit: the path ends there
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
        Room { blocks: Vec::new(), plates: Vec::new(), crates: Vec::new(), agent: (RIGHT / 2.0, FLOOR), foes: Vec::new(), hostages: Vec::new(), shots: 0, par: 1 }
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
    #[cfg(test)]
    pub fn trace(&self, angle: f32) -> Path {
        self.trace_broken(angle, 0)
    }

    pub fn trace_broken(&self, angle: f32, broken: u32) -> Path {
        self.trace_with(&self.walls(), angle, broken)
    }

    fn trace_with(&self, walls: &[((f32, f32), (f32, f32))], angle: f32, broken: u32) -> Path {
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
            // Only the outgoing segment ignores the shooter at the muzzle.
            // Every reflected segment can hit the agent.
            let agent = Person { x: self.agent.0, feet: self.agent.1, look: 0 };
            let mut stop = if bounce > 0 { agent.rect().enter(p, q).map(|t| (Hit::Agent, t)) } else { None };
            for (who, r) in self
                .hostages
                .iter()
                .enumerate()
                .map(|(i, h)| (Hit::Hostage(i), h.rect()))
                .chain(self.crates.iter().enumerate().filter(|(i, _)| broken & (1 << i) == 0).map(|(i, r)| (Hit::Crate(i), *r)))
            {
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
    #[cfg(test)]
    pub fn step_angle(k: usize) -> f32 {
        std::f32::consts::TAU * k as f32 / ANGLES as f32
    }

    /// Each shot that hits someone, by aim step: the foes it hits as
    /// bits, and whether it hits a hostage or crate; and which foes a straight shot
    /// reaches.
    #[cfg(test)]
    fn survey(&self) -> (Vec<(u32, bool)>, u32) {
        let walls = self.walls();
        let mut direct = 0;
        let shots = (0..ANGLES)
            .map(|k| {
                let path = self.trace_with(&walls, Room::step_angle(k), 0);
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

    /// The shared target mask and whether any aim in the window ends in
    /// harm. A harmful return is usable only as the campaign's final hit:
    /// play ends immediately when the last living foe goes down.
    #[cfg(test)]
    fn steady(survey: &[(u32, bool)], k: usize, w: usize) -> Option<(u32, bool)> {
        let mut mask = u32::MAX;
        let mut harm = false;
        for j in 0..w {
            let (hits, blocked) = survey[(k + j) % survey.len()];
            mask &= hits;
            harm |= blocked;
        }
        (mask != 0).then_some((mask, harm))
    }

    #[cfg(test)]
    fn good_shots(survey: &[(u32, bool)], w: usize) -> Vec<(u32, bool)> {
        let mut out = Vec::new();
        for shot in (0..survey.len()).filter_map(|k| Room::steady(survey, k, w)) {
            if !out.contains(&shot) {
                out.push(shot);
            }
        }
        out
    }

    #[cfg(test)]
    pub fn best_shot(&self, down: u32, w: usize) -> Option<f32> {
        let (survey, _) = self.survey();
        let good = Room::good_shots(&survey, w);
        (0..survey.len())
            .filter_map(|k| Room::steady(&survey, k, w).filter(|&(m, h)| m & !down != 0 && (!h || down | m == self.all_foes())).map(|(m, _)| (m, k)))
            .filter_map(|(m, k)| Room::shots_needed(&good, self.all_foes(), down | m).map(|left| (left, k)))
            .min()
            .map(|(_, k)| Room::step_angle(k + w / 2))
    }

    #[cfg(test)]
    fn shots_needed(good: &[(u32, bool)], all: u32, down: u32) -> Option<i32> {
        if down == all {
            return Some(0);
        }
        good.iter().filter(|&&(m, h)| m & !down != 0 && (!h || down | m == all)).filter_map(|&(m, _)| Room::shots_needed(good, all, down | m)).min().map(|n| n + 1)
    }
}

/// Authored lessons following Mr Bullet's core progression: a clear shot,
/// a line of targets, stairs, then bank shots and finally hostage avoidance.
/// Coordinates are original. Every entry has a fixed four-shot allowance.
pub fn level(level: i32) -> Room {
    if level >= 30 {
        return extended(level);
    }
    let mut r = Room { agent: (35.0, FLOOR), shots: 4, ..Room::empty() };
    let (blocks, plates, foes, hostages, agent, par): (&[(f32, f32, f32, f32)], &[(f32, f32, f32, f32)], &[(f32, f32)], &[(f32, f32)], (f32, f32), i32) = match level {
        0 => (&[], &[], &[(235., 352.)], &[], (35., 352.), 1),
        1 => (&[], &[], &[(180., 352.), (255., 352.)], &[], (35., 352.), 1),
        2 => (&[(120., 312., 60., 40.), (180., 272., 60., 80.), (240., 232., 60., 120.)], &[], &[(145., 312.), (205., 272.), (265., 232.)], &[], (65., 352.), 1),
        3 => (&[], &[], &[(250., 352.)], &[], (35., 352.), 1),
        4 => (&[(0., 272., 70., 80.), (215., 272., 85., 80.)], &[], &[(250., 272.), (150., 352.)], &[], (35., 272.), 2),
        5 => (&[(135., 252., 25., 100.)], &[], &[(240., 352.)], &[], (35., 352.), 1),
        6 => (&[(130., 282., 25., 70.)], &[], &[(210., 352.), (265., 352.)], &[], (35., 352.), 2),
        7 => (&[(115., 222., 25., 130.), (200., 282., 100., 70.)], &[], &[(250., 282.)], &[], (35., 352.), 1),
        8 => (&[(120., 48., 25., 160.)], &[], &[(230., 352.), (265., 352.)], &[], (35., 352.), 1),
        9 => (&[(125., 242., 30., 110.), (215., 212., 85., 12.)], &[], &[(250., 212.), (250., 352.)], &[], (35., 352.), 2),
        10 => (&[], &[], &[(240., 352.)], &[(140., 352.)], (35., 352.), 1),
        11 => (&[(210., 252., 90., 100.)], &[], &[(250., 252.)], &[(155., 352.)], (35., 352.), 1),
        12 => (&[(0., 232., 75., 120.), (210., 272., 90., 12.)], &[], &[(250., 272.), (155., 352.)], &[(250., 352.)], (35., 232.), 2),
        13 => (&[(125., 252., 25., 100.)], &[], &[(240., 352.)], &[(90., 352.)], (35., 352.), 1),
        14 => (&[(120., 48., 25., 150.), (215., 252., 85., 12.)], &[], &[(250., 252.), (250., 352.)], &[(155., 352.)], (35., 352.), 2),
        15 => (&[(125., 262., 25., 90.)], &[(140., 172., 220., 188.)], &[(240., 352.)], &[], (35., 352.), 1),
        16 => (&[(125., 302., 25., 50.), (210., 232., 90., 12.)], &[(140., 165., 220., 195.)], &[(250., 232.), (180., 352.)], &[], (35., 352.), 2),
        17 => (&[(110., 302., 25., 50.), (220., 232., 80., 120.)], &[(140., 165., 220., 195.)], &[(255., 232.), (180., 352.)], &[], (35., 352.), 2),
        18 => (&[(0., 232., 70., 120.), (125., 252., 25., 100.)], &[(210., 112., 270., 172.)], &[(240., 352.), (180., 352.)], &[], (35., 232.), 2),
        19 => (&[(125., 262., 25., 90.)], &[(140., 172., 220., 188.)], &[(255., 352.)], &[(85., 352.)], (35., 352.), 1),
        // Crossfire balcony: aim to both sides, protect the civilian below,
        // and bank around the upper-right ledge. No staircase sweep.
        20 => (&[(135., 220., 30., 12.), (220., 170., 80., 12.)], &[], &[(40., 352.), (260., 352.), (260., 170.)], &[(150., 352.)], (150., 220.), 3),
        21 => (&[(0., 192., 75., 160.), (220., 252., 80., 100.)], &[], &[(250., 252.), (140., 352.), (190., 352.)], &[], (35., 192.), 2),
        22 => (&[(120., 302., 25., 50.), (240., 212., 60., 140.)], &[], &[(265., 212.), (175., 352.), (215., 352.)], &[], (35., 352.), 3),
        23 => (&[(0., 252., 75., 100.), (135., 292., 25., 60.), (225., 232., 75., 12.)], &[], &[(250., 232.), (200., 352.), (265., 352.)], &[(110., 352.)], (35., 252.), 2),
        24 => (&[(100., 48., 25., 130.), (200., 262., 25., 90.)], &[], &[(160., 352.), (260., 352.)], &[(60., 352.)], (35., 252.), 2),
        25 => {
            (&[(0., 252., 70., 100.), (110., 292., 25., 60.), (205., 232., 95., 12.)], &[(120., 155., 180., 95.)], &[(250., 232.), (170., 352.), (265., 352.)], &[], (35., 252.), 2)
        }
        26 => (
            &[(125., 302., 25., 50.), (210., 232., 90., 12.), (0., 192., 60., 12.), (65., 110., 50., 12.)],
            &[(140., 165., 220., 195.)],
            &[(250., 232.), (180., 352.)],
            &[(90., 110.), (30., 192.)],
            (35., 352.),
            2,
        ),
        27 => (&[(0., 172., 75., 180.), (130., 272., 30., 80.), (250., 232., 50., 120.)], &[], &[(275., 232.), (100., 352.), (190., 352.), (230., 352.)], &[], (35., 172.), 3),
        28 => (
            &[(100., 292., 40., 60.), (175., 232., 40., 120.), (250., 172., 50., 180.)],
            &[(20., 125., 80., 75.)],
            &[(120., 292.), (195., 232.), (275., 172.)],
            &[(155., 352.)],
            (35., 352.),
            2,
        ),
        29 => (
            &[(0., 252., 75., 100.), (135., 302., 25., 50.), (225., 212., 75., 12.), (0., 110., 70., 12.)],
            &[(125., 145., 185., 85.)],
            &[(250., 212.), (200., 352.), (265., 352.)],
            &[(110., 352.), (30., 110.)],
            (35., 252.),
            3,
        ),
        _ => panic!("invalid Bankshot level"),
    };
    r.blocks = blocks.iter().map(|&(x, y, w, h)| Rect { x, y, w, h }).collect();
    r.plates = plates.iter().map(|&(x, y, u, v)| Plate { a: (x, y), b: (u, v) }).collect();
    r.foes = foes.iter().enumerate().map(|(i, &(x, feet))| Person { x, feet, look: (i % 3) as u8 }).collect();
    r.hostages = hostages.iter().map(|&(x, feet)| Person { x, feet, look: 0 }).collect();
    r.agent = agent;
    if agent.1 != FLOOR {
        if let Some(b) = r.blocks.iter().find(|b| b.y == agent.1 && b.x == 0.0) {
            r.agent.0 = b.x + b.w - HALF_W;
        }
    }
    r.par = par;
    if level == 3 {
        r.crates.push(Rect { x: 130.0, y: 292.0, w: 46.0, h: 60.0 });
    }
    // The agent also needs a visible platform on this balcony lesson.
    if level == 24 {
        r.blocks.push(Rect { x: 0.0, y: 252.0, w: 50.0, h: 12.0 });
    }
    r
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
            let need = Room::shots_needed(&Room::good_shots(&survey, window(l)), all, 0).expect("unclearable");
            assert!(need <= room.par, "level {} needs {} shots but par is {}", l + 1, need, room.par);
            assert_eq!(room.shots, 4);
            assert!(room.par < room.shots);
            if l < 3 {
                assert_eq!(direct, all, "opening lesson must be readable");
            }
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
    fn campaign_has_distinct_supported_readable_layouts() {
        let mut layouts = std::collections::HashSet::new();
        let mut failures = Vec::new();
        for l in 0..LEVELS {
            let r = level(l);
            let agent = Person { x: r.agent.0, feet: r.agent.1, look: 0 };
            let people: Vec<_> = r.foes.iter().chain(&r.hostages).chain(std::iter::once(&agent)).collect();
            for (i, p) in people.iter().enumerate() {
                let rect = p.rect();
                assert!(rect.x >= LEFT && rect.x + rect.w <= RIGHT && rect.y >= TOP && p.feet <= FLOOR, "level {} person out of room", l + 1);
                assert!(!r.blocks.iter().chain(&r.crates).any(|b| b.overlaps(&rect)), "level {} person inside cover", l + 1);
                assert!(p.feet == FLOOR || r.blocks.iter().any(|b| b.y == p.feet && b.x <= rect.x && b.x + b.w >= rect.x + rect.w), "level {} person unsupported", l + 1);
                assert!(!people[..i].iter().any(|q| q.rect().overlaps(&rect)), "level {} people overlap", l + 1);
            }
            let key = format!("{:?}{:?}{:?}{:?}{:?}{:?}", r.blocks, r.plates, r.crates, r.agent, r.foes, r.hostages);
            assert!(layouts.insert(key), "duplicate level {}", l + 1);
            // A solution must not rely on tracing a long, unreadable chain of rebounds.
            let (mut survey, _) = r.survey();
            for (k, item) in survey.iter_mut().enumerate() {
                let p = r.trace(Room::step_angle(k));
                let readable_distance: f32 = p.pts.windows(2).take(3).map(|w| dist(w[0], w[1])).sum();
                item.0 = p.foes.iter().filter(|&&(_, d)| d <= readable_distance).fold(0, |m, &(i, _)| m | 1 << i);
            }
            let need = Room::shots_needed(&Room::good_shots(&survey, window(l)), r.all_foes(), 0);
            if !need.is_some_and(|n| n <= r.par) {
                failures.push((l + 1, need));
            }
        }
        assert!(failures.is_empty(), "levels needing unreadable ricochets: {:?}", failures);
    }

    #[test]
    fn wooden_cover_absorbs_one_bullet_then_opens_a_path() {
        let r = level(3);
        let first = r.trace(0.0);
        assert_eq!(first.hit, Some(Hit::Crate(0)));
        assert_eq!(first.mask(), 0);
        let second = r.trace_broken(0.0, 1);
        assert_eq!(second.mask(), r.all_foes());
        assert_eq!(second.hit, Some(Hit::Agent));
    }

    #[test]
    fn a_shot_bounces_like_a_mirror() {
        let room = Room { agent: (30.0, FLOOR), shots: 4, ..Room::empty() };
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
    fn returning_bullets_hit_the_shooter_after_a_bounce() {
        let room = Room { agent: (30.0, FLOOR), shots: 4, ..Room::empty() };
        for angle in [0.0, std::f32::consts::FRAC_PI_2, -std::f32::consts::FRAC_PI_2] {
            let path = room.trace(angle);
            assert_eq!(path.hit, Some(Hit::Agent));
            assert!(path.pts.len() >= 3);
            assert!(path.len > 20.0, "outgoing shot must leave the muzzle");
        }
    }

    #[test]
    fn a_cut_shot_ends_there() {
        let foe = Person { x: 200.0, feet: FLOOR, look: 0 };
        let room = Room { agent: (30.0, FLOOR), foes: vec![foe], shots: 4, ..Room::empty() };
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
        let room = Room { agent: (30.0, FLOOR), foes: vec![foe], hostages: vec![hostage], shots: 4, ..Room::empty() };
        let p = room.trace(0.0);
        assert_eq!(p.hit, Some(Hit::Hostage(0)));
        assert!(p.foes.is_empty());
        assert!((p.len - (150.0 - HALF_W - 30.0)).abs() < 0.1);
    }
}
