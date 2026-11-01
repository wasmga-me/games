//! Offline selection helper; the browser loads the reviewed campaign.rs data.
use super::*;
#[test]
#[ignore = "rewrites the campaign data; run explicitly when curating levels"]
fn export_campaign() {
    let mut out = String::from("// Fixed campaign. See games-src/campaigns.md for design and regeneration.\n");
    out.push_str(
        r#"fn campaign(level:i32)->(Level,&'static [usize]) {
    let (plates,screws,boxes,order):(&[(i16,i16,i16,i16,u8)],&[(i16,i16,u8,u8)],&[u8],&[usize])=match level {
"#,
    );
    for l in 0..LEVELS {
        let row = {
            let (g, order) = generate_order(l);
            let plates: Vec<_> = g.plate[..g.plates as usize].iter().map(|p| (p.ax, p.ay, p.bx, p.by, p.layer)).collect();
            let screws: Vec<_> = g.screw[..g.screws as usize].iter().map(|s| (s.x, s.y, s.plate, s.colour)).collect();
            format!("(&{:?}, &{:?}, &{:?}, &{:?})", plates, screws, &g.colour[..g.boxes as usize], order)
        };
        out.push_str(&format!("        {} => {}, // Level {}\n", l, row, l + 1));
    }
    out.push_str(
        r#"
    _=>unreachable!("campaign level"), };
    let mut g=Level::empty();
    for (i,&(ax,ay,bx,by,layer)) in plates.iter().enumerate() {g.plate[i]=Plate{ax,ay,bx,by,layer,..Plate::default()};}
    for (i,&(x,y,plate,colour)) in screws.iter().enumerate() {g.screw[i]=Screw{x,y,plate,colour,..Screw::default()};}
    g.plates=plates.len() as i32; g.screws=screws.len() as i32; g.boxes=boxes.len() as i32;
    g.colour[..boxes.len()].copy_from_slice(boxes); g.open=[0,1]; g.next=2; (g,order)
}
"#,
    );
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/campaign.rs"), out).unwrap();
}
