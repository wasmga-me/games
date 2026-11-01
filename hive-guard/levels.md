# Hive Guard: terrain campaign

The first three levels teach the falling-ink rule. From level 4, low cave
ceilings prevent the same semicircle from enclosing the dog. Ink is budgeted
for the entrances in each layout. A horizontal stroke can bridge a roof gap;
a vertical stroke can slide into a guide beside a doorway. The guide must
support the stroke at both ends so bees cannot tip it aside.

These are original layouts in the draw-to-protect genre. They are not an
extracted commercial campaign. Edit `src/levels.rs`; all coordinates use the
300 × 400 drawing space. `challenge` records optional solution strokes for
tests; the production loader does not allocate those witnesses.

| Levels | Challenge |
|---|---|
| 1–3 | Ground shelter, elevated hive, then an elevated dog. |
| 4–5 | Bridge one offset roof gap with limited ink. |
| 6 | Learn a guided vertical gate under a closed roof. |
| 7–8 | Raised versions of the bridge and gate lessons. |
| 9 | Two roof openings: a single patch is insufficient. |
| 10–12 | Bridge, gate and paired roof gaps under attacks from both sides. |
| 13 | Combine a roof bridge with a side gate. |
| 14–15 | Paired roof openings, then two opposite gates. |
| 16–18 | Raised combined shelter, narrow gate, then opposite entrances. |
| 19–23 | Alternate two-roof, roof-and-gate and two-gate shelters at different elevations. |
| 24–29 | Three hives attack paired and combined openings; four spare ink units reward economical strokes. |
| 30 | Two separate roof bridges and a side gate. All three openings need protection. |

Ink ranges from 76 to 192 units after the lessons; survival time rises from
8 to 10 seconds. Ink is sized per puzzle rather than reduced mechanically on
every level. Retry keeps the same terrain and swarm seed. Saved attempts from
the earlier dome campaign are rejected (save version 3); level unlocks remain.

Run `cargo test --locked --release --manifest-path hive-guard/Cargo.toml` from
the game-source repository. The tests replay all 30 intended shelters, alternate
swarms with small pen offsets, and the following losing strategies:

- semicircles with radii 40, 49, 58 and 65 on levels 4–30;
- omitting each barrier in every puzzle with multiple openings;
- drawing an irrelevant line instead of protecting the dog;
- covering the hives instead of closing the dog's entrances.

These are regression checks against the reported shortcuts, not a proof that
only one drawing can win or a claim that numerical difficulty equals human
perceived difficulty. Alternative working shelters are welcome.
