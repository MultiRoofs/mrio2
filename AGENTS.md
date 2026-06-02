# mrio2

Reads/modifies/writes CityJSON (`.city.json`) and CityJSONSeq (`.city.jsonl`) to prepare them for the MultiRoofs project.
CityJSON editor with a ratatui TUI and a WASM web app.

## Commands

```sh
cargo build
cargo test                        # 5 unit tests in mrio2-core
cargo run -p mrio2-cli -- data/3dbag_b2.city.json
cargo run -p mrio2-cli -- data/file.city.jsonl --output-format cityjson   # convert format

# Build WASM web app
wasm-pack build crates/mrio2-web --target web --out-dir ../../web/pkg

# Serve web app locally
python3 -m http.server 8080 --directory web
```

## Project structure

| Path | Role |
|------|------|
| `crates/mrio2-core/` | Shared library: model, io, ops, stats |
| `crates/mrio2-cli/` | Binary: clap CLI + ratatui TUI |
| `crates/mrio2-web/` | WASM: wasm-bindgen bindings for the web app |
| `web/index.html` | Web frontend (oat.ink UI, bright theme) |
| `web/style.css` | Custom CSS overrides |
| `web/app.js` | Web app logic (drag-drop, operations, download, validation) |
| `web/pkg/` | Generated WASM output (gitignored) |
| `data/` | Example CityJSON/Seq files |

## References

- **Specs**: <https://www.cityjson.org/specs/2.0.2/>
- **Schemas**: <https://3d.bk.tudelft.nl/schemas/cityjson/2.0.2/>
- **Examples**: `./data/`
- **UI library**: <https://oat.ink/>

## Design constraints

- **No typed CityJSON schema.** The entire document is stored as `serde_json::Value` / `Map<String, Value>`. All JSON manipulation is generic.
- **CityJSONSeq → CityJSON**: `collapse()` merges all features' CityObjects and vertices into the header (vertex indices are remapped). CityJSON → CityJSONSeq: `expand()` splits each CityObject into its own CityJSONFeature.
- **Operations mutate in-memory only.** User must explicitly save/download to persist.
- **ratatui 0.29 / crossterm 0.28** — note that `Layout::vertical()`/`horizontal()` take constraints directly (no `.constraints()` builder).
- **Test data** in `data/` is required for `cargo test` (tests read files from `../../data/` relative to crate).
- **Feature flags**: `mrio2-core` has a `native` feature (enabled by default) that includes `ureq` for fetching extension schemas during validation. `cjval` is always available. The WASM crate disables this feature.
- **Extension schema fetching**: In TUI/native mode, extension schemas are fetched via `ureq`. In web/WASM mode, extension schemas are fetched via the browser's `fetch()` API and passed to the validator as JSON.
- **CSV import**: Core function takes CSV content as `&str` (not a file path). CLI reads the file, web app reads via JS FileReader.

## TUI keybindings

| Key | Action |
|-----|--------|
| `↑↓` / `jk` | Navigate left panel / scroll right panel |
| `Tab` | Switch focus between panels |
| `Enter` | Select operation (opens dialog) |
| `s` | Save (opens path dialog, `f` toggles output format) |
| `q` | Quit (with confirm if unsaved) |
| `Esc` | Cancel dialog / quit with no changes |

## Web app features

- **Drag-and-drop**: Drop `.city.json` or `.city.jsonl` files to load
- **Operations panel**: Click buttons to run operations, dialogs for input
- **Validation**: Fetches extension schemas via browser fetch, displays results with color-coded icons (✓ green for valid, ✗ red for errors, ⚠ orange for warnings)
- **Download**: Choose format (CityJSON/CityJSONSeq) and filename
- **Reset**: "Load different file" button at bottom of operations panel returns to start screen
