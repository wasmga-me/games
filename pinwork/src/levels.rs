//! The levels: a dungeon's stone walls and pins, what fills its chambers,
//! where the knight and the orc stand, and where the gold must land.

pub const LEVELS: i32 = 100;
include!("campaign.rs");

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

struct Layout<'a> {
    walls: &'a [Seg],
    /// each pin from its tip to its handle: it slides out toward the handle
    pins: &'a [Seg],
    fills: &'a [(u8, f32, f32, f32, f32)],
    knight: f32,
    orc: Option<(f32, f32)>,
    /// the gold must land in here
    zone: (f32, f32, f32, f32),
}

const FLOORS: (f32, f32, f32, f32) = (FRAME_L, 290.0, FRAME_R, FLOOR);

/// Eight introductory chambers, eight Hero Rescue adaptations and four
/// original replacements for repetitive late layouts. See references.md for the source and the mechanical adaptations.
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
    // Hero Rescue 2: separate gold/lava pockets on the two arms of a V.
    Layout {
        walls: &[(150., 55., 150., 238.)],
        pins: &[(150., 236., 22., 160.), (150., 236., 278., 160.), (8., 294., 278., 294.)],
        fills: &[(GOLD, 35., 105., 137., 160.), (LAVA, 164., 120., 235., 154.)],
        knight: 90.,
        orc: None,
        zone: (FRAME_L, 340., FRAME_R, FLOOR),
    },
    // Hero Rescue 1: two upper reservoirs feed a shared cooling chamber.
    Layout {
        walls: &[(150., 55., 150., 144.), (80., 160., 80., 268.), (220., 160., 220., 268.)],
        pins: &[(156., 150., 22., 150.), (144., 150., 278., 150.), (74., 274., 246., 274.)],
        fills: &[(GOLD, 30., 100., 140., 142.), (WATER, 160., 80., 278., 142.), (LAVA, 88., 225., 212., 264.)],
        knight: 150.,
        orc: None,
        zone: (FRAME_L, 340., FRAME_R, FLOOR),
    },
    // Split treasure: release both pockets, then coolant, before opening the lower lake.
    Layout {
        walls: &[(104., 55., 104., 194.), (196., 55., 196., 194.)],
        pins: &[(110., 200., 22., 200.), (190., 200., 278., 200.), (98., 128., 222., 128.), (8., 286., 278., 286.)],
        fills: &[(GOLD, 24., 146., 94., 190.), (GOLD, 206., 146., 276., 190.), (WATER, 114., 60., 186., 120.), (LAVA, 26., 242., 274., 278.)],
        knight: 150.,
        orc: None,
        zone: (FRAME_L, 340., FRAME_R, FLOOR),
    },
    // Hero Rescue 4: a side reservoir must cool the lower tank before its exit opens.
    Layout {
        walls: &[(100., 50., 100., 164.), (200., 50., 200., 274.), (100., 210., 100., 274.), (14., 170., 94., 206.)],
        pins: &[(94., 156., 226., 156.), (106., 166., 22., 166.), (94., 280., 226., 280.)],
        fills: &[(GOLD, 110., 105., 190., 148.), (WATER, 22., 95., 90., 158.), (LAVA, 108., 236., 192., 268.)],
        knight: 150.,
        orc: None,
        zone: (FRAME_L, 340., FRAME_R, FLOOR),
    },
    // Hero Rescue 5: gold above a V of lava, with water released from the side.
    Layout {
        walls: &[(110., 50., 110., 124.), (190., 50., 190., 124.), (228., 80., 228., 168.)],
        pins: &[(104., 130., 216., 130.), (150., 270., 22., 200.), (150., 270., 278., 200.), (222., 174., 278., 174.)],
        fills: &[(GOLD, 118., 80., 182., 120.), (LAVA, 100., 195., 200., 222.), (WATER, 238., 104., 278., 164.)],
        knight: 150.,
        orc: None,
        zone: (FRAME_L, 340., FRAME_R, FLOOR),
    },
    // Hero Rescue 7: a central divider keeps the lava on its own side.
    Layout {
        walls: &[(150., 55., 150., 150.)],
        pins: &[(6., 156., 278., 156.), (150., 166., 150., 382.), (144., 300., 278., 300.)],
        fills: &[(LAVA, 24., 95., 140., 148.), (GOLD, 162., 95., 278., 148.)],
        knight: 230.,
        orc: None,
        zone: (156., 340., FRAME_R, FLOOR),
    },
    // Flooded treasure: release lava onto the monster, then flood the
    // chamber and sink the coins through its coolant before opening the exit.
    Layout {
        walls: &[(110., 50., 110., 172.), (200., 50., 200., 172.), (230., 90., 230., 190.), (286., 212., 204., 242.), (80., 244., 80., 310.), (220., 244., 220., 310.)],
        pins: &[(104., 114., 226., 114.), (206., 178., 94., 178.), (224., 196., 278., 196.), (74., 316., 246., 316.)],
        fills: &[(GOLD, 120., 60., 190., 106.), (WATER, 120., 122., 190., 168.), (LAVA, 240., 142., 276., 186.)],
        knight: 150.,
        orc: Some((150., 300.)),
        zone: (FRAME_L, 350., FRAME_R, FLOOR),
    },
    // A divider keeps two halves apart: join them to burn the monster,
    // then cool the joined tank and choose its treasure exits.
    Layout {
        walls: &[(100., 50., 100., 112.), (200., 50., 200., 112.)],
        pins: &[(94., 118., 226., 118.), (150., 280., 150., 156.), (156., 286., 22., 286.), (144., 286., 278., 286.)],
        fills: &[(WATER, 110., 58., 190., 108.), (LAVA, 24., 236., 140., 276.), (GOLD, 28., 196., 130., 228.), (GOLD, 176., 200., 274., 230.)],
        knight: 150.,
        orc: Some((174., 280.)),
        zone: (FRAME_L, 340., FRAME_R, FLOOR),
    },
    // Hero Rescue 12: burn the monster, cool its chamber, then release the treasure.
    Layout {
        walls: &[(100., 50., 100., 100.), (200., 50., 200., 100.), (190., 125., 190., 190.), (70., 120., 70., 280.), (14., 206., 64., 230.)],
        pins: &[(94., 106., 226., 106.), (184., 196., 278., 196.), (64., 286., 278., 286.)],
        fills: &[(WATER, 110., 60., 190., 100.), (LAVA, 200., 136., 278., 188.), (GOLD, 80., 250., 180., 278.)],
        knight: 150.,
        orc: Some((130., 246.)),
        zone: (FRAME_L, 340., FRAME_R, FLOOR),
    },
    // Hero Rescue 18: neighbouring water/lava tanks above the gold's final gate.
    Layout {
        walls: &[(150., 50., 150., 190.)],
        pins: &[(156., 196., 22., 196.), (144., 196., 278., 196.), (8., 276., 278., 276.)],
        fills: &[(WATER, 24., 120., 140., 188.), (LAVA, 162., 142., 276., 188.), (GOLD, 162., 102., 276., 136.)],
        knight: 150.,
        orc: None,
        zone: (FRAME_L, 300., FRAME_R, FLOOR),
    },
    // Cascade: cool the upper lake, then send its remaining water into
    // the second lava chamber containing the gold, before opening the exit.
    Layout {
        walls: &[(100., 50., 100., 224.), (200., 50., 200., 224.), (68., 244., 68., 318.), (232., 244., 232., 318.)],
        pins: &[(94., 120., 226., 120.), (206., 230., 74., 230.), (62., 324., 258., 324.)],
        fills: &[(WATER, 110., 58., 190., 112.), (LAVA, 108., 194., 192., 222.), (LAVA, 78., 284., 222., 312.), (GOLD, 80., 250., 220., 278.)],
        knight: 150.,
        orc: None,
        zone: (FRAME_L, 350., FRAME_R, FLOOR),
    },
    // Hero Rescue 30: two lava reservoirs must be mixed before the final release.
    Layout {
        walls: &[(105., 50., 105., 132.), (195., 50., 195., 132.), (150., 156., 150., 226.)],
        pins: &[(99., 138., 221., 138.), (156., 232., 22., 232.), (144., 232., 278., 232.), (8., 280., 278., 280.)],
        fills: &[(WATER, 114., 60., 186., 126.), (LAVA, 24., 188., 140., 224.), (LAVA, 162., 188., 276., 224.), (GOLD, 24., 152., 140., 180.), (GOLD, 162., 152., 276., 180.)],
        knight: 150.,
        orc: None,
        zone: (FRAME_L, 300., FRAME_R, FLOOR),
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

/// The introductory quota rises from 50% to 65%; later chambers keep 65%.
/// Keeping the first eight quotas unchanged preserves existing saved attempts.
pub fn level(n: i32) -> Level {
    let l = if n < 20 { &LAYOUT[n as usize] } else { &EXTRA[(n - 20) as usize] };
    assemble(l, n)
}

fn assemble(l: &Layout, n: i32) -> Level {
    let mut walls = vec![(FRAME_L, FRAME_TOP, FRAME_L, FLOOR), (FRAME_R, FRAME_TOP, FRAME_R, FLOOR), (FRAME_L, FLOOR, FRAME_R, FLOOR)];
    walls.extend_from_slice(l.walls);
    let pins = l.pins.to_vec();
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
                grains.push((x, y, kind));
                gold += (kind == GOLD) as usize;
                x += step;
            }
            y -= rise;
            row += 1;
        }
    }
    let share = 0.5 + 0.15 * n.min(7) as f32 / 7.0;
    Level { walls, pins, grains, knight: l.knight, orc: l.orc, zone: l.zone, need: (gold as f32 * share).ceil() as usize }
}

