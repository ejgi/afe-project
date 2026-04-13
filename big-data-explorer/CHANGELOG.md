# Changelog

All notable changes to the **Forensic Nitro-Search** extension will be documented in this file.

## [1.2.1] - 2026-04-12

### Added
- **Multi-platform Native Support**: Added optimized Windows 64-bit binaries. The extension now delivers the same extreme performance on both Windows and Linux.
- **Jump to Row**: Type a line number (e.g., `30`) in the search box to jump instantly to that row.
- **Results Viewfinder**: New overlay that displays the first 10 matches instantly for quick forensic inspection.
- **Advanced Result Handling**: Automatic detection of large result sets with a threshold for opening in a new editor tab.

### Fixed
- Fixed search logic that was failing to distinguish between text queries and numerical jumps.
- Properly synchronized the search viewfinder with the Nitro-Engine backend.

## [1.2.0] - 2026-04-12

### Added
- **Nitro-Direct Architecture**: Re-engineered data loading to achieve O(1) complexity. No more long waits while scanning directories.
- **Dynamic Hardware Detection**: Automatic sensing of SSD vs. HDD storage.
- **Smart Parallelism**: The engine now adjusts its SIMD scanning strategy based on your disk type to prevent mechanical thrashing on HDDs.
- **Automatic Preamble Detection**: Now automatically detects and skips metadata headers (skip_rows) and identifies RFC-4180 compatibility.

### Fixed
- **Accessibility (A11y)**: Fixed missing keyboard event handlers and improved semantic HTML across the dashboard.
- **Speed**: Resolved a critical bottleneck where the daemon would take too long to initialize in large-scale workspaces.

## [1.1.0] - 2026-03-24
- Initial implementation of the Zen-Engine Nitro-Search core.
- Virtual dataset support and basic SIMD search.
