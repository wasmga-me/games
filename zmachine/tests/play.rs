//! Plays the openings of Zork I, II and III through the game's own keys and
//! checks what the story prints. `cargo test` in this folder.
use wg::Game;
use zmachine::{vm, Story, Term, K_ENTER};

struct Zork1;
impl Story for Zork1 {
    const STORY: &'static [u8] = include_bytes!("../../zork1/zork1.z3");
}
struct Zork2;
impl Story for Zork2 {
    const STORY: &'static [u8] = include_bytes!("../../zork2/zork2.z3");
}
struct Zork3;
impl Story for Zork3 {
    const STORY: &'static [u8] = include_bytes!("../../zork3/zork3.z3");
}

fn game<S: Story>() -> Term<S> {
    let mut t = Term::<S>::new();
    t.start(12345);
    t
}

/// Types a command and returns what the story printed after it.
fn say<S: Story>(t: &mut Term<S>, cmd: &str) -> String {
    let before = t.transcript().len();
    for b in cmd.bytes() {
        assert!(t.key(b as i32), "key {:?} not taken", b as char);
    }
    assert!(t.key(K_ENTER));
    let all = t.transcript();
    if std::env::var("SHOW").is_ok() {
        print!("{}", &all[before.min(all.len())..]);
    }
    all[before.min(all.len())..].to_string()
}

fn has(out: &str, want: &str) {
    assert!(out.contains(want), "expected {:?} in:\n{}", want, out);
}

#[test]
fn zork1_opening() {
    let mut t = game::<Zork1>();
    let start = t.transcript();
    has(&start, "ZORK I: The Great Underground Empire");
    has(&start, "West of House\nYou are standing in an open field west of a white house, with a boarded front door.");
    has(&start, "There is a small mailbox here.");
    assert_eq!(t.mode(), vm::READ);
    assert_eq!(t.status().0, "West of House");
    has(&say(&mut t, "open mailbox"), "Opening the small mailbox reveals a leaflet.");
    has(&say(&mut t, "read leaflet"), "WELCOME TO ZORK!");
    has(&say(&mut t, "north"), "North of House");
    has(&say(&mut t, "east"), "Behind House");
    has(&say(&mut t, "open window"), "With great effort, you open the window far enough to allow entry.");
    let o = say(&mut t, "enter");
    has(&o, "Kitchen");
    has(&o, "On the table is an elongated brown sack, smelling of hot peppers.");
    assert_eq!(t.status().1, 10, "entering the kitchen scores 10");
    has(&say(&mut t, "west"), "Living Room");
    has(&say(&mut t, "take lamp"), "Taken.");
    has(&say(&mut t, "inventory"), "A brass lantern");
    has(&say(&mut t, "xyzzy"), "A hollow voice says \"Fool.\"");
    has(&say(&mut t, "frobnicate"), "I don't know the word \"frobnicate\".");
    let (room, score, moves) = t.status();
    assert_eq!(room, "Living Room");
    assert_eq!(score, 10);
    assert!(moves >= 9, "moves {}", moves);
    assert_eq!(t.score(), 10);
}

#[test]
fn zork1_walkthrough_scores() {
    let mut t = game::<Zork1>();
    // the egg in the tree
    for cmd in ["n", "n", "u"] {
        say(&mut t, cmd);
    }
    has(&say(&mut t, "take egg"), "Taken.");
    assert_eq!(t.score(), 5);
    for cmd in ["d", "s", "e", "open window", "enter", "w"] {
        say(&mut t, cmd);
    }
    assert_eq!(t.score(), 15, "10 for getting into the kitchen");
    has(&say(&mut t, "open case"), "Opened.");
    has(&say(&mut t, "put egg in case"), "Done.");
    assert_eq!(t.score(), 20, "5 for the egg in the case");
    for cmd in ["take lamp", "move rug", "open trap door", "turn on lamp"] {
        say(&mut t, cmd);
    }
    has(&say(&mut t, "d"), "The trap door crashes shut");
    assert_eq!(t.status().0, "Cellar");
    assert_eq!(t.score(), 45, "25 for the cellar");
    // the painting in the gallery, up the chimney and into the case
    for cmd in ["s", "e", "take painting"] {
        say(&mut t, cmd);
    }
    assert_eq!(t.status().0, "Gallery");
    assert_eq!(t.score(), 49);
    has(&say(&mut t, "n"), "Studio");
    has(&say(&mut t, "u"), "Kitchen");
    say(&mut t, "w");
    has(&say(&mut t, "put painting in case"), "Done.");
    assert_eq!(t.score(), 55, "6 for the painting in the case");
    has(&say(&mut t, "score"), "Your score is 55");
}

