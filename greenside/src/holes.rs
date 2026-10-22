//! The course: 18 holes laid out in plan units (480 x 360, the bar along
//! the top is the score strip). A hole is its green's outline (a rail runs
//! all the way round it), inner rails, bumpers, sand, water, slopes and the
//! things that move: windmill bars turning about a hub and gates sliding to
//! and fro.

/// A rounded box: x, y, w, h and the corners' radius.
#[derive(Clone, Copy)]
pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
}

/// A slope: the ball gains (ax, ay) units/s² while on it.
#[derive(Clone, Copy)]
pub struct Slope {
    pub at: Area,
    pub ax: f32,
    pub ay: f32,
}

/// A windmill: `bars` bars of half-length `len` turning about (x, y) at
/// `speed` radians a second (negative turns the other way).
#[derive(Clone, Copy)]
pub struct Spinner {
    pub x: f32,
    pub y: f32,
    pub len: f32,
    pub speed: f32,
    pub bars: i32,
}

/// A gate: the rail from (x0, y0) to (x1, y1) slides by (dx, dy) and back,
/// taking `period` ms for the round trip, starting `phase` (0..1) along it.
#[derive(Clone, Copy)]
pub struct Mover {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub dx: f32,
    pub dy: f32,
    pub period: f32,
    pub phase: f32,
}

pub struct Hole {
    pub par: i32,
    pub tee: (f32, f32),
    pub cup: (f32, f32),
    pub shape: &'static [(f32, f32)],
    /// inner rails, x0, y0, x1, y1
    pub walls: &'static [(f32, f32, f32, f32)],
    /// x, y, radius
    pub bumpers: &'static [(f32, f32, f32)],
    pub sand: &'static [Area],
    pub water: &'static [Area],
    pub slopes: &'static [Slope],
    pub spinners: &'static [Spinner],
    pub movers: &'static [Mover],
}

impl Hole {
    /// Every fixed rail: the outline's edges and the inner rails, x0, y0, x1, y1.
    pub fn rails(&self) -> impl Iterator<Item = (f32, f32, f32, f32)> + '_ {
        let n = self.shape.len();
        let edge = move |i: usize| (self.shape[i], self.shape[(i + 1) % n]);
        (0..n).map(edge).map(|(a, b)| (a.0, a.1, b.0, b.1)).chain(self.walls.iter().copied())
    }
}

