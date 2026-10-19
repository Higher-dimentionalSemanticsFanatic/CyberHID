//! CyberHID's logic that does not need the game or Windows: bindings and
//! their file ([`config`]), the merge rules, controller kinds ([`kind`]) and
//! the state file the controls window reads. The plugin (`crates/plugin`)
//! supplies the hooks and device reads; this crate is tested on any
//! platform.
//!
//! Cyberpunk 2077 reads one XInput gamepad. Each binding takes one input of a
//! HID game controller (an axis, a button or a hat) and adds it to that
//! gamepad: triggers take the larger value, stick axes add up, buttons
//! combine. Pedals, wheels, sticks and button boxes then drive the game next
//! to (or instead of) a real gamepad.

pub mod config;
pub mod kind;
#[cfg(windows)]
pub mod win;

use serde::{Deserialize, Serialize};

pub use kind::Kind;

pub const PLUGIN_NAME: &str = "CyberHID";
/// The plugin's version (the workspace version), reported to RED4ext, in
/// `state.json` and in the mod archive's name.
pub const PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Plugin folder under the game root. RED4ext loads
/// `red4ext/plugins/<folder>/<name>.dll`, one folder level deep.
pub const PLUGIN_DIR: &str = "red4ext/plugins/cyberhid";
pub const PLUGIN_DLL: &str = "cyberhid.dll";
/// The controls window: a Cyber Engine Tweaks mod in this folder under the
/// game root. The plugin reads [`config::CONFIG_FILE`] and writes
/// [`STATE_FILE`] there (in its own folder when CET's is missing).
pub const CET_DIR: &str = "bin/x64/plugins/cyber_engine_tweaks/mods/CyberHID";
/// Written by the plugin while the game runs: controllers, their inputs
/// and what the bindings add.
pub const STATE_FILE: &str = "state.json";
/// Rewritten by the controls window every second while it is open; the
/// plugin then writes [`STATE_FILE`] 20 times a second instead of every 5.
pub const UI_FILE: &str = "ui.json";

pub const PAGE_GENERIC: u16 = 0x01;
pub const PAGE_SIMULATION: u16 = 0x02;
pub const PAGE_BUTTON: u16 = 0x09;
pub const GD_HAT_SWITCH: u16 = 0x39;

/// A HID usage as one number: page in the high half, usage id in the low.
pub const fn usage(page: u16, id: u16) -> u32 {
    (page as u32) << 16 | id as u32
}

/// The axis name HID gives a usage, for labels.
pub fn usage_label(u: u32) -> Option<&'static str> {
    let (page, id) = ((u >> 16) as u16, u as u16);
    Some(match (page, id) {
        (PAGE_GENERIC, 0x30) => "X axis",
        (PAGE_GENERIC, 0x31) => "Y axis",
        (PAGE_GENERIC, 0x32) => "Z axis",
        (PAGE_GENERIC, 0x33) => "X rotation",
        (PAGE_GENERIC, 0x34) => "Y rotation",
        (PAGE_GENERIC, 0x35) => "Z rotation",
        (PAGE_GENERIC, 0x36) => "Slider",
        (PAGE_GENERIC, 0x37) => "Dial",
        (PAGE_GENERIC, 0x38) => "Wheel",
        (PAGE_SIMULATION, 0xBA) => "Rudder",
        (PAGE_SIMULATION, 0xBB) => "Throttle",
        (PAGE_SIMULATION, 0xC4) => "Accelerator",
        (PAGE_SIMULATION, 0xC5) => "Brake",
        (PAGE_SIMULATION, 0xC6) => "Clutch",
        (PAGE_SIMULATION, 0xC8) => "Steering",
        _ => return None,
    })
}

/// `vvvv:pppp`, plus `#n` for the n-th of several identical devices.
pub fn device_id(vid: u16, pid: u16, instance: u32) -> String {
    if instance == 0 { format!("{vid:04x}:{pid:04x}") } else { format!("{vid:04x}:{pid:04x}#{instance}") }
}

// ---- bindings ------------------------------------------------------------------

