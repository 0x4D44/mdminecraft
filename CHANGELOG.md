# Changelog

All notable changes for mdminecraft.

## Unreleased (post-2025-11-20 hardening)
- Secure-by-default TLS with optional dev insecure mode (`MDM_INSECURE_TLS=1`); server PEM loading via env vars.
- Client drains reliable/unreliable channels each frame; added QUIC roundtrip integration test.
- Deterministic mining timing (frame-rate independent) and `RegionStore::chunk_exists` verification to avoid false positives.
- Cross-chunk lighting seam stitching for skylight/block light; property test prevents light amplification across seams.
- HUD shows particle budget vs active counts; block edits trigger targeted lighting recompute/remesh.
- CI runs targeted regressions (net roundtrip, lighting seam, bin smoke) alongside workspace tests.
- Implemented UI3D billboard pipeline (instanced quads, depth + overlay paths) behind `ui3d_billboards`; includes smoke test and demo example.

## 0.1.0
- Initial release (deterministic voxel sandbox MVP).
