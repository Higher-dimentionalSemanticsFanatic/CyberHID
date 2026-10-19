-- CyberHID controls window: bindings file, plugin state and input scan.
-- No ImGui here, so tests run it under plain LuaJIT (cet/tests).

local M = {}

M.CONFIG_FILE = "config.json"
M.STATE_FILE = "state.json"
M.UI_FILE = "ui.json"

-- How often the window reads the plugin's state and tells the plugin it is
-- open, in seconds.
M.STATE_EVERY = 0.05
M.HEARTBEAT_EVERY = 1
-- Changes are saved this long after the last one.
M.SAVE_AFTER = 0.3
M.SCAN_TIMEOUT = 10
-- The plugin rewrites state.json at least every 5 s.
M.STATE_STALE_MS = 15000

-- Gamepad inputs a binding can drive. `button` is the XInput bit.
M.TARGETS = {
  { id = "right_trigger", label = "Right trigger", hint = "Vehicle: accelerate", kind = "trigger", group = "Triggers" },
  { id = "left_trigger", label = "Left trigger", hint = "Vehicle: brake, reverse", kind = "trigger", group = "Triggers" },
  { id = "left_x", label = "Left stick X", hint = "Vehicle: steer", kind = "stick", group = "Sticks" },
  { id = "left_y", label = "Left stick Y", kind = "stick", group = "Sticks" },
  { id = "right_x", label = "Right stick X", kind = "stick", group = "Sticks" },
  { id = "right_y", label = "Right stick Y", kind = "stick", group = "Sticks" },
  { id = "a", label = "A", kind = "button", button = 0x1000, group = "Buttons" },
  { id = "b", label = "B", kind = "button", button = 0x2000, group = "Buttons" },
  { id = "x", label = "X", kind = "button", button = 0x4000, group = "Buttons" },
  { id = "y", label = "Y", kind = "button", button = 0x8000, group = "Buttons" },
  { id = "left_shoulder", label = "Left bumper", kind = "button", button = 0x0100, group = "Buttons" },
  { id = "right_shoulder", label = "Right bumper", kind = "button", button = 0x0200, group = "Buttons" },
  { id = "left_thumb", label = "Left stick click", kind = "button", button = 0x0040, group = "Buttons" },
  { id = "right_thumb", label = "Right stick click", kind = "button", button = 0x0080, group = "Buttons" },
  { id = "back", label = "View", kind = "button", button = 0x0020, group = "Buttons" },
  { id = "start", label = "Menu", kind = "button", button = 0x0010, group = "Buttons" },
  { id = "dpad_up", label = "D-pad up", kind = "button", button = 0x0001, group = "D-pad" },
  { id = "dpad_down", label = "D-pad down", kind = "button", button = 0x0002, group = "D-pad" },
  { id = "dpad_left", label = "D-pad left", kind = "button", button = 0x0004, group = "D-pad" },
  { id = "dpad_right", label = "D-pad right", kind = "button", button = 0x0008, group = "D-pad" },
  { id = "dpad", label = "D-pad (hat)", kind = "hat", group = "D-pad" },
}

M.TARGET = {}
for _, t in ipairs(M.TARGETS) do
  M.TARGET[t.id] = t
end

-- state.output field per analog target.
local OUTPUT = {
  right_trigger = "right_trigger", left_trigger = "left_trigger",
  left_x = "lx", left_y = "ly", right_x = "rx", right_y = "ry",
}

M.RANGES = { "whole", "rest_to_plus", "rest_to_minus" }

M.HAT_DIRECTIONS = { "Up", "Up-right", "Right", "Down-right", "Down", "Down-left", "Left", "Up-left" }

-- ---- helpers ---------------------------------------------------------------

local function clamp(v, lo, hi)
  if v ~= v then return lo end
  return math.max(lo, math.min(hi, v))
end
M.clamp = clamp

-- Whether XInput bit `b` is set in `mask`.
function M.has_bit(mask, b)
  return math.floor((mask or 0) / b) % 2 == 1
end

function M.hat_label(dir)
  if dir == nil or dir < 0 then return "Centred" end
  return M.HAT_DIRECTIONS[dir + 1] or "Centred"
end

-- ---- config.json -------------------------------------------------------------

function M.default_config()
  return { enabled = true, slot = 1, report_pad = true, bindings = {} }
end