/// An input, by its position as the plugin reads the controller, from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    Axis { index: u32 },
    Button { index: u32 },
    Hat { index: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
    A,
    B,
    X,
    Y,
    LeftShoulder,
    RightShoulder,
    Back,
    Start,
    LeftThumb,
    RightThumb,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    /// All four directions, from a hat switch.
    Dpad,
    /// Keyboard keys F13 to F24, which no common keyboard has: bound to game
    /// actions in the game's own Settings › Key Bindings.
    KeyF13,
    KeyF14,
    KeyF15,
    KeyF16,
    KeyF17,
    KeyF18,
    KeyF19,
    KeyF20,
    KeyF21,
    KeyF22,
    KeyF23,
    KeyF24,
}

/// Windows virtual-key code and set-1 scan code of F13 to F24, the keys
/// the game lists as `IK_F13` to `IK_F24` (its key ids are virtual-key
/// codes).
pub const GAME_KEYS: [(u16, u16); 12] = [
    (0x7C, 0x64),
    (0x7D, 0x65),
    (0x7E, 0x66),
    (0x7F, 0x67),
    (0x80, 0x68),
    (0x81, 0x69),
    (0x82, 0x6A),
    (0x83, 0x6B),
    (0x84, 0x6C),
    (0x85, 0x6D),
    (0x86, 0x6E),
    (0x87, 0x76),
];

impl Target {
    /// The XInput button bit, for button targets.
    pub fn button(self) -> Option<u16> {
        Some(match self {
            Target::DpadUp => 0x0001,
            Target::DpadDown => 0x0002,
            Target::DpadLeft => 0x0004,
            Target::DpadRight => 0x0008,
            Target::Start => 0x0010,
            Target::Back => 0x0020,
            Target::LeftThumb => 0x0040,
            Target::RightThumb => 0x0080,
            Target::LeftShoulder => 0x0100,
            Target::RightShoulder => 0x0200,
            Target::A => 0x1000,
            Target::B => 0x2000,
            Target::X => 0x4000,
            Target::Y => 0x8000,
            _ => return None,
        })
    }

    /// Index into [`GAME_KEYS`], for key targets.
    pub fn key(self) -> Option<usize> {
        const KEYS: [Target; 12] = [
            Target::KeyF13,
            Target::KeyF14,
            Target::KeyF15,
            Target::KeyF16,
            Target::KeyF17,
            Target::KeyF18,
            Target::KeyF19,
            Target::KeyF20,
            Target::KeyF21,
            Target::KeyF22,
            Target::KeyF23,
            Target::KeyF24,
        ];
        KEYS.iter().position(|k| *k == self)
    }

    fn is_stick(self) -> bool {
        matches!(self, Target::LeftX | Target::LeftY | Target::RightX | Target::RightY)
    }
}

/// Which part of a stick axis an axis or button drives.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Half {
    /// The whole axis: input at rest in the middle, as a wheel or stick.
    #[default]
    Full,
    /// From the middle to the positive end (right, up), as a pedal.
    Positive,
    /// From the middle to the negative end (left, down).
    Negative,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// The controller, as [`device_id`].
    pub device: String,
    pub input: Input,
    pub target: Target,
    pub invert: bool,
    /// Ignored travel, 0 to 0.5: at the rest end (pedals, triggers, half
    /// axes) or around the middle (full stick axes).
    pub deadzone: f32,
    /// Response exponent: 1 linear, above 1 finer near rest.
    pub curve: f32,
    pub half: Half,
}

// ---- matching inputs on a connected controller ----------------------------------

/// The inputs of a connected device as the reading side lists them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// Axis usages in the order they are read.
    pub axes: Vec<u32>,
    pub buttons: usize,
    pub hats: usize,
}

/// Where a binding's input sits in a [`Layout`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Axis(usize),
    Button(usize),
    Hat(usize),
}

/// Find `input` on a device that reports `layout`; `None` when the device
/// has fewer inputs of that kind.
pub fn resolve(input: &Input, layout: &Layout) -> Option<Slot> {
    match *input {
        Input::Axis { index } => ((index as usize) < layout.axes.len()).then_some(Slot::Axis(index as usize)),
        Input::Button { index } => ((index as usize) < layout.buttons).then_some(Slot::Button(index as usize)),
        Input::Hat { index } => ((index as usize) < layout.hats).then_some(Slot::Hat(index as usize)),
    }
}

