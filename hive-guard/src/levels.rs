//! The levels: ground and ledges, where the dog sits, the hives, the ink
//! and the bees.

pub const LEVELS: i32 = 30;
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
/// the arch a dog needs over it, measured from its feet: what the levels'
/// ink is reckoned from
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

/// The ten layouts: ledges, the dog's feet and facing, the hives' feet.
fn layout(k: i32) -> (Vec<Rect>, (f32, f32, bool), Vec<(f32, f32)>) {
    match k {
        // a meadow, the hive to the left
        0 => (vec![], (165.0, GROUND, true), vec![(60.0, GROUND)]),
        // the hive up on a ledge to the right
        1 => (vec![r(196.0, 160.0, 320.0, 176.0)], (110.0, GROUND, false), vec![(252.0, 160.0)]),
        // the dog on a ledge, the hive below
        2 => (vec![r(80.0, 236.0, 220.0, 252.0)], (150.0, 236.0, true), vec![(40.0, GROUND)]),
        // against a cliff on the right, the hive up on the left
        3 => (vec![r(232.0, 200.0, 320.0, GROUND), r(-20.0, 122.0, 96.0, 138.0)], (182.0, GROUND, false), vec![(46.0, 122.0)]),
        // down between two posts, the hive above
        4 => (vec![r(52.0, 290.0, 76.0, GROUND), r(224.0, 290.0, 248.0, GROUND), r(110.0, 128.0, 190.0, 144.0)], (150.0, GROUND, false), vec![(150.0, 128.0)]),
        // two hives, either side
        5 => (vec![r(200.0, 184.0, 320.0, 200.0)], (130.0, GROUND, false), vec![(38.0, GROUND), (256.0, 184.0)]),
        // up on a shelf to the right, the hive down on the left
        6 => (vec![r(156.0, 176.0, 320.0, 192.0)], (232.0, 176.0, true), vec![(60.0, GROUND)]),
        // under a roof, the hive to the right
        7 => (vec![r(92.0, 236.0, 216.0, 250.0)], (150.0, GROUND, false), vec![(264.0, GROUND)]),
        // on a step, hives low left and high right
        8 => (vec![r(-20.0, 262.0, 92.0, GROUND), r(92.0, 304.0, 230.0, GROUND), r(206.0, 126.0, 320.0, 142.0)], (160.0, 304.0, true), vec![(40.0, 262.0), (258.0, 126.0)]),
        // on an island in the air, hives either side on the ground
        _ => (vec![r(78.0, 238.0, 222.0, 254.0)], (150.0, 238.0, false), vec![(36.0, GROUND), (264.0, GROUND)]),
    }
}

/// Level n: the layouts in turn, three rounds of less ink and more bees.
pub fn level(n: i32) -> Level {
    let (mut solids, dog, hives) = layout(n % 10);
    solids.insert(0, GROUND_RECT);
    let round = n / 10;
    // an arch over the dog, with some to spare that shrinks round by round
    let arch = std::f32::consts::PI * DOME_R;
    let ink = arch * [1.7, 1.4, 1.18][round as usize] + 10.0;
    // the bees shared between the hives
    let total = (12 + 6 * round + (n % 10) / 3) as u32;
    let bees = total.div_ceil(hives.len() as u32);
    Level { solids, dog, hives, ink, bees, secs: [8.0, 10.0, 12.0][round as usize] }
}
