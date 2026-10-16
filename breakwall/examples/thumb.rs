//! Plays a while with the bot and writes the screen as raw RGBA, for the
//! card thumbnail: cargo run --release --example thumb out.rgba w h level ms
use breakwall::game::*;
use breakwall::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (w, h): (i32, i32) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let level: i32 = a[4].parse().unwrap();
    let ms: i32 = a[5].parse().unwrap();
    wg_resize(w, h);
    wg_init(7);
    // SAFETY: wg_state is the game's State, and nothing else holds it now
    let s = || unsafe { &mut *(wg_state() as *mut State) };
    s().load_level(level);
    let mut t = 0;
    while t < ms {
        bot(s(), 8.0);
        if t == (ms - 1200) / 16 * 16 {
            s().boost(B_MULTI);
            s().boost(B_LASER);
            s().boost(B_WIDE);
        }
        wg_frame(0, 16);
        s().lives = 3;
        t += 16;
    }
    let px = unsafe { std::slice::from_raw_parts(wg_fb() as *const u8, (wg_w() * wg_h() * 4) as usize) };
    std::fs::write(&a[1], px).unwrap();
    println!("score {} level {}", s().score, s().level + 1);
}