// ---- merging into the gamepad -------------------------------------------------

/// Current values of a connected device: axes 0 to 1 over their logical
/// range, hats as one of 8 directions clockwise from up (0) or centred.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Live {
    pub axes: Vec<f32>,
    pub buttons: Vec<bool>,
    pub hats: Vec<Option<u8>>,
}

/// A binding found on a connected device.
#[derive(Debug, Clone, PartialEq)]
pub struct Active {
    pub binding: Binding,
    /// Index into the connected devices' [`Live`] list.
    pub device: usize,
    pub slot: Slot,
}

/// The XInput gamepad fields.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pad {
    pub buttons: u16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub lx: i16,
    pub ly: i16,
    pub rx: i16,
    pub ry: i16,
}

/// What the bound inputs add: triggers 0 to 1, stick axes -1 to 1, and
/// the game keys held (bit n: [`GAME_KEYS`]`[n]`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Contribution {
    pub buttons: u16,
    pub keys: u16,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
}

impl Contribution {
    pub fn is_neutral(&self) -> bool {
        *self == Contribution::default()
    }

    /// `real` with these inputs added: triggers take the larger value,
    /// stick axes add up (clamped), buttons combine.
    pub fn apply(&self, real: Pad) -> Pad {
        let trig = |r: u8, c: f32| r.max((c.clamp(0.0, 1.0) * 255.0).round() as u8);
        let stick = |r: i16, c: f32| (r as i32 + (c.clamp(-1.0, 1.0) * 32767.0).round() as i32).clamp(-32768, 32767) as i16;
        Pad {
            buttons: real.buttons | self.buttons,
            left_trigger: trig(real.left_trigger, self.left_trigger),
            right_trigger: trig(real.right_trigger, self.right_trigger),
            lx: stick(real.lx, self.lx),
            ly: stick(real.ly, self.ly),
            rx: stick(real.rx, self.rx),
            ry: stick(real.ry, self.ry),
        }
    }
}

/// Travel from rest, 0 to 1, after invert, deadzone and curve.
fn from_rest(v: f32, b: &Binding) -> f32 {
    let v = if b.invert { 1.0 - v } else { v }.clamp(0.0, 1.0);
    let dz = b.deadzone.clamp(0.0, 0.5);
    let v = if v <= dz { 0.0 } else { (v - dz) / (1.0 - dz) };
    v.powf(curve(b))
}

/// -1 to 1 around the middle, after invert, deadzone and curve.
fn centred(v: f32, b: &Binding) -> f32 {
    let s = (v.clamp(0.0, 1.0) * 2.0 - 1.0) * if b.invert { -1.0 } else { 1.0 };
    let dz = b.deadzone.clamp(0.0, 0.5);
    let m = s.abs();
    let m = if m <= dz { 0.0 } else { (m - dz) / (1.0 - dz) };
    m.powf(curve(b)) * s.signum()
}

fn curve(b: &Binding) -> f32 {
    if b.curve.is_finite() { b.curve.clamp(0.2, 5.0) } else { 1.0 }
}

/// The 8-way hat direction as dpad bits.
fn hat_buttons(dir: Option<u8>) -> u16 {
    const UP: u16 = 0x0001;
    const DOWN: u16 = 0x0002;
    const LEFT: u16 = 0x0004;
    const RIGHT: u16 = 0x0008;
    match dir {
        Some(0) => UP,
        Some(1) => UP | RIGHT,
        Some(2) => RIGHT,
        Some(3) => DOWN | RIGHT,
        Some(4) => DOWN,
        Some(5) => DOWN | LEFT,
        Some(6) => LEFT,
        Some(7) => UP | LEFT,
        _ => 0,
    }
}

