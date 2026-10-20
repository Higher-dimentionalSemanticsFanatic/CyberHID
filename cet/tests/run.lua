-- Tests for the CET controls window, run with LuaJIT (CET's Lua) from the
-- repository root: luajit cet/tests/run.lua
--
-- ImGui, IconGlyphs and the enums are stand-ins that accept only names
-- CET defines (read from CET's sol_imgui.h and IconGlyphs), so a typo
-- fails here instead of in the game.

local here = (arg and arg[0] or "cet/tests/run.lua"):match("^(.*)[/\\]") or "."
package.path = here .. "/../CyberHID/?.lua;" .. here .. "/?.lua;" .. package.path

json = require("json")

local function strict(name, t)
  return setmetatable(t, {
    __index = function(_, k) error(name .. "." .. tostring(k) .. " is not defined by CET", 2) end,
  })
end

local function enum(name, keys)
  local t, n = {}, 1
  for _, k in ipairs(keys) do
    t[k] = n
    n = n * 2
  end
  return strict(name, t)
end

ImGuiCol = enum("ImGuiCol", { "Text", "TextDisabled", "FrameBg", "Border", "PlotLines", "PlotHistogram" })
ImGuiCond = enum("ImGuiCond", { "FirstUseEver" })
ImGuiTableFlags = enum("ImGuiTableFlags", { "RowBg", "SizingStretchProp" })
ImGuiTableColumnFlags = enum("ImGuiTableColumnFlags", { "WidthFixed", "WidthStretch" })
ImGuiSelectableFlags = enum("ImGuiSelectableFlags", { "SpanAllColumns" })

local glyphs = {}
for _, k in ipairs({
  "Steering", "CarClutch", "FootPrint", "Airplane", "GaugeFull", "Dialpad", "MicrosoftXboxController", "Gamepad",
  "CheckCircle", "AlertCircle", "CloseCircle", "PowerPlugOff", "LinkVariant", "Magnify", "Delete", "Plus",
  "RadioboxMarked",
}) do
  glyphs[k] = "[" .. k .. "]"
end
IconGlyphs = strict("IconGlyphs", glyphs)

-- ---- ImGui stand-in ---------------------------------------------------------------

local frame -- calls of the current draw
local clicks = {} -- button labels to report as clicked once

local function push(kind) frame.stack[kind] = (frame.stack[kind] or 0) + 1 end
local function pop(kind)
  frame.stack[kind] = (frame.stack[kind] or 0) - 1
  assert(frame.stack[kind] >= 0, "unbalanced End/Pop for " .. kind)
end

local function text(t)
  assert(type(t) == "string", "text must be a string, got " .. type(t))
  table.insert(frame.text, t)
end

local imgui = {
  Begin = function(name) assert(type(name) == "string"); push("Begin"); return true end,
  End = function() pop("Begin") end,
  BeginChild = function(name, w, h, border) assert(type(border) == "boolean"); push("Child"); return true end,
  EndChild = function() pop("Child") end,
  BeginGroup = function() push("Group") end,
  EndGroup = function() pop("Group") end,
  BeginDisabled = function(d) assert(type(d) == "boolean"); push("Disabled") end,
  EndDisabled = function() pop("Disabled") end,
  BeginTabBar = function() push("TabBar"); return true end,
  EndTabBar = function() pop("TabBar") end,
  BeginTabItem = function(label) push("TabItem"); text(label); return true end,
  EndTabItem = function() pop("TabItem") end,
  BeginTable = function(id, cols, flags) assert(type(cols) == "number" and type(flags) == "number"); push("Table"); return true end,
  EndTable = function() pop("Table") end,
  TableSetupColumn = function(label, flags, w) assert(type(flags) == "number"); text(label) end,
  TableHeadersRow = function() end,
  TableNextRow = function() end,
  TableNextColumn = function() return true end,
  PushID = function(id) push("ID") end,
  PopID = function() pop("ID") end,
  PushStyleColor = function(idx, r, g, b, a) assert(type(a) == "number"); push("StyleColor") end,
  PopStyleColor = function() pop("StyleColor") end,
  SetNextWindowSize = function() end,
  SetNextItemWidth = function() end,
  SetWindowFontScale = function() end,
  SameLine = function() end,
  Spacing = function() end,
  Separator = function() end,
  Dummy = function(w, h) assert(w == w and h == h, "NaN size") end,
  Text = text,
  TextDisabled = text,
  TextWrapped = text,
  TextColored = function(r, g, b, a, t) assert(type(a) == "number"); text(t) end,
  SetTooltip = text,
  IsItemHovered = function() return false end,
  Button = function(label) text(label); if clicks[label] then clicks[label] = nil; return true end; return false end,
  SmallButton = function(label) text(label); return false end,
  Checkbox = function(label, v) assert(type(v) == "boolean", label); text(label); return v, false end,
  Selectable = function(label, selected) assert(type(selected) == "boolean"); text(label); return selected end,
  Combo = function(label, cur, items, n)
    assert(type(items) == "table" and n == #items and n > 0, label .. ": bad item list")
    assert(cur >= 0 and cur < n, label .. ": index out of range")
    for _, i in ipairs(items) do assert(type(i) == "string") end
    text(label .. "=" .. items[cur + 1])
    return cur, false
  end,
  SliderFloat = function(label, v, lo, hi, fmt) assert(v >= lo and v <= hi, label); text(label); return v, false end,
  ProgressBar = function(f, w, h, overlay) assert(f == f and type(overlay) == "string"); text(overlay) end,
  GetContentRegionAvail = function() return 600, 400 end,
  GetCursorScreenPos = function() return 10, 10 end,
  GetFrameHeight = function() return 20 end,
  GetTextLineHeight = function() return 14 end,
  CalcTextSize = function(t) return #t * 7, 14 end,
  GetColorU32 = function() return 0 end,
  GetWindowDrawList = function() return {} end,
}
for _, f in ipairs({ "ImDrawListAddLine", "ImDrawListAddRect", "ImDrawListAddRectFilled", "ImDrawListAddCircle",
  "ImDrawListAddCircleFilled", "ImDrawListAddText" }) do
  imgui[f] = function(dl, ...)
    for _, v in ipairs({ ... }) do assert(v == v, f .. ": NaN") end
  end
end
ImGui = strict("ImGui", imgui)

local function draw(app)
  frame = { stack = {}, text = {} }
  require("ui").draw(app)
  for kind, n in pairs(frame.stack) do assert(n == 0, kind .. " left open") end
  return table.concat(frame.text, "\n")
end

-- ---- harness --------------------------------------------------------------------

local model = require("model")
local failures, count = 0, 0

local function test(name, f)
  count = count + 1
  local ok, err = xpcall(f, debug.traceback)
  if ok then
    print("ok   " .. name)
  else
    failures = failures + 1
    print("FAIL " .. name .. "\n" .. err)
  end
end

local function eq(a, b, what)
  if a ~= b then error(string.format("%s: expected %s, got %s", what or "value", tostring(b), tostring(a)), 2) end
end

local function near(a, b, what)
  if math.abs(a - b) > 1e-6 then error(string.format("%s: expected %s, got %s", what or "value", b, a), 2) end
end

local function has(text, part)
  if not text:find(part, 1, true) then error("missing text: " .. part, 2) end
end

-- Files kept in memory.
local function files(initial)
  local store = initial or {}
  return {
    store = store,
    open = function(name, mode)
      if mode == "r" then
        if store[name] == nil then return nil end
        return { read = function() return store[name] end, close = function() end }
      end
      local parts = {}
      return {
        write = function(_, s) table.insert(parts, s) end,
        close = function() store[name] = table.concat(parts) end,
      }
    end,
  }
end

local NOW = 1760000000000

local function device(t)
  return {
    id = t.id or "046d:c24f", name = t.name or "G29 Driving Force Racing Wheel", vid = 0x46d, pid = 0xc24f,
    kind = t.kind or "wheel", kind_label = t.kind_label or "Wheel", xinput = t.xinput or false,
    connected = t.connected ~= false, axes = t.axes or {}, buttons = t.buttons or 0, pressed = t.pressed or {},
    hats = t.hats or {}, recent_inputs = t.recent_inputs or {}, bound = t.bound or false,
  }
end

local function axes(...)
  local out = {}
  for i, v in ipairs({ ... }) do out[i] = { name = i == 3 and "Brake" or ("Axis " .. i), value = v } end
  return out
end

local function state(devices, extra)
  local s = {
    plugin_version = "0.2.0", updated = NOW, hooks = { "xinput1_4.dll!XInputGetState" }, hook_errors = {},
    get_state_calls = 10, merged_reads = 0, emulated_reads = 0, config_warnings = {}, config_saved = true,
    enabled = true, slot = 1, slot_pad = false, bindings = 0, active_bindings = 0, devices = devices, missing = {},
    output = { buttons = 0, left_trigger = 0, right_trigger = 0, lx = 0, ly = 0, rx = 0, ry = 0 },
  }
  for k, v in pairs(extra or {}) do s[k] = v end
  return s
end

local function app_with(store)
  local app = model.new(files(store), json, function() return NOW end)
  app:load()
  return app
end

-- ---- tests --------------------------------------------------------------------------

test("config round trip and cleaning", function()
  local c, err = model.decode_config(nil, json)
  eq(err, nil)
  eq(c.slot, 1)
  eq(#c.bindings, 0)
  c, err = model.decode_config("not json", json)
  has(err, "config.json")
  c = model.decode_config([[{"enabled":false,"slot":9,"report_pad":false,"bindings":[
    {"target":"right_trigger","device":"046d:c24f","input":"axis","index":3,"deadzone":80,"response":9},
    {"target":"jump","device":"046d:c24f"},
    {"target":"a","input":"button","index":1}
  ]}]], json)
  eq(c.enabled, false)
  eq(c.slot, 4)
  eq(c.report_pad, false)
  eq(#c.bindings, 1, "bindings kept")
  eq(c.bindings[1].deadzone, 50)
  eq(c.bindings[1].response, 5)
  eq(c.bindings[1].range, "whole")
  local again = model.decode_config(model.encode_config(c, json), json)
  eq(again.bindings[1].index, 3)
  eq(again.bindings[1].device, "046d:c24f")
  -- No bindings: still a list for the plugin.
  has(model.encode_config(model.default_config(), json), '"bindings":[]')
end)

test("response matches the plugin's merge rules", function()
  -- Same cases as crates/core: deadzone_and_curve.
  local b = { target = "right_trigger", input = "axis", deadzone = 10, response = 1, range = "whole" }
  eq(model.from_rest(0.05, b), 0)
  near(model.from_rest(0.55, b), 0.5)
  eq(model.from_rest(1, b), 1)
  b.response = 2
  near(model.from_rest(0.55, b), 0.25)
  local s = { target = "left_x", input = "axis", deadzone = 10, response = 1, range = "whole" }
  eq(model.centred(0.52, s), 0)
  eq(model.centred(0, s), -1)
  s.invert = true
  eq(model.centred(0, s), 1)
  -- Stick ranges and buttons.
  local p = { target = "left_y", input = "axis", deadzone = 0, response = 1, range = "rest_to_minus" }
  near(model.output(p, 0.75), -0.75)
  p.range = "whole"
  near(model.output(p, 0.75), 0.5)
  eq(model.output({ target = "left_y", input = "button", deadzone = 0, response = 1, range = "whole" }, 1), 1)
  eq(model.output({ target = "a", input = "axis", deadzone = 0, response = 1, invert = true }, 0.2), 1)
  eq(model.output({ target = "a", input = "axis", deadzone = 0, response = 1 }, 0.2), 0)
end)

test("scan binds the input that moved", function()
  local s = state({ device({ axes = axes(0.5, 1.0, 1.0), buttons = 4 }) })
  local base = model.snapshot(s)
  eq(model.detect(base, s, "right_trigger"), nil, "nothing moved")
  s.devices[1].axes[1].value = 0.6
  eq(model.detect(base, s, "right_trigger"), nil, "noise")
  s.devices[1].axes[3].value = 0.3
  s.devices[1].pressed = { 2 }
  local f = model.detect(base, s, "right_trigger")
  eq(f.input, "axis", "analog targets prefer axes")
  eq(f.index, 3)
  local b = model.binding_from(f, "right_trigger")
  eq(b.invert, true, "brake rests at the top")
  eq(model.detect(base, s, "a").input, "button", "button targets prefer buttons")
  eq(model.detect(base, s, "dpad"), nil, "hat target takes hats only")
  -- A pedal on a stick moves it from the middle to one end.
  eq(model.binding_from(f, "left_y").range, "rest_to_plus")
  eq(model.binding_from({ device = "x", device_name = "", input = "axis", index = 1, rest = 0.5 }, "left_x").range, "whole")
  -- A controller connected after the scan started joins the comparison.
  table.insert(s.devices, device({ id = "044f:b679", name = "T-Rudder", axes = axes(0.5) }))
  model.detect(base, s, "left_x")
  s.devices[2].axes[1].value = 0.0
  eq(model.detect(base, s, "dpad"), nil)
  s.devices[2].hats = { 2 }
  base["044f:b679"].hats = { -1 }
  eq(model.detect(base, s, "dpad").index, 1)
end)

test("app saves after changes and scans through ticks", function()
  local s = state({ device({ axes = axes(0.5, 0.5, 1.0), buttons = 8 }) })
  local store = { ["state.json"] = json.encode(s) }
  local app = app_with(store)
  eq(app.state.plugin_version, "0.2.0")
  app:opened()
  has(store["ui.json"], '"open":true')
  app:start_scan("left_trigger")
  s.devices[1].axes[3].value = 0.1
  store["state.json"] = json.encode(s)
  app:tick(0.06)
  eq(app.scan, nil, "scan finished")
  eq(#app.config.bindings, 1)
  eq(app.config.bindings[1].target, "left_trigger")
  eq(app.config.bindings[1].invert, true)
  has(app.scan_result, "Axis 3 (Brake)")
  eq(store["config.json"], nil, "not saved yet")
  app:tick(0.2)
  app:tick(0.2)
  local saved = model.decode_config(store["config.json"], json)
  eq(saved.bindings[1].target, "left_trigger")
  -- Rescan replaces in place.
  app:start_scan("left_trigger", 1)
  s.devices[1].pressed = { 5 }
  store["state.json"] = json.encode(s)
  app:tick(0.06)
  eq(#app.config.bindings, 1)
  eq(app.config.bindings[1].input, "button")
  -- Half-written or missing state is skipped; the last state stays.
  store["state.json"] = '{"devices":['
  app:tick(0.06)
  eq(app.state.plugin_version, "0.2.0")
  store["state.json"] = nil
  app:tick(0.06)
  eq(app.state.plugin_version, "0.2.0")
  -- Timeout.
  app:start_scan("a")
  for _ = 1, 101 do app:tick(0.1) end
  eq(app.scan, nil)
  has(app.scan_result, "No input moved")
  -- Closing saves pending changes.
  app:remove_binding(1)
  app:closed()
  eq(#model.decode_config(store["config.json"], json).bindings, 0)
end)

test("axis values on the -1 to 1 scale", function()
  eq(model.axis_text(0), "-1.000")
  eq(model.axis_text(0.5), "0.000")
  eq(model.axis_text(0.4999), "0.000", "no -0.000")
  eq(model.axis_text(1), "1.000")
  eq(model.axis_text(nil), "-1.000")
end)

test("window draws every tab", function()
  local wheel = device({ axes = axes(0.25, 0.5, 1.0), buttons = 25, pressed = { 3 }, hats = { 1 }, recent_inputs = { "Button 3" } })
  wheel.axes[1].raw, wheel.axes[1].min, wheel.axes[1].max = -16384, -32768, 32767
  local pad = device({ id = "045e:028e", name = "Xbox 360 Controller", kind = "gamepad", kind_label = "Gamepad", xinput = true, connected = false })
  local s = state({ wheel, pad }, { active_bindings = 2, missing = { "T-Rudder (044f:b679)" }, wine = "9.0",
    output = { buttons = 0x1000, left_trigger = 0, right_trigger = 0.5, lx = -0.25, ly = 0, rx = 0, ry = 0 } })
  local config = { enabled = true, slot = 1, report_pad = true, bindings = {
    { target = "right_trigger", device = "046d:c24f", device_name = "G29", input = "axis", index = 3, invert = true },
    { target = "right_trigger", device = "044f:b679", device_name = "T-Rudder", input = "axis", index = 1 },
    { target = "left_x", device = "046d:c24f", input = "axis", index = 1 },
    { target = "left_y", device = "046d:c24f", input = "button", index = 4, range = "rest_to_minus" },
    { target = "a", device = "046d:c24f", input = "button", index = 3 },
    { target = "dpad", device = "046d:c24f", input = "hat", index = 1 },
    { target = "b", device = "046d:c24f", input = "button", index = 40 },
  } }
  local app = app_with({ ["state.json"] = json.encode(s), ["config.json"] = json.encode(config) })
  local out = draw(app)
  has(out, "Plugin 0.2.0 · 1 XInput hook(s)")
  has(out, "Gamepad slot 1: no real gamepad, CyberHID reports one")
  has(out, "Not connected: T-Rudder (044f:b679)")
  has(out, "[Steering]  G29 Driving Force Racing Wheel##046d:c24f")
  has(out, "Axis 3 (Brake)")
  has(out, "Value (-1 to 1)")
  has(out, "-0.500")
  has(out, "-16384")
  has(out, "Right trigger")
  has(out, "Hat 1: Up-right")
  has(out, "Bound: 3: A")
  for _, t in ipairs(model.TARGETS) do
    app.selected_target = t.id
    out = draw(app)
  end
  app.selected_target = "right_trigger"
  out = draw(app)
  has(out, "Controller=T-Rudder (not connected)")
  has(out, "(controller not connected)")
  has(out, "Input 100 %")
  has(out, "Output 0 %")
  app.selected_target = "b"
  has(draw(app), "(controller has 25 button(s))")
  app.selected_target = "left_y"
  has(draw(app), "Range=Pushes - (left, down)")
  -- Scan from the window.
  app.selected_target = "x"
  clicks["[Magnify] Scan for input"] = true
  draw(app)
  has(draw(app), "Scanning, 10 s left: move an axis or press a button on any controller.")
  app.selected_device = "045e:028e"
  has(draw(app), "XInput gamepad: the game reads it directly")
  -- Plugin missing.
  out = draw(app_with({}))
  has(out, "No state.json from the CyberHID plugin")
  -- Stale state.
  local old = app_with({ ["state.json"] = json.encode(state({}, { updated = NOW - 60000 })) })
  out = draw(old)
  has(out, "state.json last written 60 s ago")
  has(out, "No game controllers listed by hid.dll.")
end)

test("init registers CET events", function()
  local events = {}
  registerForEvent = function(name, f) events[name] = f end
  local saved_io = io
  io = files({})
  local mod = dofile(here .. "/../CyberHID/init.lua")
  io = saved_io
  for _, e in ipairs({ "onInit", "onOverlayOpen", "onOverlayClose", "onUpdate", "onDraw", "onShutdown" }) do
    assert(events[e], e)
  end
  events.onInit()
  events.onOverlayOpen()
  events.onUpdate(0.1)
  frame = { stack = {}, text = {} }
  events.onDraw()
  events.onOverlayClose()
  assert(mod.app)
end)

print(string.format("%d tests, %d failed", count, failures))
os.exit(failures == 0 and 0 or 1)
