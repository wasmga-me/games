//! The layouts, which tiles are free, and deals that can always be cleared.
//!
//! Positions are in half tiles, so a tile can sit across two below it. A
//! tile is free when nothing lies on it and its left or right side is open.
//! A deal is made by playing the layout backwards: take two free tiles at
//! random until none are left, then give each such pair a matching pair of
//! faces. Playing those pairs in that order clears the board, so every deal
//! can be won.

use wg::Rng;

/// Tiles in every layout.
pub const N: usize = 144;
/// Faces: dots 1-9, bamboo 1-9, characters 1-9, the four winds, the three
/// dragons, four flowers and four seasons.
pub const FACES: u8 = 42;
pub const FLOWERS: u8 = 34;
pub const SEASONS: u8 = 38;

/// What a face matches: itself, or any flower, or any season.
pub fn class(f: u8) -> u8 {
    if f >= SEASONS {
        SEASONS
    } else if f >= FLOWERS {
        FLOWERS
    } else {
        f
    }
}

/// A block of tiles: left and top in half tiles, columns and rows of whole
/// tiles, layer.
struct Block(i8, i8, i8, i8, i8);

/// The classic turtle.
const TURTLE: &[Block] = &[
    Block(2, 0, 12, 1, 0),
    Block(6, 2, 8, 1, 0),
    Block(4, 4, 10, 1, 0),
    Block(2, 6, 12, 1, 0),
    Block(2, 8, 12, 1, 0),
    Block(4, 10, 10, 1, 0),
    Block(6, 12, 8, 1, 0),
    Block(2, 14, 12, 1, 0),
    Block(0, 7, 1, 1, 0),
    Block(26, 7, 1, 1, 0),
    Block(28, 7, 1, 1, 0),
    Block(8, 2, 6, 6, 1),
    Block(10, 4, 4, 4, 2),
    Block(12, 6, 2, 2, 3),
    Block(13, 7, 1, 1, 4),
];

/// Each layer sits across the joints of the one below, like bricks, up to
/// a ridge.
const PYRAMID: &[Block] = &[
    Block(0, 0, 10, 6, 0),
    Block(1, 1, 9, 5, 1),
    Block(2, 2, 8, 4, 2),
    Block(3, 5, 7, 1, 3),
];

/// Two brick pyramids on one wide base, with a pair of tiles at each end.
const PEAKS: &[Block] = &[
    Block(2, 0, 12, 5, 0),
    Block(0, 2, 1, 1, 0),
    Block(0, 6, 1, 1, 0),
    Block(26, 2, 1, 1, 0),
    Block(26, 6, 1, 1, 0),
    Block(3, 1, 5, 4, 1),
    Block(15, 1, 5, 4, 1),
    Block(4, 2, 4, 3, 2),
    Block(16, 2, 4, 3, 2),
    Block(5, 3, 3, 2, 3),
    Block(17, 3, 3, 2, 3),
    Block(6, 4, 2, 1, 4),
    Block(18, 4, 2, 1, 4),
];

/// Walls two tiles high round a full floor, a tower on each corner and a
/// keep in the middle.
const FORTRESS: &[Block] = &[
    Block(0, 0, 10, 6, 0),
    Block(0, 0, 10, 1, 1),
    Block(0, 10, 10, 1, 1),
    Block(0, 2, 1, 4, 1),
    Block(18, 2, 1, 4, 1),
    Block(6, 4, 4, 2, 1),
    Block(0, 0, 10, 1, 2),
    Block(0, 10, 10, 1, 2),
    Block(0, 2, 1, 4, 2),
    Block(18, 2, 1, 4, 2),
    Block(6, 4, 4, 2, 2),
    Block(0, 0, 1, 1, 3),
    Block(18, 0, 1, 1, 3),
    Block(0, 10, 1, 1, 3),
    Block(18, 10, 1, 1, 3),
    Block(6, 4, 4, 2, 3),
];

/// In the catalog `setup` order.
const LAYOUTS: [&[Block]; 4] = [TURTLE, PYRAMID, PEAKS, FORTRESS];
pub const LAYOUT_COUNT: i32 = LAYOUTS.len() as i32;