#[cfg(test)]
pub(super) fn candidate(seed: u32, family: usize) -> (Level, String, Vec<usize>) {
    let mut rng = wg::Rng::new(seed);
    let bases = [10, 14, 15, 18];
    let base = bases[family % 4];
    let l = &LAYOUT[base];
    // Recompose reservoir contents and dimensions inside a proven routing
    // family. Reservoir roles, exits and the monster make the pull order
    // matter; acceptance is decided by simulation, never by geometry alone.
    let dx = rng.below(25) as f32 - 12.;
    let sy = 0.84 + rng.below(20) as f32 / 100.;
    let fx = |x: f32| if x <= 22. || x >= 278. { x } else { x + dx };
    let fy = |y: f32| if y >= 340. { y } else { 50. + (y - 50.) * sy };
    let seg = |(a, b, c, d): Seg| (fx(a), fy(b), fx(c), fy(d));
    let mut walls: Vec<_> = l.walls.iter().copied().map(seg).collect();
    let pins: Vec<_> = l.pins.iter().copied().map(seg).collect();
    let mut fills: Vec<_> = l
        .fills
        .iter()
        .map(|&(k, a, b, c, d)| {
            let trim = if k == WATER { 0. } else { rng.below(12) as f32 };
            (k, fx(a), fy(b) + trim, fx(c), fy(d))
        })
        .collect();
    // In the split-treasure family, choose a gold/water upper stack or
    // twin treasure pockets. This changes which pin must be released first.
    if base == 10 && rng.below(2) == 0 {
        fills.swap(0, 2);
        let a = fills[0];
        let b = fills[2];
        fills[0] = (a.0, b.1, b.2, b.3, b.4);
        fills[2] = (b.0, a.1, a.2, a.3, a.4);
    }
    // A sloping lower chute sends the mix to a left or right collection bay.
    let right = rng.below(2) == 0;
    let ledge = if right { (FRAME_L, fy(300.), fx(100.), fy(322.)) } else { (FRAME_R, fy(300.), fx(200.), fy(322.)) };
    if base == 10 || base == 15 {
        walls.push(ledge);
    }
    let knight = if right { 210. } else { 90. };
    let orc = l.orc.map(|(x, y)| (fx(x), if y >= GROUND { y } else { fy(y) }));
    let layout = Layout { walls: &[], pins: &[], fills: &[], knight, orc, zone: l.zone };
    let text = format!("Layout{{walls:&{:?},pins:&{:?},fills:&{:?},knight:{knight:?},orc:{orc:?},zone:{:?}}}", walls, pins, fills, l.zone);
    // Borrow the candidate arrays for assembly without storing them globally.
    let temp = Layout { walls: &walls, pins: &pins, fills: &fills, ..layout };
    let lv = assemble(&temp, 20);
    let order = match base {
        10 => vec![0, 1, 2, 3],
        14 => vec![2, 1, 0, 3],
        15 => vec![1, 0, 3],
        _ => vec![0, 1, 2],
    };
    (lv, text, order)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Ignore left/right reflection, segment direction, ordering and gold
    // quota: none of those make a repeated chamber a new puzzle.
    fn chambers(n: i32, flip: bool) -> Vec<(u8, i32, i32, i32, i32)> {
        let lv = level(n);
        let fx = |x: f32| if flip { 300.0 - x } else { x };
        let mut parts = Vec::new();
        for (kind, segments) in [(0, &lv.walls), (1, &lv.pins)] {
            for &(x0, y0, x1, y1) in segments {
                let a = (fx(x0) as i32, y0 as i32);
                let b = (fx(x1) as i32, y1 as i32);
                let (a, b) = if a < b { (a, b) } else { (b, a) };
                parts.push((kind, a.0, a.1, b.0, b.1));
            }
        }
        parts.sort();
        parts
    }

    #[test]
    fn added_chambers_keep_their_handles_visible() {
        for n in 8..LEVELS {
            let mut handles: Vec<(f32, f32)> = Vec::new();
            for &(ax, ay, bx, by) in &level(n).pins {
                let length = (bx - ax).hypot(by - ay);
                let x = bx + (bx - ax) / length * crate::sim::RING;
                let y = by + (by - ay) / length * crate::sim::RING;
                let margin = crate::sim::RING;
                assert!(handles.iter().all(|&(hx, hy)| (x - hx).hypot(y - hy) >= 2.0 * margin + 2.0), "level {} has overlapping handles", n + 1);
                handles.push((x, y));
                assert!(x >= margin && x <= 300.0 - margin && y >= 40.0 + margin && y <= 400.0 - margin, "level {} has an off-screen handle at {}, {}", n + 1, x, y);
            }
        }
    }

    #[test]
    fn campaign_has_no_repeated_or_mirrored_chambers() {
        assert_eq!(LEVELS, 100);
        for a in 0..LEVELS {
            for b in 0..a {
                for flip in [false, true] {
                    assert_ne!(chambers(a, false), chambers(b, flip), "levels {} and {} repeat", a + 1, b + 1);
                }
            }
        }
    }
}

#[cfg(test)]
pub(super) fn routes(n: i32) -> (&'static [usize], &'static [usize]) {
    ROUTES[(n - 20) as usize]
}

#[cfg(test)]
pub(super) fn exported(n: i32) -> Option<(Level, String, Vec<usize>, Vec<usize>)> {
    let l = EXTRA.get((n - 20) as usize)?;
    let (win, loss) = ROUTES[(n - 20) as usize];
    let text = format!("Layout{{walls:&{:?},pins:&{:?},fills:&{:?},knight:{:?},orc:{:?},zone:{:?}}}", l.walls, l.pins, l.fills, l.knight, l.orc, l.zone);
    Some((assemble(l, n), text, win.to_vec(), loss.to_vec()))
}
