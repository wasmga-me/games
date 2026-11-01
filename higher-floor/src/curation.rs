//! Offline selection helper; the browser loads the reviewed campaign.rs data.
use super::*;
#[test]
#[ignore = "rewrites the campaign data; run explicitly when curating levels"]
fn export_campaign() {
    let mut out = String::from("// Fixed campaign. See games-src/campaigns.md for design and regeneration.\n");
    out.push_str(
        r#"fn campaign(level:i32)->Tower {
    let (floors,start,rooms):(i32,i32,&[(i8,i32,i8,i8,i8)])=match level {
"#,
    );
    for l in 0..LEVELS {
        let row = {
            let t = generate(l);
            let rooms: Vec<_> = t.rooms[..t.n()].iter().map(|r| (r.kind, r.val, r.look, r.grow, r.locked)).collect();
            format!("({}, {}, &{:?})", t.floors, t.start, rooms)
        };
        out.push_str(&format!("        {} => {}, // Level {}\n", l, row, l + 1));
    }
    out.push_str(
        r#"
    _=>unreachable!("campaign level"), };
    let mut t=Tower{cols:2,floors,start,rooms:[Room::default();MAXROOMS]};
    for (i,&(kind,val,look,grow,locked)) in rooms.iter().enumerate() { t.rooms[i]=Room{kind,val,look,grow,locked}; } t
}
"#,
    );
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/campaign.rs"), out).unwrap();
}
