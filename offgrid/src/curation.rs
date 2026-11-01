//! Offline selection helper; the browser loads the reviewed campaign.rs data.
use super::*;
#[test]
#[ignore = "rewrites the campaign data; run explicitly when curating levels"]
fn export_campaign() {
    let mut out = String::from("// Fixed campaign. See games-src/campaigns.md for design and regeneration.\n");
    out.push_str(
        r#"fn campaign(level:i32)->Board {
    let (n,cells):(i32,&[u8])=match level {
"#,
    );
    for l in 0..LEVELS {
        let row = {
            let b = generate(l);
            format!("({}, &{:?})", b.n, &b.cell[..(b.n * b.n) as usize])
        };
        out.push_str(&format!("        {} => {}, // Level {}\n", l, row, l + 1));
    }
    out.push_str(
        r#"
    _=>unreachable!("campaign level"), };
    let mut b=Board{n,cell:[0;MAXN*MAXN]}; b.cell[..cells.len()].copy_from_slice(cells); b
}
"#,
    );
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/campaign.rs"), out).unwrap();
}
