//! A level in play: the world it starts from, the goal, and the commands
//! the player ran on it.

use crate::git::{World, ERR};
use crate::levels::LEVELS;

/// The repository a level starts from.
pub fn start(level: usize) -> World {
    let l = &LEVELS[level];
    let mut w = World::new(level as u32 * 7919 + 17, l.files);
    for c in l.setup {
        w.run(c);
        debug_assert!(
            !w.out.iter().any(|o| o.0 == ERR),
            "level {level} setup '{c}': {:?}",
            w.out
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_solution_reaches_its_goal() {
        for (i, l) in LEVELS.iter().enumerate() {
            let s = start(i);
            let mut w = start(i);
            assert!(
                s.snap() != goal(i).snap(),
                "level {i} {} starts solved",
                l.title
            );
            for c in l.solution {
                assert!(w.snap() != goal(i).snap(), "level {i} solved before '{c}'");
                let changed = w.run(c);
                assert!(
                    !w.out.iter().any(|o| o.0 == ERR),
                    "level {i} {} '{c}': {:#?}",
                    l.title,
                    w.out
                );
                assert!(changed, "level {i} '{c}' changed nothing");
            }
            assert!(w.snap() == goal(i).snap(), "level {i}");
        }
    }
}
