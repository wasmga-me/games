# Pinwork level references

The campaign has 100 fixed chambers. Levels 1–8 retain the original tutorial
layouts and gold quotas. Eight of levels 9–20 adapt arrangements from the browser version
of [Hero Rescue on Plays.org](https://plays.org/hero-rescue/), inspected on
2026-11-01. The [playable reference](https://plays.org/game/hero-rescue/) has 40
stages. Levels 11, 15, 16 and 19 replace repetitive adaptations with original
mechanical challenges, described below. These notes refer to its numbered layouts, not the different mobile games
that share the Hero Rescue name.

| Pinwork | Reference layout | Arrangement retained |
| --- | --- | --- |
| 9 | 2 | Gold and lava in separate pockets above the two arms of a V. |
| 10 | 1 | Separate upper gold and water reservoirs above a lava chamber. |
| 11 | Original replacement | Two treasure pockets must both enter a shared cooling tank before its exit opens. |
| 12 | 4 | A side reservoir feeds coolant into a lower hazard tank. |
| 13 | 5 | Top treasure gate, V-shaped lava basin and a side water reservoir. |
| 14 | 7 | A center divider separates the safe treasure route from lava. |
| 15 | Original replacement | Kill the monster with the side lava chute, release coolant, then sink gold through the pool and open its exit. |
| 16 | Original replacement | Remove a divider to expose the monster to lava, then cool the joined tank and choose its exits. |
| 17 | 12 | Lava must kill the monster before water makes the route safe. |
| 18 | 18 | Neighbouring water and lava reservoirs above a final release gate. |
| 19 | Original replacement | Cool two successive lava chambers using one water reservoir; the lower chamber contains the gold. |
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
For the opening twenty chambers, the test enumerates every order of every pin subset, waiting for settling or the
game's maximum evaluation interval between pulls. Additional checks reject
repeated/mirrored geometry and ensure no chamber resolves before player input.
Reference screenshots and downloaded browser data are research material outside
the published source tree. No reference artwork, scripts or level files are
redistributed.

The replacement levels are checked for a minimum of four necessary pulls on
11 and 15, and three on 16 and 19. Their winning routes cannot reduce to opening
a gold gate and a final exit while leaving every hazard untouched. For 11, the
winning order releases both treasure pockets before the coolant; the full
simulation checks that this remains safe and that releasing the exit too soon
loses. These requirements supplement the geometric duplicate checks.

Gold displaces liquid and sinks, but still collides with pins, walls and stone.
Only touching lava particles that cool together form one rock. Separate stones
do not weld on later contact, and any disconnected fragments are split into
independent bodies. In-progress saves restart their chamber when loaded, so
there is no stale particle-body state to migrate.

Levels 21–100 are fixed variants of the multi-step families introduced at 11,
15, 16 and 19. They vary reservoir sizes, contents and lower chutes. They are
original variants, not further extracted reference levels or eighty new
mechanics. `src/campaign.rs` stores their geometry and test-only winning/losing
routes. The offline exporter checks the complete particle simulation, rejects
exact or mirrored geometry, and requires every pull in the winning witness.
Normal tests replay both routes and all single-pull omissions for each variant.
