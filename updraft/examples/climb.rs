//! Climbs with the bot on each pace and prints how high it got:
//! cargo run --release --example climb [seeds] [minutes]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seeds: u32 = a.get(1).map(|v| v.parse().unwrap()).unwrap_or(6);
    let mins: i32 = a.get(2).map(|v| v.parse().unwrap()).unwrap_or(10);
    for pace in 0..3 {
        let runs: Vec<(i32, i32)> = (1..=seeds).map(|k| updraft::bot::climb(pace, k * 7919, mins * 60_000)).map(|s| (s.height(), s.time_ms / 1000)).collect();
        println!("pace {pace}: (m, s) {runs:?}");
    }
}
