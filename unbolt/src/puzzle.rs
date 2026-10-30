//! The board: plates held down by screws, the colour boxes the screws go
//! into, the spare holes, and the level deals.
use wg::Rng;

pub const LEVELS: i32 = 30;
pub const MAXP: usize = 24;
pub const MAXS: usize = 48;
pub const MAXB: usize = MAXS / 3;
/// spare holes for screws no box wants yet
pub const SPARE: usize = 5;
pub const COLOURS: u8 = 7;
/// sizes on the board, which runs 0..1000 each way: a plate's half width,
/// a screw's radius, and how close a plate above must come to cover a screw
pub const PLATE_R: i32 = 52;
pub const SCREW_R: i32 = 34;
const COVER: f32 = (PLATE_R as f32) + SCREW_R as f32 * 0.8;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Default)]
pub struct Plate {
    pub ax: i16,
    pub ay: i16,
    pub bx: i16,
    pub by: i16,
    pub layer: u8,
    pub gone: u8,
    pub _pad: [u8; 2],
}

pub const ON_BOARD: u8 = 0;
pub const IN_BOX: u8 = 1;
pub const IN_SPARE: u8 = 2;
pub const DONE: u8 = 3;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Default)]
pub struct Screw {
    pub x: i16,
    pub y: i16,
    pub plate: u8,
    pub colour: u8,
    pub state: u8,
    /// box a's holes are a * 3 + 0..3, spare holes 0..SPARE
    pub slot: u8,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq)]
pub struct Level {
    pub plates: i32,
    pub screws: i32,
    pub boxes: i32,
    /// the two open boxes (-1 when none is left), how full each is, and the next in line
    pub open: [i32; 2],
    pub fill: [i32; 2],
    pub next: i32,
    pub plate: [Plate; MAXP],
    pub screw: [Screw; MAXS],
    /// each box's colour, in the order they come
    pub colour: [u8; MAXB],
    /// the screw in each spare hole, plus one; 0 when empty
    pub spare: [u8; 8],
}
// SAFETY: #[repr(C)], integers only, padding spelled out
unsafe impl wg::Plain for Level {}

/// what a tap set off, in order, for the animations
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Ev {
    /// a screw went to a box hole or a spare hole
    Moved(usize),
    /// open box a filled up and left
    BoxDone(usize, u8, [u8; 3]),
    /// a new box came into place a
    BoxIn(usize),
    PlateGone(usize),
}