const fn a(x: f32, y: f32, w: f32, h: f32, r: f32) -> Area {
    Area { x, y, w, h, r }
}
const fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> [(f32, f32); 4] {
    [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

const NONE: Hole = Hole {
    par: 2,
    tee: (0.0, 0.0),
    cup: (0.0, 0.0),
    shape: &[],
    walls: &[],
    bumpers: &[],
    sand: &[],
    water: &[],
    slopes: &[],
    spinners: &[],
    movers: &[],
};

pub const COUNT: usize = 18;

pub static HOLES: [Hole; COUNT] = [
    // 1: a straight run with a kink in the far rail
    Hole {
        par: 2,
        tee: (90.0, 210.0),
        cup: (395.0, 180.0),
        shape: &[(50.0, 160.0), (300.0, 160.0), (340.0, 130.0), (440.0, 130.0), (440.0, 240.0), (50.0, 240.0)],
        ..NONE
    },
    // 2: round the corner
    Hole {
        par: 3,
        tee: (85.0, 290.0),
        cup: (385.0, 95.0),
        shape: &[(40.0, 240.0), (330.0, 240.0), (330.0, 55.0), (440.0, 55.0), (440.0, 335.0), (40.0, 335.0)],
        ..NONE
    },
    // 3: three bumpers in the way
    Hole {
        par: 2,
        tee: (240.0, 310.0),
        cup: (240.0, 90.0),
        shape: &[(120.0, 50.0), (360.0, 50.0), (420.0, 110.0), (420.0, 290.0), (360.0, 345.0), (120.0, 345.0), (60.0, 290.0), (60.0, 110.0)],
        bumpers: &[(240.0, 195.0, 18.0), (170.0, 150.0, 13.0), (310.0, 150.0, 13.0)],
        ..NONE
    },
    // 4: a rail down the middle and sand guarding the cup
    Hole {
        par: 3,
        tee: (85.0, 150.0),
        cup: (395.0, 150.0),
        shape: &[(40.0, 95.0), (440.0, 95.0), (440.0, 330.0), (40.0, 330.0)],
        walls: &[(240.0, 95.0, 240.0, 255.0)],
        sand: &[a(320.0, 205.0, 70.0, 70.0, 35.0), a(150.0, 270.0, 60.0, 45.0, 22.0)],
        ..NONE
    },
    // 5: there and back round a long rail
    Hole {
        par: 3,
        tee: (85.0, 285.0),
        cup: (85.0, 115.0),
        shape: &rect(40.0, 60.0, 440.0, 340.0),
        walls: &[(40.0, 200.0, 330.0, 200.0)],
        sand: &[a(380.0, 170.0, 60.0, 60.0, 30.0)],
        ..NONE
    },
    // 6: the bridge
    Hole {
        par: 3,
        tee: (85.0, 285.0),
        cup: (400.0, 115.0),
        shape: &rect(40.0, 70.0, 440.0, 330.0),
        water: &[a(185.0, 70.0, 110.0, 100.0, 0.0), a(185.0, 230.0, 110.0, 100.0, 0.0)],
        ..NONE
    },
    // 7: uphill
    Hole {
        par: 2,
        tee: (90.0, 200.0),
        cup: (395.0, 200.0),
        shape: &[(50.0, 150.0), (440.0, 150.0), (440.0, 250.0), (50.0, 250.0)],
        slopes: &[Slope { at: a(170.0, 150.0, 150.0, 100.0, 0.0), ax: -240.0, ay: 0.0 }],
        ..NONE
    },
    // 8: the windmill
    Hole {
        par: 3,
        tee: (85.0, 200.0),
        cup: (395.0, 200.0),
        shape: &rect(40.0, 90.0, 440.0, 310.0),
        walls: &[(240.0, 90.0, 240.0, 172.0), (240.0, 228.0, 240.0, 310.0)],
        spinners: &[Spinner { x: 240.0, y: 200.0, len: 34.0, speed: 1.7, bars: 2 }],
        ..NONE
    },
    // 9: zigzag
    Hole {
        par: 4,
        tee: (85.0, 300.0),
        cup: (400.0, 300.0),
        shape: &rect(40.0, 50.0, 440.0, 345.0),
        walls: &[(145.0, 50.0, 145.0, 260.0), (240.0, 135.0, 240.0, 345.0), (335.0, 50.0, 335.0, 260.0)],
        ..NONE
    },
    // 10: round the pond
    Hole {
        par: 2,
        tee: (75.0, 200.0),
        cup: (405.0, 200.0),
        shape: &[(240.0, 48.0), (450.0, 200.0), (240.0, 352.0), (30.0, 200.0)],
        water: &[a(185.0, 150.0, 110.0, 100.0, 50.0)],
        ..NONE
    },
    // 11: sliding gates
    Hole {
        par: 3,
        tee: (85.0, 200.0),
        cup: (400.0, 200.0),
        shape: &rect(40.0, 135.0, 440.0, 265.0),
        movers: &[
            Mover { x0: 185.0, y0: 140.0, x1: 185.0, y1: 205.0, dx: 0.0, dy: 55.0, period: 2600.0, phase: 0.0 },
            Mover { x0: 295.0, y0: 195.0, x1: 295.0, y1: 260.0, dx: 0.0, dy: -55.0, period: 2000.0, phase: 0.25 },
        ],
        ..NONE
    },
    // 12: the valley
    Hole {
        par: 2,
        tee: (95.0, 305.0),
        cup: (240.0, 95.0),
        shape: &rect(55.0, 55.0, 425.0, 345.0),
        slopes: &[
            Slope { at: a(55.0, 55.0, 170.0, 290.0, 0.0), ax: 110.0, ay: 0.0 },
            Slope { at: a(255.0, 55.0, 170.0, 290.0, 0.0), ax: -110.0, ay: 0.0 },
        ],
        sand: &[a(150.0, 60.0, 40.0, 40.0, 20.0), a(290.0, 60.0, 40.0, 40.0, 20.0)],
        ..NONE
    },
    // 13: behind the wall
    Hole {
        par: 3,
        tee: (90.0, 295.0),
        cup: (385.0, 105.0),
        shape: &rect(40.0, 55.0, 440.0, 345.0),
        walls: &[(310.0, 55.0, 310.0, 255.0), (150.0, 55.0, 150.0, 170.0)],
        bumpers: &[(230.0, 120.0, 14.0)],
        ..NONE
    },
    // 14: two windmills
    Hole {
        par: 3,
        tee: (80.0, 200.0),
        cup: (405.0, 200.0),
        shape: &rect(40.0, 125.0, 440.0, 275.0),
        walls: &[(170.0, 125.0, 170.0, 168.0), (170.0, 232.0, 170.0, 275.0), (310.0, 125.0, 310.0, 168.0), (310.0, 232.0, 310.0, 275.0)],
        spinners: &[
            Spinner { x: 170.0, y: 200.0, len: 38.0, speed: 1.5, bars: 1 },
            Spinner { x: 310.0, y: 200.0, len: 38.0, speed: -2.1, bars: 1 },
        ],
        ..NONE
    },
    // 15: the island
    Hole {
        par: 3,
        tee: (85.0, 200.0),
        cup: (405.0, 200.0),
        shape: &rect(40.0, 60.0, 440.0, 340.0),
        water: &[
            a(220.0, 60.0, 220.0, 80.0, 0.0),
            a(220.0, 260.0, 220.0, 80.0, 0.0),
            a(285.0, 182.0, 56.0, 36.0, 18.0),
        ],
        bumpers: &[(140.0, 110.0, 12.0), (140.0, 290.0, 12.0)],
        ..NONE
    },
    // 16: the long snake
    Hole {
        par: 4,
        tee: (85.0, 100.0),
        cup: (400.0, 300.0),
        shape: &rect(40.0, 50.0, 440.0, 350.0),
        walls: &[(40.0, 150.0, 350.0, 150.0), (130.0, 250.0, 440.0, 250.0)],
        bumpers: &[(240.0, 200.0, 14.0)],
        sand: &[a(385.0, 60.0, 50.0, 50.0, 25.0), a(45.0, 290.0, 50.0, 50.0, 25.0)],
        ..NONE
    },
    // 17: pinball
    Hole {
        par: 3,
        tee: (240.0, 315.0),
        cup: (240.0, 85.0),
        shape: &[(170.0, 45.0), (310.0, 45.0), (420.0, 150.0), (420.0, 260.0), (330.0, 350.0), (150.0, 350.0), (60.0, 260.0), (60.0, 150.0)],
        walls: &[(195.0, 120.0, 285.0, 120.0)],
        bumpers: &[(160.0, 200.0, 13.0), (240.0, 225.0, 13.0), (320.0, 200.0, 13.0), (200.0, 165.0, 10.0), (280.0, 165.0, 10.0)],
        sand: &[a(115.0, 120.0, 45.0, 45.0, 22.0), a(320.0, 120.0, 45.0, 45.0, 22.0)],
        ..NONE
    },
    // 18: the last hole
    Hole {
        par: 4,
        tee: (85.0, 300.0),
        cup: (95.0, 105.0),
        shape: &rect(40.0, 55.0, 440.0, 345.0),
        walls: &[(40.0, 200.0, 300.0, 200.0)],
        spinners: &[Spinner { x: 370.0, y: 200.0, len: 52.0, speed: 1.3, bars: 2 }],
        water: &[a(150.0, 75.0, 90.0, 80.0, 30.0)],
        slopes: &[Slope { at: a(40.0, 210.0, 260.0, 135.0, 0.0), ax: 0.0, ay: 70.0 }],
        sand: &[a(45.0, 150.0, 45.0, 45.0, 22.0)],
        ..NONE
    },
];
