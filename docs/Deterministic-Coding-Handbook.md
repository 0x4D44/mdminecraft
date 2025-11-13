# Deterministic Coding Handbook

1. **No wall-clock reads inside simulation systems.** Always express time via `SimTick` or `SimDuration`.
2. **Seed all randomness explicitly.** Use `mdminecraft_core::scoped_rng` with domain-specific entropy, never `rand::thread_rng`.
3. **Avoid floating-point driven branching.** Quantize thresholds or compare integers so replays remain stable across platforms.
4. **Emit structured events for observable actions.** If a system mutates world state, emit an event so the replay log can assert behavior.
5. **Treat tests as first-class citizens.** Headless worldtests and property tests must run deterministically on CI; add them alongside new features.
