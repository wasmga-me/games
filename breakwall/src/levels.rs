//! The levels: up to 12 rows of 14 bricks. A row of 7 is the left half and
//! is mirrored, a row of 14 is used as it is.
//!   r o y g c b p w  coloured bricks, one hit
//!   s                silver, takes more hits
//!   G                gold, never breaks
//!   x                bomb, takes its neighbours with it
//!   .                empty

pub const COLS: usize = 14;
pub const ROWS: usize = 12;
pub const COUNT: usize = 30;

pub static LEVELS: &[&[&str]] = &[
    // 1
    &["", "rrrrrrr", "ooooooo", "yyyyyyy", "ggggggg", "bbbbbbb"],
    // 2
    &["......w", ".....wy", "....wyo", "...wyor", "..wyorp", ".wyorpb", "wyorpbc"],
    // 3
    &[
        "r.r.r.r.r.r.r.", ".o.o.o.o.o.o.o", "y.y.y.y.y.y.y.", ".g.g.g.g.g.g.g", "c.c.c.c.c.c.c.",
        ".b.b.b.b.b.b.b",
    ],
    // 4
    &["", "sssssss", "rrrrrrr", "ooooooo", "", "yyyyyyy", "ggggggg", "sssssss"],
    // 5
    &["", "r..o..y", "r..o..y", "r..o..y", "r..o..y", "s..s..s", "G..G..."],
    // 6
    &["......c", ".....cb", "....cbp", "...cbpr", "..cbprx", "...cbpr", "....cbp", ".....cb", "......c"],
    // 7
    &[
        "..g........g..", "...g......g...", "..gggggggggg..", ".gg.gggggg.gg.", "gggggggggggggg",
        "g.gggggggggg.g", "g.g........g.g", "...gg....gg...",
    ],
    // 8
    &["xrrrrrr", "ooooooo", "", "yyyyyyx", "ggggggg", "", "xbbbbbb", "ppppppp"],
    // 9
    &["GGGGG..", "G....s.", "G.rrrr.", "G.rooo.", "G.royy.", "G.royg.", "s......"],
    // 10
    &[
        "..rrr....rrr..", ".rrprr..rrprr.", ".rpprrrrrrrrr.", ".rrrrrrrrrrrr.", "..rrrrrrrrrr..",
        "...rrrrrrrr...", "....rrrrrr....", ".....rrrr.....", "......rr......",
    ],
    // 11
    &["s.s.s.s", ".r.o.y.", "s.s.s.s", ".g.c.b.", "s.s.s.s", ".p.w.r."],
    // 12
    &["", "ggggggg", "gxgggxg", "ggggggg", "", "bbbbbbb", "bxbbbxb", "bbbbbbb"],
    // 13
    &["ccccccc", "bbbbbbb", "pp....y", "pp....y", "ss.....", "GG....."],
    // 14
    &[
        "rr..rr..rr..rr", ".oo..oo..oo..o", "..yy..yy..yy..", "g..gg..gg..gg.", "cc..cc..cc..cc",
        ".bb..bb..bb..b", "..pp..pp..pp..", "w..ww..ww..ww.",
    ],
    // 15
    &["rrrrrrr", "ooooooo", "yyyyyyy", "GGGGG..", "", "ggggggg", "ccccccc"],
    // 16
    &["...bbbb", "..bpppp", ".bprrrr", ".bprxxx", ".bprrrr", "..bpppp", "...bbbb"],
    // 17
    &["s.....s", "rrrrrrr", "s.....s", "ooooooo", "s.....s", "yyyyyyy", "s.....s", "ggggggg", "s.....s"],
    // 18
    &[
        "....wwwwww....", "...wwwwwwww...", "..wwwwwwwwww..", "..w...ww...w..", "..ww.wwww.ww..",
        "..wwwwwwwwww..", "...www..www...", "....wwwwww....", "....w.ww.w....",
    ],
    // 19
    &[
        "s.s.s.s.s.s.s.", ".r.r.r.r.r.r.r", "s.s.s.s.s.s.s.", ".y.y.y.y.y.y.y", "s.s.s.s.s.s.s.",
        ".b.b.b.b.b.b.b", "s.s.s.s.s.s.s.",
    ],
    // 20
    &["xxxxxxx", "r.....r", "rxxxxxr", "r.....r", "rrrrrrr", "", "ooooooo", "oxoxoxo"],
    // 21
    &["G......", "Grrrr..", "GGG....", "ooooo..", "..GG...", "yyyyyyy", "", "ggg..gg"],
    // 22
    &["x.....r", "......r", "......r", "rrrrrrr", "......r", "......r", "x.....r", "sssssss"],
    // 23
    &[
        "r.............", "rr............", "rro...........", "rroo..........", "rrooy.........",
        "rrooyy........", "rrooyyg.......", "rrooyygg......", "rrooyyggc.....", "rrooyyggcc....",
        "rrooyyggccb...", "rrooyyggccbb..",
    ],
    // 24
    &["bb...pp", "sbb..pp", "bbb..pp", "", "..ccc..", "..cGc..", "..ccc..", "", "yy...xx", "yy...xx"],
    // 25
    &[
        "...yyyyyyyy...", "..yyyyyyyyyy..", ".yyGyyyyyyGyy.", ".yyyyyyyyyyyy.", ".yyyyyyyyyyyy.",
        ".yyGyyyyyyGyy.", "..yyGGGGGGyy..", "...yyyyyyyy...",
    ],
    // 26
    &["GGGGGGG", "Gsssss.", "Gsxxxx.", "Gsxppp.", "Gsxppp.", "Gsssss.", "GGGGG.."],
    // 27
    &[
        "c.b.c.b.c.b.c.", ".b.c.b.c.b.c.b", "c.b.c.b.c.b.c.", "..s...s...s...", ".s...s...s...s",
        "..G...G...G...", "", "pppppppppppppp",
    ],
    // 28
    &[".sss...", ".rrr...", ".ooo...", ".yyy...", ".ggg...", ".ccc...", ".bbb...", ".ppp...", ".GGG..x"],
    // 29
    &["sssssss", "srrrrrr", "sryyyyy", "srygggg", "srygxxx", "srygggg", "sryyyyy", "srrrrrr", "sssssss"],
    // 30
    &[
        "GGG.GGG", "sssssss", "xpppppp", "pbbbbbb", "pbccccc", "pbcxxxx", "pbccccc", "pbbbbbb", "xpppppp",
        "sssssss", "G.....G",
    ],
];

