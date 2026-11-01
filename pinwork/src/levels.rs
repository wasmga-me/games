//! The levels: a dungeon's stone walls and pins, what fills its chambers,
//! where the knight and the orc stand, and where the gold must land.

pub const LEVELS: i32 = 2 * LAYOUT.len() as i32;

/// what a grain is
pub const GOLD: u8 = 0;
pub const LAVA: u8 = 1;
pub const WATER: u8 = 2;
pub const STONE: u8 = 3;

/// a wall's and a pin's half thickness, plan units
pub const WALL_R: f32 = 5.0;
pub const PIN_R: f32 = 3.5;
/// the frame round the dungeon: its sides and floor
pub const FRAME_L: f32 = 14.0;
pub const FRAME_R: f32 = 286.0;
pub const FRAME_TOP: f32 = 40.0;
pub const FLOOR: f32 = 384.0;
/// where people stand: the floor's top
pub const GROUND: f32 = FLOOR - WALL_R;

type Seg = (f32, f32, f32, f32);

struct Layout {
    walls: &'static [Seg],
    /// each pin from its tip to its handle: it slides out toward the handle
    pins: &'static [Seg],
    fills: &'static [(u8, f32, f32, f32, f32)],
    knight: f32,
    orc: Option<(f32, f32)>,
    /// the gold must land in here
    zone: (f32, f32, f32, f32),
}

const FLOORS: (f32, f32, f32, f32) = (FRAME_L, 290.0, FRAME_R, FLOOR);

/// The layouts, easiest first: played as built, then mirrored needing more
/// of their gold.
const LAYOUT: &[Layout] = &[
    // gold or lava, the lava over the knight
    Layout {
        walls: &[(150.0, 70.0, 150.0, 222.0)],
        pins: &[(156.0, 228.0, 0.0, 228.0), (144.0, 228.0, 300.0, 228.0)],
        fills: &[(GOLD, 30.0, 180.0, 140.0, 222.0), (LAVA, 160.0, 186.0, 278.0, 222.0)],
        knight: 215.0,
        orc: None,
        zone: FLOORS,
    },
    // lava on the gold, water above: the water first
    Layout {
        walls: &[(90.0, 60.0, 90.0, 256.0), (210.0, 60.0, 210.0, 256.0)],
        pins: &[(84.0, 130.0, 236.0, 130.0), (216.0, 262.0, 64.0, 262.0)],
        fills: &[(WATER, 96.0, 80.0, 204.0, 124.0), (LAVA, 96.0, 184.0, 204.0, 220.0), (GOLD, 96.0, 224.0, 204.0, 256.0)],
        knight: 150.0,
        orc: None,
        zone: FLOORS,
    },
    // the gold's pin is locked by the water tank's side; lava on the left
    Layout {
        walls: &[(110.0, 90.0, 110.0, 226.0), (190.0, 90.0, 190.0, 226.0), (80.0, 60.0, 80.0, 150.0), (222.0, 250.0, 286.0, 250.0)],
        pins: &[(104.0, 232.0, 204.0, 232.0), (227.0, 246.0, 227.0, 60.0), (86.0, 156.0, 0.0, 156.0)],
        fills: &[(GOLD, 117.0, 170.0, 183.0, 224.0), (WATER, 233.0, 175.0, 280.0, 244.0), (LAVA, 20.0, 100.0, 74.0, 150.0)],
        knight: 150.0,
        orc: None,
        zone: FLOORS,
    },
    // the gold's pin runs into the lava's: the lava first, onto the
    // orc, and the gate stays shut
    Layout {
        walls: &[(120.0, 70.0, 120.0, 206.0), (190.0, 70.0, 190.0, 206.0)],
        pins: &[(6.0, 212.0, 112.0, 212.0), (136.0, 212.0, 300.0, 212.0), (150.0, 382.0, 150.0, 300.0)],
        fills: &[(GOLD, 24.0, 166.0, 112.0, 206.0), (LAVA, 196.0, 166.0, 280.0, 206.0)],
        knight: 60.0,
        orc: Some((232.0, GROUND)),
        zone: (FRAME_L, 290.0, 150.0, FLOOR),
    },
    // the gold's pin is locked by the lava tank's side: water on the
    // lava first, then the side, then the gold
    Layout {
        walls: &[(100.0, 60.0, 100.0, 150.0), (200.0, 50.0, 200.0, 176.0), (120.0, 176.0, 205.0, 176.0), (125.0, 50.0, 125.0, 94.0)],
        pins: &[(6.0, 156.0, 96.0, 156.0), (125.0, 106.0, 125.0, 172.0), (120.0, 99.0, 206.0, 99.0)],
        fills: &[(GOLD, 20.0, 100.0, 94.0, 150.0), (WATER, 131.0, 60.0, 194.0, 94.0), (LAVA, 131.0, 120.0, 194.0, 170.0)],
        knight: 60.0,
        orc: None,
        zone: FLOORS,
    },
    // water down a chute onto the lava, then the rest
    Layout {
        walls: &[(90.0, 60.0, 90.0, 270.0), (210.0, 60.0, 210.0, 146.0), (210.0, 196.0, 210.0, 270.0), (281.0, 166.0, 216.0, 190.0)],
        pins: &[(216.0, 150.0, 64.0, 150.0), (84.0, 276.0, 236.0, 276.0), (204.0, 152.0, 300.0, 152.0)],
        fills: &[(GOLD, 100.0, 110.0, 200.0, 144.0), (WATER, 216.0, 106.0, 280.0, 146.0), (LAVA, 96.0, 238.0, 204.0, 270.0)],
        knight: 150.0,
        orc: None,
        zone: FLOORS,
    },
    // an orc on the gold over the knight: lava onto the orc, water onto
    // the lava, then the floor
    Layout {
        walls: &[(100.0, 50.0, 100.0, 256.0), (200.0, 50.0, 200.0, 256.0)],
        pins: &[(94.0, 96.0, 226.0, 96.0), (206.0, 146.0, 74.0, 146.0), (94.0, 256.0, 226.0, 256.0)],
        fills: &[(WATER, 106.0, 60.0, 194.0, 90.0), (LAVA, 106.0, 104.0, 194.0, 140.0), (GOLD, 106.0, 232.0, 194.0, 252.0)],
        knight: 150.0,
        orc: Some((150.0, 228.0)),
        zone: FLOORS,
    },
    // the gold's pin is locked by the orc's gate: lava onto the orc,
    // water onto the lava, then the gate and the gold
    Layout {
        walls: &[(110.0, 90.0, 110.0, 226.0), (190.0, 90.0, 190.0, 226.0), (222.0, 80.0, 222.0, 130.0), (222.0, 150.0, 222.0, 200.0)],
        pins: &[(104.0, 232.0, 204.0, 232.0), (227.0, 382.0, 227.0, 222.0), (216.0, 206.0, 300.0, 206.0), (216.0, 136.0, 300.0, 136.0)],
        fills: &[(GOLD, 117.0, 170.0, 183.0, 224.0), (LAVA, 230.0, 176.0, 280.0, 200.0), (WATER, 230.0, 96.0, 280.0, 130.0)],
        knight: 150.0,
        orc: Some((258.0, GROUND)),
        zone: FLOORS,
    },
];

