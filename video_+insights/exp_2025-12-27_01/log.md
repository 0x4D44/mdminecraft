# mdcraft exploratory log (exp_2025-12-27_01)

Session start: 2025-12-27

## Iteration 1
- Plan: run headless scripted walk-square with periodic screenshots for a quick visual sweep.
- Expectation: capture 20-30 frames and review logs for warnings/errors.
- Screenshots tag: iter1_*

- Run notes: build + headless run; process exceeded 120s timeout; captured 2 frames.
- Screenshots: iter1_tick_00000020.png, iter1_tick_00000040.png
- Experience: initial run felt slow to complete; visuals look stable in the limited captures.

## Iteration 2
- Plan: rerun with max ticks and keep screenshots; check if runtime exits cleanly.
- Screenshots tag: iter2_*
- Run notes: still exceeded 120s timeout; captured 2 frames.
- Screenshots: iter2_tick_00000020.png, iter2_tick_00000040.png
- Experience: consistent visuals; runtime still slow to terminate.

## Iteration 3
- Plan: reduce max ticks to 40 and give a longer timeout to allow clean exit.
- Screenshots tag: iter3_*
- Run notes: completed in ~113s; warning about missing VK_LAYER_KHRONOS_validation persisted.
- Screenshots: iter3_tick_00000010.png, iter3_tick_00000020.png, iter3_tick_00000020_n0001.png, iter3_tick_00000040.png
- Experience: run finally completed; captures are steady but turnaround is still slow.

## Iteration 4
- Plan: reduce resolution to 640x360, max ticks 60, screenshot every 10 ticks to see if runtime speeds up.
- Screenshots tag: iter4_*
- Run notes: timed out at 180s; produced 6 frames.
- Screenshots: iter4_tick_00000010.png, iter4_tick_00000020.png, iter4_tick_00000030.png, iter4_tick_00000040.png, iter4_tick_00000050.png, iter4_tick_00000060.png
- Experience: still slow to finish; visuals remain stable.

## Iteration 5 (planned)
- Plan: use headless automation step mode as a stand-in for keyboard input, manually stepping + requesting tagged screenshots.
- Screenshots tag: iter5_*
- Notes: will record movement steps + any oddities.

## Iteration 5 (executed)
- Plan: use headless automation step mode for controlled movement + tagged screenshots.
- Screenshots tag: iter5_*
- Run notes: connected via automation server; used set_actions/pulse + step + screenshot; shutdown cleanly.
- Screenshots: tick_00000010_tag-iter5_forward_10.png, tick_00000020_tag-iter5_strafe_right_10.png, tick_00000025_tag-iter5_turn_45.png, tick_00000035_tag-iter5_sprint_forward_10.png, tick_00000040_tag-iter5_turn_90.png, tick_00000045_tag-iter5_jump_pulse.png
- State snapshot (tick 45): pos=[4.226,66.721,9.203], yaw=90.0, pitch=-1.5698, health=16.2, hunger=20.0, mobs_total=62.
- Issue found: `set_view` angles are radians but README didn’t mention units; I initially sent degrees, which clamped pitch to ~-90°.
- Fix: documented set_view units + pitch behavior in README.
- Experience: once I switched to automation step mode, it felt precise and reproducible; visuals show we were close to a cliff/water edge.

## Iteration 6
- Plan: repeat automation steps using radians for yaw/pitch and a fixed world seed for more stable repro.
- Screenshots tag: iter6_*
- Run notes: automation sequence completed; shutdown clean; consistent tick timings.
- Screenshots: tick_00000012_tag-iter6_forward_12.png, tick_00000024_tag-iter6_strafe_left_12.png, tick_00000028_tag-iter6_turn_90.png, tick_00000043_tag-iter6_sprint_forward_15.png, tick_00000047_tag-iter6_look_up.png, tick_00000053_tag-iter6_jump_pulse.png
- State snapshot (tick 53): pos=[15.5,80.871,8.520], yaw=1.5708, pitch=0.2, health=20, hunger=20, mobs_total=62.
- Experience: motion felt smoother once using radians; views are now level and readable.

## Iteration 7 (planned)
- Plan: use automation + commands to summon mobs and verify headless mob markers; capture tagged screenshots.
- Screenshots tag: iter7_*

## Iteration 8 (planned)
- Plan: summon mobs at ground level to verify marker placement/scale; capture screenshots.
- Screenshots tag: iter8_*

