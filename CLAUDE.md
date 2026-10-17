# CLAUDE.md

CyberHID is a RED4ext plugin for Cyberpunk 2077 that merges HID game
controllers (pedals, wheels, sticks) into the XInput gamepad the game
reads. Bindings are set in the game through the Mod Settings mod. Rust
(edition 2024), cross-built as a Windows DLL; the game runs on Windows or
under Proton.

## Commands

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings                                # CI fails on any warning
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings # the Windows-only code

# The DLL (mingw-w64 gcc package for the linker)
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo build -p cyberhid --release --target x86_64-pc-windows-gnu

# After changing the in-game menu
UPDATE_REDS=1 cargo test -p cyberhid-core menu::
```

All three checks must pass before pushing.

## Layout

- `crates/core` (`cyberhid-core`): everything testable on any platform:
  bindings and merge rules (`contribution`: triggers take the larger value,
  stick axes add and clamp, buttons combine), `Moves` (inputs last moved,
  for `status.json`), `Status`, and `menu` (generates
  `reds/CyberHID.reds`, the class Mod Settings shows as Settings › Mods ›
  CyberHID, and parses Mod Settings' `user.ini`). `win`: the Windows
  `hid.dll` reader (Windows only). Keep it free of game and hook code.
- `crates/plugin` (`cyberhid`, cdylib `cyberhid.dll`): RED4ext ABI v1
  entry points written by hand (`Supports`, `Query`, `Main`), XInput hooks
  through RED4ext's hooking API, one thread per open controller, a monitor
  thread that reloads `user.ini` and writes `status.json`. Empty on
  non-Windows targets.
- `.github/workflows/build.yml`: lint, tests, the DLL and
  `CyberHID-<version>.zip` (DLL + README under `red4ext/plugins/cyberhid/`,
  script under `r6/scripts/cyberhid/`). Ticking "release" publishes it.

## Rules

- `README.md` is the user documentation and ships in the archive: update it
  with every change players see (menu entries, `status.json` fields,
  install layout, requirements).
- The `user.ini` format was read from Mod Settings' DLL, not seen from a
  real game: keep `menu::parse` tolerant (case, key prefixes, enum values
  as numbers or names).
- No game offsets or game-version-specific code: the plugin declares
  itself runtime-independent to RED4ext.
- Version: `version` in the root `Cargo.toml` (also `PLUGIN_VERSION`).
  Write `docs/releases/v<version>.md` before a release; the release job
  puts it above the install notes. Only release when the maintainer asks.
