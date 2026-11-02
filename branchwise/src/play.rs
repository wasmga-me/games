//! A level in play: the world it starts from, the goal, and the commands
//! the player ran on it.

use crate::git::World;
#[cfg(feature = "selfcheck")]
use crate::git::ERR;
use crate::levels::LEVELS;

/// The repository a level starts from, before it is scoped.
fn start(level: usize) -> World {
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

/// The repository a level starts from and the one its goal describes,
/// both judging only names one of them has, so files, branches and tags
/// the player adds along the way don't count.
pub fn start_and_goal(level: usize) -> (World, World) {
    let mut s = start(level);
    let mut g = s.clone();
    for c in LEVELS[level].solution {
        g.run(c);
    }
    g.out.clear();
    let mut names = s.names();
    names.extend(g.names());
    let names = std::rc::Rc::new(names);
    s.set_scope(names.clone());
    g.set_scope(names);
    (s, g)
}