/// What every active binding adds right now.
pub fn contribution(active: &[Active], live: &[Live]) -> Contribution {
    let mut c = Contribution::default();
    for a in active {
        let Some(dev) = live.get(a.device) else { continue };
        let b = &a.binding;
        // Normalise the input: axes and buttons to 0..1, hats straight to bits.
        let raw = match a.slot {
            Slot::Axis(i) => match dev.axes.get(i) {
                Some(v) => *v,
                None => continue,
            },
            Slot::Button(i) => match dev.buttons.get(i) {
                Some(p) => f32::from(u8::from(*p)),
                None => continue,
            },
            Slot::Hat(i) => {
                if b.target == Target::Dpad {
                    c.buttons |= hat_buttons(dev.hats.get(i).copied().flatten());
                }
                continue;
            }
        };
        let is_axis = matches!(a.slot, Slot::Axis(_));
        if let Some(bit) = b.target.button() {
            if from_rest(raw, b) > 0.5 {
                c.buttons |= bit;
            }
            continue;
        }
        if let Some(k) = b.target.key() {
            if from_rest(raw, b) > 0.5 {
                c.keys |= 1 << k;
            }
            continue;
        }
        if b.target.is_stick() {
            let v = match (b.half, is_axis) {
                (Half::Full, true) => centred(raw, b),
                (Half::Negative, _) => -from_rest(raw, b),
                _ => from_rest(raw, b),
            };
            let field = match b.target {
                Target::LeftX => &mut c.lx,
                Target::LeftY => &mut c.ly,
                Target::RightX => &mut c.rx,
                _ => &mut c.ry,
            };
            *field = (*field + v).clamp(-1.0, 1.0);
            continue;
        }
        let v = from_rest(raw, b);
        match b.target {
            Target::LeftTrigger => c.left_trigger = c.left_trigger.max(v),
            Target::RightTrigger => c.right_trigger = c.right_trigger.max(v),
            _ => {}
        }
    }
    c
}

// ---- finding input numbers ------------------------------------------------------

/// How many inputs [`Moves::recent`] keeps.
pub const RECENT_INPUTS: usize = 5;

/// The inputs moved on one controller, newest first, so players can look
/// up which number an input has: an axis once it travels a third of its
/// range from rest, a button when pressed, a hat when pushed. Rest is where
/// each input was in the first report read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Moves {
    rest: Option<Live>,
    away: Vec<bool>,
    pub recent: Vec<String>,
}

impl Moves {
    /// Take the controller's newest values; true when [`Moves::recent`]
    /// changed.
    pub fn update(&mut self, now: &Live) -> bool {
        let Some(rest) = &mut self.rest else {
            self.rest = Some(now.clone());
            self.away = vec![false; now.axes.len()];
            return false;
        };
        let mut moved = Vec::new();
        for (i, (v, r)) in now.axes.iter().zip(&rest.axes).enumerate() {
            let d = (v - r).abs();
            if !self.away[i] && d > 1.0 / 3.0 {
                self.away[i] = true;
                moved.push(format!("Axis {}", i + 1));
            } else if self.away[i] && d < 0.1 {
                self.away[i] = false;
            }
        }
        for (i, (b, was)) in now.buttons.iter().zip(&mut rest.buttons).enumerate() {
            if *b && !*was {
                moved.push(format!("Button {}", i + 1));
            }
            *was = *b;
        }
        for (i, (h, was)) in now.hats.iter().zip(&mut rest.hats).enumerate() {
            if h.is_some() && was.is_none() {
                moved.push(format!("Hat {}", i + 1));
            }
            *was = *h;
        }
        // Inputs moved together keep their order (axes, buttons, hats).
        for name in moved.iter().rev() {
            self.recent.retain(|r| r != name);
            self.recent.insert(0, name.clone());
        }
        self.recent.truncate(RECENT_INPUTS);
        !moved.is_empty()
    }
}

// ---- state file ---------------------------------------------------------------

/// Round to 3 decimals, which keeps the state file short.
fn milli(v: f32) -> f32 {
    (v * 1000.0).round() / 1000.0
}

