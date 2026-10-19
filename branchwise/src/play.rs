//! A level in play: the world it starts from, the goal, and the commands
//! the player ran on it.

use crate::git::World;
#[cfg(feature = "selfcheck")]
use crate::git::ERR;
use crate::levels::LEVELS;

/// The repository a level starts from.
pub fn start(level: usize) -> World {
    let l = &LEVELS[level];
    let mut w = World::new(level as u32 * 7919 + 17, l.files);
    for c in l.setup {
        w.run(c);
        #[cfg(feature = "selfcheck")]
        if w.out.iter().any(|o| o.0 == ERR) {
            crate::check::log(&format!("setup '{c}': {:?}", w.out));
        }
        w.out.clear();
    }
    w.freeze();
    w
}

/// The repository its goal describes.
pub fn goal(level: usize) -> World {
    let mut w = start(level);
    for c in LEVELS[level].solution {
        w.run(c);
    }
    w.out.clear();
    w
}
