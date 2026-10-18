//! The bindings file, `config.json`, written by the controls window and
//! read by the plugin whenever it changes.
//!
//! ```json
//! { "enabled": true, "slot": 1, "report_pad": true,
//!   "bindings": [ { "target": "right_trigger", "device": "046d:c24f",
//!                   "device_name": "G29 Driving Force Racing Wheel",
//!                   "input": "axis", "index": 3, "invert": true,
//!                   "deadzone": 5, "response": 1.6, "range": "whole" } ] }
//! ```
//!
//! Inputs are numbered from 1, as the window shows them. A binding that
//! can't be read is skipped and reported; the others still apply.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Binding, Half, Input, Target};

pub const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub enabled: bool,
    /// Gamepad slot 1 to 4.
    pub slot: u32,
    /// Answer the game with a gamepad on the slot when no real one is
    /// connected there.
    pub report_pad: bool,
    pub bindings: Vec<Bound>,
}

impl Default for Config {
    fn default() -> Self {
        Config { enabled: true, slot: 1, report_pad: true, bindings: Vec::new() }
    }
}

/// One binding as the file stores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bound {
    pub target: Target,
    /// [`crate::device_id`].
    pub device: String,
    /// For the window's "not connected" line.
    pub device_name: String,
    pub input: Source,
    /// From 1.
    pub index: u32,
    pub invert: bool,
    /// Percent, 0 to 50.
    pub deadzone: f32,
    /// Exponent: 1 linear, above 1 finer near rest.
    pub response: f32,
    pub range: Range,
}

impl Default for Bound {
    fn default() -> Self {
        Bound {
            target: Target::A,
            device: String::new(),
            device_name: String::new(),
            input: Source::Button,
            index: 1,
            invert: false,
            deadzone: 0.0,
            response: 1.0,
            range: Range::Whole,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Axis,
    Button,
    Hat,
}

/// Which part of a stick axis the input drives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Range {
    Whole,
    RestToPlus,
    RestToMinus,
}

impl Bound {
    /// The binding the merge rules use.
    pub fn binding(&self) -> Binding {
        let index = self.index.max(1) - 1;
        let input = match self.input {
            Source::Axis => Input::Axis { index },
            Source::Button => Input::Button { index },
            Source::Hat => Input::Hat { index },
        };
        let half = match (self.range, self.input) {
            (Range::RestToMinus, _) => Half::Negative,
            (Range::RestToPlus, _) | (Range::Whole, Source::Button) => Half::Positive,
            _ => Half::Full,
        };
        let deadzone = if self.deadzone.is_finite() { self.deadzone.clamp(0.0, 50.0) / 100.0 } else { 0.0 };
        Binding { device: self.device.clone(), input, target: self.target, invert: self.invert, deadzone, curve: self.response, half }
    }
}

impl Config {
    /// Read `config.json`. Fails only when the file is no JSON object;
    /// bindings that don't fit are skipped and listed in the second value.
    pub fn parse(text: &str) -> Result<(Config, Vec<String>), String> {
        let v: Value = serde_json::from_str(text).map_err(|e| format!("{CONFIG_FILE}: {e}"))?;
        let Value::Object(o) = v else { return Err(format!("{CONFIG_FILE}: not a JSON object")) };
        let d = Config::default();
        let mut c = Config {
            enabled: o.get("enabled").and_then(Value::as_bool).unwrap_or(d.enabled),
            slot: o.get("slot").and_then(Value::as_f64).map_or(d.slot, |s| (s.round() as i64).clamp(1, 4) as u32),
            report_pad: o.get("report_pad").and_then(Value::as_bool).unwrap_or(d.report_pad),
            bindings: Vec::new(),
        };
        let mut warnings = Vec::new();
        let list = match o.get("bindings") {
            Some(Value::Array(a)) => a.as_slice(),
            // An empty Lua table may be written as an object.
            Some(Value::Object(m)) if m.is_empty() => &[],
            None => &[],
            Some(_) => {
                warnings.push("bindings: not a list".into());
                &[]
            }
        };
        for (i, b) in list.iter().enumerate() {
            match serde_json::from_value::<Bound>(b.clone()) {
                Ok(b) if b.device.is_empty() => warnings.push(format!("binding {}: no device", i + 1)),
                Ok(b) => c.bindings.push(b),
                Err(e) => warnings.push(format!("binding {}: {e}", i + 1)),
            }
        }
        Ok((c, warnings))
    }

    pub fn bindings(&self) -> Vec<Binding> {
        self.bindings.iter().map(Bound::binding).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bindings_and_skips_bad_ones() {
        let text = r#"{ "enabled": false, "slot": 7, "bindings": [
            { "target": "right_trigger", "device": "046d:c24f", "device_name": "G29", "input": "axis",
              "index": 3, "invert": true, "deadzone": 5, "response": 1.6 },
            { "target": "left_y", "device": "046d:c24f", "input": "button", "index": 2 },
            { "target": "left_x", "device": "046d:c24f", "input": "axis", "index": 1, "range": "rest_to_minus" },
            { "target": "jump", "device": "046d:c24f" },
            { "target": "a", "input": "button", "index": 1 }
        ] }"#;
        let (c, warnings) = Config::parse(text).unwrap();
        assert!(!c.enabled);
        assert_eq!(c.slot, 4, "clamped");
        assert!(c.report_pad, "default");
        assert_eq!(c.bindings.len(), 3);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        assert!(warnings[0].starts_with("binding 4:"), "{warnings:?}");
        assert_eq!(warnings[1], "binding 5: no device");

        let b = c.bindings();
        assert_eq!(b[0].device, "046d:c24f");
        assert_eq!(b[0].input, Input::Axis { index: 2 });
        assert_eq!(b[0].target, Target::RightTrigger);
        assert!(b[0].invert);
        assert!((b[0].deadzone - 0.05).abs() < 1e-6);
        assert_eq!(b[0].curve, 1.6);
        assert_eq!(b[1].input, Input::Button { index: 1 });
        assert_eq!(b[1].half, Half::Positive, "a button drives a stick from rest");
        assert_eq!(b[2].half, Half::Negative);
    }

    #[test]
    fn tolerates_lua_shapes_and_rejects_non_objects() {
        let (c, w) = Config::parse(r#"{"bindings":{},"slot":2.0}"#).unwrap();
        assert!(c.bindings.is_empty() && w.is_empty());
        assert_eq!(c.slot, 2);
        assert!(Config::parse("[]").is_err());
        assert!(Config::parse("{").is_err());
        assert_eq!(Config::parse("{}").unwrap().0, Config::default());
    }
}
