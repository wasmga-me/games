//! Offline selection helper; the browser loads the reviewed campaign.rs data.
use super::*;
#[test]
#[ignore = "rewrites the campaign data; run explicitly when curating levels"]
fn export_campaign() {
    let mut out = String::from("// Fixed campaign. See games-src/campaigns.md for design and regeneration.\n");
    out.push_str(
        r#"fn campaign(level:i32)->(Yard,&'static [usize]) {
    let (w,h,cells,queue,order):(i32,i32,&[u8],&[u8],&[usize])=match level {
"#,
    );
    for l in 0..LEVELS {
        let row = {
            let (y, order) = generate_order(l);
            format!("({}, {}, &{:?}, &{:?}, &{:?})", y.w, y.h, &y.cell[..(y.w * y.h) as usize], &y.queue[..y.buses as usize], order)
        };
        out.push_str(&format!("        {} => {}, // Level {}\n", l, row, l + 1));
    }
    out.push_str(
        r#"
    _=>unreachable!("campaign level"), };
    let mut y=Yard::empty(w,h); y.cell[..cells.len()].copy_from_slice(cells); y.queue[..queue.len()].copy_from_slice(queue); y.buses=queue.len() as i32; (y,order)
}
"#,
    );
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/campaign.rs"), out).unwrap();
}
