//! Plays a hole with the bot and stops partway through a putt, writing the
//! screen as raw RGBA for the card picture:
//! cargo run --release --example thumb out.rgba w h hole putts ms theme
use greenside::golf::*;
use greenside::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let num = |i: usize| a[i].parse::<i32>().unwrap();
    let (w, h, i, putts, ms) = (num(2), num(3), num(4) - 1, num(5), num(6));
    wg_resize(w, h);
    wg_option(0, i);
    wg_option(1, num(7));
    wg_init(1);
    // SAFETY: wg_state is the game's State, and nothing else holds it now
    let s = || unsafe { &mut *(wg_state() as *mut State) };
    let f = bot::Field::new(hole(i));
    for n in 0..putts {
        let (aim, power) = bot::best(s(), &f, 0.0);
        s().aim = aim;
        s().power = power;
        s().putt();
        let mut t = 0;
        while (s().phase == ROLL || s().phase == SPLASH) && (n + 1 < putts || t < ms) {
            wg_frame(0, 16);
            t += 16;
        }
    }
    wg_frame(0, 16);
    let px = unsafe { std::slice::from_raw_parts(wg_fb() as *const u8, (wg_w() * wg_h() * 4) as usize) };
    std::fs::write(&a[1], px).unwrap();
}