-- A binding with every field set and in range; nil when it names no
-- known target or device.
function M.clean_binding(b)
  if type(b) ~= "table" or not M.TARGET[b.target] or type(b.device) ~= "string" or b.device == "" then
    return nil
  end
  local input = (b.input == "axis" or b.input == "button" or b.input == "hat") and b.input or "button"
  local range = "whole"
  for _, r in ipairs(M.RANGES) do
    if b.range == r then range = r end
  end
  return {
    target = b.target,
    device = b.device,
    device_name = type(b.device_name) == "string" and b.device_name or "",
    input = input,
    index = math.max(1, math.floor(tonumber(b.index) or 1)),
    invert = b.invert == true,
    deadzone = clamp(tonumber(b.deadzone) or 0, 0, 50),
    response = clamp(tonumber(b.response) or 1, 0.2, 5),
    range = range,
  }
end

-- config.json's text as a config table; the defaults and an error when it
-- can't be read.
function M.decode_config(text, json)
  local c = M.default_config()
  if text == nil then return c end
  local ok, v = pcall(json.decode, text)
  if not ok or type(v) ~= "table" then
    return c, "config.json is no JSON object"
  end
  if type(v.enabled) == "boolean" then c.enabled = v.enabled end
  if type(v.report_pad) == "boolean" then c.report_pad = v.report_pad end
  c.slot = clamp(math.floor((tonumber(v.slot) or 1) + 0.5), 1, 4)
  for _, b in ipairs(type(v.bindings) == "table" and v.bindings or {}) do
    local clean = M.clean_binding(b)
    if clean then table.insert(c.bindings, clean) end
  end
  return c
end

function M.encode_config(c, json)
  local bindings = {}
  for _, b in ipairs(c.bindings) do
    table.insert(bindings, M.clean_binding(b))
  end
  return json.encode({ enabled = c.enabled, slot = c.slot, report_pad = c.report_pad, bindings = bindings })
end

-- The bindings of one target, with their place in config.bindings.
function M.bindings_for(c, target)
  local out = {}
  for i, b in ipairs(c.bindings) do
    if b.target == target then table.insert(out, { index = i, binding = b }) end
  end
  return out
end

-- ---- state.json ---------------------------------------------------------------

function M.device(state, id)
  for _, d in ipairs(state and state.devices or {}) do
    if d.id == id then return d end
  end
  return nil
end

-- The binding's input as read now: 0 to 1 (axes, buttons) or a hat
-- direction; nil when the device or input isn't there.
function M.input_value(state, b)
  local d = M.device(state, b.device)
  if not d or not d.connected then return nil end
  if b.input == "axis" then
    local a = d.axes and d.axes[b.index]
    return a and a.value
  elseif b.input == "button" then
    if b.index > (d.buttons or 0) then return nil end
    for _, p in ipairs(d.pressed or {}) do
      if p == b.index then return 1 end
    end
    return 0
  else
    local h = d.hats and d.hats[b.index]
    return h
  end
end

