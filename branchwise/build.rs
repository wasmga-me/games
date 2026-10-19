// Links libgit2 (built for WASI by libgit2/build.sh, run here when its
// output is missing or older than the glue).
use std::path::Path;
use std::process::Command;

fn main() {
    let out = Path::new("libgit2/deps/out");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }
    for f in ["libgit2/build.sh", "libgit2/shim.c", "libgit2/wasi.diff"] {
        println!("cargo:rerun-if-changed={f}");
    }
    let stale = |f: &str| {
        let t = |p: &Path| p.metadata().and_then(|m| m.modified()).ok();
        match (t(&out.join("libbwshim.a")), t(Path::new(f))) {
            (Some(a), Some(b)) => b > a,
            _ => true,
        }
    };
    if stale("libgit2/shim.c") || stale("libgit2/build.sh") || stale("libgit2/wasi.diff") {
        let ok = Command::new("sh").arg("libgit2/build.sh").status().map(|s| s.success());
        assert!(matches!(ok, Ok(true)), "libgit2/build.sh failed");
    }
    let dir = std::fs::canonicalize(out).unwrap();
    println!("cargo:rustc-link-search=native={}", dir.display());
    for l in [
        "bwshim",
        "git2",
        "c",
        "wasi-emulated-mman",
        "wasi-emulated-signal",
        "wasi-emulated-process-clocks",
        "wasi-emulated-getpid",
        "clang_rt.builtins-wasm32",
    ] {
        println!("cargo:rustc-link-lib=static={l}");
    }
}
