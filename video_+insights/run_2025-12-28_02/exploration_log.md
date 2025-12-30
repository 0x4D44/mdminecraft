# mdminecraft exploratory session log
Date: 2025-12-28
Run: run_2025-12-28_02

## Notes
- Approach: headless automation harness with scripted inputs + tagged screenshots.
- Focus: movement variety, camera orientation, and action pulses (attack/use), with frequent snapshots.
- Tags below map 1:1 to screenshot filenames for video stitching.

## Update log (planned)
- Update 01 (tag: log-01_spawn): Baseline spawn snapshot after a short step.
- Update 02 (tag: log-02_look_down): Pitch down to inspect terrain close-up.
- Update 03 (tag: log-03_walk_forward): Short forward walk to verify movement.
- Update 04 (tag: log-04_strafe_left): Strafe left to test lateral input.
- Update 05 (tag: log-05_turn_back): 180-degree turn to validate yaw updates.
- Update 06 (tag: log-06_sprint): Sprint burst to check speed changes.
- Update 07 (tag: log-07_crouch): Crouch input to confirm state change.
- Update 08 (tag: log-08_jump): Jump pulse + landing check.
- Update 09 (tag: log-09_attack): Attack click while looking down (block interaction attempt).
- Update 10 (tag: log-10_use): Use click while looking down (placement attempt).

## Run results (headless automation)
- Completed the full scripted sequence with 10 tagged screenshots and clean shutdown.
- Experience: the scene felt stable and readable in short ticks; looking down + action pulses produced clear close-up frames for the video.
- Noted a host warning about missing Vulkan validation layer (environmental, not a game logic issue).

## Screenshot map (latest run)
- Update 01 (tag: log-01_spawn) -> tick_00000005_tag-log-01_spawn.png
- Update 02 (tag: log-02_look_down) -> tick_00000010_tag-log-02_look_down.png
- Update 03 (tag: log-03_walk_forward) -> tick_00000020_tag-log-03_walk_forward.png
- Update 04 (tag: log-04_strafe_left) -> tick_00000030_tag-log-04_strafe_left.png
- Update 05 (tag: log-05_turn_back) -> tick_00000035_tag-log-05_turn_back.png
- Update 06 (tag: log-06_sprint) -> tick_00000045_tag-log-06_sprint.png
- Update 07 (tag: log-07_crouch) -> tick_00000050_tag-log-07_crouch.png
- Update 08 (tag: log-08_jump) -> tick_00000055_tag-log-08_jump.png
- Update 09 (tag: log-09_attack) -> tick_00000060_tag-log-09_attack.png
- Update 10 (tag: log-10_use) -> tick_00000065_tag-log-10_use.png

## Issues found + fixes
- Build warning: unused label in automation server after the prior timeout fix.
- Fix: removed the unused label so builds stay clean.
- Code: src/automation/server.rs
