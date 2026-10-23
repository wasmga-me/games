//! Times wg_frame while the bot climbs: cargo run --release --example bench [w h]
use std::time::Instant;
use updraft::play::*;
use updraft::*;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let num = |i: usize, d: i32| a.get(i).map_or(d, |v| v.parse().unwrap());
    let (w, h) = (num(1, 1920), num(2, 1440));
    wg_resize(w, h);
    for theme in 0..4 {
        wg_option(0, 1);
        wg_option(1, theme);
        wg_init(7);
        // SAFETY: wg_state is the game's State, and nothing else holds it now
        let s = || unsafe { &mut *(wg_state() as *mut State) };
        let (mut n, mut ms, mut worst) = (0, 0.0f64, 0.0f64);
        while n < 3000 && !s().over() {
            let to = bot::steer(s());
            let t = Instant::now();
            s().frame(0, Some(to), 16, &mut NoFx);
            wg_frame(0, 0);
            let d = t.elapsed().as_secs_f64() * 1000.0;
            (n, ms, worst) = (n + 1, ms + d, worst.max(d));
        }
        println!("{w}x{h} theme {theme}: {:.2} ms avg, {:.2} worst over {n} frames, {} m", ms / n as f64, worst, s().height());
    }
}
