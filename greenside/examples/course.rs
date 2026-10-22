//! Draws holes as raw RGBA after the bot has putted a few times, for checking
//! the course and making the card picture:
//! cargo run --release --example course out-dir w h putts [holes...]
use greenside::golf::*;
use greenside::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (w, h): (i32, i32) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let putts: i32 = a[4].parse().unwrap();
    let holes: Vec<i32> = if a.len() > 5 { a[5..].iter().map(|v| v.parse::<i32>().unwrap() - 1).collect() } else { (0..18).collect() };
    wg_resize(w, h);
    for i in holes {
        wg_option(0, i);
        wg_init(1);
        // SAFETY: wg_state is the game's State, and nothing else holds it now
        let s = || unsafe { &mut *(wg_state() as *mut State) };
        let f = bot::Field::new(hole(i));
        for _ in 0..putts {
            if s().phase != AIM || s().hole != i {
                break;
            }
            let (aim, power) = bot::best(s(), &f, 0.0);
            s().aim = aim;
            s().power = power;
            s().putt();
            while s().phase == ROLL || s().phase == SPLASH {
                wg_frame(0, 16);
            }
        }
        wg_frame(0, 16);
        let px = unsafe { std::slice::from_raw_parts(wg_fb() as *const u8, (wg_w() * wg_h() * 4) as usize) };
        std::fs::write(format!("{}/hole{:02}.rgba", a[1], i + 1), px).unwrap();
    }
}
