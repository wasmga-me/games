# Pinwork level references

The campaign has 20 distinct chambers. Levels 1–8 retain the original tutorial
layouts and gold quotas. Levels 9–20 adapt arrangements from the browser version
of [Hero Rescue on Plays.org](https://plays.org/hero-rescue/), inspected on
2026-11-01. The [playable reference](https://plays.org/game/hero-rescue/) has 40
stages. These notes refer to its numbered layouts, not the different mobile games
that share the Hero Rescue name.

| Pinwork | Reference layout | Arrangement retained |
| --- | --- | --- |
| 9 | 2 | Gold and lava in separate pockets above the two arms of a V. |
| 10 | 1 | Separate upper gold and water reservoirs above a lava chamber. |
| 11 | 3 | A zigzag descent with a separately held lava pocket. |
| 12 | 4 | A side reservoir feeds coolant into a lower hazard tank. |
| 13 | 5 | Top treasure gate, V-shaped lava basin and a side water reservoir. |
| 14 | 7 | A center divider separates the safe treasure route from lava. |
| 15 | 8 | Treasure travels beside an obstruction while lava takes another route. |
| 16 | 11 | Treasure descends through independently gated lava hazards. |
| 17 | 12 | Lava must kill the monster before water makes the route safe. |
| 18 | 18 | Neighbouring water and lava reservoirs above a final release gate. |
| 19 | 23 | Sloped supports direct the descent past a retained lava pocket. |
| 20 | 30 | Water above two separate lava pockets and a final treasure gate. |

These are adaptations of the arrangements and decision sequences, not copies of
the original game files. Pinwork has a stationary knight and particle treasure;
the reference also has a walking hero, princess, spikes, boulders and sliding
rails. The new chambers use Pinwork's existing materials and pullable pins in
place of those extra systems. Dimensions, reservoirs and final gates are adjusted
to this simulator. Gold has to reach the floor below the last gate; merely holding
it in a low chamber does not count as collecting it. Water, lava, stone and coins
use the same physical interactions as the introductory campaign.

Each chamber is checked for a winning pull sequence and a losing pull sequence.
The test enumerates every order of every pin subset, waiting for settling or the
game's maximum evaluation interval between pulls. Additional checks reject
repeated/mirrored geometry and ensure no chamber resolves before player input.
Reference screenshots and downloaded browser data are research material outside
the published source tree. No reference artwork, scripts or level files are
redistributed.
