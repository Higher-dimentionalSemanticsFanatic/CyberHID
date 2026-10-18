//! What kind of controller a device is, for the controls window's icon and
//! label: read from the HID usages it reports, then from its name.

use serde::Serialize;

use crate::{PAGE_GENERIC, PAGE_SIMULATION, usage};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Wheel,
    Pedals,
    RudderPedals,
    FlightStick,
    Throttle,
    ButtonBox,
    Gamepad,
    #[default]
    Joystick,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Wheel => "Wheel",
            Kind::Pedals => "Pedals",
            Kind::RudderPedals => "Rudder pedals",
            Kind::FlightStick => "Flight stick",
            Kind::Throttle => "Throttle",
            Kind::ButtonBox => "Button box",
            Kind::Gamepad => "Gamepad",
            Kind::Joystick => "Joystick",
        }
    }
}

const STEERING: u32 = usage(PAGE_SIMULATION, 0xC8);
const RUDDER: u32 = usage(PAGE_SIMULATION, 0xBA);
const THROTTLE: u32 = usage(PAGE_SIMULATION, 0xBB);
const PEDALS: [u32; 3] = [usage(PAGE_SIMULATION, 0xC4), usage(PAGE_SIMULATION, 0xC5), usage(PAGE_SIMULATION, 0xC6)];
const GAMEPAD: u32 = usage(PAGE_GENERIC, 0x05);
const SIM_AUTOMOBILE: u32 = usage(PAGE_SIMULATION, 0x02);
const SIM_FLIGHT: u32 = usage(PAGE_SIMULATION, 0x01);

/// Name words per kind, checked in order (lowercase). Pedals come before
/// wheels: brands such as "Moza Racing" name their pedals too.
const NAMES: [(&[&str], Kind); 6] = [
    (&["rudder"], Kind::RudderPedals),
    (&["pedal"], Kind::Pedals),
    (&["wheel", "racing", "driving", "steering"], Kind::Wheel),
    (&["throttle", "quadrant"], Kind::Throttle),
    (&["button box", "buttonbox", "panel"], Kind::ButtonBox),
    (&["hotas", "stick", "joystick", "flight", "yoke"], Kind::FlightStick),
];

/// `top` is the top-level collection usage, `axes` the axis usages read.
/// First match wins: XInput or a gamepad collection, simulation axes, name
/// words, a simulation collection, inputs.
pub fn classify(xinput: bool, top: u32, axes: &[u32], buttons: usize, name: &str) -> Kind {
    if xinput || top == GAMEPAD {
        return Kind::Gamepad;
    }
    if axes.contains(&STEERING) {
        return Kind::Wheel;
    }
    if axes.contains(&RUDDER) {
        return Kind::RudderPedals;
    }
    if axes.iter().any(|a| PEDALS.contains(a)) {
        return Kind::Pedals;
    }
    if axes.contains(&THROTTLE) {
        return Kind::Throttle;
    }
    let name = name.to_lowercase();
    if let Some((_, k)) = NAMES.iter().find(|(words, _)| words.iter().any(|w| name.contains(w))) {
        return *k;
    }
    match top {
        SIM_AUTOMOBILE => Kind::Wheel,
        SIM_FLIGHT => Kind::FlightStick,
        _ if axes.is_empty() && buttons > 0 => Kind::ButtonBox,
        _ => Kind::Joystick,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JOYSTICK: u32 = usage(PAGE_GENERIC, 0x04);
    const X: u32 = usage(PAGE_GENERIC, 0x30);

    #[test]
    fn usages_then_names() {
        assert_eq!(classify(true, JOYSTICK, &[X], 10, "Wireless Controller"), Kind::Gamepad);
        assert_eq!(classify(false, JOYSTICK, &[X, PEDALS[1]], 0, "G29"), Kind::Pedals, "a brake axis");
        assert_eq!(classify(false, JOYSTICK, &[STEERING, PEDALS[0]], 0, ""), Kind::Wheel, "steering before pedals");
        // SDL under Proton reports generic axes only: the name decides.
        assert_eq!(classify(false, JOYSTICK, &[X], 25, "Logitech G29 Driving Force Racing Wheel"), Kind::Wheel);
        assert_eq!(classify(false, JOYSTICK, &[X], 0, "Thrustmaster T-Rudder"), Kind::RudderPedals);
        assert_eq!(classify(false, JOYSTICK, &[X], 0, "MOZA Racing SR-P Pedals"), Kind::Pedals);
        assert_eq!(classify(false, JOYSTICK, &[X], 32, "VKB Gladiator NXT EVO Space Combat Edition Stick"), Kind::FlightStick);
        assert_eq!(classify(false, JOYSTICK, &[X], 14, "Logitech X56 Throttle"), Kind::Throttle);
        assert_eq!(classify(false, SIM_AUTOMOBILE, &[X], 0, "Sim device"), Kind::Wheel);
        assert_eq!(classify(false, JOYSTICK, &[], 32, "Leo Bodnar BU0836"), Kind::ButtonBox);
        assert_eq!(classify(false, JOYSTICK, &[X], 4, "Generic USB device"), Kind::Joystick);
        assert_eq!(Kind::RudderPedals.label(), "Rudder pedals");
    }
}
