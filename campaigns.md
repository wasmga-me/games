# Puzzle campaigns

Ten campaigns, 100 fixed levels each (1,000 total). The opening campaigns are
preserved, except Bankshot 21, which replaces a repeated layout. These are original layouts built around the reference
mechanics, not extracted copies of complete commercial campaigns.

## Editable levels

| Game | Fixed level data | Opening and progression |
|---|---|---|
| Pinwork | [pinwork/src/levels.rs](pinwork/src/levels.rs), [pinwork/src/campaign.rs](pinwork/src/campaign.rs) | Twenty opening chambers, then eighty simulated variants of the four later multi-step families. See [reference and physics notes](pinwork/references.md). |
| Bankshot | [bankshot/src/room.rs](bankshot/src/room.rs), [bankshot/src/campaign.rs](bankshot/src/campaign.rs) | Thirty opening rooms, then independently arranged platforms, hostages and reflectors requiring two or three shots. Returning bullets can hit the agent. |
| Decant | [decant/src/campaign.rs](decant/src/campaign.rs) | Two colors and one empty tube; whole runs, uneven runs, then separated layers. Add a color every five levels, up to eight. |
| Pipeline / Flowline | [flowline/src/campaign.rs](flowline/src/campaign.rs) | A single broken joint, two bends, an upward detour, a double-back, then a winding route. Boards grow from 3×3 to 8×8 in five-level chapters. |
| Higher Floor | [higher-floor/src/campaign.rs](higher-floor/src/campaign.rs) | Four absorption/tie lessons; potions at 5, doubling at 7, halving at 11, poison at 15, growing enemies at 19, keys at 23. |
| Boarding | [boarding/src/campaign.rs](boarding/src/campaign.rs) | Nine passengers first: match the current bus, open a path, use the bench. Chapters grow through 3×3, 3×4, 4×4, 4×5, 5×5 and 5×6, with a 6×6 finale. |
| Unpark | [unpark/src/campaign.rs](unpark/src/campaign.rs) | Two-car directional lesson, then a car blocked at both ends. Cones at 6, fences at 11, long vehicles at 16. Later lots require successive groups of cars to leave. |
| Offgrid | [offgrid/src/campaign.rs](offgrid/src/campaign.rs) | Three-arrow chain, then branching dependencies. Later chapters use crosses, rings, diagonal bands, combs and full boards, from 3×3 to 9×9. |
| Loose Ends / Unbolt | [unbolt/src/campaign.rs](unbolt/src/campaign.rs) | Two plates/two colors, mixed colors, a covering bridge, then spare-slot planning. Later chapters combine three to six plate layers. |
| Hive Guard | [hive-guard/src/levels.rs](hive-guard/src/levels.rs) | Three drawing lessons, then roof bridges, guided vertical gates, paired openings and combinations. The finale requires sealing two roof openings and a side entrance. See [design notes](hive-guard/levels.md). |

The `campaign.rs` files contain explicit level entries, indexed from zero.
Pinwork and Hive Guard retain their opening layouts in `levels.rs`; Bankshot
retains its first thirty rooms in `room.rs`.
Their loading functions name the tuple fields and construct the playable board.
Decant layers are bottom-first. Boarding includes the passenger dispatch order;
Unbolt includes the screw removal order. These witnesses are replayed in tests,
not shown as hints. Hive Guard's coordinates are in its 300×400 drawing space.

The first lessons are authored directly. Later Decant, Boarding, Unpark, Offgrid
and Unbolt layouts were selected offline from candidates satisfying the chapter
constraints. Pipeline extends authored routes with winding paths of at least six turns. Higher Floor uses authored
opening arithmetic and validated later towers. The shipped games load the fixed
data directly; they do not search for a new layout when a level starts.

## Difficulty decisions

- Decant uses color fragmentation and palette chapters to select puzzles. A DFS
  path length is retained as a witnessed scoring target, **not an optimal par or
  a measure of human difficulty**. One spare tube makes storage decisions matter;
  tests find dead ends in at least 20 campaign levels. The two-color opening can
  be explored safely with Undo.
- Unpark and Offgrid measure simultaneous removal rounds: remove everything
  currently free, then count how many more rounds are needed. This measures
  blocking dependencies without rewarding mere visual density or array order.
