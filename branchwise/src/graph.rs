//! Lays a repository out as a commit graph and draws it: time runs left to
//! right on wide screens and top to bottom on tall ones, each branch keeps
//! a lane, and names sit on pills by their commits.

use crate::git::{Head, Repo, World};
use std::collections::HashMap;
use wg::Screen;

pub struct Theme {
    pub bg: u32,
    pub panel: u32,
    pub line: u32,
    pub ink: u32,
    pub muted: u32,
    pub accent: u32,
    pub code: u32,
    pub good: u32,
    pub bad: u32,
    pub warn: u32,
    pub lanes: [u32; 6],
}

pub const THEMES: [Theme; 2] = [
    Theme {
        bg: 0xf4efe6,
        panel: 0xfffaf1,
        line: 0xd8cfbf,
        ink: 0x1d1a2e,
        muted: 0x6e6880,
        accent: 0x654ff0,
        code: 0x5b3fd6,
        good: 0x1f8a5b,
        bad: 0xc8372d,
        warn: 0xa86a00,
        lanes: [0x654ff0, 0x1a9c8c, 0xe0663a, 0xc9952a, 0x2f7fd8, 0xc8478f],
    },
    Theme {
        bg: 0x15131f,
        panel: 0x1f1c2c,
        line: 0x353048,
        ink: 0xf1edf9,
        muted: 0x9c96b2,
        accent: 0x9d8cff,
        code: 0xb7a9ff,
        good: 0x4fd39a,
        bad: 0xff6b5e,
        warn: 0xf2c14e,
        lanes: [0x8b78ff, 0x2fc4b2, 0xff8a5b, 0xf2c14e, 0x5aa8ff, 0xf07ab8],
    },
];

/// Where things went on screen, for taps.
#[derive(Clone, Debug, PartialEq)]
pub enum Spot {
    Commit(String, String),
    Name(String),
}

/// Commits some name or HEAD still reaches.
pub fn reachable(r: &Repo) -> Vec<bool> {
    let mut reach = vec![false; r.commits.len()];
    let mut tips: Vec<usize> = r
        .branches
        .iter()
        .chain(&r.tags)
        .chain(&r.remote)
        .map(|b| b.1)
        .collect();
    tips.extend(r.wts.iter().filter_map(|w| r.head_of(&w.head)));
    for t in tips {
        for (k, v) in r.ancestors(t).into_iter().enumerate() {
            reach[k] |= v;
        }
    }
    reach
}

/// Lanes and generations for every commit (index order); commits not
/// `shown` get no lane of their own.
pub fn layout(r: &Repo, shown: &[bool]) -> Vec<(f32, f32)> {
    let n = r.commits.len();
    let mut out = vec![(0.0, 0.0); n];
    if n == 0 {
        return out;
    }
    let mut gen = vec![0usize; n];
    for i in 0..n {
        gen[i] = r.commits[i]
            .parents
            .iter()
            .map(|&p| gen[p] + 1)
            .max()
            .unwrap_or(0);
    }
    let reach = reachable(r);
    // the first-parent line of main, kept straight
    let mut trunk = vec![false; n];
    let main = r
        .branch("main")
        .or(r.remote.iter().find(|x| x.0 == "origin/main").map(|x| x.1))
        .or(r.branches.first().map(|b| b.1));
    let mut c = main;
    while let Some(x) = c {
        trunk[x] = true;
        c = r.commits[x].parents.first().copied();
    }
    let mut kids: Vec<Vec<usize>> = vec![vec![]; n];
    let mut roots = vec![];
    for i in (0..n).filter(|&i| shown[i]) {
        match r.commits[i].parents.first() {
            Some(&p) => kids[p].push(i),
            None => roots.push(i),
        }
    }
    let key = |i: usize| (!trunk[i], !reach[i], i);
    for k in kids.iter_mut() {
        k.sort_by_key(|&i| key(i));
    }
    roots.sort_by_key(|&i| key(i));
    fn width(i: usize, kids: &[Vec<usize>], w: &mut [usize]) -> usize {
        let s: usize = kids[i].iter().map(|&k| width(k, kids, w)).sum();
        w[i] = s.max(1);
        w[i]
    }
    let mut w = vec![0; n];
    for &r0 in &roots {
        width(r0, &kids, &mut w);
    }
    let mut lane = vec![0usize; n];
    let mut stack: Vec<(usize, usize)> = vec![];
    let mut at = 0;
    for &r0 in &roots {
        stack.push((r0, at));
        at += w[r0];
    }
    while let Some((i, l)) = stack.pop() {
        lane[i] = l;
        let mut off = l;
        for &k in &kids[i] {
            stack.push((k, off));
            off += w[k];
        }
    }
    for i in 0..n {
        out[i] = (gen[i] as f32, lane[i] as f32);
    }
    out
}

