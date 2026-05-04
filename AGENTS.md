# mrio2

Reads/modifies/writes CityJSON (`.city.json`) and CityJSONSeq (`.city.jsonl`) to prepare them for the MultiRoofs project.
CityJSON editor with a ratatui TUI. 

## Commands

```sh
cargo build
cargo test                        # 4 unit tests in src/io.rs
cargo run -- data/3dbag_b2.city.json
cargo run -- data/file.city.jsonl --output-format cityjson   # convert format
```

## Project structure

| Path | Role |
|------|------|
| `src/main.rs` | CLI parsing (clap) |
| `src/model.rs` | `CityJsonDocument`, `InputFormat`/`OutputFormat` |
| `src/io.rs` | Read/write both formats, `collapse()`/`expand()` with vertex remapping |
| `src/ops.rs` | `remove_attribute`, `rename_attribute`, `add_attributes_from_csv`, `roofer2multiroofs`, `validate_schema` |
| `src/stats.rs` | File statistics computation |
| `src/tui.rs` | ratatui app: operations panel, scrollable overview, dialogs |
| `data/` | Example CityJSON/Seq files |

## Design constraints

- **No typed CityJSON schema.** The entire document is stored as `serde_json::Value` / `Map<String, Value>`. All JSON manipulation is generic.
- **CityJSONSeq → CityJSON**: `collapse()` merges all features' CityObjects and vertices into the header (vertex indices are remapped). CityJSON → CityJSONSeq: `expand()` splits each CityObject into its own CityJSONFeature.
- **Operations mutate in-memory only.** User must explicitly `s`ave to persist.
- **ratatui 0.29 / crossterm 0.28** — note that `Layout::vertical()`/`horizontal()` take constraints directly (no `.constraints()` builder).
- **Test data** in `data/` is required for `cargo test` (tests read files from there).

## TUI keybindings

| Key | Action |
|-----|--------|
| `↑↓` / `jk` | Navigate left panel / scroll right panel |
| `Tab` | Switch focus between panels |
| `Enter` | Select operation (opens dialog) |
| `s` | Save (opens path dialog, `f` toggles output format) |
| `q` | Quit (with confirm if unsaved) |
| `Esc` | Cancel dialog / quit with no changes |