## Iteration 7 (executed)
- Plan: summon mobs with automation + commands and capture headless markers.
- Screenshots tag: iter7_*
- Run notes: teleported to y=101; markers appear high in sky; likely spawned near camera but in another vertical slice.
- Screenshots: tick_00000010_tag-iter7_cow_front.png, tick_00000015_tag-iter7_zombie_back.png, tick_00000020_tag-iter7_skeleton_right.png
- Issue observed: headless mob markers appear extremely large and clipped; they seem to ignore distance scaling, making them overwhelming in the frame.

## Iteration 8
- Plan: spawn mobs at ground-ish y=65 to check if markers behave better.
- Screenshots tag: iter8_*
- Run notes: no visible markers near the expected area; world geometry dominates view.
- Screenshots: tick_00000010_tag-iter8_mobs_ground.png, tick_00000012_tag-iter8_turn_90.png, tick_00000014_tag-iter8_turn_180.png
- Issue observed: mob markers often not visible (either culled or outside view), while in other cases markers are massively oversized.

## Iteration 9 (planned)
- Plan: re-run mob marker capture after reducing headless marker scale; compare visual size.
- Screenshots tag: iter9_*

## Iteration 9 (executed)
- Plan: re-run mob marker capture after reducing headless marker scale to 6.0.
- Screenshots tag: iter9_*
- Screenshots: tick_00000010_tag-iter9_mobs_front.png, tick_00000015_tag-iter9_turn_90.png, tick_00000020_tag-iter9_turn_180.png
- Result: markers are smaller but still dominate; scale still too large for distant mobs.
- Fix: reduce headless marker scale again (6.0 -> 3.0) for better framing.

## Iteration 10 (planned)
- Plan: re-run mob marker capture with scale 3.0 to confirm visibility without overwhelming the frame.
- Screenshots tag: iter10_*

## Iteration 10 (executed)
- Plan: re-run mob marker capture with scale 3.0.
- Screenshots tag: iter10_*
- Screenshots: tick_00000010_tag-iter10_mobs_front.png, tick_00000015_tag-iter10_turn_90.png, tick_00000020_tag-iter10_turn_180.png
- Result: markers are smaller but still visually dominant at distance; no distance falloff.
- Fix: switch to distance-based scaling (clamp 2–8 px; ref distance 32m) so far mobs don’t overwhelm frames.

## Iteration 11 (planned)
- Plan: re-run mob marker capture with distance-based scaling to confirm readability.
- Screenshots tag: iter11_*

## Iteration 11 (executed)
- Plan: re-run mob marker capture with distance-based scaling.
- Screenshots tag: iter11_*
- Screenshots: tick_00000010_tag-iter11_mobs_front.png, tick_00000015_tag-iter11_turn_90.png, tick_00000020_tag-iter11_turn_180.png
- Result: distance scaling helps but markers still look too dominant at close range.
- Fix: tighten scale range and add max-distance cull to reduce clutter.

## Iteration 12 (planned)
- Plan: re-run mob marker capture after scale range + cull adjustments.
- Screenshots tag: iter12_*

## Iteration 12 (executed)
- Plan: re-run mob marker capture after tightening scale range + culling distant markers.
- Screenshots tag: iter12_*
- Screenshots: tick_00000010_tag-iter12_mobs_front.png, tick_00000015_tag-iter12_turn_90.png, tick_00000020_tag-iter12_turn_180.png
- Result: markers are still prominent but less overwhelming; distant clutter reduced by max-distance cull.
- Experience: the markers now feel usable for quick headless QA without drowning the scene.

## Iteration 13 (planned)
- Plan: exercise /weather (rain/clear/thunder) and inspect /help output for accuracy; capture tagged screenshots.
- Screenshots tag: iter13_*

## Iteration 13 (executed)
- Plan: exercise /weather modes + check /help output for accuracy.
- Screenshots tag: iter13_*
- Screenshots: tick_00000025_tag-iter13_rain.png, tick_00000035_tag-iter13_clear.png, tick_00000055_tag-iter13_thunder.png
- Issue found: /help lists `/weather <clear|rain>` but `/weather thunder` is valid and works.
- Fix: update /help output to include thunder.

## Iteration 14 (planned)
- Plan: test /dimension switching (overworld -> nether -> end -> overworld) and confirm state updates + screenshots.
- Screenshots tag: iter14_*

## Iteration 14 (executed)
- Plan: test /dimension switching and capture screenshots.
- Result: /dimension nether command timed out (automation server reported "timeout waiting for response").
- Issue found: dimension switch loads all chunks (update_chunks(usize::MAX)) and can stall headless automation.
- Fix: bound chunk load during dimension switch when headless to keep automation responsive.

