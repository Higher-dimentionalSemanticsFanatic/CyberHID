# CLAUDE.md

CyberHID is a RED4ext plugin for Cyberpunk 2077 that merges HID game
controllers (pedals, wheels, sticks) into the XInput gamepad the game
reads. Bindings are set in a Cyber Engine Tweaks window (Lua, ImGui).
Rust (edition 2024), cross-built as a Windows DLL; the game runs on
Windows or under Proton.

## Commands

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings                                # CI fails on any warning
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings # the Windows-only code

# The DLL (mingw-w64 gcc package for the linker)
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo build -p cyberhid --release --target x86_64-pc-windows-gnu

luajit cet/tests/run.lua   # the CET window, with stand-ins for CET's ImGui
```

All four checks must pass before pushing.

## Layout

- `crates/core` (`cyberhid-core`): everything testable on any platform:
  bindings and merge rules (`contribution`: triggers take the larger value,
  stick axes add and clamp, buttons combine), `config` (`config.json`,
  written by the window), `kind` (controller type from HID usages and
  name), `Moves` (inputs last moved) and `Status` (`state.json`, read by
  the window). `win`: the Windows `hid.dll` reader (Windows only). Keep it
  free of game and hook code.
- `crates/plugin` (`cyberhid`, cdylib `cyberhid.dll`): RED4ext ABI v1
  entry points written by hand (`Supports`, `Query`, `Main`), XInput hooks
  through RED4ext's hooking API, game keys F13-F24 through `SendInput`
  (only while the game's window is in front), one thread per open
  controller, a monitor thread that reloads `config.json` and writes
  `state.json` (20 Hz while the window's `ui.json` heartbeat is fresh).
  Files live in the CET mod folder (`CET_DIR`), else the plugin folder.
  Empty on non-Windows targets.
- `cet/CyberHID`: the CET mod. `model.lua` (config, state, scan, the same
  response maths as `contribution`; no ImGui), `ui.lua` (the window),
  `init.lua` (CET events). CET's sandbox: `io.open` and `os.rename` only
  inside the mod folder, `os.rename` refuses an existing target.
  `cet/tests/run.lua` checks every ImGui, enum and IconGlyphs name against
  lists read from CET's sources; add a name there only after checking
  CET's `sol_imgui.h` / IconGlyphs.
- `.github/workflows/build.yml`: lint, tests, the DLL and
  `CyberHID-<version>.zip` (DLL + README under `red4ext/plugins/cyberhid/`,
  Lua under `bin/x64/plugins/cyber_engine_tweaks/mods/CyberHID/`). Ticking
  "release" publishes it.

## Rules

- `README.md` is the user documentation and ships in the archive: update it
  with every change players see (window labels, `state.json` fields,
  install layout, requirements).
- `config.json` is written by Lua (rxi json.lua: an empty table may come
  out as `{}` or `[]`): keep `Config::parse` tolerant and skip bad
  bindings instead of failing the file.
- `model.lua` repeats the response maths of `crates/core`
  (`from_rest`, `centred`): change both together; the Lua tests reuse the
  Rust test's numbers.
- No game offsets or game-version-specific code: the plugin declares
  itself runtime-independent to RED4ext.
- Version: `version` in the root `Cargo.toml` (also `PLUGIN_VERSION`).
  Write `docs/releases/v<version>.md` before a release; the release job
  puts it above the install notes. Only release when the maintainer asks.