-- Why a binding has no effect now; nil when it is active.
function M.binding_problem(state, b)
  local d = M.device(state, b.device)
  if not d then return "controller not connected" end
  if d.xinput then return "XInput gamepad: read by the game, not by CyberHID" end
  if not d.connected then return "controller not opened" end
  local count = ({ axis = #(d.axes or {}), button = d.buttons or 0, hat = #(d.hats or {}) })[b.input]
  if b.index > count then
    return string.format("controller has %d %s(s)", count, b.input)
  end
  return nil
end

function M.input_label(d, input, index)
  if input == "axis" then
    local a = d and d.axes and d.axes[index]
    if a and a.name ~= string.format("Axis %d", index) then
      return string.format("Axis %d (%s)", index, a.name)
    end
    return string.format("Axis %d", index)
  elseif input == "button" then
    return string.format("Button %d", index)
  end
  return string.format("Hat %d", index)
end

function M.binding_label(state, b)
  local d = M.device(state, b.device)
  local name = d and d.name or (b.device_name ~= "" and b.device_name or b.device)
  return name .. " · " .. M.input_label(d, b.input, b.index)
end

-- What CyberHID adds to one gamepad input now, from state.output.
function M.target_output(state, target)
  local out = state and state.output
  if not out then return 0 end
  local t = M.TARGET[target]
  if t.button then return M.has_bit(out.buttons, t.button) and 1 or 0 end
  if t.kind == "hat" then
    local any = false
    for _, id in ipairs({ "dpad_up", "dpad_down", "dpad_left", "dpad_right" }) do
      any = any or M.has_bit(out.buttons, M.TARGET[id].button)
    end
    return any and 1 or 0
  end
  return out[OUTPUT[target]] or 0
end

-- Inputs bound on a device: "axis:3" -> { "Right trigger", ... }.
function M.bound_inputs(c, device)
  local out = {}
  for _, b in ipairs(c.bindings) do
    if b.device == device then
      local key = b.input .. ":" .. b.index
      out[key] = out[key] or {}
      table.insert(out[key], M.TARGET[b.target].label)
    end
  end
  return out
end

-- ---- response (same rules as the plugin's merge, crates/core) ---------------

local function response(r)
  return clamp(r or 1, 0.2, 5)
end

-- Travel from rest, 0 to 1, after invert, deadzone and response.
function M.from_rest(v, b)
  if b.invert then v = 1 - v end
  v = clamp(v, 0, 1)
  local dz = clamp((b.deadzone or 0) / 100, 0, 0.5)
  v = v <= dz and 0 or (v - dz) / (1 - dz)
  return v ^ response(b.response)
end

-- -1 to 1 around the middle, after invert, deadzone and response.
function M.centred(v, b)
  local s = clamp(v, 0, 1) * 2 - 1
  if b.invert then s = -s end
  local dz = clamp((b.deadzone or 0) / 100, 0, 0.5)
  local m = math.abs(s)
  m = m <= dz and 0 or (m - dz) / (1 - dz)
  local sign = s < 0 and -1 or (s > 0 and 1 or 0)
  return m ^ response(b.response) * sign
end

-- What one binding sends for input value `v` (0 to 1): 0 to 1 for
-- triggers and buttons, -1 to 1 for sticks.
function M.output(b, v)
  local t = M.TARGET[b.target]
  if t.kind == "button" then
    return M.from_rest(v, b) > 0.5 and 1 or 0
  elseif t.kind == "stick" then
    if b.range == "rest_to_minus" then return -M.from_rest(v, b) end
    if b.input == "axis" and b.range == "whole" then return M.centred(v, b) end
    return M.from_rest(v, b)
  end
  return M.from_rest(v, b)
end

-- ---- scan -------------------------------------------------------------------

-- Every connected controller's inputs now, to compare against.
function M.snapshot(state)
  local base = {}
  for _, d in ipairs(state and state.devices or {}) do
    if d.connected then
      local s = { axes = {}, pressed = {}, hats = {} }
      for i, a in ipairs(d.axes or {}) do s.axes[i] = a.value end
      for _, p in ipairs(d.pressed or {}) do s.pressed[p] = true end
      for i, h in ipairs(d.hats or {}) do s.hats[i] = h end
      base[d.id] = s
    end
  end
  return base
end

-- The first input moved since `base`: an axis a third of its range, a
-- button pressed, a hat pushed. Hat targets take hats only; the others
-- axes and buttons, preferring their own kind. Controllers new since
-- `base` are added to it.
function M.detect(base, state, target)
  local kind = M.TARGET[target].kind
  local found = { axis = nil, button = nil, hat = nil }
  for _, d in ipairs(state and state.devices or {}) do
    local b = base[d.id]
    if d.connected and not b then
      base[d.id] = M.snapshot({ devices = { d } })[d.id]
    elseif d.connected then
      for i, a in ipairs(d.axes or {}) do
        local rest = b.axes[i] or a.value
        if not found.axis and math.abs(a.value - rest) > 1 / 3 then
          found.axis = { device = d.id, device_name = d.name, input = "axis", index = i, rest = rest }
        end
      end
      for _, p in ipairs(d.pressed or {}) do
        if not found.button and not b.pressed[p] then
          found.button = { device = d.id, device_name = d.name, input = "button", index = p }
        end
      end
      for i, h in ipairs(d.hats or {}) do
        if not found.hat and h >= 0 and (b.hats[i] or -1) < 0 then
          found.hat = { device = d.id, device_name = d.name, input = "hat", index = i }
        end
      end
    end
  end
  if kind == "hat" then return found.hat end
  if kind == "button" then return found.button or found.axis end
  return found.axis or found.button
end

-- A new binding for a scanned input, set up for where the input rests: a
-- pedal resting at the top end is inverted, and one bound to a stick
-- moves it from the middle to one end.
function M.binding_from(found, target)
  local b = M.clean_binding({
    target = target, device = found.device, device_name = found.device_name,
    input = found.input, index = found.index,
  })
  local rest = found.rest
  if found.input == "axis" and rest then
    b.invert = rest > 0.8
    if M.TARGET[target].kind == "stick" and (rest < 0.2 or rest > 0.8) then
      b.range = "rest_to_plus"
    end
  end
  if found.input == "button" and M.TARGET[target].kind == "stick" then
    b.range = "rest_to_plus"
  end
  return b
end

-- ---- the app ------------------------------------------------------------------

local App = {}
App.__index = App

-- `files` is io, `json` the JSON library, `now_ms` returns Unix
-- milliseconds (tests pass stand-ins).
function M.new(files, json, now_ms)
  return setmetatable({
    files = files, json = json, now_ms = now_ms,
    config = M.default_config(),
    config_error = nil, save_error = nil,
    state = nil, state_error = nil,
    dirty = false, since_change = 0,
    state_clock = 0, heartbeat_clock = 0,
    scan = nil, scan_result = nil,
    selected_device = nil, selected_target = "right_trigger",
  }, App)
end

function App:read(name)
  local f = self.files.open(name, "r")
  if not f then return nil end
  local text = f:read("*a")
  f:close()
  return text
end

function App:write(name, text)
  local f, err = self.files.open(name, "w")
  if not f then return false, err or ("can't write " .. name) end
  f:write(text)
  f:close()
  return true
end

function App:load()
  self.config, self.config_error = M.decode_config(self:read(M.CONFIG_FILE), self.json)
  self:read_state()
end

function App:read_state()
  local text = self:read(M.STATE_FILE)
  -- Missing for a moment while the plugin replaces it: keep the last state
  -- (a stopped plugin shows as stale instead).
  if not text then
    if not self.state then self.state_error = "no state.json" end
    return
  end
  local ok, v = pcall(self.json.decode, text)
  -- A read can meet a half-written file: keep the last state.
  if ok and type(v) == "table" then
    self.state, self.state_error = v, nil
  end
end

-- Mark the bindings changed; they are saved shortly after.
function App:changed()
  self.dirty = true
  self.since_change = 0
end

function App:save()
  local ok, err = self:write(M.CONFIG_FILE, M.encode_config(self.config, self.json))
  self.save_error = not ok and err or nil
  self.dirty = not ok
end

-- The plugin's state is older than it writes it.
function App:state_stale()
  return self.state ~= nil and self.now_ms() - (self.state.updated or 0) > M.STATE_STALE_MS
end

function App:opened()
  self:heartbeat()
  self:read_state()
end

function App:closed()
  if self.dirty then self:save() end
  self.scan = nil
end

function App:heartbeat()
  self:write(M.UI_FILE, self.json.encode({ open = true, time = math.floor(self.now_ms() / 1000) }))
end

-- Called every frame while the window shows.
function App:tick(dt)
  self.heartbeat_clock = self.heartbeat_clock + dt
  if self.heartbeat_clock >= M.HEARTBEAT_EVERY then
    self.heartbeat_clock = 0
    self:heartbeat()
  end
  self.state_clock = self.state_clock + dt
  if self.state_clock >= M.STATE_EVERY then
    self.state_clock = 0
    self:read_state()
    self:scan_step()
  end
  if self.dirty then
    self.since_change = self.since_change + dt
    if self.since_change >= M.SAVE_AFTER then self:save() end
  end
  if self.scan then
    self.scan.time = self.scan.time + dt
    if self.scan.time >= M.SCAN_TIMEOUT then
      self.scan = nil
      self.scan_result = "No input moved in " .. M.SCAN_TIMEOUT .. " s."
    end
  end
end

-- Wait for an input to move, then bind it to `target`; `replace` is the
-- config.bindings index to replace, nil to add.
function App:start_scan(target, replace)
  self.scan = { target = target, replace = replace, base = M.snapshot(self.state), time = 0 }
  self.scan_result = nil
end

function App:cancel_scan()
  self.scan = nil
end

function App:scan_step()
  local s = self.scan
  if not s then return end
  local found = M.detect(s.base, self.state, s.target)
  if not found then return end
  local b = M.binding_from(found, s.target)
  if s.replace and self.config.bindings[s.replace] then
    self.config.bindings[s.replace] = b
  else
    table.insert(self.config.bindings, b)
  end
  self.scan = nil
  self.scan_result = "Bound " .. M.binding_label(self.state, b) .. "."
  self.selected_device = b.device
  self:changed()
end

function App:remove_binding(index)
  table.remove(self.config.bindings, index)
  -- A running scan may point at a binding that moved.
  self.scan = nil
  self:changed()
end

M.App = App

return M