## Iteration 15 (planned)
- Plan: re-run dimension switching after bounded chunk load; capture screenshots for overworld/nether/end.
- Screenshots tag: iter15_*

## Iteration 15 (executed)
- Plan: re-run dimension switching after bounding headless chunk load.
- Screenshots tag: iter15_*
- Screenshots: tick_00000005_tag-iter15_overworld.png, tick_00000015_tag-iter15_nether.png, tick_00000025_tag-iter15_end.png, tick_00000035_tag-iter15_overworld_return.png
- Result: /dimension switching completes quickly; screenshots captured for all dimensions.
- Experience: headless automation feels responsive after bounded chunk load.

## Iteration 16 (planned)
- Plan: test /give with item:id and tool syntax; then /clear with maxCount=0 (dry-run) and verify counts.
- Screenshots tag: iter16_*

## Iteration 16 (executed)
- Plan: test /give item:id and tool syntax; /clear with maxCount=0 dry-run.
- Screenshots tag: iter16_*
- Results: /give item:3 5 -> "Gave 5× Item(3)"; /give tool:pickaxe:diamond 1 -> "Gave 1× Tool(Pickaxe, Diamond)"; /clear item:3 0 -> "Found 5 matching items".
- Screenshots: tick_00000005_tag-iter16_inventory_test.png
- No issues found in give/clear flow.

## Iteration 17 (planned)
- Plan: run headless automation with --no-render and verify chunks load (spawn height should be terrain-based, not default 100).
- Screenshots tag: iter17_* (no screenshots expected due to --no-render).

## Iteration 17 (executed)
- Plan: headless --no-render sanity check to ensure chunks load.
- Result: get_state reports y≈81.62 (terrain-based spawn), confirming chunks load without render.
- Issue found: update_chunks returned early when render resources were missing, so no-render headless worlds would never load chunks (spawn defaulted to y=100, empty world).
- Fix: remove early return so chunk data loads even without render resources.

## Iteration 18 (planned)
- Plan: headless --no-render dimension switching (overworld -> nether -> end) to ensure no timeouts.
- Screenshots tag: iter18_* (no screenshots expected due to --no-render).

## Iteration 18 (executed)
- Plan: headless --no-render dimension switching.
- Result: dimension switches completed without timeout; state returned at tick 20.
- No new issues found.

## Iteration 19 (planned)
- Plan: exercise /kill followed by /respawn in headless automation; confirm state changes and capture screenshot.
- Screenshots tag: iter19_*

## Iteration 19 (executed)
- Plan: /kill then /respawn via commands; capture screenshot.
- Result: /respawn works in headless automation; state shows full health after respawn.
- Screenshots: tick_00000009_tag-iter19_respawned.png
- Fix: added /respawn command to command parser and CommandContext to support headless QA.


## Iteration 20 (planned)
- Plan: probe command parsing for invalid numeric block IDs using /setblock and /fill; capture response + screenshots.
- Screenshots tag: iter20_*

## Iteration 20 (partial)
- Run notes: automation client timed out before the full script completed; still saw /setblock accept id=9999.
- Screenshots: none (run ended before captures).
- Experience: connection timeout was a little frustrating, but the partial output hinted at a real issue.

## Iteration 21 (executed)
- Plan: retry invalid block ID test with screenshots + full command responses.
- Screenshots tag: iter21_*
- Screenshots: tick_00000003_tag-iter21_invalid_setblock.png, tick_00000004_tag-iter21_invalid_fill.png
- Result: /setblock and /fill accepted id=9999 and reported success.
- Issue found: command parser accepts unknown numeric block IDs, allowing creation of invalid/invisible blocks and potential downstream errors.
- Fix: validate numeric block IDs against the block registry in command parsing (setblock/fill + fill filters).
- Experience: having the screenshots + logs together made the bug feel concrete and reproducible.

## Iteration 22 (executed)
- Plan: verify invalid block IDs are rejected after the parser fix.
- Screenshots tag: iter22_*
- Screenshots: tick_00000003_tag-iter22_invalid_block_rejected.png
- Result: /setblock and /fill now return "Error: Unknown block id" and do not place blocks.
- Experience: felt satisfying to see the commands hard-fail cleanly; the parser is safer now.

