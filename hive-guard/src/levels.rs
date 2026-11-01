//! The levels: ground and ledges, where the dog sits, the hives, the ink
//! and the bees.

pub const LEVELS: i32 = 100;
include!("campaign.rs");
/// the ground's top, plan units
pub const GROUND: f32 = 352.0;
/// plan units to a model unit for the dog and the hives, and a bee's height
pub const DOG_U: f32 = 35.0;
pub const HIVE_U: f32 = 34.0;
pub const BEE_H: f32 = 15.0;
/// the circle bees must not reach: the dog's middle and radius
pub const DOG_R: f32 = 24.0;
const DOG_UP: f32 = 25.0;
/// a hive's half width and height
const HIVE_W: f32 = 21.0;
const HIVE_H: f32 = 44.0;
/// Reference arch for the opening lessons.
#[cfg(test)]
pub const DOME_R: f32 = 58.0;

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

const fn r(x0: f32, y0: f32, x1: f32, y1: f32) -> Rect {
    Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
}

#[derive(Clone)]
pub struct Level {
    /// the ground and the ledges
    pub solids: Vec<Rect>,
    /// the dog's feet, and whether it faces left
    pub dog: (f32, f32, bool),
    /// each hive's foot
    pub hives: Vec<(f32, f32)>,
    /// how long a line can be, plan units
    pub ink: f32,
    /// bees from each hive, and how long they fly, seconds
    pub bees: u32,
    pub secs: f32,
}

/// The circle at p of radius rad against a box: the way out and how deep
/// it is in.
fn rect_contact(b: &Rect, p: (f32, f32), rad: f32) -> Option<((f32, f32), f32)> {
    let cx = p.0.clamp(b.x, b.x + b.w);
    let cy = p.1.clamp(b.y, b.y + b.h);
    let (dx, dy) = (p.0 - cx, p.1 - cy);
    let d2 = dx * dx + dy * dy;
    if d2 > 0.0 {
        if d2 >= rad * rad {
            return None;
        }
        let d = d2.sqrt();
        return Some(((dx / d, dy / d), rad - d));
    }
    // the middle is inside: out the nearest side
    let sides = [(p.0 - b.x, (-1.0, 0.0)), (b.x + b.w - p.0, (1.0, 0.0)), (p.1 - b.y, (0.0, -1.0)), (b.y + b.h - p.1, (0.0, 1.0))];
    let (gap, n) = sides.into_iter().min_by(|a, b| a.0.total_cmp(&b.0)).unwrap();
    Some((n, gap + rad))
}

impl Level {
    /// the dog's circle: middle and radius
    pub fn dog_circle(&self) -> (f32, f32, f32) {
        (self.dog.0, self.dog.1 - DOG_UP, DOG_R)
    }
    fn hive_rects(&self) -> impl Iterator<Item = Rect> + '_ {
        self.hives.iter().map(|&(x, y)| r(x - HIVE_W, y - HIVE_H, x + HIVE_W, y))
    }
    /// Where a hive's bees come out.
    pub fn door(&self, h: usize) -> (f32, f32) {
        let (x, y) = self.hives[h];
        (x, y - 6.0)
    }
    /// The deepest overlap of a circle with the ground, the ledges, the
    /// hives and (with `dog`) the dog: the way out and the depth.
    pub fn contact(&self, p: (f32, f32), rad: f32, dog: bool, hives: bool) -> Option<((f32, f32), f32)> {
        let mut best: Option<((f32, f32), f32)> = None;
        let mut keep = |c: Option<((f32, f32), f32)>| {
            if let Some(c) = c {
                if best.is_none_or(|b| c.1 > b.1) {
                    best = Some(c);
                }
            }
        };
        for b in &self.solids {
            keep(rect_contact(b, p, rad));
        }
        if hives {
            for b in self.hive_rects() {
                keep(rect_contact(&b, p, rad));
            }
        }
        if dog {
            let (cx, cy, cr) = self.dog_circle();
            let (dx, dy) = (p.0 - cx, p.1 - cy);
            let d = (dx * dx + dy * dy).sqrt();
            if d < cr + rad {
                keep(Some(if d > 0.01 { ((dx / d, dy / d), cr + rad - d) } else { ((0.0, -1.0), cr + rad) }));
            }
        }
        best
    }
    /// whether a circle there overlaps anything the line cannot cross
    pub fn solid(&self, p: (f32, f32), rad: f32) -> bool {
        self.contact(p, rad, true, true).is_some()
    }
}

