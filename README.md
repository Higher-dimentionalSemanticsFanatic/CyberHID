# CyberHID

A RED4ext plugin for Cyberpunk 2077 that adds HID game controllers
(pedals, wheels, flight sticks, throttles, button boxes) to the XInput
gamepad the game reads. Triggers take the larger value of the gamepad and
the bound inputs, stick axes add up, buttons combine. Works next to a
gamepad or with mouse and keyboard alone. Bindings are set in the game,
under **Settings › Mods › CyberHID**.

Early version: not yet confirmed in game with real controllers.

## Download

Get `CyberHID-<version>.zip` from
[Releases](https://github.com/Higher-dimentionalSemanticsFanatic/CyberHID/releases).
Each release has a `SHA256SUMS` file to check the download against.

## Install

Needs [RED4ext](https://www.nexusmods.com/cyberpunk2077/mods/2380) and
[Mod Settings](https://www.nexusmods.com/cyberpunk2077/mods/4885) (which
needs redscript and ArchiveXL): bindings are set in Mod Settings' menu.

Unpack the archive into the game folder, or install it with a mod manager
(it is laid out from the game folder). It holds:

- `red4ext/plugins/cyberhid/cyberhid.dll`: the plugin
- `red4ext/plugins/cyberhid/README.md`: this file
- `r6/scripts/cyberhid/CyberHID.reds`: the in-game settings

To uninstall, remove the folders `red4ext/plugins/cyberhid` and
`r6/scripts/cyberhid`.

## Settings › Mods › CyberHID

- **General**: **Bindings on**, **Gamepad slot**, **Report a gamepad when
  none is connected** (with no gamepad on the slot, the plugin reports one
  while a bound controller is connected).
- One section per analog gamepad input (**Right trigger**, **Left
  trigger**, both sticks' X and Y): **Controller**, **Input**, **Invert**,
  **Deadzone (%)**, **Response**, and **Stick range** on stick axes.
- **Buttons**: controller and input for each gamepad button and d-pad
  direction. An axis counts as pressed past half its travel.
- **D-pad (hat switch)**: a hat switch drives all four d-pad directions.

The plugin reads the values again within a second of Mod Settings saving
them, also while the game runs.

## Finding the numbers

The menu binds by number: **Controller 1** to **4**, and **Axis**,
**Button** or **Hat** 1 to n on that controller.

- **Controller numbers**: game controllers that are not XInput gamepads,
  numbered by USB vendor and product ID (the same numbers every session
  while the same controllers are connected).
- **Input numbers**: the order the plugin reads the controller in.

While the game runs, the plugin writes
`red4ext/plugins/cyberhid/status.json`. Each entry under `devices` is one
controller: `controller` is its number, `axis_names` and `axis_values` its
axes and where they are now (percent), and `recent_inputs` the last inputs
moved, newest first. Press the brake pedal once and it shows as, for
example, `"Axis 3"` at the top of `recent_inputs`. Many pedals rest at the
top of their range (`axis_values` near 100): turn **Invert** on for those.

`missing` lists bound controller numbers with no controller connected,
`active_bindings` the bindings in use, `hooks` the XInput functions the
plugin hooked.

## How it works

At game start the plugin hooks `XInputGetState`, `XInputGetStateEx` and
`XInputGetCapabilities` in every `xinput*.dll` through RED4ext's hooking
API. It reads game controllers through Windows' `hid.dll` (under Proton:
Wine's, the same path the game's other input goes through) and adds the
bound inputs to what the game reads from the chosen gamepad slot. The
bindings come from Mod Settings' `red4ext/plugins/mod_settings/user.ini`;
no game scripts call into the plugin. It uses no game offsets, so it does
not depend on the game version.

## Building

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# The DLL (Windows target; from Linux with the mingw-w64 gcc package)
rustup target add x86_64-pc-windows-gnu
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo build -p cyberhid --release --target x86_64-pc-windows-gnu
```

- `crates/core` (`cyberhid-core`): bindings, merge rules, `status.json`,
  the in-game menu and the Windows `hid.dll` reader; tested on any
  platform. The menu script `crates/core/reds/CyberHID.reds` is generated
  by `menu::script`: after changing the menu, run
  `UPDATE_REDS=1 cargo test -p cyberhid-core menu::`.
- `crates/plugin` (`cyberhid`): the RED4ext entry points, the XInput hooks
  and the device threads. Builds only for Windows targets (empty
  elsewhere).

CI builds the archive on every push. Running the `build` workflow with
"release" ticked publishes it as release `v<version>` (the workspace
version in `Cargo.toml`) with `SHA256SUMS`; 0.x versions as pre-releases.

CyberHID started in [CPMX2077](https://github.com/Higher-dimentionalSemanticsFanatic/LinuxCPMX2077),
a Cyberpunk 2077 mod manager, and needs nothing from it.

## License and disclaimer

CyberHID is free software under the [GNU GPL v3](LICENSE). It is a
fan-made mod and is not made by, endorsed by or affiliated with CD PROJEKT
RED or Nexus Mods. Cyberpunk 2077 is a trademark of CD PROJEKT S.A.