## Iteration 23 (executed)
- Plan: test invalid block IDs in /give and /clear to see if command parser allows them.
- Screenshots tag: iter23_*
- Screenshots: tick_00000003_tag-iter23_invalid_block_item.png
- Result: /give block:9999 succeeded and /clear block:9999 reported matches.
- Issue found: numeric block IDs in item tokens aren’t validated, so invalid block items can be created/cleared.
- Experience: felt like the earlier setblock fix wasn’t fully consistent; glad this surfaced quickly.

## Iteration 24 (executed)
- Plan: verify invalid block IDs are rejected in /give and /clear after parser fix.
- Screenshots tag: iter24_*
- Screenshots: tick_00000003_tag-iter24_invalid_block_item_rejected.png
- Result: /give block:9999 and /clear block:9999 now return "Error: Unknown block id".
- Experience: this feels more consistent with /setblock and /fill; nice closure on the command‑safety story.

## Iteration 25 (executed)
- Plan: roam a short path with automation movement/turns and capture a few scenic frames.
- Screenshots tag: iter25_*
- Screenshots: tick_00000020_tag-iter25_forward.png, tick_00000045_tag-iter25_turn_right.png, tick_00000047_tag-iter25_jump.png
- Result: movement + camera controls behaved normally; no visual anomalies spotted in these frames.
- Notes: some step responses took longer than my client timeout, so I had to reconnect to finish the sequence.
- Experience: the reconnects were a little clunky, but the scenery looked stable once the frames came through.

## Iteration 26 (executed)
- Plan: verify `--automation-exit-when-disconnected` exits the headless process once the client disconnects.
- Screenshots tag: iter26_*
- Screenshots: tick_00000000_tag-iter26_exit_on_disconnect.png
- Result: process exited shortly after the client disconnected; behavior matches the flag’s intent.
- Experience: quick and clean; nice to have for automation runs.

## Iteration 27 (executed)
- Plan: confirm screenshot requests fail gracefully under `--no-render` mode.
- Screenshots tag: iter27_* (expected none)
- Result: server returned `unsupported` with message "render disabled"; shutdown succeeded.
- Experience: clean error path; nothing to fix here.

## Iteration 28 (attempted)
- Plan: place a water source via /setblock and observe whether fluids flow over time.
- Screenshots tag: iter28_*
- Result: the automation client timed out mid‑run; partial data showed water placement, but follow‑up screenshots failed due to controller disconnect.
- Experience: a bit of a hiccup; needed to rerun with a tighter sequence.

## Iteration 29 (executed)
- Plan: re-run water flow test with before/after screenshots.
- Screenshots tag: iter29_*
- Screenshots: tick_00000002_tag-iter29_water_place.png
- Result: water placed; follow‑up after 40 ticks failed due to controller timeout.
- Issue suspected: /setblock places fluid but doesn’t schedule the fluid sim, so water remains static unless another update touches it.

## Iteration 30 (attempted)
- Plan: repeat water flow test after code change; capture before/after.
- Screenshots tag: iter30_*
- Screenshots: tick_00000002_tag-iter30_water_place.png
- Result: step request timed out; follow‑up screenshot couldn’t be captured (controller timeouts).
- Experience: too much time spent on reconnects; decided to shorten the test window.

## Iteration 31 (executed)
- Plan: shorter water flow sequence with multiple quick checkpoints.
- Screenshots tag: iter31_*
- Screenshots: tick_00000002_tag-iter31_water_t2.png, tick_00000007_tag-iter31_water_t7.png, tick_00000012_tag-iter31_water_t12.png, tick_00000017_tag-iter31_water_t17.png
- Result: water now visibly spreads between ticks 2→7 and stabilizes by tick 12/17.
- Fix: when /setblock or /fill places a fluid (or waterlogged block), schedule the fluid simulator via `on_fluid_placed` so fluids actually flow.
- Experience: seeing the flow kick in felt like a real “physics is alive” moment.

## Iteration 32 (executed)
- Plan: validate lava + water interaction after the fluid scheduling fix.
- Screenshots tag: iter32_*
- Screenshots: tick_00000002_tag-iter32_lava_water_t2.png, tick_00000018_tag-iter32_lava_water_t18.png
- Result: water spread visually by tick 18; lava interaction wasn’t clearly visible from this angle (likely occluded).
- Notes: need a tighter camera angle or a closer interaction setup to confirm obsidian/cobblestone formation.