const GROUND_RECT: Rect = r(-200.0, GROUND, 500.0, 520.0);

/// The three open-air drawing lessons.
fn layout(k: i32) -> (Vec<Rect>, (f32, f32, bool), Vec<(f32, f32)>) {
    match k {
        // a meadow, the hive to the left
        0 => (vec![], (165.0, GROUND, true), vec![(60.0, GROUND)]),
        // the hive up on a ledge to the right
        1 => (vec![r(196.0, 160.0, 320.0, 176.0)], (110.0, GROUND, false), vec![(252.0, 160.0)]),
        // the dog on a ledge, the hive below
        2 => (vec![r(80.0, 236.0, 220.0, 252.0)], (150.0, 236.0, true), vec![(40.0, GROUND)]),
        _ => unreachable!("campaign level"),
    }
}

/// Terrain puzzles after three free-drawing lessons. A short bridge,
/// sliding gate, paired roof patches, roof-and-gate or two gates is
/// required; the ink cannot encircle the dog independently of the terrain.
fn challenge(n: i32, stroke: impl FnMut(&[(f32, f32)])) -> Level {
    // type, dog x, floor y, half-width, wall height. Explicit authored order
    // alternates lessons and combinations instead of ten repeating arenas.
    let (kind, x, y, w, h): (u8, f32, f32, f32, f32) = match n {
        3 => (0, 150., 352., 40., 68.),
        4 => (0, 110., 290., 46., 72.),
        5 => (1, 155., 352., 42., 70.),
        6 => (0, 180., 250., 50., 78.),
        7 => (1, 110., 280., 44., 76.),
        8 => (2, 150., 352., 48., 74.),
        9 => (0, 145., 300., 54., 80.),
        10 => (1, 182., 310., 46., 82.),
        11 => (2, 112., 270., 46., 80.),
        12 => (3, 150., 352., 44., 72.),
        13 => (2, 180., 290., 50., 86.),
        14 => (4, 150., 320., 44., 78.),
        15 => (3, 110., 286., 46., 76.),
        16 => (1, 150., 242., 48., 88.),
        17 => (4, 184., 352., 46., 84.),
        18 => (2, 150., 240., 52., 78.),
        19 => (3, 180., 310., 48., 82.),
        20 => (4, 110., 286., 48., 86.),
        21 => (2, 110., 352., 54., 90.),
        22 => (3, 150., 270., 50., 88.),
        23 => (4, 178., 260., 50., 80.),
        24 => (2, 175., 330., 56., 94.),
        25 => (3, 110., 330., 52., 92.),
        26 => (4, 150., 352., 52., 94.),
        27 => (3, 170., 250., 54., 84.),
        28 => (4, 112., 244., 54., 88.),
        29 => (5, 150., 320., 56., 96.),
        30.. => EXTRA[(n - 30) as usize],
        _ => unreachable!(),
    };
    build(n, (kind, x, y, w, h), stroke)
}

