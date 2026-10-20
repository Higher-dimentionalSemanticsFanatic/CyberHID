# CyberHID

A RED4ext plugin for Cyberpunk 2077 that adds HID game controllers
(pedals, wheels, flight sticks, throttles, button boxes) to the XInput
gamepad the game reads. Triggers take the larger value of the gamepad and
the bound inputs, stick axes add up, buttons combine. Works next to a
gamepad or with mouse and keyboard alone.

Bindings are set in the **CyberHID** window in the Cyber Engine Tweaks
overlay: it lists the connected controllers with their type and live
axes, buttons and hats, and binds a gamepad input by moving the control
you want for it.

Early version: not yet confirmed in game with real controllers.

## Download

Get `CyberHID-<version>.zip` from
[Releases](https://github.com/Higher-dimentionalSemanticsFanatic/CyberHID/releases).
Each release has a `SHA256SUMS` file to check the download against.

## Install

Needs [RED4ext](https://www.nexusmods.com/cyberpunk2077/mods/2380) and
[Cyber Engine Tweaks](https://www.nexusmods.com/cyberpunk2077/mods/107)
(CET).

Unpack the archive into the game folder, or install it with a mod manager
(it is laid out from the game folder). It holds:

- `red4ext/plugins/cyberhid/cyberhid.dll`: the plugin
- `red4ext/plugins/cyberhid/README.md`: this file
- `bin/x64/plugins/cyber_engine_tweaks/mods/CyberHID/`: the CyberHID
  window (`init.lua`, `model.lua`, `ui.lua`)

To uninstall, remove the folders `red4ext/plugins/cyberhid` and
`bin/x64/plugins/cyber_engine_tweaks/mods/CyberHID` (the second one also
holds your bindings).

**Upgrading from 0.1.0**: delete `r6/scripts/cyberhid` (the old Settings ›
Mods › CyberHID menu). Mod Settings is no longer needed, and bindings made
there are not carried over.

### Linux (Proton)

Under Proton, controllers reach the game through SDL by default, which
passes at most 6 axes per device and may turn a controller with many
buttons into a second gamepad. To pass a controller to the game as it is,
list it in the Steam launch options by USB vendor and product ID:

```
PROTON_ENABLE_HIDRAW=0x046d/0xc24f,0x044f/0xb679 %command%
```

(Proton 9 and later; Proton 10 also takes `PROTON_ENABLE_HIDRAW=1` for
every device.) Proton opens these through `/dev/hidraw*`, which needs read
and write access for your user; a udev rule with `TAG+="uaccess"` gives
it. [CPMX2077](https://github.com/Higher-dimentionalSemanticsFanatic/LinuxCPMX2077)
does both: Game panel › **CyberHID controllers** › **Set launch option**,
and **CyberHID controller access** for the udev rule. `lsusb` lists the
IDs.

## The CyberHID window

Open the CET overlay (its key is set when CET first starts). The
**CyberHID** window shows while the overlay is open.

The lines at the top show what the plugin reports: its version and the
XInput functions it hooked, how many bindings are active, the gamepad
slot (a real gamepad on it, or one reported by CyberHID), and bound
controllers that are not connected.

### Controllers

Every game controller the game can see, with an icon and type (wheel,
pedals, rudder pedals, flight stick, throttle, button box, gamepad,
joystick). The type is read from the HID usages the controller reports
(steering, brake, rudder, throttle axes) and otherwise from its name.

Select a controller to see its inputs live:

- **Axes**: each axis with its HID name (`Axis 3 (Brake)`), its
  position on the -1 to 1 scale of KDE's and SDL's controller settings
  (**Value (-1 to 1)**: -1 one end, 0 the middle, 1 the other end), the
  raw number in the device's input report (**HID value**; hover it for
  the device's declared range) and what it is bound to. The first two
  axes of a stick also show as a dot in a square, the way it moves.
- **Buttons**: numbered squares, lit while pressed, outlined green when
  bound.
- **Hats**: each hat switch's direction.
- **Last moved**: the inputs moved most recently.

XInput gamepads are listed but not read: the game reads them itself.

### Bindings

The gamepad inputs on the left (triggers, sticks, buttons, d-pad), each
with what it is bound to and what CyberHID sends for it now. Select one:

- **Scan for input**, then move the axis or press the button you want,
  on any controller (10 s). Triggers and sticks take the axis that moved
  a third of its range first, buttons the button pressed first; the
  **D-pad (hat)** takes a hat. A pedal resting at the top of its
  range gets **Invert** set; one bound to a stick moves the stick from
  the middle to one end.
- **Add** adds a binding to pick by hand under **Controller** and
  **Input**.
- Each binding shows its input and output live, with **Scan again** and
  **Remove**, and:
  - **Invert**: for pedals and triggers that rest at the top.
  - **Deadzone**: travel ignored at rest (around the middle for a whole
    stick axis), 0 to 50 %.
  - **Response**: 1.0 is linear; above 1 gives finer control near rest,
    below 1 quicker. **Linear**, **Progressive** and **Strong** set 1.0,
    1.6 and 2.5. The graph shows the curve and where the input is on it.
  - **Range** (sticks): **Whole axis** for a wheel or stick resting in
    the middle, **Rest to +** or **Rest to -** for a pedal or button
    pushing the stick one way.
  - An axis bound to a button counts as pressed past half its travel.

Several bindings on one gamepad input combine like the gamepad does.
Changes save to `config.json` at once and apply within a second.

### Settings

- **Enabled**: bound input is added to the gamepad.
- **Gamepad slot**: the XInput slot the game reads, slot 1 unless several
  gamepads are connected.
- **Report a gamepad when none is connected**: with no real gamepad on
  the slot, the plugin reports one while a binding is active. Off, bound
  input reaches the game only while a real gamepad is connected.
- **Plugin**: version, Wine version under Proton, hooked functions and
  how often the game read the gamepad.

## Files

In the window's folder, `bin/x64/plugins/cyber_engine_tweaks/mods/CyberHID/`
(the plugin's own folder when CET is not installed):

- `config.json`: the bindings, written by the window.
- `state.json`: written by the plugin, 20 times a second while the window
  is open and every 5 seconds otherwise: each controller's inputs, what
  the bindings send (`output`), `missing` controllers, `hooks`. Each axis
  has `value` (0 to 1 over its range), `raw` (the report value) and its
  logical range `min` to `max`.
- `ui.json`: rewritten every second while the window is open.

## How it works

At game start the plugin hooks `XInputGetState`, `XInputGetStateEx` and
`XInputGetCapabilities` in every `xinput*.dll` through RED4ext's hooking
API. It reads game controllers through Windows' `hid.dll` (under Proton:
Wine's, the same path the game's other input goes through) and adds the
bound inputs to what the game reads from the chosen gamepad slot. The
window and the plugin talk only through the files above; no game scripts
call into the plugin. It uses no game offsets, so it does not depend on
the game version.

## Building

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
luajit cet/tests/run.lua    # the CET window

# The DLL (Windows target; from Linux with the mingw-w64 gcc package)
rustup target add x86_64-pc-windows-gnu
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo build -p cyberhid --release --target x86_64-pc-windows-gnu
```

- `crates/core` (`cyberhid-core`): bindings and `config.json`, merge
  rules, controller types, `state.json` and the Windows `hid.dll` reader;
  tested on any platform.
- `crates/plugin` (`cyberhid`): the RED4ext entry points, the XInput hooks
  and the device threads. Builds only for Windows targets (empty
  elsewhere).
- `cet/CyberHID`: the CET window. `cet/tests` runs it under LuaJIT with
  stand-ins for CET's ImGui bindings.

CI builds the archive on every push. Running the `build` workflow with
"release" ticked publishes it as release `v<version>` (the workspace
version in `Cargo.toml`) with `SHA256SUMS`; 0.x versions as pre-releases.

CyberHID started in [CPMX2077](https://github.com/Higher-dimentionalSemanticsFanatic/LinuxCPMX2077),
a Cyberpunk 2077 mod manager, and needs nothing from it.

## License and disclaimer

CyberHID is free software under the [GNU GPL v3](LICENSE). It is a
fan-made mod and is not made by, endorsed by or affiliated with CD PROJEKT
RED or Nexus Mods. Cyberpunk 2077 is a trademark of CD PROJEKT S.A.
`cet/tests/json.lua` is rxi's json.lua (MIT license, notice in the file).