## Iteration 33 (executed)
- Plan: probe command parsing with non-finite coordinates.
- Result: `/tp nan 0 0` succeeded and reported “Teleported to NaN …”; state output silently clamped NaN to 0.
- Issue found: non-finite coordinates are accepted, which can poison camera/player state with NaNs.
- Fix: reject non-finite values in `parse_coord`.

## Iteration 34 (executed)
- Plan: verify non-finite coordinates are rejected after the parser fix.
- Result: `/tp nan 0 0` now returns “Error: Invalid coordinate: nan”.
- Experience: feels safer and more predictable for automated testing.

## Iteration 35 (attempted)
- Plan: test lava+water interaction with broader base and multiple snapshots.
- Screenshots tag: iter35_*
- Run notes: initial command batch succeeded (placed water+lava), but client timed out while stepping; captured t2 and a follow-up probe at tick 12.
- Screenshots: tick_00000002_tag-iter35_lava_water_t2.png, tick_00000012_tag-iter35_lava_water_probe.png
- Experience: automation timeouts again; needed shorter scripts and longer client timeout to finish.

## Iteration 36 (attempted)
- Plan: repeat lava+water test with shorter script.
- Screenshots tag: iter36_*
- Run notes: got t2 screenshot, then client timed out during the next step.
- Screenshots: tick_00000002_tag-iter36_lava_water_t2.png
- Experience: timeouts persisted; decided to increase harness client timeout.

## Iteration 37 (executed)
- Plan: repeat lava+water test with longer client timeout; capture t2/t12/t22.
- Screenshots tag: iter37_*
- Screenshots: tick_00000002_tag-iter37_lava_water_t2.png, tick_00000012_tag-iter37_lava_water_t12.png, tick_00000022_tag-iter37_lava_water_t22.png
- Result: lava appeared to remain intact when adjacent to water; no obsidian/cobblestone conversion seen.
- Issue found: fluid interaction logic only runs when the target block is replaceable, so adjacent opposing fluids don’t convert on contact.
- Fix: allow fluid interaction to trigger even when neighbor is another fluid; add test for water/lava horizontal conversion.
- Experience: the missing interaction felt like a subtle physics regression; nice to catch via the stepwise snapshots.

## Iteration 38 (executed)
- Plan: capture a baseline non-interaction view at the same camera angle for comparison.
- Screenshots tag: iter38_*
- Screenshots: tick_00000002_tag-iter38_baseline_t2.png, tick_00000012_tag-iter38_baseline_t12.png, tick_00000022_tag-iter38_baseline_t22.png
- Result: baseline frames show no placed fluids, used as a visual control.
- Experience: handy comparison frames for the video edit.

## Iteration 39 (executed)
- Plan: place water/lava adjacent (offset by 1) for clearer contact; capture t2/t12/t22.
- Screenshots tag: iter39_*
- Screenshots: tick_00000002_tag-iter39_lava_water_t2.png, tick_00000012_tag-iter39_lava_water_t12.png, tick_00000022_tag-iter39_lava_water_t22.png
- Result: obsidian appears immediately where lava meets water; water spreads around it.
- Experience: satisfying to see the interaction kick in once the logic was fixed.

## Iteration 40 (executed)
- Plan: test lava fire ignition near flammable blocks (oak planks) in headless.
- Screenshots tag: iter40_*
- Screenshots: tick_00000002_tag-iter40_fire_t2.png, tick_00000012_tag-iter40_fire_t12.png, tick_00000022_tag-iter40_fire_t22.png
- Result: no visible fire on or above planks within 22 ticks; lava spread dominated view.
- Notes: fire ignition appears probabilistic or too slow for this short window; no bug confirmed.
- Experience: hard to validate visually with the lava sheet covering the setup.

## Iteration 41 (executed)
- Plan: force a flowing lava block, then place adjacent water and observe conversion.
- Screenshots tag: iter41_*
- Screenshots: tick_00000005_tag-iter41_lava_flow_t5.png, tick_00000007_tag-iter41_cobble_t7.png, tick_00000017_tag-iter41_cobble_t17.png
- Result: water meeting flowing lava now yields cobblestone (visible at t7/t17).
- Fix verified: fluid interaction conversion now distinguishes source vs flowing (obsidian vs cobblestone).
- Experience: seeing the cobble appear exactly when the water hits was a great confirmation shot.

## Iteration 42 (executed)
- Plan: water source near lava source for vertical/horizontal interaction check.
- Screenshots tag: iter42_*
- Screenshots: tick_00000005_tag-iter42_lava_flow_t5.png, tick_00000012_tag-iter42_lava_flow_t12.png, tick_00000020_tag-iter42_lava_flow_t20.png
- Result: water/lava interaction yields obsidian when lava is a source; stable over time.
- Experience: the obsidian block stays consistent as water spreads around it, good visual proof.