- Boarding and Unbolt retain an actual winning sequence and use sampled
  unplanned-play losses to select later boards. A random loss percentage is only
  a balancing signal. Unbolt's first five lessons deliberately teach the rules
  before requiring the later campaign's loss-pressure check.
- Higher Floor introduces one new room type at a time. Early towers reward
  choosing weaker enemies; later towers require ordering modifiers. Tests check
  both a winning route and a reachable losing choice. Room numbers stay below
  5,000; the boss remains inaccessible until the other rooms are cleared.
- Pipeline allows inspection before the first turn. Every board has a sealed
  route whose tested clockwise solution fits the timer at three seconds per
  turn. This is a minimum interaction allowance, not a claim about every player's
  solving speed. New board sizes begin with a shorter consolidation lesson.
- Hive Guard uses low cave ceilings and a per-layout ink allowance from level 4.
  Players must use terrain to close entrances. Tests replay every intended
  shelter, alternate swarms and small drawing offsets; they reject four sizes
  of the repeated semicircle. Every opening in a combined puzzle is necessary:
  omitting any one barrier loses, as do irrelevant lines and hive caps.

## References and adaptation limits

The established reference review used [Water Color Sort](https://poki.com/en/g/water-color-sort),
Car Out / Parking Jam 3D, Hero Tower Wars, Plumber / Hero Pipe Rescue,
Bus Jam and Screw Jam. The site repository's `docs/ad-games.md` records the store
links and which mechanics were observed. Full commercial level data was not
available. Hive Guard follows the draw-to-protect family and Offgrid follows
arrow-removal puzzles; no particular original campaign is claimed for either.
No proprietary artwork, APKs or level files were imported in this redesign.

## Verification and editing

From the games source repository (or `games-src/` in the site checkout):

```sh
python3 check-ad-games.py pinwork bankshot decant flowline higher-floor boarding unpark offgrid unbolt hive-guard
```

The suites replay all 1,000 levels and check distinct layouts, opening complexity,
staged mechanics, conservation, solvability, loss/retry and the existing save /
physics regressions. Undo/retry continues to reproduce the same board.
Offline export tests and the existing exhaustive tests are intentionally
ignored during normal verification.

Edit `campaign.rs` directly to adjust an individual level, then run that game's
suite. To deliberately regenerate the extension after editing its offline selection
rules, run:

```sh
cargo test --release --manifest-path decant/Cargo.toml export_campaign -- --ignored
cargo test --release --manifest-path decant/Cargo.toml
```

Substitute another game ID as needed. Each of the ten games has an ignored
`export_campaign` test. The seven puzzle exporters preserve the first thirty
entries; Bankshot and Hive Guard emit only their extensions. Pinwork preserves
existing valid, geometrically distinct extension entries. `curation.rs` is only compiled into native tests. Review exported
layouts visually before committing. Regeneration can overwrite manual campaign
edits, so it is never part of a normal build or test.

Campaign revisions invalidate in-progress snapshots whose level numbers could
otherwise resolve to different layouts on retry. Separate completed-level
progress is preserved. New snapshot versions: Decant 2, Flowline 3, Higher Floor
5, Boarding 3, Unpark 3, Offgrid 2, Unbolt 2, Hive Guard 3.

## Extended campaigns and continuous climb

Later chapters hold board dimensions and numeric ranges within readable limits;
more levels do not mean arbitrarily smaller targets or shorter timers. The
fixed layouts are selected offline and replayed by native game tests. Bankshot
requires a three-degree safe aim window and limits selected target paths to two
reflections. Pinwork replays winning and losing sequences in the particle
simulation and rejects any witness that can omit a required pull. Hive Guard
adds shelters with three and four independent openings and repeats the dome,
hive-cap and omitted-barrier checks on the extension.

Pinwork's extension varies reservoir geometry, material quantities and chutes
within four established puzzle families; it does not introduce eighty new
mechanics. Similarity within a family remains a design limitation even when
exact and mirrored duplicates are rejected.

Higher Floor additionally offers an endless mode with one scrolling tower.
Each floor has two encounters with a safe and a losing order. New floors appear
above the hero, power carries forward, and marked halving traps keep arithmetic
bounded. Tests replay 1,000 continuous floors per game run and test generation
across 150 seeds. The fixed 100-level campaign remains a separate mode.
