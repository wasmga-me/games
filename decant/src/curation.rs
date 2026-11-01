//! Offline selection helper; the browser loads the reviewed campaign.rs data.
use super::*;
#[test]
#[ignore = "rewrites the campaign data; run explicitly when curating levels"]
fn export_campaign() {
    let mut out = String::from("// Fixed campaign. See games-src/campaigns.md for design and regeneration.\n");
    out.push_str(
        r#"fn campaign(level:i32)->(Stand,i32) {
    let (rows,par):(&[[u8;CAP]],i32)=match level {
"#,
    );
    for l in 0..LEVELS {
        let row = {
            let (s, par) = if l < 30 { campaign(l) } else { generate(l) };
            format!("(&{:?}, {})", &s.tube[..s.n as usize], par)
        };
        out.push_str(&format!("        {} => {}, // Level {}\n", l, row, l + 1));
    }
    out.push_str(
        r#"
    _=>unreachable!("campaign level"), };
    let mut s=Stand::empty(rows.len() as i32); s.tube[..rows.len()].copy_from_slice(rows); (s,par)
}
"#,
    );
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/campaign.rs"), out).unwrap();
}