## Iteration 43 (executed)
- Plan: let lava flow first, then place water to confirm flowing-lava interaction result.
- Screenshots tag: iter43_*
- Screenshots: tick_00000010_tag-iter43_cobble_t10.png, tick_00000018_tag-iter43_cobble_t18.png
- Result: flowing lava meeting water yields cobblestone, matching updated logic.
- Experience: the cobble appears clearly as water touches the flowing edge — great validation frame.

## Iteration 44 (executed)
- Plan: force obsidian then swap to flowing lava to confirm cobble conversion in-place.
- Screenshots tag: iter44_*
- Screenshots: tick_00000002_tag-iter44_obsidian_t2.png, tick_00000004_tag-iter44_cobble_t4.png
- Result: obsidian appears with lava source; switching to lava_flowing converts to cobblestone on contact.
- Experience: very clear A/B shot for the video.

## Iteration 45 (executed)
- Plan: simple lava spread timing to see if fire appears without nearby flammables.
- Screenshots tag: iter45_*
- Screenshots: tick_00000002_tag-iter45_fire_t2.png, tick_00000012_tag-iter45_fire_t12.png, tick_00000022_tag-iter45_fire_t22.png
- Result: no fire (expected, no flammable neighbors). Lava spread visuals are stable.
- Experience: good control run to confirm no false fire.

## Iteration 46 (executed)
- Plan: lava adjacent to oak planks; extended timeline to check ignition.
- Screenshots tag: iter46_*
- Screenshots: tick_00000002_tag-iter46_fire_t2.png, tick_00000022_tag-iter46_fire_t22.png, tick_00000062_tag-iter46_fire_t62.png
- Result: no visible fire by tick 62; likely low ignition rate or occluded by lava spread.
- Notes: not enough evidence to call a bug; might need a longer run or different camera angle.
- Experience: still hard to validate fire visually in headless shots.

## Iteration 42–44 (retro)
- Fix detail: updated fluid interaction logic to use incoming lava source/flowing state; incoming water into flowing lava now yields cobblestone reliably.
- Tests: added horizontal interaction tests for both directions (water->lava and lava->water with flowing lava).
- Visuals: iter42/iter43/iter44 capture obsidian vs cobblestone conversion sequences.

## Iteration 47 (executed)
- Plan: validate infinite water source creation (two sources with a flowing middle), then remove a source and observe.
- Screenshots tag: iter47_*
- Screenshots: tick_00000002_tag-iter47_infinite_t2.png, tick_00000010_tag-iter47_infinite_t10.png, tick_00000018_tag-iter47_infinite_t18.png
- Result: water spreads and remains stable; hard to visually confirm the exact center block becomes a source (camera angle/overlay).
- Notes: I added a unit test to confirm infinite-water conversion deterministically.
- Experience: visuals are subtle in headless mode, but the test gives confidence.

## Iteration 48 (executed)
- Plan: planks adjacent to lava, early screenshots to see if fire spawns quickly.
- Screenshots tag: iter48_*
- Screenshots: tick_00000006_tag-iter48_fire_t6.png, tick_00000012_tag-iter48_fire_t12.png
- Result: no fire visible; lava spreads over time and may occlude checks.
- Experience: still hard to capture ignition in headless frames.

## Iteration 49 (executed)
- Plan: directly place a flowing water block between two sources to validate infinite source promotion.
- Screenshots tag: iter49_*
- Screenshots: tick_00000002_tag-iter49_flowing_set_t2.png
- Result: water rendered as a full surface (hard to distinguish source vs flowing visually), but sim should promote to source after the fix.
- Notes: added unit test to validate the exact block ID conversion deterministically.
- Experience: visually subtle, but good for the “why tests matter” part of the video.

## Iteration 50 (analysis)
- Issue found: infinite water sources were not forming at all — check existed but never promoted flowing water to a source.
- Fix: integrate infinite-source promotion into the fluid update loop and guard it with `!is_falling` to avoid creating sources in waterfalls.
- Tests: added/ran `test_infinite_water_creates_source`.
- Experience: the logic gap was subtle; tests + targeted headless shots helped confirm behavior.