/// Brick kinds in the grid; 0 is empty.
pub const EMPTY: u8 = 0;
/// 1..=8 are the coloured bricks, in the order of COLOURS.
pub const SILVER: u8 = 9;
pub const GOLD: u8 = 10;
pub const BOMB: u8 = 11;

const COLOURS: &[u8] = b"roygcbpw";

pub fn kind_of(ch: u8) -> u8 {
    match ch {
        b's' => SILVER,
        b'G' => GOLD,
        b'x' => BOMB,
        b'.' => EMPTY,
        _ => match COLOURS.iter().position(|&c| c == ch) {
            Some(i) => i as u8 + 1,
            None => EMPTY,
        },
    }
}

/// The kind at row r, column c of level n (0-based).
pub fn brick(n: usize, r: usize, c: usize) -> u8 {
    let rows = LEVELS[n];
    if r >= rows.len() {
        return EMPTY;
    }
    let row = rows[r].as_bytes();
    let ch = match row.len() {
        0 => b'.',
        7 => row[if c < 7 { c } else { 13 - c }],
        _ => row[c],
    };
    kind_of(ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_matches() {
        assert_eq!(LEVELS.len(), COUNT);
    }

    #[test]
    fn rows_have_7_or_14_known_bricks() {
        for (n, rows) in LEVELS.iter().enumerate() {
            assert!(rows.len() <= ROWS, "level {} has {} rows", n + 1, rows.len());
            for row in rows.iter() {
                assert!(matches!(row.len(), 0 | 7 | 14), "level {}: {:?}", n + 1, row);
                for ch in row.bytes() {
                    assert!(ch == b'.' || kind_of(ch) != EMPTY, "level {}: {:?}", n + 1, ch as char);
                }
            }
        }
    }

    /// Every brick that can break is reachable from below without passing
    /// through gold, so no level is left unwinnable.
    #[test]
    fn every_breakable_brick_is_reachable() {
        for n in 0..LEVELS.len() {
            // one row of free space under the grid stands for the open field
            let mut seen = [[false; COLS]; ROWS + 1];
            let mut stack: [(usize, usize); (ROWS + 1) * COLS] = [(0, 0); (ROWS + 1) * COLS];
            let mut sp = 0;
            for c in 0..COLS {
                seen[ROWS][c] = true;
                stack[sp] = (ROWS, c);
                sp += 1;
            }
            while sp > 0 {
                sp -= 1;
                let (r, c) = stack[sp];
                let next = [(r.wrapping_sub(1), c), (r + 1, c), (r, c.wrapping_sub(1)), (r, c + 1)];
                for (nr, nc) in next {
                    if nr > ROWS || nc >= COLS || seen[nr][nc] {
                        continue;
                    }
                    if nr < ROWS && brick(n, nr, nc) == GOLD {
                        continue;
                    }
                    seen[nr][nc] = true;
                    stack[sp] = (nr, nc);
                    sp += 1;
                }
            }
            let mut breakable = 0;
            for r in 0..ROWS {
                for c in 0..COLS {
                    let k = brick(n, r, c);
                    if k != EMPTY && k != GOLD {
                        breakable += 1;
                        assert!(seen[r][c], "level {} row {} col {} can't be reached", n + 1, r, c);
                    }
                }
            }
            assert!(breakable > 0, "level {} has nothing to break", n + 1);
        }
    }
}
