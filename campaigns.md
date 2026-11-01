# Puzzle campaigns

Eight campaigns, 30 levels each. Pinwork and Bankshot keep their previously
reviewed campaigns. These are original layouts built around the reference
mechanics, not extracted copies of complete commercial campaigns.

## Editable levels

| Game | Fixed level data | Opening and progression |
|---|---|---|
| Decant | [decant/src/campaign.rs](decant/src/campaign.rs) | Two colors and one empty tube; whole runs, uneven runs, then separated layers. Add a color every five levels, up to seven. |
| Pipeline / Flowline | [flowline/src/campaign.rs](flowline/src/campaign.rs) | A single broken joint, two bends, an upward detour, a double-back, then a winding route. Boards grow from 3×3 to 8×8 in five-level chapters. |
| Higher Floor | [higher-floor/src/campaign.rs](higher-floor/src/campaign.rs) | Four absorption/tie lessons; potions at 5, doubling at 7, halving at 11, poison at 15, growing enemies at 19, keys at 23. |
| Boarding | [boarding/src/campaign.rs](boarding/src/campaign.rs) | Nine passengers first: match the current bus, open a path, use the bench. Chapters grow through 3×3, 3×4, 4×4, 4×5, 5×5 and 5×6, with a 6×6 finale. |
| Unpark | [unpark/src/campaign.rs](unpark/src/campaign.rs) | Two-car directional lesson, then a car blocked at both ends. Cones at 6, fences at 11, long vehicles at 16. Later lots require successive groups of cars to leave. |
| Offgrid | [offgrid/src/campaign.rs](offgrid/src/campaign.rs) | Three-arrow chain, then branching dependencies. Later chapters use crosses, rings, diagonal bands, combs and full boards, from 3×3 to 9×9. |
| Loose Ends / Unbolt | [unbolt/src/campaign.rs](unbolt/src/campaign.rs) | Two plates/two colors, mixed colors, a covering bridge, then spare-slot planning. Later chapters combine three to six plate layers. |
| Hive Guard | [hive-guard/src/levels.rs](hive-guard/src/levels.rs) | Thirty distinct terrain arrangements instead of ten repeated three times. Elevation, cliffs, roofs, two-hive crossfire, then three-hive finales. |

The seven `campaign.rs` files contain explicit level entries, indexed from zero.
Their loading functions name the tuple fields and construct the playable board.
Decant layers are bottom-first. Boarding includes the passenger dispatch order;
Unbolt includes the screw removal order. These witnesses are replayed in tests,
not shown as hints. Hive Guard's coordinates are in its 300×400 drawing space.

The first lessons are authored directly. Later Decant, Boarding, Unpark, Offgrid
and Unbolt layouts were selected offline from candidates satisfying the chapter
constraints. Pipeline uses authored route patterns. Higher Floor uses authored
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
- Hive Guard keeps enough ink for a tested protective arch. Each terrain passes
  a real physics replay with that shelter and loses when the player only caps
  the hives. Later terrain changes attack directions instead of simply replaying
  the same map with a shorter allowance.

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
python3 check-ad-games.py decant flowline higher-floor boarding unpark offgrid unbolt hive-guard
```

The suites replay all 240 levels and check distinct layouts, opening complexity,
staged mechanics, conservation, solvability, loss/retry and the existing save /
physics regressions. Undo/retry continues to reproduce the same board.
Seven export tests and the existing exhaustive/tuning tests are intentionally
ignored during normal verification.

Edit `campaign.rs` directly to adjust an individual level, then run that game's
suite. To deliberately reselect the **whole** campaign after editing its offline
rules in `puzzle.rs`, run:

```sh
cargo test --release --manifest-path decant/Cargo.toml export_campaign -- --ignored
cargo test --release --manifest-path decant/Cargo.toml
```

Substitute another game ID as needed; Hive Guard is authored directly and has
no exporter. `curation.rs` is only compiled into native tests. Review exported
layouts visually before committing. Regeneration overwrites manual campaign
edits, so it is never part of a normal build or test.

Campaign revisions invalidate in-progress snapshots whose level numbers could
otherwise resolve to different layouts on retry. Separate completed-level
progress is preserved. New snapshot versions: Decant 2, Flowline 3, Higher Floor
4, Boarding 3, Unpark 3, Offgrid 2, Unbolt 2, Hive Guard 2.