## Iteration 51 (executed)
- Plan: place water then replace with stone slab to see if it retains waterlogging.
- Screenshots tag: iter51_*
- Screenshots: tick_00000002_tag-iter51_waterlogged_slab_t2.png, tick_00000010_tag-iter51_waterlogged_slab_t10.png
- Result: water disappears once slab is placed; it does not auto-waterlog (expected).
- Experience: confirms manual waterlogging is required.

## Iteration 52 (executed)
- Plan: explicitly set waterlogged slab state and check if it behaves as a source.
- Screenshots tag: iter52_*
- Screenshots: tick_00000002_tag-iter52_waterlogged_slab_t2.png, tick_00000008_tag-iter52_waterlogged_slab_t8.png
- Result: water spreads from the slab, consistent with waterlogged blocks acting as sources.
- Experience: a clearer waterlogged-case reference for the video.

## Iteration 53 (executed)
- Plan: use blockstate properties to set a waterlogged slab via /setblock.
- Result: `/setblock stone_slab[waterlogged=true]` failed with “slab supports: half”.
- Issue found: command parser doesn’t accept `waterlogged=` property even for waterloggable blocks.
- Fix: added support for `waterlogged` in blockstate parsing; rejects on non-waterloggable blocks.
- Tests: added `parses_setblock_with_waterlogged_property` and `rejects_waterlogged_property_for_non_waterloggable_block`.
- Experience: neat, concrete UX bug uncovered by trying a natural command.

## Iteration 54 (executed)
- Plan: retry /setblock with waterlogged property after parser fix.
- Screenshots tag: iter54_*
- Screenshots: tick_00000002_tag-iter54_waterlogged_prop_t2.png, tick_00000008_tag-iter54_waterlogged_prop_t8.png
- Result: waterlogged slab placed successfully and water spreads from it.
- Experience: feels much more intuitive; great before/after story for the video.

## Iteration 55 (executed)
- Plan: set waterlogged stairs via blockstate properties.
- Screenshots tag: iter55_*
- Screenshots: tick_00000002_tag-iter55_waterlogged_stairs_t2.png
- Result: waterlogged stairs placed successfully and emit water like a source.
- Experience: confirms the waterlogged parser fix applies beyond slabs.

## Iteration 56 (executed)
- Plan: set a waterlogged fence via blockstate properties.
- Screenshots tag: iter56_*
- Screenshots: tick_00000002_tag-iter56_waterlogged_fence_t2.png
- Result: fence placed with waterlogged state and water spreads; no issues.
- Experience: nice confirmation that waterlogged props are consistent across block families.

## Iteration 57 (executed)
- Plan: waterlogged slab with multiple blockstate props (waterlogged + half=top).
- Screenshots tag: iter57_*
- Screenshots: tick_00000002_tag-iter57_waterlogged_slab_top_t2.png
- Result: combined props parse and place correctly; water spreads as expected.
- Experience: confirms multiple property parsing works in any order.

## Iteration 58 (executed)
- Plan: waterlogged cobblestone wall via blockstate properties.
- Screenshots tag: iter58_*
- Screenshots: tick_00000002_tag-iter58_waterlogged_wall_t2.png
- Result: wall placed with waterlogged state; water spreads as expected.

## Iteration 59 (executed)
- Plan: waterlogged glass pane via blockstate properties.
- Screenshots tag: iter59_*
- Screenshots: tick_00000002_tag-iter59_waterlogged_pane_t2.png
- Result: pane placed with waterlogged state; water spreads as expected.

## Iteration 60 (executed)
- Plan: waterlogged iron bars via blockstate properties.
- Screenshots tag: iter60_*
- Screenshots: tick_00000002_tag-iter60_waterlogged_bars_t2.png
- Result: bars placed with waterlogged state; water spreads as expected.

## Iteration 61 (executed)
- Plan: waterlogged stone brick wall via blockstate properties.
- Screenshots tag: iter61_*
- Screenshots: tick_00000002_tag-iter61_waterlogged_brickwall_t2.png
- Result: wall placed with waterlogged state; water spreads as expected.

## Iteration 62 (executed)
- Plan: waterlogged fence gate via blockstate properties.
- Screenshots tag: iter62_*
- Screenshots: tick_00000002_tag-iter62_waterlogged_fencegate_t2.png
- Result: fence gate placed with waterlogged state; water spreads as expected.

## Iteration 63 (executed)
- Plan: lava over planks to see fire ignition (baseline layout from earlier runs).
- Screenshots tag: iter63_*
- Screenshots: tick_00000002_tag-iter63_fire_spawn_t2.png, tick_00000006_tag-iter63_fire_spawn_t6.png
- Result: no visible fire; still raining in this view.
- Experience: felt like a false negative—suspected rain or setup issue.