#[derive(Clone, Copy, Default)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// A layout's tiles and how they block each other.
pub struct Shape {
    pub pos: [Pos; N],
    /// tiles lying on each tile (higher up and overlapping it)
    above: Vec<Vec<u8>>,
    /// tiles touching each one's left and right side, on its layer
    left: Vec<Vec<u8>>,
    right: Vec<Vec<u8>>,
    /// the order to draw them in: low layers first, then from the top left,
    /// so a tile's sides are covered by its neighbours
    pub order: [u8; N],
    /// the highest layer
    pub top: i32,
}

impl Shape {
    pub fn new(layout: usize) -> Shape {
        let mut pos = [Pos::default(); N];
        let mut n = 0;
        for b in LAYOUTS[layout] {
            for r in 0..b.3 as i32 {
                for c in 0..b.2 as i32 {
                    pos[n] = Pos {
                        x: b.0 as i32 + 2 * c,
                        y: b.1 as i32 + 2 * r,
                        z: b.4 as i32,
                    };
                    n += 1;
                }
            }
        }
        assert!(n == N, "layout {} has {} tiles", layout, n);
        let mut above = vec![Vec::new(); N];
        let mut left = vec![Vec::new(); N];
        let mut right = vec![Vec::new(); N];
        for i in 0..N {
            for j in 0..N {
                let (a, b) = (pos[i], pos[j]);
                if (a.y - b.y).abs() >= 2 {
                    continue;
                }
                if b.z > a.z && (a.x - b.x).abs() < 2 {
                    above[i].push(j as u8);
                } else if b.z == a.z && b.x < a.x && a.x - b.x <= 2 {
                    left[i].push(j as u8);
                } else if b.z == a.z && b.x > a.x && b.x - a.x <= 2 {
                    right[i].push(j as u8);
                }
            }
        }
        let mut order = [0u8; N];
        for (i, o) in order.iter_mut().enumerate() {
            *o = i as u8;
        }
        order.sort_by_key(|&i| {
            let p = pos[i as usize];
            (p.z, p.x + p.y, p.x)
        });
        let top = pos.iter().map(|p| p.z).max().unwrap_or(0);
        Shape {
            pos,
            above,
            left,
            right,
            order,
            top,
        }
    }

    /// Whether tile i is in play and can be taken.
    pub fn free(&self, gone: &[u8; N], i: usize) -> bool {
        let out = |v: &Vec<u8>| v.iter().all(|&j| gone[j as usize] != 0);
        gone[i] == 0 && out(&self.above[i]) && (out(&self.left[i]) || out(&self.right[i]))
    }

    /// The free tiles, in drawing order.
    pub fn free_tiles(&self, gone: &[u8; N]) -> Vec<usize> {
        self.order
            .iter()
            .map(|&i| i as usize)
            .filter(|&i| self.free(gone, i))
            .collect()
    }

    /// How many matching pairs of free tiles there are, and one of them to
    /// hint (from a face with only two free when there is one: taking those
    /// never spoils another match).
    pub fn open_pairs(&self, gone: &[u8; N], face: &[u8; N]) -> (i32, Option<(usize, usize)>) {
        let mut by: [Vec<usize>; FACES as usize] = core::array::from_fn(|_| Vec::new());
        for i in self.free_tiles(gone) {
            by[class(face[i]) as usize].push(i);
        }
        let mut count = 0;
        let mut best: Option<(usize, usize, usize)> = None;
        for v in by.iter() {
            let k = v.len();
            count += (k * k.saturating_sub(1) / 2) as i32;
            if k >= 2 {
                // two or four free are the safest to take; three the least
                let rank = match k {
                    2 => 0,
                    4 => 1,
                    _ => 2,
                };
                if best.map_or(true, |b| rank < b.2) {
                    best = Some((v[0], v[1], rank));
                }
            }
        }
        (count, best.map(|b| (b.0, b.1)))
    }

    /// Faces for every tile still in play (gone 0) that can all be cleared,
    /// made from `pairs` (each a matching pair of faces), or false when the
    /// tiles left can't be cleared in any order (one left on another).
    pub fn deal(
        &self,
        gone: &[u8; N],
        pairs: &mut [(u8, u8)],
        face: &mut [u8; N],
        rng: &mut Rng,
    ) -> bool {
        let left = gone.iter().filter(|&&g| g == 0).count();
        if left != pairs.len() * 2 {
            return false;
        }
        let mut taken = Vec::with_capacity(pairs.len());
        for _ in 0..300 {
            let mut g = *gone;
            taken.clear();
            loop {
                let free = self.free_tiles(&g);
                if free.len() < 2 {
                    break;
                }
                let a = free[rng.below(free.len() as u32) as usize];
                let mut b = a;
                while b == a {
                    b = free[rng.below(free.len() as u32) as usize];
                }
                g[a] = 1;
                g[b] = 1;
                taken.push((a, b));
            }
            if taken.len() == pairs.len() {
                rng.shuffle(pairs);
                for (&(a, b), &(fa, fb)) in taken.iter().zip(pairs.iter()) {
                    let swap = rng.below(2) == 1;
                    face[a] = if swap { fb } else { fa };
                    face[b] = if swap { fa } else { fb };
                }
                return true;
            }
        }
        false
    }
}

