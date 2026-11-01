//! Offline selection helper; the browser loads the reviewed campaign.rs data.
use super::*;
#[test]
#[ignore = "rewrites the campaign data; run explicitly when curating levels"]
fn export_campaign() {
    let mut out = String::from("// Fixed campaign. See games-src/campaigns.md for design and regeneration.\n");
    out.push_str(
        r#"fn campaign(level:i32)->Grid {
    let (n,tap,drain,pipes,turns):(i32,i32,i32,&[u8],&[u8])=match level {
"#,
    );
    for l in 0..LEVELS {
        let row = {
            let g = if l < 30 { campaign(l) } else { generate(l) };
            format!("({}, {}, {}, &{:?}, &{:?})", g.n, g.tap, g.drain, &g.pipe[..(g.n * g.n) as usize], &g.rot[..(g.n * g.n) as usize])
        };
        out.push_str(&format!("        {} => {}, // Level {}\n", l, row, l + 1));
    }
    out.push_str(
        r#"
    _=>unreachable!("campaign level"), };
    let mut g=Grid{n,tap,drain,pipe:[0;MAXN*MAXN],rot:[0;MAXN*MAXN]};
    g.pipe[..pipes.len()].copy_from_slice(pipes); g.rot[..turns.len()].copy_from_slice(turns); g
}
"#,
    );
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/campaign.rs"), out).unwrap();
}