## Iteration 64 (executed)
- Plan: same ignition test but longer runtime to catch delayed fire.
- Screenshots tag: iter64_*
- Screenshots: tick_00000020_tag-iter64_fire_spawn_t20.png
- Result: no visible fire; later screenshot attempts timed out.
- Experience: automation hiccup; needed shorter steps to avoid timeouts.

## Iteration 65 (executed)
- Plan: shorter steps (every 10 ticks) to avoid timeouts; capture more frames.
- Screenshots tag: iter65_*
- Screenshots: tick_00000010_tag-iter65_fire_spawn_t10.png, tick_00000020_tag-iter65_fire_spawn_t20.png, tick_00000030_tag-iter65_fire_spawn_post.png, tick_00000040_tag-iter65_fire_spawn_post_t10.png
- Result: still no visible fire.
- Experience: nudged me toward validating if fire rendering or ignition rules were the issue.

## Iteration 66 (executed)
- Plan: shift to a side view while lava sits above planks (rain still visible).
- Screenshots tag: iter66_*
- Screenshots: tick_00000004_tag-iter66_fire_side_t4.png, tick_00000008_tag-iter66_fire_side_t8.png, tick_00000016_tag-iter66_fire_side_t16.png
- Result: no fire visible.
- Experience: likely weather + placement confusion; not confident in setup.

## Iteration 67 (executed)
- Plan: directly place a fire block to confirm rendering works.
- Screenshots tag: iter67_*
- Screenshots: tick_00000001_tag-iter67_fire_direct_t1.png, tick_00000001_tag-iter67_fire_direct_side.png
- Result: fire renders correctly when placed by command.
- Experience: relieved—rendering is fine; ignition logic/setup is the issue.

## Iteration 68 (executed)
- Plan: /weather clear + side view with lava ignition layout.
- Screenshots tag: iter68_*
- Screenshots: tick_00000002_tag-iter68_fire_clear_t2.png, tick_00000006_tag-iter68_fire_clear_t6.png, tick_00000014_tag-iter68_fire_clear_t14.png
- Result: no fire visible; the view likely missed the ignition area.
- Experience: frustrating; realized I was probably looking at the wrong spot.

## Iteration 69 (executed)
- Plan: lava at same level as planks (no rain) to see ignition.
- Screenshots tag: iter69_*
- Screenshots: tick_00000002_tag-iter69_fire_lava_t2.png, tick_00000006_tag-iter69_fire_lava_t6.png, tick_00000014_tag-iter69_fire_lava_t14.png
- Result: no fire observed.
- Experience: pointed to “setup mismatch” with the simplified ignition rules.

## Iteration 70 (executed)
- Plan: reproduce the unit test layout using lava_flowing + oak_log below air.
- Screenshots tag: iter70_*
- Screenshots: tick_00000004_tag-iter70_flowing_fire_t4.png, tick_00000008_tag-iter70_flowing_fire_t8.png
- Result: no fire seen; view alignment still off.
- Experience: decided to do a tight top-down pass next.

## Iteration 71 (executed)
- Plan: tight top-down view of the lava/log ignition layout.
- Screenshots tag: iter71_*
- Screenshots: tick_00000004_tag-iter71_fire_top_t4.png, tick_00000004_tag-iter71_fire_side_t4.png, tick_00000008_tag-iter71_fire_side_t8.png
- Result: fire visible in the top-down shot; side view partially caught it.
- Experience: confirmation that ignition works with the “air above flammable below” setup.

## Iteration 72 (executed)
- Plan: same layout, shorter timing to capture the ignition moment.
- Screenshots tag: iter72_*
- Screenshots: tick_00000002_tag-iter72_fire_lava_top_t2.png, tick_00000006_tag-iter72_fire_lava_top_t6.png
- Result: fire appears by tick 6.
- Experience: clear success case, good candidate for the video montage.

## Iteration 73 (executed)
- Plan: lava source + adjacent flammable (air above) to see if it ignites.
- Screenshots tag: iter73_*
- Screenshots: tick_00000001_tag-iter73_fire_source_t1.png, tick_00000005_tag-iter73_fire_source_t5.png
- Result: no fire within 5 ticks; likely due to lava filling adjacent air before ignition.
- Experience: suggests the simplified ignition rule is sensitive to flow/air availability.
