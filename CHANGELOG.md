# Changelog

## 0.2.0 — 2026-06-04

### Added

- Workspace restructure: monorepo split into `mrio2-core`, `mrio2-cli`, `mrio2-web` crates
- Web app (WASM) with drag-and-drop, operations panel, validation, and download
- **Roof total area** operation — computes and adds per-object roof area attribute
- **Set CRS EPSG** operation — changes the CRS EPSG code of the document
- **Validate schema** operation — validates against CityJSON schema via `cjval`
- **Validate with extensions** — fetches extension schemas and validates against them
- Extension schema loading during validation (native via ureq, WASM via browser fetch)
- GitHub CI deploy workflow for hosting the web app
- Version display on web app
- Warning dialog when saving overwrites an existing file

### Changed

- Operations restructured to return `OpReport` with summary, affected count, and error flag
- Roofer → MultiRoofs now also adds roof-total-area attribute
- Web app styling improvements with oat.ink UI framework
- TUI input dialog supports numeric entry for roof area operator parameters

### Fixed

- Cursor position preserved when writing in TUI save dialog
- Removed duplicate "Validate schema" entry from operations list

## 0.1.0 — 2026-05-04

Initial release.

### Added

- Read both CityJSON (`.city.json`) and CityJSONSeq (`.city.jsonl`) — auto-detected by extension
- Write both formats with conversion between them (collapse/expand with vertex remapping)
- TUI with two panels: operations list and scrollable file overview
- Tab-based focus switching between panels (border highlight)
- Operations:
  - **Attribute: delete** — remove an attribute from all CityObjects
  - **Attribute: rename** — rename an attribute across all CityObjects
  - **Attributes: add from CSV** — bulk-add attributes from a CSV file (auto-detects `;` / `,` delimiter)
  - **Roofer → MultiRoofs** — merge BuildingParts into parents, clean up geometry, rename `b3_volume`, add multiroofs extension
- File statistics panel: object counts by type, attribute inventory with sample values, CRS, extensions
- CLI flags: `--output`, `--output-format` (cityjson / cityjsonseq / auto)
- Dialog system for operation parameters, save path input, and quit confirmation