/// [`STATE_FILE`]: what the plugin found in the running game.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Status {
    pub plugin_version: String,
    /// Wine's version when the game runs under Wine or Proton.
    pub wine: Option<String>,
    /// Unix seconds.
    pub started: u64,
    /// Unix milliseconds.
    pub updated: u64,
    /// `module!function` for every XInput function hooked.
    pub hooks: Vec<String>,
    pub hook_errors: Vec<String>,
    /// XInputGetState calls by the game, any slot.
    pub get_state_calls: u64,
    /// Reads of the bound slot that carried bound input.
    pub merged_reads: u64,
    /// Reads answered with a reported gamepad (no real one on the slot).
    pub emulated_reads: u64,
    /// `config.json` could not be read.
    pub config_error: Option<String>,
    /// Bindings that `config.json` lists but the plugin skipped.
    pub config_warnings: Vec<String>,
    pub config_saved: bool,
    pub enabled: bool,
    /// Gamepad slot 1 to 4.
    pub slot: u32,
    /// A real gamepad answered the last read of the slot.
    pub slot_pad: bool,
    pub bindings: usize,
    /// Bindings whose controller and input were found.
    pub active_bindings: usize,
    /// Every game controller connected, XInput gamepads included.
    pub devices: Vec<StatusDevice>,
    /// Bound controllers that are not connected: `name (id)`.
    pub missing: Vec<String>,
    /// What the bindings add to the gamepad right now.
    pub output: Output,
    /// Game keys the plugin holds down now (bit 0: F13); keys in
    /// `output.keys` are held only while the game window is in front.
    pub keys_held: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct StatusDevice {
    /// [`device_id`].
    pub id: String,
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub kind: Kind,
    /// [`Kind::label`].
    pub kind_label: String,
    /// An XInput gamepad, read by the game itself (not opened).
    pub xinput: bool,
    /// Opened and reading.
    pub connected: bool,
    pub axes: Vec<AxisStatus>,
    pub buttons: usize,
    /// Buttons held down now, numbered from 1.
    pub pressed: Vec<u32>,
    /// Each hat: -1 centred, else 0 (up) to 7 clockwise.
    pub hats: Vec<i8>,
    /// [`Moves::recent`].
    pub recent_inputs: Vec<String>,
    /// A binding uses it.
    pub bound: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct AxisStatus {
    /// What HID calls it (`Brake`, `X axis`), or `Axis n`.
    pub name: String,
    /// Position now, 0 to 1 over its range.
    pub value: f32,
}

/// [`Contribution`] as written to the state file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct Output {
    pub buttons: u16,
    /// Game keys held: bit 0 is F13.
    pub keys: u16,
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub lx: f32,
    pub ly: f32,
    pub rx: f32,
    pub ry: f32,
}

impl From<Contribution> for Output {
    fn from(c: Contribution) -> Self {
        Output {
            buttons: c.buttons,
            keys: c.keys,
            left_trigger: milli(c.left_trigger),
            right_trigger: milli(c.right_trigger),
            lx: milli(c.lx),
            ly: milli(c.ly),
            rx: milli(c.rx),
            ry: milli(c.ry),
        }
    }
}

