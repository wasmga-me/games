//! Climbs with the bot for a while and writes the screen as raw RGBA, for
//! the card picture: cargo run --release --example shot out.rgba w h ms [pace theme seed]
use updraft::play::*;
use updraft::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let num = |i: usize, d: i32| a.get(i).map_or(d, |v| v.parse().unwrap());
    let (w, h, ms) = (num(2, 360), num(3, 560), num(4, 20_000));
    wg_resize(w, h);
    wg_option(0, num(5, 1));
    wg_option(1, num(6, 0));
    wg_init(num(7, 7) as u32);
    // SAFETY: wg_state is the game's State, and nothing else holds it now
    let s = || unsafe { &mut *(wg_state() as *mut State) };
    let mut t = 0;
    while t < ms && !s().over() {
        let to = bot::steer(s());
        s().frame(0, Some(to), 16, &mut NoFx);
        wg_frame(0, 0);
        t += 16;
    }
    let px = unsafe { std::slice::from_raw_parts(wg_fb() as *const u8, (wg_w() * wg_h() * 4) as usize) };
    std::fs::write(&a[1], px).unwrap();
    println!("{} m, {} gems", s().height(), s().gem_count);
}
