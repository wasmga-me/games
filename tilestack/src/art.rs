//! The pictures on the faces: art.bin, made by art/build.mjs from
//! FluffyStuff's riichi-mahjong-tiles (CC0) and our own flowers, seasons and
//! white dragon, drawn once at one size and scaled to the tile here.

use wg::Screen;

static ART: &[u8] = include_bytes!("art.bin");

fn u16_at(i: usize) -> usize {
    u16::from_le_bytes([ART[i], ART[i + 1]]) as usize
}
fn u32_at(i: usize) -> usize {
    u32::from_le_bytes([ART[i], ART[i + 1], ART[i + 2], ART[i + 3]]) as usize
}

/// Pictures stored, and their size in pixels.
#[cfg(test)]
fn count() -> usize {
    u16_at(4)
}
fn size() -> (usize, usize) {
    (u16_at(6), u16_at(8))
}

/// One picture: its colours (premultiplied 0xAABBGGRR) and packed pixels.
fn find(face: usize) -> (&'static [u8], &'static [u8]) {
    let mut o = 10;
    for f in 0.. {
        let colours = ART[o] as usize;
        let pal = &ART[o + 1..o + 1 + colours * 4];
        o += 1 + colours * 4;
        let len = u32_at(o);
        let data = &ART[o + 4..o + 4 + len];
        if f == face {
            return (pal, data);
        }
        o += 4 + len;
    }
    unreachable!()
}

/// The picture's pixels as premultiplied r, g, b, a from 0 to 1.
fn pixels(face: usize) -> Vec<[f32; 4]> {
    let (w, h) = size();
    let (pal, data) = find(face);
    let colour = |k: u8| {
        let k = k as usize * 4;
        [0, 1, 2, 3].map(|c| pal[k + c] as f32 / 255.0)
    };
    let mut out = Vec::with_capacity(w * h);
    let mut i = 0;
    while i < data.len() && out.len() < w * h {
        let n = data[i] as usize;
        if n < 128 {
            out.extend(data[i + 1..i + 2 + n].iter().map(|&k| colour(k)));
            i += 2 + n;
        } else {
            let c = colour(data[i + 1]);
            out.extend(core::iter::repeat(c).take(n - 126));
            i += 2;
        }
    }
    out.resize(w * h, [0.0; 4]);
    out
}

/// Weights for resampling `n` source pixels onto the screen pixels `p0..p1`,
/// the picture spanning from screen x `a` to `b`: for each screen pixel the
/// first source pixel and the weights from there. A tent as wide as one
/// screen pixel when shrinking (so every source pixel counts), as one source
/// pixel when growing.
fn taps(n: usize, a: f32, b: f32, p0: i32, p1: i32) -> Vec<(usize, Vec<f32>)> {
    let k = n as f32 / (b - a).max(1e-3);
    let reach = k.max(1.0);
    (p0..p1)
        .map(|p| {
            let u = (p as f32 + 0.5 - a) * k;
            let tent = |j: i32| (1.0 - (j as f32 + 0.5 - u).abs() / reach).max(0.0);
            let (lo, hi) = ((u - reach - 0.5).floor() as i32, (u + reach - 0.5).ceil() as i32 + 1);
            // the tent's whole weight, counting the transparent pixels beyond
            // the picture's edges, so the edges fade
            let total: f32 = (lo..hi).map(tent).sum::<f32>().max(1e-6);
            let (lo, hi) = (lo.max(0), hi.min(n as i32).max(0));
            let ws = (lo..hi.max(lo)).map(|j| tent(j) / total).collect();
            (lo.min(n as i32) as usize, ws)
        })
        .collect()
}

/// Draws picture `face` filling the plan box x, y, w, h.
pub fn draw(s: &mut Screen, face: u8, x: f32, y: f32, w: f32, h: f32) {
    let (sw, sh) = size();
    let (ax, ay) = s.px(x, y);
    let (bx, by) = s.px(x + w, y + h);
    let (cols, rows) = s.size_px();
    let (x0, x1) = ((ax.floor() as i32).max(0), (bx.ceil() as i32).min(cols));
    let (y0, y1) = ((ay.floor() as i32).max(0), (by.ceil() as i32).min(rows));
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let src = pixels(face as usize);
    let tx = taps(sw, ax, bx, x0, x1);
    let ty = taps(sh, ay, by, y0, y1);
    let dw = (x1 - x0) as usize;
    // across first: every source row at the screen's columns
    let mut across = vec![[0.0f32; 4]; dw * sh];
    for r in 0..sh {
        let row = &src[r * sw..(r + 1) * sw];
        for (i, (lo, ws)) in tx.iter().enumerate() {
            let mut acc = [0.0f32; 4];
            for (j, &wt) in ws.iter().enumerate() {
                let p = row[lo + j];
                for c in 0..4 {
                    acc[c] += p[c] * wt;
                }
            }
            across[r * dw + i] = acc;
        }
    }
    // then down, blended straight onto the screen
    for (jy, (lo, ws)) in ty.iter().enumerate() {
        let line = s.row_px(y0 + jy as i32);
        for i in 0..dw {
            let mut acc = [0.0f32; 4];
            for (j, &wt) in ws.iter().enumerate() {
                let p = across[(lo + j) * dw + i];
                for c in 0..4 {
                    acc[c] += p[c] * wt;
                }
            }
            let a = acc[3].clamp(0.0, 1.0);
            if a <= 0.002 {
                continue;
            }
            let d = &mut line[x0 as usize + i];
            let mut out = 0xFF00_0000u32;
            for c in 0..3 {
                let under = ((*d >> (8 * c)) & 0xFF) as f32 / 255.0;
                let v = (acc[c] + under * (1.0 - a)).clamp(0.0, 1.0);
                out |= ((v * 255.0 + 0.5) as u32) << (8 * c);
            }
            *d = out;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_face_unpacks_whole() {
        assert_eq!(count(), crate::board::FACES as usize);
        let (w, h) = size();
        for f in 0..count() {
            let (pal, data) = find(f);
            // the packed pixels cover the picture exactly
            let (mut n, mut i) = (0, 0);
            while i < data.len() {
                let b = data[i] as usize;
                let (px, step) = if b < 128 { (b + 1, b + 2) } else { (b - 126, 2) };
                for k in 0..if b < 128 { b + 1 } else { 1 } {
                    assert!((data[i + 1 + k] as usize) < pal.len() / 4, "face {f}");
                }
                n += px;
                i += step;
            }
            assert_eq!(n, w * h, "face {f}");
            assert!(pixels(f).iter().any(|p| p[3] > 0.9), "face {f} is empty");
        }
    }
}