#[test]
fn save_restore_restart() {
    let mut t = game::<Zork1>();
    say(&mut t, "open mailbox");
    say(&mut t, "take leaflet");
    has(&say(&mut t, "save"), "Ok.");
    say(&mut t, "drop leaflet");
    say(&mut t, "north");
    assert_eq!(t.status().0, "North of House");
    let o = say(&mut t, "restore");
    has(&o, "Ok.");
    assert_eq!(t.status().0, "West of House");
    has(&say(&mut t, "inventory"), "A leaflet");
    // a restore with nothing saved fails in a new game
    let mut fresh = game::<Zork1>();
    has(&say(&mut fresh, "restore"), "Failed.");
    // restart asks first
    let o = say(&mut t, "restart");
    has(&o, "Do you wish to restart?");
    let o = say(&mut t, "y");
    has(&o, "West of House");
    has(&say(&mut t, "inventory"), "empty-handed");
    // quit ends the run
    say(&mut t, "quit");
    say(&mut t, "y");
    assert_eq!(t.mode(), vm::QUIT);
    assert!(t.over());
}

/// One frame with nothing pressed; its events.
fn frame<S: Story>(t: &mut Term<S>) -> u32 {
    t.update(&wg::Input { held: 0, pressed: 0, dt: 16 })
}

#[test]
fn restart_reports_the_run() {
    let mut t = game::<Zork1>();
    assert_eq!(frame(&mut t) & wg::EV_GAMEOVER, 0);
    for cmd in ["n", "e", "open window", "enter"] {
        say(&mut t, cmd);
    }
    assert_eq!(t.score(), 10);
    has(&say(&mut t, "restart"), "Do you wish to restart?");
    has(&say(&mut t, "y"), "West of House");
    // the next frame ends the run that restarted, with the score it reached
    assert_ne!(frame(&mut t) & wg::EV_GAMEOVER, 0);
    assert_eq!(t.score(), 10);
    assert!(!t.over());
    // and the new run plays on from nothing
    assert_eq!(frame(&mut t) & wg::EV_GAMEOVER, 0);
    assert_eq!(t.score(), 0);
    assert_eq!(t.mode(), vm::READ);
    // quitting still ends a run once
    say(&mut t, "quit");
    say(&mut t, "y");
    assert_ne!(frame(&mut t) & wg::EV_GAMEOVER, 0);
    assert_eq!(frame(&mut t) & wg::EV_GAMEOVER, 0);
}

#[test]
fn snapshot_resumes() {
    // the wg save is the whole machine: copying it into a new game resumes
    let mut a = game::<Zork1>();
    for cmd in ["n", "e", "open window", "enter", "take sack", "save", "w"] {
        say(&mut a, cmd);
    }
    let bytes: Vec<u8> = wg::bytes_of(&*a.save()).to_vec();
    let mut b = Term::<Zork1>::new();
    b.start(1);
    // SAFETY: the same Plain type, byte for byte
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), b.save() as *mut _ as *mut u8, bytes.len()) };
    assert!(b.loaded());
    assert_eq!(b.transcript(), a.transcript());
    has(&say(&mut b, "look"), "Living Room");
    has(&say(&mut b, "inventory"), "A brown sack");
    has(&say(&mut b, "restore"), "Ok.");
    assert_eq!(b.status().0, "Kitchen");
    // the same commands give the same text in both
    let x = say(&mut a, "open sack");
    let y = say(&mut b, "open sack");
    assert!(!x.is_empty() && !y.is_empty());
}

