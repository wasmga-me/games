//! Draws one view until it is sharp and writes the screen as raw RGBA, for
//! the card picture:
//! cargo run --release --example shot out.rgba w h cx cy span [set colours]
use fractal_dive::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let int = |i: usize, d: i32| a.get(i).map_or(d, |v| v.parse().unwrap());
    let num = |i: usize, d: f64| a.get(i).map_or(d, |v| v.parse().unwrap());
    let (w, h) = (int(2, 480), int(3, 420));
    wg_resize(w, h);
    wg_option(O_SET as i32, int(7, 0));
    wg_option(O_COLOURS as i32, int(8, 1));
    wg_init(1);
    // SAFETY: wg_state is the game's Save, and nothing else holds it now
    let s = unsafe { &mut *(wg_state() as *mut Save) };
    s.view = view::View { cx: num(4, -0.743643), cy: num(5, 0.131825), span: num(6, 0.0016) };
    for _ in 0..2000 {
        wg_frame(0, 16);
    }
    let px = unsafe { std::slice::from_raw_parts(wg_fb() as *const u8, (wg_w() * wg_h() * 4) as usize) };
    std::fs::write(&a[1], px).unwrap();
}
