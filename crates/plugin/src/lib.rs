//! CyberHID: a RED4ext plugin that adds HID game controllers (pedals,
//! wheels, flight sticks, button boxes) to the XInput gamepad Cyberpunk
//! 2077 reads. Bindings are set in the CyberHID window of the Cyber Engine
//! Tweaks overlay, which saves them to `config.json`; the plugin reads them
//! at start and again whenever they change, and writes `state.json` with
//! the controllers and their inputs for the window. Windows only: on other
//! targets this crate builds empty.

#[cfg(windows)]
mod plugin;
