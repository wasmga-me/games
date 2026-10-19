//! Plays every lesson's solution and checks it reaches the goal; built only
//! with the selfcheck feature and run by tools/check.mjs.

use crate::git::ERR;
use crate::levels::LEVELS;
use crate::play::{goal, start};

static mut TEXT: String = String::new();

pub(crate) fn log(s: &str) {
    unsafe {
        let t = &mut *std::ptr::addr_of_mut!(TEXT);
        t.push_str(s);
        t.push('\n');
    }
}

#[no_mangle]
pub extern "C" fn bw_check_text() -> *const u8 {
    unsafe { (&*std::ptr::addr_of!(TEXT)).as_ptr() }
}
#[no_mangle]
pub extern "C" fn bw_check_len() -> usize {
    unsafe { (&*std::ptr::addr_of!(TEXT)).len() }
}
#[no_mangle]
pub extern "C" fn bw_check_clear() {
    unsafe { (&mut *std::ptr::addr_of_mut!(TEXT)).clear() }
}
#[no_mangle]
pub extern "C" fn bw_check_levels() -> usize {
    LEVELS.len()
}

/// 0 when level `i`'s solution clears it; the text says what went wrong.
/// With `verbose`, logs every command's output.
#[no_mangle]
pub extern "C" fn bw_check(i: usize, verbose: i32) -> i32 {
    let l = &LEVELS[i];
    log(&format!("== {i} {}", l.title));
    let s = start(i);
    if verbose != 0 {
        for (_, t) in &s.out {
            log(&format!("  setup: {t}"));
        }
    }
    let g = goal(i).snap();
    if s.snap() == g {
        log("starts solved");
        return 1;
    }
    let mut w = s;
    for c in l.solution {
        if w.snap() == g {
            log(&format!("solved before '{c}'"));
            return 1;
        }
        log(&format!("$ {c}"));
        let changed = w.run(c);
        if verbose != 0 || w.out.iter().any(|o| o.0 == ERR) {
            for (k, t) in &w.out {
                log(&format!("  {}{t}", if *k == ERR { "! " } else { "" }));
            }
        }
        if w.out.iter().any(|o| o.0 == ERR) {
            return 1;
        }
        if !changed {
            log("changed nothing");
            return 1;
        }
        w.out.clear();
    }
    if w.snap() != g {
        log(&format!("not at the goal:\n{:?}\n{:?}", w.snap(), g));
        return 1;
    }
    0
}

/// Runs typed lines on a level's start, logging the output (for trying
/// commands from tools/check.mjs).
#[no_mangle]
pub extern "C" fn bw_try(i: usize, ptr: *const u8, len: usize) {
    let lines = unsafe { std::str::from_utf8_unchecked(std::slice::from_raw_parts(ptr, len)) };
    let mut w = start(i);
    for c in lines.lines() {
        log(&format!("$ {c}"));
        w.run(c);
        for (k, t) in &w.out {
            log(&format!("  {}{t}", if *k == ERR { "! " } else { "" }));
        }
        w.out.clear();
    }
}

#[no_mangle]
pub extern "C" fn bw_alloc(n: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(n);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}
