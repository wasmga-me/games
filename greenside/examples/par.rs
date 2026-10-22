//! Plays every hole with the bot, sharp and sloppy, and prints the strokes:
//! cargo run --release --example par [seeds]
use greenside::*;

fn main() {
    let seeds: u32 = std::env::args().nth(1).map(|v| v.parse().unwrap()).unwrap_or(6);
    let (mut sharp_all, mut casual_all, mut par_all) = (0, 0.0, 0);
    for i in 0..18 {
        let par = golf::hole(i).par;
        let sharp = bot::hole(i, 0.0, 1);
        let casual: Vec<i32> = (1..=seeds).map(|s| bot::hole(i, 1.0, s * 7919)).collect();
        let avg = casual.iter().sum::<i32>() as f32 / seeds as f32;
        println!("hole {:2} par {} sharp {} casual {:.2} {:?}", i + 1, par, sharp, avg, casual);
        sharp_all += sharp;
        casual_all += avg;
        par_all += par;
    }
    println!("par {par_all} sharp {sharp_all} casual {casual_all:.1}");
}