/// The 72 pairs of a full set, flowers and seasons paired at random.
pub fn full_pairs(rng: &mut Rng) -> Vec<(u8, u8)> {
    let mut pairs = Vec::with_capacity(N / 2);
    for f in 0..FLOWERS {
        pairs.push((f, f));
        pairs.push((f, f));
    }
    for g in [FLOWERS, SEASONS] {
        let mut four = [g, g + 1, g + 2, g + 3];
        rng.shuffle(&mut four);
        pairs.push((four[0], four[1]));
        pairs.push((four[2], four[3]));
    }
    pairs
}

/// The faces still in play, paired by what they match.
pub fn pairs_left(gone: &[u8; N], face: &[u8; N], rng: &mut Rng) -> Option<Vec<(u8, u8)>> {
    let mut by: [Vec<u8>; FACES as usize] = core::array::from_fn(|_| Vec::new());
    for i in 0..N {
        if gone[i] == 0 {
            by[class(face[i]) as usize].push(face[i]);
        }
    }
    let mut pairs = Vec::new();
    for v in by.iter_mut() {
        if v.len() % 2 != 0 {
            return None;
        }
        rng.shuffle(v);
        for p in v.chunks(2) {
            pairs.push((p[0], p[1]));
        }
    }
    Some(pairs)
}

/// Whether `face` holds a full set: four of each suit, wind and dragon tile,
/// one of each flower and season.
pub fn full_set(face: &[u8; N]) -> bool {
    let mut n = [0; FACES as usize];
    for &f in face.iter() {
        if f >= FACES {
            return false;
        }
        n[f as usize] += 1;
    }
    n.iter()
        .enumerate()
        .all(|(f, &k)| k == if (f as u8) < FLOWERS { 4 } else { 1 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_have_144_tiles_and_none_overlap() {
        for l in 0..LAYOUTS.len() {
            let s = Shape::new(l);
            for i in 0..N {
                for j in i + 1..N {
                    let (a, b) = (s.pos[i], s.pos[j]);
                    assert!(
                        !(a.z == b.z && (a.x - b.x).abs() < 2 && (a.y - b.y).abs() < 2),
                        "layout {} tiles {} and {} overlap",
                        l,
                        i,
                        j
                    );
                }
            }
        }
    }

    #[test]
    fn deals_are_full_sets_with_pairs_to_take() {
        for l in 0..LAYOUTS.len() {
            let s = Shape::new(l);
            for seed in 1..40u32 {
                let mut rng = Rng::new(seed * 7919);
                let mut pairs = full_pairs(&mut rng);
                let mut face = [0u8; N];
                assert!(
                    s.deal(&[0; N], &mut pairs, &mut face, &mut rng),
                    "layout {} seed {}",
                    l,
                    seed
                );
                assert!(full_set(&face));
                let (open, hint) = s.open_pairs(&[0; N], &face);
                assert!(open > 0 && hint.is_some());
            }
        }
    }

    #[test]
    fn reshuffle_keeps_faces() {
        let s = Shape::new(0);
        let mut rng = Rng::new(5);
        let mut pairs = full_pairs(&mut rng);
        let mut face = [0u8; N];
        assert!(s.deal(&[0; N], &mut pairs, &mut face, &mut rng));
        let mut gone = [0u8; N];
        // take ten hinted pairs
        for _ in 0..10 {
            let (_, h) = s.open_pairs(&gone, &face);
            let (a, b) = h.unwrap();
            gone[a] = 1;
            gone[b] = 1;
        }
        let before = face;
        let mut pairs = pairs_left(&gone, &face, &mut rng).unwrap();
        assert!(s.deal(&gone, &mut pairs, &mut face, &mut rng));
        assert!(full_set(&face));
        for i in 0..N {
            if gone[i] != 0 {
                assert_eq!(face[i], before[i]);
            }
        }
    }
}
