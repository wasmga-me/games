//! Times wg_frame while the bot plays: cargo run --release --example bench [w h]
use greenside::golf::*;
use greenside::*;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let num = |i: usize, d: i32| a.get(i).map_or(d, |v| v.parse().unwrap());
    let (w, h) = (num(1, 1920), num(2, 1440));
    wg_resize(w, h);
    let one = std::env::var("ONE").is_ok();
    for theme in 0..if one { 1 } else { 4 } {
        for i in if one { vec![0] } else { vec![0, 6, 9, 15] } {
            wg_option(0, i);
            wg_option(1, theme);
            wg_init(1);
            // SAFETY: wg_state is the game's State, and nothing else holds it now
            let s = || unsafe { &mut *(wg_state() as *mut State) };
            let f = bot::Field::new(hole(i));
            let (mut n, mut ms, mut worst, mut drawn) = (0, 0.0f64, 0.0f64, 0);
            for _ in 0..2 {
                // aiming still for a while, then the putt
                for _ in 0..30 {
                    let t = Instant::now();
                    wg_frame(0, 16);
                    let d = t.elapsed().as_secs_f64() * 1000.0;
                    (n, ms, worst) = (n + 1, ms + d, worst.max(d));
                    drawn += wg_drawn();
                }
                let (aim, power) = bot::best(s(), &f, 0.0);
                s().aim = aim;
                s().power = power;
                s().putt();
                while s().phase == ROLL || s().phase == SPLASH {
                    let t = Instant::now();
                    wg_frame(0, 16);
                    let d = t.elapsed().as_secs_f64() * 1000.0;
                    (n, ms, worst) = (n + 1, ms + d, worst.max(d));
                    drawn += wg_drawn();
                }
            }
            println!("{w}x{h} theme {theme} hole {:2}: {:.2} ms avg, {:.2} worst, {drawn}/{n} drawn", i + 1, ms / n as f64, worst);
        }
    }
}