impl StatusDevice {
    /// Inputs of a device as it reads now (`live` is `None` for a device
    /// that isn't open).
    pub fn inputs(&mut self, layout: &Layout, live: Option<&Live>) {
        self.axes = layout
            .axes
            .iter()
            .enumerate()
            .map(|(i, u)| AxisStatus {
                name: usage_label(*u).map_or_else(|| format!("Axis {}", i + 1), String::from),
                value: live.and_then(|l| l.axes.get(i)).map_or(0.0, |v| milli(*v)),
            })
            .collect();
        self.buttons = layout.buttons;
        self.pressed = live
            .map(|l| l.buttons.iter().enumerate().filter(|(_, p)| **p).map(|(i, _)| i as u32 + 1).collect())
            .unwrap_or_default();
        self.hats = (0..layout.hats).map(|i| live.and_then(|l| l.hats.get(i).copied().flatten()).map_or(-1, |d| d as i8)).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bind(input: Input, target: Target) -> Binding {
        Binding { device: "046d:c24f".into(), input, target, invert: false, deadzone: 0.0, curve: 1.0, half: Half::Full }
    }

    #[test]
    fn resolves_by_position() {
        let layout = Layout { axes: vec![usage(PAGE_GENERIC, 0x30), usage(PAGE_GENERIC, 0x31)], buttons: 2, hats: 1 };
        assert_eq!(resolve(&Input::Axis { index: 1 }, &layout), Some(Slot::Axis(1)));
        assert_eq!(resolve(&Input::Axis { index: 2 }, &layout), None);
        assert_eq!(resolve(&Input::Button { index: 1 }, &layout), Some(Slot::Button(1)));
        assert_eq!(resolve(&Input::Hat { index: 0 }, &layout), Some(Slot::Hat(0)));
        assert_eq!(resolve(&Input::Hat { index: 1 }, &layout), None);
    }

    fn active(b: Binding, slot: Slot) -> Active {
        Active { binding: b, device: 0, slot }
    }

    #[test]
    fn pedals_drive_the_triggers() {
        let live = vec![Live { axes: vec![0.5, 1.0], buttons: vec![], hats: vec![] }];
        let gas = active(bind(Input::Axis { index: 0 }, Target::RightTrigger), Slot::Axis(0));
        // This brake pedal rests at the high end.
        let mut brake = bind(Input::Axis { index: 1 }, Target::LeftTrigger);
        brake.invert = true;
        let c = contribution(&[gas, active(brake, Slot::Axis(1))], &live);
        assert_eq!(c.right_trigger, 0.5);
        assert_eq!(c.left_trigger, 0.0);
        let pad = c.apply(Pad { right_trigger: 200, left_trigger: 10, ..Pad::default() });
        assert_eq!(pad.right_trigger, 200, "the larger trigger value wins");
        assert_eq!(pad.left_trigger, 10);
        assert_eq!(c.apply(Pad::default()).right_trigger, 128);
    }

    #[test]
    fn deadzone_and_curve() {
        let mut b = bind(Input::Axis { index: 0 }, Target::RightTrigger);
        b.deadzone = 0.1;
        assert_eq!(from_rest(0.05, &b), 0.0);
        assert!((from_rest(0.55, &b) - 0.5).abs() < 1e-6);
        assert_eq!(from_rest(1.0, &b), 1.0);
        b.curve = 2.0;
        assert!((from_rest(0.55, &b) - 0.25).abs() < 1e-6);
        // Full stick axis: deadzone around the middle.
        let mut s = bind(Input::Axis { index: 0 }, Target::LeftX);
        s.deadzone = 0.1;
        assert_eq!(centred(0.52, &s), 0.0);
        assert_eq!(centred(0.0, &s), -1.0);
        s.invert = true;
        assert_eq!(centred(0.0, &s), 1.0);
    }

    #[test]
    fn sticks_add_and_buttons_combine() {
        let live = vec![Live { axes: vec![1.0, 0.75], buttons: vec![true, false], hats: vec![Some(1)] }];
        let wheel = active(bind(Input::Axis { index: 0 }, Target::LeftX), Slot::Axis(0));
        let mut pedal = bind(Input::Axis { index: 1 }, Target::LeftY);
        pedal.half = Half::Negative;
        let a = active(bind(Input::Button { index: 0 }, Target::A), Slot::Button(0));
        let b = active(bind(Input::Button { index: 1 }, Target::B), Slot::Button(1));
        let hat = active(bind(Input::Hat { index: 0 }, Target::Dpad), Slot::Hat(0));
        let c = contribution(&[wheel, active(pedal, Slot::Axis(1)), a, b, hat], &live);
        assert_eq!(c.lx, 1.0);
        assert_eq!(c.ly, -0.75);
        assert_eq!(c.buttons, 0x1000 | 0x0001 | 0x0008);
        let pad = c.apply(Pad { lx: 20000, buttons: 0x2000, ..Pad::default() });
        assert_eq!(pad.lx, 32767, "clamped");
        assert_eq!(pad.buttons, 0x1000 | 0x2000 | 0x0001 | 0x0008);
        assert!(Contribution::default().is_neutral());
        assert_eq!(Contribution::default().apply(pad), pad);
    }

    #[test]
    fn buttons_and_pedals_hold_game_keys() {
        let live = vec![Live { axes: vec![0.9], buttons: vec![true], hats: vec![] }];
        let f13 = active(bind(Input::Button { index: 0 }, Target::KeyF13), Slot::Button(0));
        let f24 = active(bind(Input::Axis { index: 0 }, Target::KeyF24), Slot::Axis(0));
        let c = contribution(&[f13, f24], &live);
        assert_eq!(c.keys, 1 | 1 << 11);
        assert_eq!(c.buttons, 0, "keys leave the gamepad alone");
        assert_eq!(c.apply(Pad::default()), Pad::default());
        assert_eq!(Target::KeyF24.key().map(|k| GAME_KEYS[k].0), Some(0x87));
        assert_eq!(Target::A.key(), None);
        let json: Target = serde_json::from_str("\"key_f17\"").unwrap();
        assert_eq!(json, Target::KeyF17);
    }

    #[test]
    fn a_button_on_a_stick_pushes_one_way() {
        let live = vec![Live { axes: vec![], buttons: vec![true], hats: vec![] }];
        let mut b = bind(Input::Button { index: 0 }, Target::RightY);
        b.half = Half::Negative;
        assert_eq!(contribution(&[active(b.clone(), Slot::Button(0))], &live).ry, -1.0);
        b.half = Half::Full;
        assert_eq!(contribution(&[active(b, Slot::Button(0))], &live).ry, 1.0);
    }

    #[test]
    fn moved_inputs_newest_first() {
        let live = |axes: [f32; 2], button: bool, hat: Option<u8>| Live { axes: axes.to_vec(), buttons: vec![false, button], hats: vec![hat] };
        let mut m = Moves::default();
        // Brake pedal resting at the top; held button at start doesn't count.
        assert!(!m.update(&Live { buttons: vec![false, true], ..live([0.5, 1.0], false, None) }));
        assert!(!m.update(&live([0.55, 0.95], false, None)), "noise");
        assert!(m.update(&live([0.5, 0.2], false, None)));
        assert_eq!(m.recent, ["Axis 2"]);
        assert!(!m.update(&live([0.5, 0.5], false, None)), "still away");
        assert!(!m.update(&live([0.5, 1.0], false, None)), "back at rest");
        assert!(m.update(&live([0.5, 1.0], true, Some(2))));
        assert_eq!(m.recent, ["Button 2", "Hat 1", "Axis 2"]);
        assert!(m.update(&live([0.5, 0.0], true, Some(2))));
        assert_eq!(m.recent, ["Axis 2", "Button 2", "Hat 1"], "a repeat moves to the front");
        assert_eq!(device_id(0x46d, 0xc24f, 0), "046d:c24f");
        assert_eq!(device_id(0x46d, 0xc24f, 1), "046d:c24f#1");
    }

    #[test]
    fn state_lists_inputs_as_read() {
        let layout = Layout { axes: vec![usage(PAGE_SIMULATION, 0xC5), usage(PAGE_GENERIC, 0x40)], buttons: 4, hats: 2 };
        let live = Live { axes: vec![0.123_456, 1.0], buttons: vec![false, true, false, true], hats: vec![Some(2), None] };
        let mut d = StatusDevice::default();
        d.inputs(&layout, Some(&live));
        let axes: Vec<_> = d.axes.iter().map(|a| (a.name.as_str(), a.value)).collect();
        assert_eq!(axes, [("Brake", 0.123), ("Axis 2", 1.0)]);
        assert_eq!((d.buttons, d.pressed.as_slice(), d.hats.as_slice()), (4, &[2, 4][..], &[2, -1][..]));
        // Not open: inputs listed at rest.
        d.inputs(&layout, None);
        assert_eq!((d.axes[0].value, d.pressed.len(), d.hats.as_slice()), (0.0, 0, &[-1, -1][..]));

        let out = Output::from(Contribution { left_trigger: 0.333_33, lx: -0.5, buttons: 0x1000, ..Contribution::default() });
        assert_eq!((out.left_trigger, out.lx, out.buttons), (0.333, -0.5, 0x1000));
        let json = serde_json::to_value(Status { devices: vec![d], ..Status::default() }).unwrap();
        assert_eq!(json["devices"][0]["kind"], "joystick");
        assert_eq!(json["output"]["right_trigger"], 0.0);
    }
}