#[test]
fn zork2_opening() {
    let mut t = game::<Zork2>();
    let start = t.transcript();
    has(&start, "ZORK II: The Wizard of Frobozz");
    has(&start, "Inside the Barrow");
    has(&say(&mut t, "take sword"), "Taken.");
    has(&say(&mut t, "take lamp"), "Taken.");
    has(&say(&mut t, "s"), "Narrow Tunnel");
    has(&say(&mut t, "s"), "Foot Bridge");
    has(&say(&mut t, "s"), "Great Cavern");
    assert_eq!(t.status().0, "Great Cavern");
}

#[test]
fn zork3_opening() {
    let mut t = game::<Zork3>();
    let start = t.transcript();
    has(&start, "ZORK III: The Dungeon Master");
    if std::env::var("SHOW").is_ok() {
        print!("{}", start);
    }
    has(&start, "Endless Stair");
    has(&start, "Your old friend, the brass lantern, is at your feet.");
    has(&say(&mut t, "inventory"), "You are empty-handed.");
    has(&say(&mut t, "take lamp"), "Taken.");
    has(&say(&mut t, "turn on lamp"), "The lamp is now on.");
    let o = say(&mut t, "s");
    has(&o, "Junction");
    assert_eq!(t.status().0, "Junction");
}

#[test]
fn verify_and_typing() {
    let mut t = game::<Zork1>();
    has(&say(&mut t, "$verify"), "The disk is correct.");
    // backspace, and the history
    for b in "lok".bytes() {
        t.key(b as i32);
    }
    t.key(8);
    t.key(8);
    for b in "ook".bytes() {
        t.key(b as i32);
    }
    t.key(K_ENTER);
    has(&t.transcript(), ">look\nWest of House");
    t.key(zmachine::K_UP);
    t.key(zmachine::K_UP);
    assert!(t.transcript().ends_with(">$verify"));
}

/// Many commands at random: the machine never stops on an instruction.
fn wander<S: Story>(seed: u32) {
    const WORDS: &[&str] = &[
        "n",
        "s",
        "e",
        "w",
        "ne",
        "nw",
        "se",
        "sw",
        "u",
        "d",
        "look",
        "inventory",
        "take all",
        "drop all",
        "open door",
        "close door",
        "read book",
        "turn on lamp",
        "turn off lamp",
        "eat food",
        "kill troll with sword",
        "attack thief",
        "pray",
        "wait",
        "score",
        "diagnose",
        "climb tree",
        "go in",
        "out",
        "take lamp",
        "take sword",
        "examine me",
        "jump",
        "yes",
        "no",
        "again",
        "save",
        "restore",
        "listen",
        "say hello",
        "open bag",
        "put all in bag",
        "light match",
        "tie rope to railing",
        "push button",
        "brief",
        "verbose",
        "superbrief",
        "move rug",
        "open trap door",
        "enter",
        "wave wand",
        "zork",
        "plugh",
        "dig",
        "swim",
        "throw sword at me",
        "take all but lamp",
        "x lamp. n. s",
    ];
    let mut t = game::<S>();
    let mut r = seed;
    for _ in 0..3000 {
        r ^= r << 13;
        r ^= r >> 17;
        r ^= r << 5;
        if t.over() {
            t.key(K_ENTER); // starts again
            continue;
        }
        assert_eq!(t.mode(), vm::READ);
        let cmd = WORDS[r as usize % WORDS.len()];
        say(&mut t, cmd);
        let s = t.transcript();
        assert!(!s.contains("The story stopped"), "{}", &s[s.len().saturating_sub(800)..]);
    }
}

#[test]
fn zork1_wanders() {
    wander::<Zork1>(1);
    wander::<Zork1>(77);
}
#[test]
fn zork2_wanders() {
    wander::<Zork2>(2);
    wander::<Zork2>(99);
}
#[test]
fn zork3_wanders() {
    wander::<Zork3>(3);
    wander::<Zork3>(55);
}