pub(super) fn build(n: i32, (kind, x, y, w, h): (u8, f32, f32, f32, f32), mut stroke: impl FnMut(&[(f32, f32)])) -> Level {
    let (l, rr, top) = (x - w, x + w, y - h);
    let mut solids = vec![GROUND_RECT, r(l - 14., y, rr + 14., y + 16.)];
    let side = |solids: &mut Vec<Rect>, xx: f32, door: bool| {
        if door {
            solids.push(r(xx - 9., top, xx + 9., y - 47.));
            solids.push(r(xx - 9., y - 27., xx + 9., y));
        } else {
            solids.push(r(xx - 7., top, xx + 7., y));
        }
    };
    side(&mut solids, l, kind == 4 || kind >= 6);
    side(&mut solids, rr, kind == 1 || kind >= 3);
    if kind == 1 || kind == 4 {
        solids.push(r(l - 7., top - 12., rr + 7., y - 52.));
    }
    if kind == 2 || kind == 5 || kind == 7 {
        solids.extend([r(l - 7., top, l + 14., top + 12.), r(rr - 14., top, rr + 7., top + 12.)]);
        solids.push(r(x - 20., top, x + 20., y - 52.));
        solids.extend([r(l - 23., top - 12., l - 19., top + 4.), r(x - 3., top - 12., x + 3., top), r(rr + 19., top - 12., rr + 23., top + 4.)]);
        stroke(&[(l - 10., top - 6.), (x - 8., top - 6.)]);
        stroke(&[(x + 8., top - 6.), (rr + 10., top - 6.)]);
    } else if kind == 0 || kind == 3 || kind == 6 {
        // Offset roof opening: a short plank must bridge the actual gap.
        let gap = if kind == 0 && n % 2 == 0 { x + 8. } else { x - 30. };
        solids.push(r(l - 7., top, gap, y - 52.));
        solids.push(r(gap + 28., top, rr + 7., y - 52.));
        solids.extend([r(gap - 33., top - 12., gap - 29., top), r(gap + 57., top - 12., gap + 61., top)]);
        stroke(&[(gap - 20., top - 6.), (gap + 48., top - 6.)]);
    }
    if kind == 1 || kind >= 3 {
        solids.extend([r(rr + 7., y, rr + 27., y + 16.), r(rr + 21., y - 27., rr + 27., y), r(rr + 21., y - 100., rr + 27., y - 47.)]);
        stroke(&[(rr + 14., y - if kind == 5 { 76. } else { 85. }), (rr + 14., y - 4.)]);
    }
    if kind == 4 || kind >= 6 {
        solids.extend([r(l - 27., y, l - 7., y + 16.), r(l - 27., y - 27., l - 21., y), r(l - 27., y - 100., l - 21., y - 47.)]);
        stroke(&[(l - 14., y - 85.), (l - 14., y - 4.)]);
    }
    let length = match kind {
        0 => 68.,
        1 => 81.,
        2 => 2. * (w + 2.),
        3 => 149.,
        4 => 162.,
        5 => 2. * (w + 2.) + 72.,
        6 => 230.,
        7 => 2. * (w + 2.) + 162.,
        _ => unreachable!(),
    };
    let hives = if n < 8 {
        vec![(if x < 150. { 265. } else { 35. }, GROUND)]
    } else if n < 23 {
        vec![(30., GROUND), (270., GROUND)]
    } else {
        vec![(30., GROUND), (270., GROUND), (150., 110.)]
    };
    if n >= 23 {
        solids.push(r(119., 110., 181., 124.));
    }
    let ink = length + if n < 12 { 8. } else { 4. };
    Level { solids, dog: (x, y, n % 2 == 0), hives, ink, bees: 8 + (n.min(29) / 6) as u32, secs: 8. + (n.min(29) / 10) as f32 }
}

pub fn level(n: i32) -> Level {
    if n >= 3 {
        return challenge(n, |_| {});
    }
    let (mut solids, dog, hives) = layout(n);
    solids.insert(0, GROUND_RECT);
    Level { solids, dog, hives, ink: 300. - n as f32 * 20., bees: 10, secs: 8. }
}

#[cfg(test)]
pub fn solution(n: i32) -> Vec<Vec<(f32, f32)>> {
    let mut strokes = Vec::new();
    challenge(n, |s| strokes.push(s.to_vec()));
    strokes
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn thirty_distinct_terrains_with_supported_dogs() {
        let mut seen = std::collections::HashSet::new();
        for n in 0..LEVELS {
            let lv = level(n);
            let terrain: Vec<_> = lv.solids.iter().map(|r| (r.x as i32, r.y as i32, r.w as i32, r.h as i32)).collect();
            let hives: Vec<_> = lv.hives.iter().map(|&(x, y)| (x as i32, y as i32)).collect();
            assert!(seen.insert((terrain, hives, lv.dog.0 as i32, lv.dog.1 as i32)), "repeated terrain {}", n + 1);
            assert!(lv.solids.iter().any(|r| r.y == lv.dog.1 && lv.dog.0 > r.x && lv.dog.0 < r.x + r.w));
            if n >= 3 {
                assert!(lv.ink < 300.);
            }
        }
    }
}
