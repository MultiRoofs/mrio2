# Changelog

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
