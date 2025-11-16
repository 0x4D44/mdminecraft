# Contributing to mdminecraft

Thank you for your interest in contributing to mdminecraft! This document provides guidelines and information for contributors.

---

## Table of Contents

1. [Code of Conduct](#code-of-conduct)
2. [Getting Started](#getting-started)
3. [Development Setup](#development-setup)
4. [Making Changes](#making-changes)
5. [Testing](#testing)
6. [Code Style](#code-style)
7. [Commit Messages](#commit-messages)
8. [Pull Request Process](#pull-request-process)
9. [Determinism Requirements](#determinism-requirements)
10. [Performance Considerations](#performance-considerations)

---

## Code of Conduct

This project follows a simple code of conduct:

- Be respectful and considerate
- Welcome newcomers and help them learn
- Focus on constructive feedback
- Assume good intentions

---

## Getting Started

### Prerequisites

- Rust 1.75 or newer
- Git
- A GPU with Vulkan/DirectX 12/Metal support (for render crate development)
- Familiarity with Rust and cargo

### First Steps

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/mdminecraft.git`
3. Add upstream remote: `git remote add upstream https://github.com/0x4D44/mdminecraft.git`
4. Build the project: `cargo build`
5. Run tests: `cargo test --all`

---

## Development Setup

### Building

```bash
# Development build (faster, includes debug symbols)
cargo build

# Release build (optimized, slower compilation)
cargo build --release

# Build specific crate
cargo build --package mdminecraft-world
```

### Running Tests

```bash
# All tests
cargo test --all

# Specific crate tests
cargo test --package mdminecraft-world

# Specific test
cargo test --package mdminecraft-world test_heightmap_deterministic

# Worldtests (large-scale integration tests)
cargo test --package mdminecraft-world --test '*worldtest' -- --nocapture
```

### Code Quality Tools

```bash
# Run clippy (linter)
cargo clippy --all-targets --all-features

# Format code
cargo fmt --all

# Check code without building
cargo check --all
```

---

## Making Changes

### Workflow

1. Create a feature branch from `main`:
   ```bash
   git checkout -b feature/my-feature
   ```

2. Make your changes

3. Add tests for new functionality

4. Ensure all tests pass:
   ```bash
   cargo test --all
   ```

5. Format your code:
   ```bash
   cargo fmt --all
   ```

6. Run clippy:
   ```bash
   cargo clippy --all-targets --all-features
   ```

7. Commit your changes (see [Commit Messages](#commit-messages))

8. Push to your fork:
   ```bash
   git push origin feature/my-feature
   ```

9. Open a Pull Request

### Types of Contributions

**Bug Fixes:**
- Include a test that reproduces the bug
- Explain the bug and the fix in PR description
- Reference any related issues

**New Features:**
- Discuss major features in an issue first
- Include comprehensive tests
- Update documentation
- Ensure determinism is maintained (if applicable)

**Performance Improvements:**
- Include benchmarks showing improvement
- Ensure correctness is not compromised
- Update performance baselines if significant

**Documentation:**
- Keep documentation up-to-date with code changes
- Improve existing documentation
- Add examples where helpful

**Tests:**
- Add missing test coverage
- Improve existing tests
- Add property tests for invariants
- Create worldtests for large-scale validation

---

## Testing

### Test Requirements

All changes should include appropriate tests:

**Unit Tests:**
- Test individual functions and structs
- Fast (<1ms per test)
- Located in same file as implementation (`#[cfg(test)]` module)

**Integration Tests:**
- Test component interactions
- Located in `tests/` directory
- Example: `crates/world/tests/chunk_lighting_integration_test.rs`

**Property Tests:**
- Validate invariants across randomized inputs
- Use `proptest` framework
- Located in dedicated `fuzz.rs` files
- Example: `crates/world/src/terrain/fuzz.rs`

**Worldtests:**
- Large-scale integration tests
- Validate entire subsystems
- Export metrics for regression detection
- Located in `crates/world/tests/*_worldtest.rs`
- See [Worldtest Usage Guide](wrk_docs/2025.11.15%20-%20DOC%20-%20Worldtest%20Usage%20Guide.md)

### Writing Tests

```rust
// Unit test example
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_creation() {
        let chunk = Chunk::new(ChunkPos { x: 0, z: 0 });
        assert_eq!(chunk.position().x, 0);
    }
}

// Property test example
#[cfg(test)]
mod prop_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_heightmap_continuous(seed: u64, x: i32, z: i32) {
            let hm = Heightmap::generate(seed, x, z);
            // Validate continuity invariant
            assert!(validate_continuity(&hm));
        }
    }
}
```

---

## Code Style

### Rust Style

Follow standard Rust conventions:

- Use `cargo fmt` for automatic formatting
- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Prefer explicit over implicit
- Use meaningful variable names
- Add comments for complex logic

### Documentation

```rust
/// Brief description of the function.
///
/// Longer description with details about:
/// - Parameters
/// - Return value
/// - Panics (if any)
/// - Examples (if helpful)
///
/// # Examples
///
/// ```
/// let chunk = Chunk::new(ChunkPos { x: 0, z: 0 });
/// assert_eq!(chunk.size(), 4096);
/// ```
pub fn my_function(param: Type) -> ReturnType {
    // Implementation
}
```

### Naming Conventions

- `snake_case` for variables, functions, modules
- `PascalCase` for types, traits, enums
- `SCREAMING_SNAKE_CASE` for constants
- Descriptive names (prefer `chunk_position` over `cp`)

---

## Commit Messages

### Format

```
[Category] Brief description (50 chars or less)

More detailed explanation if needed. Wrap at 72 characters.
Include motivation for the change and contrast with previous behavior.

- Bullet points are fine
- Use present tense ("Add feature" not "Added feature")
- Reference issues (#123) if applicable

Technical details or implementation notes can go here.
```

### Categories

- `[Core]` - Core types and utilities
- `[World]` - World generation, chunks, persistence
- `[Render]` - GPU rendering
- `[Net]` - Networking and protocol
- `[Physics]` - Collision detection and physics
- `[ECS]` - Entity-Component-System
- `[Test]` - Testing infrastructure or tests
- `[Docs]` - Documentation updates
- `[Build]` - Build system or dependencies
- `[Fix]` - Bug fixes
- `[Perf]` - Performance improvements
- `[Refactor]` - Code refactoring
- `[Stage#]` - Stage-specific work (e.g., `[Stage5]`)

### Examples

```
[World] Add cave generation algorithm

Implement 3D Perlin noise-based cave generation with configurable
density and size parameters. Caves are deterministic from world seed
and integrate with existing terrain generation pipeline.

- Add CaveGenerator with noise-based carving
- Update TerrainGenerator to apply caves after heightmap
- Add property tests for cave determinism
- Update worldtest to validate cave generation

Closes #42
```

```
[Fix] Correct lighting propagation at chunk boundaries

Fixed issue where light values were not properly propagated across
chunk boundaries, causing visible seams in dark areas.

The bug was caused by not querying neighbor chunks when propagating
light near chunk edges. Updated BFS algorithm to check adjacent
chunks and queue border updates correctly.

Fixes #156
```

---

## Pull Request Process

### Before Submitting

1. ✅ All tests pass (`cargo test --all`)
2. ✅ Code is formatted (`cargo fmt --all`)
3. ✅ Clippy has no warnings (`cargo clippy --all-targets`)
4. ✅ Documentation is updated
5. ✅ Commit messages follow guidelines
6. ✅ PR description is clear and complete

### PR Description Template

```markdown
## Description

Brief description of changes.

## Type of Change

- [ ] Bug fix
- [ ] New feature
- [ ] Performance improvement
- [ ] Documentation update
- [ ] Refactoring
- [ ] Other (please describe)

## Changes Made

- Change 1
- Change 2
- Change 3

## Testing

Describe how you tested your changes:
- Unit tests added/modified
- Integration tests added/modified
- Worldtests affected
- Manual testing performed

## Performance Impact

If applicable:
- Benchmarks before/after
- Memory usage changes
- Any regressions addressed

## Checklist

- [ ] Tests pass locally
- [ ] Code is formatted
- [ ] Clippy passes
- [ ] Documentation updated
- [ ] Determinism maintained (if applicable)
- [ ] Performance benchmarks reviewed (if applicable)

## Related Issues

Fixes #(issue number)
Closes #(issue number)
Related to #(issue number)
```

### Review Process

1. Automated checks run (tests, linting)
2. Code review by maintainers
3. Address feedback
4. Approval and merge

### After Merge

- Your branch will be deleted
- Pull latest changes: `git pull upstream main`
- Update your fork: `git push origin main`

---

## Determinism Requirements

**Critical:** The engine must remain 100% deterministic for multiplayer and replay functionality.

### Rules for Deterministic Code

**DO:**
- ✅ Use deterministic PRNG seeded from world seed + position
- ✅ Use fixed-point math where possible
- ✅ Document any floating-point operations
- ✅ Use `BTreeMap` for deterministic iteration order
- ✅ Base all randomness on `SimTick` and world seed

**DON'T:**
- ❌ Use `rand::thread_rng()` or system entropy
- ❌ Use `HashMap` iteration in generation code
- ❌ Use `std::time::SystemTime` in simulation
- ❌ Use platform-specific functions
- ❌ Depend on execution order of parallel code

### Testing Determinism

All changes to world generation, entity logic, or physics MUST include determinism tests:

```rust
#[test]
fn test_my_feature_deterministic() {
    let result1 = generate_with_seed(12345);
    let result2 = generate_with_seed(12345);
    assert_eq!(result1, result2, "Results must be identical with same seed");
}
```

Run the determinism worldtest to validate:
```bash
cargo test --package mdminecraft-world --test determinism_worldtest -- --nocapture
```

---

## Performance Considerations

### Performance Guidelines

1. **Measure First** - Profile before optimizing
2. **Test Impact** - Run benchmarks before and after changes
3. **Maintain Correctness** - Never sacrifice correctness for speed
4. **Document Trade-offs** - Explain performance decisions

### Running Benchmarks

```bash
# Run worldtests with metrics export
cargo test --release --package mdminecraft-world --test '*worldtest' -- --nocapture

# View metrics
cat target/metrics/*.json | jq .

# Compare with baselines (see wrk_docs/2025.11.15 - BAS - Performance Baselines.md)
```

### Performance Targets

See [Performance Baselines](wrk_docs/2025.11.15%20-%20BAS%20-%20Performance%20Baselines.md) for current targets.

**Regression Thresholds:**
- 5% degradation: ⚠️ Warning (investigate)
- 10% degradation: ❌ Blocking (must fix)

---

## Getting Help

### Resources

- **Documentation:** [wrk_docs/](wrk_docs/)
- **Architecture:** [Architecture Overview](wrk_docs/2025.11.15%20-%20DOC%20-%20Architecture%20Overview.md)
- **Worldtest Guide:** [Worldtest Usage Guide](wrk_docs/2025.11.15%20-%20DOC%20-%20Worldtest%20Usage%20Guide.md)

### Questions?

- Open a [GitHub Discussion](https://github.com/0x4D44/mdminecraft/discussions)
- Ask in the issue you're working on
- Check existing issues and PRs for similar work

---

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

---

Thank you for contributing to mdminecraft! 🎮🌍