/// Positions eased toward the layout, per commit id.
#[derive(Default)]
pub struct Motion {
    pub at: HashMap<String, (f32, f32)>,
}

impl Motion {
    /// Moves every commit toward its place; true while any is moving.
    pub fn step(&mut self, r: &Repo, target: &[(f32, f32)], dt: f32) -> bool {
        let mut moving = false;
        let k = 1.0 - (-dt / 90.0).exp();
        for (i, c) in r.commits.iter().enumerate() {
            let t = target[i];
            let start = c
                .parents
                .first()
                .and_then(|p| self.at.get(&r.commits[*p].id).copied())
                .unwrap_or(t);
            let p = self.at.entry(c.id.clone()).or_insert(start);
            let d = (t.0 - p.0, t.1 - p.1);
            if d.0.abs() + d.1.abs() > 0.004 {
                p.0 += d.0 * k;
                p.1 += d.1 * k;
                moving = true;
            } else {
                *p = t;
            }
        }
        moving
    }
}

pub struct Area {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// How a graph is drawn.
pub struct Look {
    pub vertical: bool,
    pub small: bool,
    /// origin: a bare repository, no HEAD
    pub bare: bool,
    pub shown: Vec<bool>,
}

/// Draws a repository's graph in an area, and notes where its commits and
/// names went (for taps).
#[allow(clippy::too_many_arguments)]
pub fn draw(
    s: &mut Screen,
    t: &Theme,
    world: &World,
    r: &Repo,
    pos: &dyn Fn(usize) -> (f32, f32),
    a: &Area,
    look: &Look,
    hover: Option<&str>,
    spots: &mut Vec<(f32, f32, f32, f32, Spot)>,
) {
    let (vertical, small) = (look.vertical, look.small);
    let n = r.commits.len();
    if n == 0 {
        let msg = if world.inited {
            "No commits yet"
        } else {
            "Not a repository yet"
        };
        s.text_c(
            msg,
            a.x + a.w / 2.0,
            a.y + a.h / 2.0,
            if small { 13.0 } else { 16.0 },
            wg::hex(t.muted),
            1.0,
        );
        return;
    }
    let shown = &look.shown;
    let lay: Vec<(f32, f32)> = (0..n).map(pos).collect();
    let max_gen = (0..n)
        .filter(|&i| shown[i])
        .map(|i| lay[i].0)
        .fold(0.0f32, f32::max);
    let max_lane = (0..n)
        .filter(|&i| shown[i])
        .map(|i| lay[i].1)
        .fold(0.0f32, f32::max);
    let min_gen = (0..n)
        .filter(|&i| shown[i])
        .map(|i| lay[i].0)
        .fold(f32::MAX, f32::min)
        .min(max_gen);
    // names on pills: 0 HEAD, 1 the branch HEAD is on, 2 a branch, 3 origin/x, 4 a tag
    let mut names: Vec<Vec<(String, u8)>> = vec![vec![]; n];
    for (w, wt) in r.wts.iter().enumerate() {
        if let (Head::Detached(c), false) = (&wt.head, look.bare) {
            names[*c].push((
                if w == r.cur || r.wts.len() == 1 {
                    "HEAD".into()
                } else {
                    format!("HEAD ../{}", wt.path)
                },
                0,
            ));
        }
    }
    for (name, c) in &r.branches {
        match r.checked_out(name) {
            Some(w) if w == r.cur && !look.bare => names[*c].push((name.clone(), 1)),
            Some(w) if !look.bare => names[*c].push((format!("{name} ../{}", r.wts[w].path), 2)),
            _ => names[*c].push((name.clone(), 2)),
        }
    }
    for (name, c) in &r.remote {
        names[*c].push((name.clone(), 3));
    }
    for (name, c) in &r.tags {
        names[*c].push((name.clone(), 4));
    }
    for v in names.iter_mut() {
        v.sort_by_key(|x| x.1);
    }
    let fs = if small { 10.5 } else { 13.0 };
    let ph = if small { 15.0 } else { 20.0 };
    let gap = 4.0;
    let pill_w = |s: &Screen, name: &str, kind: u8| -> f32 {
        if kind == 1 {
            s.text_width("HEAD", fs * 0.8) + 10.0 + s.text_width(name, fs) + 14.0
        } else {
            s.text_width(name, fs) + 14.0
        }
    };
    let row_w = |s: &Screen, v: &[(String, u8)]| -> f32 {
        v.iter().map(|(nm, k)| pill_w(s, nm, *k) + gap).sum::<f32>()
    };
    // labels need room above a node (wide) or beside the graph (tall)
    let (len, across) = if vertical { (a.h, a.w) } else { (a.w, a.h) };
    let pad_a = if small { 22.0 } else { 34.0 };
    let pad_lab = if vertical {
        0.0
    } else {
        (ph + 3.0) * 2.0 + 6.0
    };
    // tall: the names of one generation share a row
    let mut rows: HashMap<i32, f32> = HashMap::new();
    for i in (0..n).filter(|&i| shown[i]) {
        *rows.entry(lay[i].0.round() as i32).or_insert(0.0) += row_w(s, &names[i]);
    }
    let tail = if vertical {
        rows.values().fold(0.0f32, |m, &v| m.max(v)).min(a.w * 0.55) + 8.0
    } else {
        0.0
    };
    let mut row_x: HashMap<i32, f32> = HashMap::new();
    let span_g = (max_gen - min_gen).max(1.0);
    let step_g = ((len - 2.0 * pad_a) / span_g).min(if small { 64.0 } else { 120.0 });
    let step_l = ((across - pad_a - pad_lab - tail) / max_lane.max(1.0))
        .min(if small { 56.0 } else { 120.0 })
        .max(10.0);
    let r0 = (step_g.min(step_l) * 0.3).clamp(
        if small { 6.0 } else { 9.0 },
        if small { 13.0 } else { 21.0 },
    );
    let used_g = (max_gen - min_gen) * step_g;
    let used_l = max_lane * step_l;
    let start_g = if vertical {
        a.y + (a.h - used_g) / 2.0
    } else {
        a.x + (a.w - used_g) / 2.0
    } - min_gen * step_g;
    let start_l = if vertical {
        a.x + ((a.w - tail) - used_l) / 2.0
    } else {
        a.y + pad_lab + (a.h - pad_lab - used_l) / 2.0
    };
    let xy = |p: (f32, f32)| -> (f32, f32) {
        let g = start_g + p.0 * step_g;
        let l = start_l + p.1 * step_l;
        if vertical {
            (l, g)
        } else {
            (g, l)
        }
    };
    let reach = reachable(r);
    let colour = |i: usize| wg::hex(t.lanes[(lay[i].1.round() as usize) % t.lanes.len()]);
    let line_w = (r0 * 0.22).max(1.6);
    // edges first
    for i in (0..n).filter(|&i| shown[i]) {
        let (x1, y1) = xy(lay[i]);
        for (pi, &p) in r.commits[i].parents.iter().enumerate() {
            let (x0, y0) = xy(lay[p]);
            let c = if pi == 0 { colour(i) } else { colour(p) };
            let c = wg::mix(wg::hex(t.line), c, 0.7);
            let c = if reach[i] {
                c
            } else {
                wg::mix(wg::hex(t.bg), c, 0.3)
            };
            curve(s, (x0, y0), (x1, y1), vertical, line_w, c);
        }
    }
    let bis = world.bisect.as_ref().filter(|_| !look.bare);
    for i in (0..n).filter(|&i| shown[i]) {
        let (x, y) = xy(lay[i]);
        let c = &r.commits[i];
        let col = if reach[i] {
            colour(i)
        } else {
            wg::mix(wg::hex(t.bg), colour(i), 0.35)
        };
        let hov = hover == Some(c.id.as_str());
        if reach[i] {
            s.disc(x, y + r0 * 0.12, r0 + 1.0, 0, 0.12);
        }
        s.disc(
            x,
            y,
            r0,
            if hov {
                wg::mix(col, wg::hex(t.ink), 0.25)
            } else {
                col
            },
            1.0,
        );
        if let Some(b) = bis {
            let mark = if b.found == Some(i) || b.bad == Some(i) {
                Some(t.bad)
            } else if b.good.contains(&i) {
                Some(t.good)
            } else {
                None
            };
            if let Some(m) = mark {
                s.ring(x, y, r0 + 3.5, 2.5, wg::hex(m), 1.0);
            }
        }
        let size = (r0 * 0.62).min(13.0);
        if size >= 7.0 {
            s.text_c(
                &c.id,
                x,
                y,
                s.fit_size(&c.id, size, r0 * 1.7),
                wg::hex(0xffffff),
                if reach[i] { 1.0 } else { 0.7 },
            );
        }
        spots.push((
            x - r0 - 3.0,
            y - r0 - 3.0,
            2.0 * r0 + 6.0,
            2.0 * r0 + 6.0,
            Spot::Commit(c.id.clone(), c.msg.clone()),
        ));
    }
    let right = start_l + used_l + r0 + 10.0;
    for i in (0..n).filter(|&i| shown[i]) {
        if names[i].is_empty() {
            continue;
        }
        let (x, y) = xy(lay[i]);
        // tall: one row right of the graph; wide: stacked above the node
        let row = row_x.entry(lay[i].0.round() as i32).or_insert(right);
        let mut px = *row;
        let mut py = y - r0 - 6.0 - ph;
        if vertical && px > x + r0 + 12.0 {
            let mut d = x + r0 + 3.0;
            while d < px - 4.0 {
                s.line(d, y, (d + 4.0).min(px - 4.0), y, 0.8, wg::hex(t.muted), 0.7);
                d += 8.0;
            }
        }
        for (name, kind) in &names[i] {
            let w = pill_w(s, name, *kind);
            let (qx, qy) = if vertical {
                let q = (px, y - ph / 2.0);
                px += w + gap;
                q
            } else {
                let q = (x - w / 2.0, py);
                py -= ph + 3.0;
                q
            };
            if vertical {
                *row_x.get_mut(&(lay[i].0.round() as i32)).unwrap() = px;
            }
            pill(s, t, qx, qy, w, ph, fs, name, *kind);
            if *kind != 0 {
                let bare = name.split(' ').next().unwrap_or(name).to_string();
                spots.push((qx, qy, w, ph, Spot::Name(bare)));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn pill(s: &mut Screen, t: &Theme, x: f32, y: f32, w: f32, h: f32, fs: f32, name: &str, kind: u8) {
    let (fill, ink, edge) = match kind {
        0 => (t.ink, t.bg, t.ink),
        1 => (t.accent, 0xffffff, t.accent),
        2 => (t.panel, t.ink, t.ink),
        3 => (t.panel, t.muted, t.muted),
        _ => (t.warn, 0xffffff, t.warn),
    };
    s.rect(
        x - 1.2,
        y - 1.2,
        w + 2.4,
        h + 2.4,
        h / 2.0 + 1.0,
        wg::hex(edge),
        1.0,
    );
    s.rect(x, y, w, h, h / 2.0, wg::hex(fill), 1.0);
    if kind == 1 {
        let hw = s.text_width("HEAD", fs * 0.8) + 10.0;
        s.rect(
            x + 2.0,
            y + 2.0,
            hw,
            h - 4.0,
            (h - 4.0) / 2.0,
            wg::hex(0xffffff),
            0.25,
        );
        s.text("HEAD", x + 7.0, y + h / 2.0, fs * 0.8, wg::hex(ink), 1.0);
        s.text(name, x + hw + 6.0, y + h / 2.0, fs, wg::hex(ink), 1.0);
    } else {
        s.text(name, x + 7.0, y + h / 2.0, fs, wg::hex(ink), 1.0);
    }
}

/// A line from parent to child that bends into the child's lane.
fn curve(s: &mut Screen, a: (f32, f32), b: (f32, f32), vertical: bool, w: f32, c: u32) {
    let alpha = 1.0;
    let same = if vertical {
        (a.0 - b.0).abs() < 0.5
    } else {
        (a.1 - b.1).abs() < 0.5
    };
    if same {
        s.line(a.0, a.1, b.0, b.1, w, c, alpha);
        return;
    }
    // cubic with both handles along the time axis
    let (c1, c2) = if vertical {
        let m = (a.1 + b.1) / 2.0;
        ((a.0, m), (b.0, m))
    } else {
        let m = (a.0 + b.0) / 2.0;
        ((m, a.1), (m, b.1))
    };
    let mut prev = a;
    let steps = 14;
    for k in 1..=steps {
        let t = k as f32 / steps as f32;
        let u = 1.0 - t;
        let p = (
            u * u * u * a.0 + 3.0 * u * u * t * c1.0 + 3.0 * u * t * t * c2.0 + t * t * t * b.0,
            u * u * u * a.1 + 3.0 * u * u * t * c1.1 + 3.0 * u * t * t * c2.1 + t * t * t * b.1,
        );
        s.line(prev.0, prev.1, p.0, p.1, w, c, alpha);
        prev = p;
    }
}
