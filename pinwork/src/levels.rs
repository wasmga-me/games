//! The levels: a dungeon's stone walls and pins, what fills its chambers,
//! where the knight and the orc stand, and where the gold must land.

pub const LEVELS: i32 = 24;
pub const LAYOUTS: i32 = 8;

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

const LAYOUT: [Layout; LAYOUTS as usize] = [
    // one pin over the knight
    Layout {
        walls: &[(110.0, 90.0, 110.0, 226.0), (190.0, 90.0, 190.0, 226.0)],
        pins: &[(104.0, 232.0, 222.0, 232.0)],
        fills: &[(GOLD, 117.0, 170.0, 183.0, 224.0)],
        knight: 150.0,
        orc: None,
        zone: (FRAME_L, 290.0, FRAME_R, FLOOR),
    },
    // gold and lava side by side: only the gold
    Layout {
        walls: &[(150.0, 70.0, 150.0, 222.0)],
        pins: &[(156.0, 228.0, 0.0, 228.0), (144.0, 228.0, 300.0, 228.0)],
        fills: &[(GOLD, 40.0, 190.0, 140.0, 222.0), (LAVA, 170.0, 190.0, 270.0, 222.0)],
        knight: 70.0,
        orc: None,
        zone: (FRAME_L, 290.0, FRAME_R, FLOOR),
    },
    // lava on the gold, water above: the water first
    Layout {
        walls: &[(90.0, 60.0, 90.0, 256.0), (210.0, 60.0, 210.0, 256.0)],
        pins: &[(84.0, 130.0, 236.0, 130.0), (216.0, 262.0, 64.0, 262.0)],
        fills: &[(WATER, 96.0, 80.0, 204.0, 124.0), (LAVA, 96.0, 184.0, 204.0, 220.0), (GOLD, 96.0, 224.0, 204.0, 256.0)],
        knight: 150.0,
        orc: None,
        zone: (FRAME_L, 290.0, FRAME_R, FLOOR),
    },
    // an orc behind a gate, lava above it: keep the gate shut
    Layout {
        walls: &[(120.0, 70.0, 120.0, 206.0), (190.0, 70.0, 190.0, 206.0)],
        pins: &[(126.0, 212.0, 0.0, 212.0), (184.0, 212.0, 300.0, 212.0), (150.0, 382.0, 150.0, 300.0)],
        fills: &[(GOLD, 30.0, 166.0, 112.0, 206.0), (LAVA, 196.0, 166.0, 280.0, 206.0)],
        knight: 60.0,
        orc: Some((232.0, GROUND)),
        zone: (FRAME_L, 290.0, 150.0, FLOOR),
    },
    // water down a chute onto the lava, then the rest
    Layout {
        walls: &[(90.0, 60.0, 90.0, 270.0), (210.0, 60.0, 210.0, 146.0), (210.0, 196.0, 210.0, 270.0), (281.0, 166.0, 216.0, 190.0)],
        pins: &[(216.0, 150.0, 64.0, 150.0), (84.0, 276.0, 236.0, 276.0), (204.0, 152.0, 300.0, 152.0)],
        fills: &[(GOLD, 100.0, 110.0, 200.0, 144.0), (WATER, 216.0, 106.0, 280.0, 146.0), (LAVA, 96.0, 238.0, 204.0, 270.0)],
        knight: 150.0,
        orc: None,
        zone: (FRAME_L, 290.0, FRAME_R, FLOOR),
    },
    // a slanted pin turns the lava to the orc: leave it in
    Layout {
        walls: &[(120.0, 60.0, 120.0, 150.0), (200.0, 60.0, 200.0, 150.0), (100.0, 60.0, 100.0, 160.0), (190.0, 330.0, 190.0, 379.0), (108.0, 172.0, 108.0, 200.0)],
        pins: &[(114.0, 156.0, 226.0, 156.0), (112.0, 200.0, 262.0, 262.0), (106.0, 166.0, 0.0, 166.0)],
        fills: &[(LAVA, 126.0, 96.0, 194.0, 150.0), (GOLD, 24.0, 120.0, 94.0, 160.0)],
        knight: 60.0,
        orc: Some((262.0, GROUND)),
        zone: (FRAME_L, 290.0, 190.0, FLOOR),
    },
    // water over the lava over the orc: the water would save it
    Layout {
        walls: &[(190.0, 60.0, 190.0, 236.0), (120.0, 60.0, 120.0, 206.0), (150.0, 320.0, 150.0, 379.0)],
        pins: &[(126.0, 212.0, 0.0, 212.0), (184.0, 236.0, 300.0, 236.0), (184.0, 140.0, 300.0, 140.0)],
        fills: &[(GOLD, 30.0, 166.0, 112.0, 206.0), (LAVA, 196.0, 184.0, 280.0, 228.0), (WATER, 196.0, 84.0, 280.0, 134.0)],
        knight: 60.0,
        orc: Some((235.0, GROUND)),
        zone: (FRAME_L, 290.0, 150.0, FLOOR),
    },
    // an orc on a pin above the knight, lava above the orc
    Layout {
        walls: &[(100.0, 150.0, 100.0, 256.0), (200.0, 150.0, 200.0, 256.0), (100.0, 60.0, 100.0, 140.0), (200.0, 60.0, 200.0, 140.0), (214.0, 60.0, 214.0, 190.0)],
        pins: &[(94.0, 256.0, 226.0, 256.0), (206.0, 146.0, 74.0, 146.0), (208.0, 196.0, 300.0, 196.0)],
        fills: &[(LAVA, 106.0, 104.0, 194.0, 140.0), (GOLD, 220.0, 146.0, 280.0, 190.0)],
        knight: 150.0,
        orc: Some((150.0, 252.5)),
        zone: (FRAME_L, 290.0, FRAME_R, FLOOR),
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

/// a grain's radius, plan units
pub const GRAIN_R: f32 = 4.0;

/// Level n: the layouts in turn, then mirrored, then as first with more
/// gold needed.
pub fn level(n: i32) -> Level {
    let round = n / LAYOUTS;
    let l = &LAYOUT[(n % LAYOUTS) as usize];
    let flip = round == 1;
    let fx = |x: f32| if flip { 300.0 - x } else { x };
    let seg = |s: &Seg| (fx(s.0), s.1, fx(s.2), s.3);
    let mut walls = vec![(FRAME_L, FRAME_TOP, FRAME_L, FLOOR), (FRAME_R, FRAME_TOP, FRAME_R, FLOOR), (FRAME_L, FLOOR, FRAME_R, FLOOR)];
    walls.extend(l.walls.iter().map(seg));
    let pins = l.pins.iter().map(seg).collect();
    let mut grains = Vec::new();
    let mut gold = 0;
    for &(kind, x0, y0, x1, y1) in l.fills {
        // packed in rows, every other row shifted half a grain
        let step = 2.0 * GRAIN_R;
        let mut y = y1 - GRAIN_R;
        let mut row = 0;
        while y >= y0 + GRAIN_R - 0.01 {
            let mut x = x0 + GRAIN_R + if row % 2 == 1 { GRAIN_R * 0.5 } else { 0.0 };
            while x <= x1 - GRAIN_R + 0.01 {
                grains.push((fx(x), y, kind));
                gold += (kind == GOLD) as usize;
                x += step;
            }
            y -= step * 0.9;
            row += 1;
        }
    }
    let share = [0.5, 0.55, 0.65][round as usize];
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