fn seg_dist(px: f32, py: f32, p: &Plate) -> f32 {
    let (ax, ay, bx, by) = (p.ax as f32, p.ay as f32, p.bx as f32, p.by as f32);
    let (dx, dy) = (bx - ax, by - ay);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((px - ax) * dx + (py - ay) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (ax + dx * t, ay + dy * t);
    ((px - qx).powi(2) + (py - qy).powi(2)).sqrt()
}

impl Level {
    pub fn empty() -> Level {
        Level {
            plates: 0,
            screws: 0,
            boxes: 0,
            open: [-1, -1],
            fill: [0, 0],
            next: 0,
            plate: [Plate::default(); MAXP],
            screw: [Screw::default(); MAXS],
            colour: [0; MAXB],
            spare: [0; 8],
        }
    }
    /// a plate on a higher layer lies over the screw
    pub fn blocked(&self, i: usize) -> bool {
        let s = &self.screw[i];
        let layer = self.plate[s.plate as usize].layer;
        (0..self.plates as usize).any(|q| {
            let p = &self.plate[q];
            p.gone == 0 && p.layer > layer && seg_dist(s.x as f32, s.y as f32, p) < COVER
        })
    }
    pub fn free(&self, i: usize) -> bool {
        i < self.screws as usize && self.screw[i].state == ON_BOARD && !self.blocked(i)
    }
    /// whether open box a takes colour c and has room
    fn wants(&self, a: usize, c: u8) -> bool {
        self.open[a] >= 0 && self.colour[self.open[a] as usize] == c && self.fill[a] < 3
    }
    /// the open box that takes colour c, if one has room
    fn box_for(&self, c: u8) -> Option<usize> {
        (0..2).find(|&a| self.wants(a, c))
    }
    pub fn spare_used(&self) -> usize {
        self.spare[..SPARE].iter().filter(|&&s| s != 0).count()
    }
    /// whether tapping screw i would take it anywhere
    pub fn takes(&self, i: usize) -> bool {
        self.free(i) && (self.box_for(self.screw[i].colour).is_some() || self.spare_used() < SPARE)
    }
    /// no screw that can be taken has anywhere to go
    pub fn stuck(&self) -> bool {
        !self.cleared() && !(0..self.screws as usize).any(|i| self.takes(i))
    }
    pub fn cleared(&self) -> bool {
        self.open == [-1, -1] && self.next >= self.boxes
    }

    fn into_box(&mut self, i: usize, a: usize, ev: &mut Vec<Ev>) {
        let f = self.fill[a] as usize;
        self.screw[i].state = IN_BOX;
        self.screw[i].slot = (a * 3 + f) as u8;
        self.fill[a] += 1;
        ev.push(Ev::Moved(i));
        if self.fill[a] == 3 {
            self.box_done(a, ev);
        }
    }
    fn box_done(&mut self, a: usize, ev: &mut Vec<Ev>) {
        let mut held = [0u8; 3];
        for j in 0..self.screws as usize {
            let s = &mut self.screw[j];
            if s.state == IN_BOX && s.slot as usize / 3 == a {
                held[s.slot as usize % 3] = j as u8;
                s.state = DONE;
            }
        }
        ev.push(Ev::BoxDone(a, self.colour[self.open[a] as usize], held));
        self.fill[a] = 0;
        if self.next < self.boxes {
            self.open[a] = self.next;
            self.next += 1;
            ev.push(Ev::BoxIn(a));
            // screws waiting in the spare holes go in; filling it brings
            // on the box after, so ask again each time
            while let Some(k) = (0..SPARE).find(|&k| self.spare[k] > 0 && self.wants(a, self.screw[self.spare[k] as usize - 1].colour)) {
                let j = self.spare[k] as usize - 1;
                self.spare[k] = 0;
                self.into_box(j, a, ev);
            }
        } else {
            self.open[a] = -1;
        }
    }

    /// takes screw i off the board; None when it can't go anywhere
    pub fn tap(&mut self, i: usize) -> Option<Vec<Ev>> {
        if !self.takes(i) {
            return None;
        }
        let mut ev = Vec::new();
        let p = self.screw[i].plate as usize;
        // off the plate first, so the plate falls before any box moves on
        let c = self.screw[i].colour;
        let target = self.box_for(c);
        self.screw[i].state = IN_SPARE;
        if (0..self.screws as usize).all(|j| self.screw[j].plate as usize != p || self.screw[j].state != ON_BOARD) {
            self.plate[p].gone = 1;
            ev.push(Ev::PlateGone(p));
        }
        match target {
            Some(a) => self.into_box(i, a, &mut ev),
            None => {
                let k = (0..SPARE).find(|&k| self.spare[k] == 0).unwrap();
                self.spare[k] = i as u8 + 1;
                self.screw[i].slot = k as u8;
                ev.push(Ev::Moved(i));
            }
        }
        // plate gone first in the list reads oddly for the animation; keep the screw first
        if let Some(pos) = ev.iter().position(|e| matches!(e, Ev::PlateGone(_))) {
            let g = ev.remove(pos);
            ev.insert(1.min(ev.len()), g);
        }
        Some(ev)
    }

    pub fn valid(&self) -> bool {
        let (np, ns, nb) = (self.plates as usize, self.screws as usize, self.boxes as usize);
        if np == 0 || np > MAXP || ns == 0 || ns > MAXS || nb * 3 != ns || !(0..=self.boxes).contains(&self.next) {
            return false;
        }
        let in_box = |a: usize| (0..ns).filter(|&j| self.screw[j].state == IN_BOX && self.screw[j].slot as usize / 3 == a).count();
        (0..2).all(|a| (self.open[a] == -1 && self.fill[a] == 0 || (0..self.next).contains(&self.open[a]) && (0..3).contains(&self.fill[a])) && in_box(a) == self.fill[a] as usize)
            && self.colour[..nb].iter().all(|&c| (1..=COLOURS).contains(&c))
            && self.colour[nb..].iter().all(|&c| c == 0)
            && self.spare[SPARE..].iter().all(|&s| s == 0)
            && self.spare[..SPARE].iter().enumerate().all(|(k, &s)| {
                s == 0 || (s as usize <= ns && self.screw[s as usize - 1].state == IN_SPARE && self.screw[s as usize - 1].slot as usize == k)
            })
            && (0..ns).all(|j| {
                let s = &self.screw[j];
                (s.plate as usize) < np
                    && (1..=COLOURS).contains(&s.colour)
                    && s.state <= DONE
                    && (s.state != IN_BOX || (s.slot as usize) < 6)
                    && (s.state != IN_SPARE || self.spare[s.slot as usize % 8] as usize == j + 1)
            })
            && self.screw[ns..].iter().all(|s| *s == Screw::default())
            && (0..np).all(|q| self.plate[q].gone <= 1 && self.plate[q]._pad == [0; 2])
            && self.plate[np..].iter().all(|p| *p == Plate::default())
    }
}

/// the level's size: layers, plates and colours grow over the 30
fn shape(level: i32) -> (u8, usize, u8) {
    let layers = 2 + (level * 4 + 14) / (LEVELS - 1);
    let plates = 3 + (level * 13 + 14) / (LEVELS - 1);
    let colours = 3 + (level * 4 + 14) / (LEVELS - 1);
    (layers as u8, plates as usize, colours as u8)
}

/// The deal for a level, the same for everyone. Plates go down layer by
/// layer; then a random order of taking the screws off is played, and the
/// colours are handed out along it with a little shuffling, so that order
/// always works with the spare holes.
pub fn deal(level: i32) -> Level {
    let (layers, want, colours) = shape(level);
    let mut rng = Rng::new(0x5343_5257 ^ (level as u32 + 1).wrapping_mul(2_654_435_761));
    loop {
        let mut lv = Level::empty();
        let mut placed = 0;
        for k in 0..want {
            let layer = (k * layers as usize / want) as u8;
            let mut best: Option<(i32, Plate, Vec<(i16, i16)>)> = None;
            for _ in 0..60 {
                let ang = [0.0f32, 90.0, 45.0, 135.0][rng.below(4) as usize].to_radians();
                let len = 150.0 + rng.below(280) as f32;
                let (cx, cy) = (150.0 + rng.below(700) as f32, 150.0 + rng.below(700) as f32);
                let (dx, dy) = (ang.cos() * len / 2.0, ang.sin() * len / 2.0);
                let p = Plate { ax: (cx - dx) as i16, ay: (cy - dy) as i16, bx: (cx + dx) as i16, by: (cy + dy) as i16, layer, gone: 0, _pad: [0; 2] };
                let ends = [p.ax, p.ay, p.bx, p.by];
                if ends.iter().any(|&v| !(70..=930).contains(&v)) {
                    continue;
                }
                let mut holes = vec![(p.ax, p.ay), (p.bx, p.by)];
                if len > 330.0 {
                    holes.push(((p.ax + p.bx) / 2, (p.ay + p.by) / 2));
                }
                // screws keep apart, and plates on one layer don't touch
                let apart = holes.iter().all(|&(x, y)| {
                    lv.screw[..lv.screws as usize].iter().all(|s| ((s.x - x) as f32).hypot((s.y - y) as f32) > 92.0)
                });
                let clear = lv.plate[..placed].iter().filter(|q| q.layer == layer).all(|q| {
                    (0..=8).all(|t| {
                        let f = t as f32 / 8.0;
                        let (x, y) = (p.ax as f32 + (p.bx - p.ax) as f32 * f, p.ay as f32 + (p.by - p.ay) as f32 * f);
                        seg_dist(x, y, q) > PLATE_R as f32 * 2.2
                    })
                });
                if !apart || !clear {
                    continue;
                }
                // over as many screws below as it can
                let covers = lv.screw[..lv.screws as usize].iter().filter(|s| seg_dist(s.x as f32, s.y as f32, &p) < COVER).count() as i32;
                let score = covers * 10 + rng.below(8) as i32;
                if best.as_ref().map_or(true, |b| score > b.0) {
                    best = Some((score, p, holes));
                }
            }
            if let Some((_, p, holes)) = best {
                lv.plate[placed] = p;
                for (x, y) in holes {
                    let n = lv.screws as usize;
                    if n < MAXS {
                        lv.screw[n] = Screw { x, y, plate: placed as u8, ..Screw::default() };
                        lv.screws += 1;
                    }
                }
                placed += 1;
            }
        }
        lv.plates = placed as i32;
        // a whole number of boxes: drop middle screws, or plates, until it fits
        while lv.screws % 3 != 0 {
            let n = lv.screws as usize;
            let p = lv.screw[n - 1].plate;
            lv.screw[n - 1] = Screw::default();
            lv.screws -= 1;
            if !lv.screw[..n - 1].iter().any(|s| s.plate == p) {
                lv.plates -= 1;
                lv.plate[lv.plates as usize] = Plate::default();
            }
        }
        if (lv.plates as usize) + 1 < want || lv.screws < 6 {
            continue;
        }
        lv.boxes = lv.screws / 3;
        let nb = lv.boxes as usize;
        // box colours: never the same twice running
        for b in 0..nb {
            let mut c = 1 + rng.below(colours as u32) as u8;
            while b > 0 && c == lv.colour[b - 1] || b > 1 && c == lv.colour[b - 2] && colours > 2 {
                c = 1 + rng.below(colours as u32) as u8;
            }
            lv.colour[b] = c;
        }
        lv.open = [0, if nb > 1 { 1 } else { -1 }];
        lv.next = nb.min(2) as i32;
        // a random order the screws can come off in
        let mut trial = lv;
        let mut order = Vec::new();
        for s in trial.screw[..trial.screws as usize].iter_mut() {
            s.colour = 1;
        }
        while order.len() < lv.screws as usize {
            let free: Vec<usize> = (0..lv.screws as usize).filter(|&i| trial.screw[i].state == ON_BOARD && !trial.blocked(i)).collect();
            let i = free[rng.below(free.len() as u32) as usize];
            trial.screw[i].state = DONE;
            let p = trial.screw[i].plate as usize;
            if trial.screw[..trial.screws as usize].iter().all(|s| s.plate as usize != p || s.state != ON_BOARD) {
                trial.plate[p].gone = 1;
            }
            order.push(i);
        }
        // the colours along that order, shuffled within a window
        let window = 2 + (level as u32) / 3;
        for _ in 0..40 {
            let mut seq: Vec<u8> = lv.colour[..nb].iter().flat_map(|&c| [c, c, c]).collect();
            for k in 0..seq.len() {
                let j = (k + rng.below(window) as usize).min(seq.len() - 1);
                seq.swap(k, j);
            }
            let mut test = lv;
            for (k, &i) in order.iter().enumerate() {
                test.screw[i].colour = seq[k];
            }
            let mut play = test;
            if order.iter().all(|&i| play.tap(i).is_some()) && play.cleared() {
                return test;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// greedy play: a screw a box wants first, then the one that frees most
    pub fn greedy(lv: &Level) -> bool {
        let mut g = *lv;
        while !g.cleared() {
            let n = g.screws as usize;
            let pick = (0..n).filter(|&i| g.takes(i)).max_by_key(|&i| {
                let wanted = g.box_for(g.screw[i].colour).is_some() as i32;
                wanted * 100 - i as i32
            });
            match pick {
                Some(i) => {
                    g.tap(i);
                }
                None => return false,
            }
        }
        true
    }

    /// four spare screws of one colour and a box of it: three fill it, the
    /// fourth waits for its own box, not the one that comes next
    #[test]
    fn spares_go_only_to_their_colour() {
        let mut lv = Level::empty();
        lv.plates = 1;
        lv.boxes = 4;
        lv.screws = 12;
        lv.colour[..4].copy_from_slice(&[2, 3, 1, 4]);
        lv.open = [0, 1];
        lv.next = 2;
        for (k, j) in (0..4).enumerate() {
            lv.screw[j] = Screw { colour: 1, state: IN_SPARE, slot: k as u8, ..Screw::default() };
            lv.spare[k] = j as u8 + 1;
        }
        for j in 4..12 {
            lv.screw[j] = Screw { colour: [2, 2, 2, 3, 3, 3, 4, 4][j - 4], ..Screw::default() };
        }
        assert!(lv.valid());
        // fill box 0 (colour 2): box 2 (colour 1) comes in and takes three spares
        for j in 4..7 {
            lv.tap(j).unwrap();
        }
        assert_eq!(lv.open[0], 3, "the colour 1 box filled from the spares and left");
        assert_eq!(lv.fill[0], 0, "the colour 4 box took no colour 1 screw");
        assert_eq!(lv.spare_used(), 1);
        assert!(lv.valid());
    }

    #[test]
    fn every_level_deals() {
        for l in 0..LEVELS {
            let lv = deal(l);
            assert!(lv.valid(), "level {}", l + 1);
            let (layers, _, colours) = shape(l);
            println!("level {} layers {} colours {} plates {} screws {} greedy {}", l + 1, layers, colours, lv.plates, lv.screws, greedy(&lv));
        }
    }
}
