//! Offline room selection. No random search runs in the browser.
use super::*;
use wg::Rng;

fn readable(r: &Room) -> Vec<(u32, bool)> {
    let (mut survey, _) = r.survey();
    for (k, item) in survey.iter_mut().enumerate() {
        let p = r.trace(Room::step_angle(k));
        let distance: f32 = p.pts.windows(2).take(3).map(|w| dist(w[0], w[1])).sum();
        item.0 = p.foes.iter().filter(|&&(_, d)| d <= distance).fold(0, |m, &(i, _)| m | 1 << i);
    }
    survey
}
fn candidate(rng: &mut Rng, level: i32) -> Option<Room> {
    let mut r = Room { agent: (if rng.below(2) == 0 { 25. } else { 275. }, FLOOR), shots: 4, ..Room::empty() };
    let mut positions = vec![(25., FLOOR), (75., FLOOR), (125., FLOOR), (175., FLOOR), (225., FLOOR), (275., FLOOR)];
    for col in 0..3 {
        if rng.below(5) == 0 {
            continue;
        }
        let x = 55. + col as f32 * 70.;
        let y = [170., 205., 240., 275.][rng.below(4) as usize];
        let w = [35., 45., 55.][rng.below(3) as usize];
        let h = if rng.below(2) == 0 { 12. } else { FLOOR - y };
        r.blocks.push(Rect { x, y, w, h });
        positions.push((x + w / 2., y));
    }
    if level % 3 == 0 {
        let x = 95. + rng.below(4) as f32 * 20.;
        let y = 100. + rng.below(3) as f32 * 15.;
        r.plates.push(Plate { a: (x, y), b: (x + 45., y + if rng.below(2) == 0 { 25. } else { -25. }) });
    }
    positions.retain(|&(x, feet)| {
        let rect = Person { x, feet, look: 0 }.rect();
        (x - r.agent.0).abs() > HALF_W * 2. && !r.blocks.iter().any(|b| b.overlaps(&rect))
    });
    rng.shuffle(&mut positions);
    let foes = 3 + usize::from(level >= 60 && rng.below(2) == 0);
    let hostages = 1 + usize::from(level >= 80 && rng.below(2) == 0);
    if positions.len() < foes + hostages {
        return None;
    }
    for &(x, feet) in &positions[..foes] {
        r.foes.push(Person { x, feet, look: (rng.below(3)) as u8 });
    }
    for &(x, feet) in &positions[foes..foes + hostages] {
        r.hostages.push(Person { x, feet, look: 0 });
    }
    Some(r)
}

#[test]
#[ignore = "offline campaign export"]
fn export_campaign() {
    let mut out = String::from("// Generated offline; every room is replayed in tests.\nfn extended(level:i32)->Room {match level {\n");
    let mut seen = std::collections::HashSet::new();
    for l in 30..LEVELS {
        let mut rng = Rng::new(0xBAA5_2026 ^ (l as u32).wrapping_mul(7919));
        let mut accepted = None;
        for _ in 0..4000 {
            let Some(mut r) = candidate(&mut rng, l) else { continue };
            let (_, direct) = r.survey();
            if direct == r.all_foes() {
                continue;
            }
            let survey = readable(&r);
            let Some(par) = Room::shots_needed(&Room::good_shots(&survey, window(l)), r.all_foes(), 0) else { continue };
            if !(2..=3).contains(&par) {
                continue;
            }
            let key = format!("{:?}{:?}{:?}{:?}{:?}", r.blocks, r.plates, r.agent, r.foes, r.hostages);
            if !seen.insert(key) {
                continue;
            }
            r.par = par;
            accepted = Some(r);
            break;
        }
        let r = accepted.expect("bounded campaign selection failed");
        out.push_str(&format!(
            "{l} => Room {{ blocks: vec!{:?}, plates: vec!{:?}, crates: vec![], agent: {:?}, foes: vec!{:?}, hostages: vec!{:?}, shots: 4, par: {} }},\n",
            r.blocks, r.plates, r.agent, r.foes, r.hostages, r.par
        ));
        eprintln!("accepted level {}", l + 1);
    }
    out.push_str("_=>unreachable!(\"campaign level\")}}\n");
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/campaign.rs"), out).unwrap();
}
