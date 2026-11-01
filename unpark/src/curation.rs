//! Offline selection helper; the browser loads the reviewed campaign.rs data.
use super::*;
#[test]
#[ignore = "rewrites the campaign data; run explicitly when curating levels"]
fn export_campaign() {
    let mut out = String::from("// Fixed campaign. See games-src/campaigns.md for design and regeneration.\n");
    out.push_str(
        r#"fn campaign(level:i32)->Lot {
    let (w,h,props,fences,cars):(i32,i32,&[(usize,u8)],&[usize],&[(i8,i8,i8,i8,i8)])=match level {
"#,
    );
    for l in 0..LEVELS {
        let row = {
            let g = if l < 30 { campaign(l) } else { generate(l) };
            let props: Vec<_> = g.props.iter().enumerate().filter(|(_, v)| **v != 0).map(|(i, &v)| (i, v)).collect();
            let fences: Vec<_> = g.fence.iter().enumerate().filter(|(_, v)| **v != 0).map(|(i, _)| i).collect();
            let cars: Vec<_> = g.cars[..g.n as usize].iter().map(|c| (c.x, c.y, c.len, c.dir, c.model)).collect();
            format!("({}, {}, &{:?}, &{:?}, &{:?})", g.w, g.h, props, fences, cars)
        };
        out.push_str(&format!("        {} => {}, // Level {}\n", l, row, l + 1));
    }
    out.push_str(
        r#"
    _=>unreachable!("campaign level"), };
    let mut g=Lot::empty(w,h); for &(i,v) in props {g.props[i]=v;} for &i in fences {g.fence[i]=1;}
    for (i,&(x,y,len,dir,model)) in cars.iter().enumerate() {g.cars[i]=Car{x,y,len,dir,model,out:0};} g.n=cars.len() as i32; g
}
"#,
    );
    std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/src/campaign.rs"), out).unwrap();
}