#[derive(Clone)]
pub struct Level {
    /// walls and pins as segments, a pin from its tip to its handle
    pub walls: Vec<Seg>,
    pub pins: Vec<Seg>,
    /// each grain's place and kind
    pub grains: Vec<(f32, f32, u8)>,
    pub knight: f32,
    pub orc: Option<(f32, f32)>,
    pub zone: (f32, f32, f32, f32),
    /// gold coins the knight needs
    pub need: usize,
}

/// a coin's radius, plan units
pub const GRAIN_R: f32 = 4.0;
/// how far apart drops of water and lava sit at rest, and the size of
/// each one (and of a stone made from one) against walls and coins
pub const SPACING: f32 = 5.0;
pub const DROP_R: f32 = 2.5;
/// how far apart rows of drops start: packed as tight as they go
pub const ROW_RISE: f32 = SPACING * 0.866;

/// Level n, needing a larger share of its gold the further on it is.
pub fn level(n: i32) -> Level {
    let l = &LAYOUT[n as usize % LAYOUT.len()];
    let flip = n as usize >= LAYOUT.len();
    let fx = |x: f32| if flip { 300.0 - x } else { x };
    let seg = |s: &Seg| (fx(s.0), s.1, fx(s.2), s.3);
    let mut walls = vec![(FRAME_L, FRAME_TOP, FRAME_L, FLOOR), (FRAME_R, FRAME_TOP, FRAME_R, FLOOR), (FRAME_L, FLOOR, FRAME_R, FLOOR)];
    walls.extend(l.walls.iter().map(seg));
    let pins = l.pins.iter().map(seg).collect();
    let mut grains = Vec::new();
    let mut gold = 0;
    for &(kind, x0, y0, x1, y1) in l.fills {
        // packed in rows, every other row shifted half a coin or drop
        let (step, rise, shift) = if kind == GOLD { (2.0 * GRAIN_R, 1.8 * GRAIN_R, GRAIN_R * 0.5) } else { (SPACING, ROW_RISE, SPACING / 2.0) };
        let r = step / 2.0;
        let mut y = y1 - r;
        let mut row = 0;
        while y >= y0 + r - 0.01 {
            let mut x = x0 + r + if row % 2 == 1 { shift } else { 0.0 };
            while x <= x1 - r + 0.01 {
                grains.push((fx(x), y, kind));
                gold += (kind == GOLD) as usize;
                x += step;
            }
            y -= rise;
            row += 1;
        }
    }
    let share = 0.5 + 0.15 * n as f32 / (LEVELS - 1) as f32;
    let (zx0, zy0, zx1, zy1) = l.zone;
    let zone = if flip { (300.0 - zx1, zy0, 300.0 - zx0, zy1) } else { (zx0, zy0, zx1, zy1) };
    Level {
        walls,
        pins,
        grains,
        knight: fx(l.knight),
        orc: l.orc.map(|(x, y)| (fx(x), y)),
        zone,
        need: (gold as f32 * share).ceil() as usize,
    }
}
