-- CyberHID controls window (ImGui through Cyber Engine Tweaks).

local model = require("model")

local ui = {}

local G = IconGlyphs or {}
local function icon(name)
  return G[name] or ""
end

local KIND_ICON = {
  wheel = "Steering", pedals = "CarClutch", rudder_pedals = "FootPrint", flight_stick = "Airplane",
  throttle = "GaugeFull", button_box = "Dialpad", gamepad = "MicrosoftXboxController", joystick = "Gamepad",
}

local OK = { 0.35, 0.85, 0.45, 1 }
local WARN = { 1, 0.75, 0.25, 1 }
local BAD = { 1, 0.42, 0.42, 1 }

local function colored(c, text)
  ImGui.TextColored(c[1], c[2], c[3], c[4], text)
end

local function wrapped(c, text)
  ImGui.PushStyleColor(ImGuiCol.Text, c[1], c[2], c[3], c[4])
  ImGui.TextWrapped(text)
  ImGui.PopStyleColor()
end

local function style(idx)
  return ImGui.GetColorU32(idx, 1)
end

local function rgba(c, a)
  return ImGui.GetColorU32(c[1], c[2], c[3], a or c[4])
end

local function percent(v)
  return string.format("%.0f %%", (v or 0) * 100)
end

local PROTON_NOTE = "Under Proton, controllers reach the game through SDL, at most 6 axes per device, unless "
  .. "PROTON_ENABLE_HIDRAW lists them. CPMX2077 sets it: Game panel › CyberHID controllers."

-- ---- drawing -------------------------------------------------------------------

-- A bar for -1 to 1, filled from the middle.
local function centred_bar(v, w, label)
  local dl = ImGui.GetWindowDrawList()
  local x, y = ImGui.GetCursorScreenPos()
  local h = ImGui.GetFrameHeight()
  local mid = x + w / 2
  ImGui.ImDrawListAddRectFilled(dl, x, y, x + w, y + h, style(ImGuiCol.FrameBg), 3)
  local to = mid + model.clamp(v or 0, -1, 1) * w / 2
  ImGui.ImDrawListAddRectFilled(dl, math.min(mid, to), y, math.max(mid, to), y + h, style(ImGuiCol.PlotHistogram))
  ImGui.ImDrawListAddLine(dl, mid, y, mid, y + h, style(ImGuiCol.Text))
  local tw = ImGui.CalcTextSize(label)
  ImGui.ImDrawListAddText(dl, x + (w - tw) / 2, y + (h - ImGui.GetTextLineHeight()) / 2, style(ImGuiCol.Text), label)
  ImGui.Dummy(w, h)
end

-- Position of two axes in a square, as a stick's travel.
local function crosshair(xv, yv, size)
  local dl = ImGui.GetWindowDrawList()
  local x, y = ImGui.GetCursorScreenPos()
  ImGui.ImDrawListAddRectFilled(dl, x, y, x + size, y + size, style(ImGuiCol.FrameBg), 4)
  ImGui.ImDrawListAddLine(dl, x + size / 2, y, x + size / 2, y + size, style(ImGuiCol.Border))
  ImGui.ImDrawListAddLine(dl, x, y + size / 2, x + size, y + size / 2, style(ImGuiCol.Border))
  ImGui.ImDrawListAddCircleFilled(dl, x + model.clamp(xv, 0, 1) * size, y + model.clamp(yv, 0, 1) * size, 5,
    style(ImGuiCol.PlotHistogram))
  ImGui.Dummy(size, size)
end

-- A hat switch: a dot towards its direction.
local function hat(dir, size)
  local dl = ImGui.GetWindowDrawList()
  local x, y = ImGui.GetCursorScreenPos()
  local cx, cy, r = x + size / 2, y + size / 2, size / 2 - 2
  ImGui.ImDrawListAddCircle(dl, cx, cy, r, style(ImGuiCol.Border), 24, 1.5)
  if dir and dir >= 0 then
    local a = dir * math.pi / 4
    ImGui.ImDrawListAddCircleFilled(dl, cx + math.sin(a) * r * 0.6, cy - math.cos(a) * r * 0.6, 5,
      style(ImGuiCol.PlotHistogram))
  else
    ImGui.ImDrawListAddCircleFilled(dl, cx, cy, 3, style(ImGuiCol.TextDisabled))
  end
  ImGui.Dummy(size, size)
end

-- Numbered buttons, lit while pressed and outlined when bound.
local function button_grid(d, bound)
  local count = d.buttons or 0
  if count == 0 then return end
  local dl = ImGui.GetWindowDrawList()
  local x0, y0 = ImGui.GetCursorScreenPos()
  local avail = ImGui.GetContentRegionAvail()
  local cw, ch, gap = 32, 22, 4
  local per_row = math.max(1, math.floor((avail + gap) / (cw + gap)))
  local pressed = {}
  for _, p in ipairs(d.pressed or {}) do pressed[p] = true end
  for n = 1, count do
    local r, c = math.floor((n - 1) / per_row), (n - 1) % per_row
    local x, y = x0 + c * (cw + gap), y0 + r * (ch + gap)
    local fill = pressed[n] and style(ImGuiCol.PlotHistogram) or style(ImGuiCol.FrameBg)
    ImGui.ImDrawListAddRectFilled(dl, x, y, x + cw, y + ch, fill, 3)
    if bound["button:" .. n] then
      ImGui.ImDrawListAddRect(dl, x, y, x + cw, y + ch, rgba(OK), 3, 0, 1.5)
    end
    local label = tostring(n)
    local tw = ImGui.CalcTextSize(label)
    ImGui.ImDrawListAddText(dl, x + (cw - tw) / 2, y + (ch - ImGui.GetTextLineHeight()) / 2, style(ImGuiCol.Text), label)
  end
  local rows = math.ceil(count / per_row)
  ImGui.Dummy(math.min(count, per_row) * (cw + gap), rows * (ch + gap))
end

-- Output against travel for the binding's deadzone and response, with
-- the input's place on it.
local function response_curve(b, travel, size)
  local dl = ImGui.GetWindowDrawList()
  local x, y = ImGui.GetCursorScreenPos()
  ImGui.ImDrawListAddRectFilled(dl, x, y, x + size, y + size, style(ImGuiCol.FrameBg), 3)
  local shape = { deadzone = b.deadzone, response = b.response }
  local px, py
  for i = 0, 32 do
    local t = i / 32
    local qx, qy = x + t * size, y + size - model.from_rest(t, shape) * size
    if px then ImGui.ImDrawListAddLine(dl, px, py, qx, qy, style(ImGuiCol.PlotLines), 1.5) end
    px, py = qx, qy
  end
  if travel then
    ImGui.ImDrawListAddCircleFilled(dl, x + travel * size, y + size - model.from_rest(travel, shape) * size, 4,
      style(ImGuiCol.PlotHistogram))
  end
  ImGui.Dummy(size, size)
end

-- ---- status ----------------------------------------------------------------------

local function slot_line(app)
  local s, c = app.state, app.config
  local slot = "Gamepad slot " .. c.slot .. ": "
  if s.slot_pad then
    return OK, slot .. "real gamepad connected, bound input added to it"
  elseif c.report_pad and (s.active_bindings or 0) > 0 then
    return OK, slot .. "no real gamepad, CyberHID reports one"
  elseif c.report_pad then
    return WARN, slot .. "no real gamepad; one is reported once a binding is active"
  end
  return WARN, slot .. "no real gamepad and Report a gamepad is off: bound input not sent"
end

local function status(app)
  local s, c = app.state, app.config
  if not s then
    wrapped(BAD, icon("AlertCircle") .. " No state.json from the CyberHID plugin in this folder: "
      .. "the RED4ext plugin is not loaded (see red4ext/logs).")
    return
  end
  if app:state_stale() then
    local age = math.floor((app.now_ms() - (s.updated or 0)) / 1000)
    wrapped(BAD, icon("AlertCircle") .. string.format(" state.json last written %d s ago: plugin not running.", age))
  end
  local hooks = #(s.hooks or {})
  if hooks == 0 then
    colored(BAD, icon("CloseCircle") .. " Plugin " .. (s.plugin_version or "?") .. ": no XInput function hooked")
  else
    colored(OK, icon("CheckCircle") .. string.format(" Plugin %s · %d XInput hook(s)", s.plugin_version or "?", hooks))
  end
  ImGui.SameLine()
  ImGui.TextDisabled(string.format("· Bindings: %d of %d active", s.active_bindings or 0, #c.bindings))
  if not c.enabled then
    colored(WARN, icon("AlertCircle") .. " Off: Settings › Enabled")
  else
    local col, line = slot_line(app)
    colored(col, line)
  end
  if s.missing and #s.missing > 0 then
    wrapped(WARN, icon("PowerPlugOff") .. " Not connected: " .. table.concat(s.missing, ", "))
  end
  if app.config_error then wrapped(BAD, app.config_error) end
  if s.config_error then wrapped(BAD, "Plugin: " .. s.config_error) end
  if app.save_error then wrapped(BAD, "config.json not saved: " .. tostring(app.save_error)) end
end

-- ---- Controllers tab ---------------------------------------------------------------

local function device_detail(app, d)
  if not d then
    ImGui.TextDisabled("Select a controller.")
    return
  end
  ImGui.SetWindowFontScale(1.5)
  ImGui.Text(icon(KIND_ICON[d.kind]) .. " " .. d.name)
  ImGui.SetWindowFontScale(1)
  ImGui.TextDisabled(string.format("%s · USB %s", d.kind_label or "", d.id))
  if d.xinput then
    ImGui.TextWrapped("XInput gamepad: the game reads it directly. CyberHID adds bound input to the gamepad in slot "
      .. app.config.slot .. ".")
    return
  end
  if not d.connected then
    wrapped(WARN, "Found by hid.dll, not opened for reading.")
    return
  end
  if d.recent_inputs and #d.recent_inputs > 0 then
    ImGui.TextDisabled("Last moved: " .. table.concat(d.recent_inputs, ", "))
  end
  local bound = model.bound_inputs(app.config, d.id)
  ImGui.Spacing()
  ImGui.Text(string.format("Axes (%d)", #d.axes))
  -- Sticks: the first two axes as the stick moves (a wheel's second axis
  -- is usually a pedal).
  local stick = d.kind == "flight_stick" or d.kind == "joystick"
  if stick and #d.axes >= 2 then
    crosshair(d.axes[1].value, d.axes[2].value, 110)
    if ImGui.IsItemHovered() then ImGui.SetTooltip("Axis 1 (left to right) and axis 2 (top to bottom)") end
    ImGui.SameLine()
  end
  ImGui.BeginGroup()
  if ImGui.BeginTable("axes", 4, ImGuiTableFlags.SizingStretchProp) then
    ImGui.TableSetupColumn("Axis", ImGuiTableColumnFlags.WidthFixed, 150)
    ImGui.TableSetupColumn("Value (-1 to 1)", ImGuiTableColumnFlags.WidthStretch)
    ImGui.TableSetupColumn("HID value", ImGuiTableColumnFlags.WidthFixed, 75)
    ImGui.TableSetupColumn("Bound to", ImGuiTableColumnFlags.WidthFixed, 130)
    ImGui.TableHeadersRow()
    for i, a in ipairs(d.axes) do
      ImGui.TableNextRow()
      ImGui.TableNextColumn()
      ImGui.Text(model.input_label(d, "axis", i))
      ImGui.TableNextColumn()
      ImGui.ProgressBar(a.value, -1, 0, model.axis_text(a.value))
      ImGui.TableNextColumn()
      if a.raw then
        ImGui.Text(string.format("%d", a.raw))
        if ImGui.IsItemHovered() and a.min and a.max then
          ImGui.SetTooltip(string.format("Value in the device's input report; logical range %d to %d", a.min, a.max))
        end
      else
        ImGui.TextDisabled("-")
      end
      ImGui.TableNextColumn()
      local to = bound["axis:" .. i]
      if to then colored(OK, table.concat(to, ", ")) else ImGui.TextDisabled("-") end
    end
    ImGui.EndTable()
  end
  ImGui.EndGroup()
  ImGui.Spacing()
  ImGui.Text(string.format("Buttons (%d)", d.buttons or 0))
  button_grid(d, bound)
  local lines = {}
  for n = 1, d.buttons or 0 do
    local to = bound["button:" .. n]
    if to then table.insert(lines, n .. ": " .. table.concat(to, ", ")) end
  end
  if #lines > 0 then ImGui.TextDisabled("Bound: " .. table.concat(lines, " · ")) end
  if d.hats and #d.hats > 0 then
    ImGui.Spacing()
    ImGui.Text(string.format("Hats (%d)", #d.hats))
    for i, h in ipairs(d.hats) do
      if i > 1 then ImGui.SameLine() end
      ImGui.BeginGroup()
      hat(h, 44)
      ImGui.TextDisabled(string.format("Hat %d: %s", i, model.hat_label(h)))
      ImGui.EndGroup()
    end
  end
end

local function controllers_tab(app)
  local s = app.state
  local devices = s and s.devices or {}
  if not model.device(s, app.selected_device) then
    app.selected_device = devices[1] and devices[1].id
  end
  local _, h = ImGui.GetContentRegionAvail()
  ImGui.BeginChild("cyberhid_devices", 290, h, true)
  if #devices == 0 then
    ImGui.TextWrapped("No game controllers listed by hid.dll.")
    if s and s.wine then
      ImGui.Spacing()
      ImGui.TextWrapped(PROTON_NOTE)
    end
  end
  for _, d in ipairs(devices) do
    local selected = app.selected_device == d.id
    if ImGui.Selectable(icon(KIND_ICON[d.kind]) .. "  " .. d.name .. "##" .. d.id, selected) ~= selected then
      app.selected_device = d.id
    end
    local marks = d.xinput and " · XInput" or (not d.connected and " · not opened" or "")
    ImGui.TextDisabled("    " .. (d.kind_label or "") .. " · " .. d.id .. marks)
    if d.bound then
      ImGui.SameLine()
      colored(OK, icon("LinkVariant"))
    end
  end
  for _, m in ipairs(s and s.missing or {}) do
    ImGui.TextDisabled(icon("PowerPlugOff") .. "  " .. m)
    ImGui.TextDisabled("    bound, not connected")
  end
  ImGui.EndChild()
  ImGui.SameLine()
  ImGui.BeginChild("cyberhid_device", 0, h, true)
  device_detail(app, model.device(s, app.selected_device))
  ImGui.EndChild()
end

-- ---- Bindings tab -----------------------------------------------------------------

-- Controllers CyberHID reads, for the binding editor.
local function readable(state)
  local out = {}
  for _, d in ipairs(state and state.devices or {}) do
    if d.connected and not d.xinput then table.insert(out, d) end
  end
  return out
end

-- Inputs of a device a target can take: hats for the d-pad hat target,
-- else axes and buttons (button targets list buttons first).
local function input_choices(d, t)
  local out = {}
  local function add(kind, n)
    for i = 1, n do table.insert(out, { input = kind, index = i, label = model.input_label(d, kind, i) }) end
  end
  local axes = d and #(d.axes or {}) or 0
  local buttons = d and d.buttons or 0
  if t.kind == "hat" then
    add("hat", d and #(d.hats or {}) or 0)
  elseif t.kind == "button" then
    add("button", buttons)
    add("axis", axes)
  else
    add("axis", axes)
    add("button", buttons)
  end
  return out
end

local RANGE_AXIS = { "Whole axis (rests in the middle)", "Rest to + (right, up)", "Rest to - (left, down)" }
local RANGE_BUTTON = { "Pushes + (right, up)", "Pushes - (left, down)" }

local function source_combos(app, b, t)
  local devices = readable(app.state)
  local names, ids, current = {}, {}, nil
  for i, d in ipairs(devices) do
    names[i], ids[i] = d.name .. " (" .. d.id .. ")", d.id
    if d.id == b.device then current = i end
  end
  if not current then
    table.insert(names, 1, (b.device_name ~= "" and b.device_name or b.device) .. " (not connected)")
    table.insert(ids, 1, b.device)
    current = 1
  end
  ImGui.SetNextItemWidth(320)
  local pick, changed = ImGui.Combo("Controller", current - 1, names, #names)
  if changed and ids[pick + 1] ~= b.device then
    local d = model.device(app.state, ids[pick + 1])
    b.device, b.device_name = d.id, d.name
    local first = input_choices(d, t)[1]
    if first then b.input, b.index = first.input, first.index end
    app:changed()
  end
  local d = model.device(app.state, b.device)
  local choices = input_choices(d, t)
  local labels, at = {}, nil
  for i, c in ipairs(choices) do
    labels[i] = c.label
    if c.input == b.input and c.index == b.index then at = i end
  end
  if not at then
    table.insert(labels, 1, model.input_label(d, b.input, b.index))
    table.insert(choices, 1, { input = b.input, index = b.index })
    at = 1
  end
  ImGui.SetNextItemWidth(320)
  local ipick, ichanged = ImGui.Combo("Input", at - 1, labels, #labels)
  if ichanged then
    local c = choices[ipick + 1]
    b.input, b.index = c.input, c.index
    app:changed()
  end
end

local function live_rows(app, b, t)
  local v = model.input_value(app.state, b)
  if b.input == "hat" then
    ImGui.Text("Input: " .. model.hat_label(v))
    return
  end
  if b.input == "axis" then
    ImGui.ProgressBar(v or 0, 320, 0, "Input " .. (v and percent(v) or "-"))
  else
    ImGui.Text("Input: " .. (v == nil and "-" or (v > 0 and "pressed" or "released")))
  end
  if v == nil then return end
  local out = model.output(b, v)
  if t.kind == "button" then
    if out > 0 then colored(OK, "Output: pressed") else ImGui.TextDisabled("Output: released") end
  elseif t.kind == "stick" then
    centred_bar(out, 320, string.format("Output %+.0f %%", out * 100))
  else
    ImGui.ProgressBar(out, 320, 0, "Output " .. percent(out))
  end
end

local function options(app, b, t)
  if b.input == "hat" then return end
  local inv, ch = ImGui.Checkbox("Invert", b.invert)
  if ch then b.invert = inv; app:changed() end
  if ImGui.IsItemHovered() then ImGui.SetTooltip("For pedals and triggers that rest at the top of their range.") end
  if t.kind == "button" then
    if b.input == "axis" then ImGui.TextDisabled("Pressed past half travel.") end
    return
  end
  if t.kind == "stick" then
    local items = b.input == "axis" and RANGE_AXIS or RANGE_BUTTON
    local at
    if b.input == "axis" then
      at = ({ whole = 0, rest_to_plus = 1, rest_to_minus = 2 })[b.range] or 0
    else
      at = b.range == "rest_to_minus" and 1 or 0
    end
    ImGui.SetNextItemWidth(320)
    local pick, rch = ImGui.Combo("Range", at, items, #items)
    if rch then
      if b.input == "axis" then
        b.range = model.RANGES[pick + 1]
      else
        b.range = pick == 1 and "rest_to_minus" or "rest_to_plus"
      end
      app:changed()
    end
  end
  if b.input ~= "axis" then return end
  ImGui.BeginGroup()
  ImGui.SetNextItemWidth(200)
  local dz, dch = ImGui.SliderFloat("Deadzone", b.deadzone, 0, 50, "%.0f %%")
  if dch then b.deadzone = dz; app:changed() end
  if ImGui.IsItemHovered() then
    ImGui.SetTooltip("Travel ignored at rest (around the middle for a whole stick axis).")
  end
  ImGui.SetNextItemWidth(200)
  local r, rch = ImGui.SliderFloat("Response", b.response, 0.5, 3, "%.1f")
  if rch then b.response = r; app:changed() end
  if ImGui.IsItemHovered() then ImGui.SetTooltip("1.0 linear; above 1 finer control near rest, below 1 quicker.") end
  if ImGui.SmallButton("Linear") then b.response = 1; app:changed() end
  ImGui.SameLine()
  if ImGui.SmallButton("Progressive") then b.response = 1.6; app:changed() end
  ImGui.SameLine()
  if ImGui.SmallButton("Strong") then b.response = 2.5; app:changed() end
  ImGui.EndGroup()
  ImGui.SameLine()
  local v = model.input_value(app.state, b)
  local travel
  if v then
    if t.kind == "stick" and b.range == "whole" then
      travel = math.abs(model.clamp(v, 0, 1) * 2 - 1)
    else
      travel = model.clamp(b.invert and 1 - v or v, 0, 1)
    end
  end
  response_curve(b, travel, 92)
end

-- One binding of the selected target; true when it was removed.
local function binding_editor(app, entry, t, n)
  local b = entry.binding
  ImGui.PushID("binding" .. entry.index)
  ImGui.Separator()
  ImGui.Text(string.format("Binding %d", n))
  local problem = model.binding_problem(app.state, b)
  if problem then
    ImGui.SameLine()
    colored(WARN, "(" .. problem .. ")")
  end
  source_combos(app, b, t)
  if ImGui.Button(icon("Magnify") .. " Scan again") then app:start_scan(t.id, entry.index) end
  ImGui.SameLine()
  if ImGui.Button(icon("Delete") .. " Remove") then
    app:remove_binding(entry.index)
    ImGui.PopID()
    return true
  end
  live_rows(app, b, t)
  options(app, b, t)
  ImGui.PopID()
  return false
end

local function target_detail(app, t)
  ImGui.SetWindowFontScale(1.3)
  ImGui.Text(t.label)
  ImGui.SetWindowFontScale(1)
  if t.hint then ImGui.TextDisabled(t.hint) end
  local out = model.target_output(app.state, t.id)
  if t.kind == "stick" then
    centred_bar(out, 320, string.format("Sent to the game %+.0f %%", out * 100))
  elseif t.kind == "trigger" then
    ImGui.ProgressBar(out, 320, 0, "Sent to the game " .. percent(out))
  elseif out > 0 then
    colored(OK, "Sent to the game: pressed")
  else
    ImGui.TextDisabled("Sent to the game: released")
  end
  if ImGui.IsItemHovered() then ImGui.SetTooltip("All bindings of this input combined, as the plugin adds them.") end
  ImGui.Spacing()
  local scanning = app.scan and app.scan.target == t.id
  if scanning then
    local what = t.kind == "hat" and "push a hat switch" or "move an axis or press a button"
    wrapped(WARN, icon("Magnify") .. string.format(" Scanning, %d s left: %s on any controller.",
      math.max(0, math.ceil(model.SCAN_TIMEOUT - app.scan.time)), what))
    if ImGui.Button("Cancel") then app:cancel_scan() end
  else
    if ImGui.Button(icon("Magnify") .. " Scan for input") then app:start_scan(t.id, nil) end
    if ImGui.IsItemHovered() then ImGui.SetTooltip("Binds the next input that moves on any controller.") end
    local devices = readable(app.state)
    local choices = devices[1] and input_choices(devices[1], t) or {}
    ImGui.SameLine()
    ImGui.BeginDisabled(choices[1] == nil)
    if ImGui.Button(icon("Plus") .. " Add") then
      local d = devices[1]
      table.insert(app.config.bindings, model.clean_binding({
        target = t.id, device = d.id, device_name = d.name, input = choices[1].input, index = choices[1].index,
      }))
      app:changed()
    end
    ImGui.EndDisabled()
    if ImGui.IsItemHovered() then ImGui.SetTooltip("Add a binding and pick its controller and input.") end
  end
  if app.scan_result and not app.scan then ImGui.TextDisabled(app.scan_result) end
  local entries = model.bindings_for(app.config, t.id)
  if #entries == 0 then
    ImGui.TextDisabled("Not bound.")
  end
  for n, entry in ipairs(entries) do
    if binding_editor(app, entry, t, n) then break end
  end
end

local function target_summary(app, t)
  local entries = model.bindings_for(app.config, t.id)
  if #entries == 0 then return nil end
  local first = entries[1].binding
  local d = model.device(app.state, first.device)
  local text = model.input_label(d, first.input, first.index)
  if #entries > 1 then text = text .. string.format(" +%d", #entries - 1) end
  return text, model.binding_problem(app.state, first) == nil
end

local function bindings_tab(app)
  local _, h = ImGui.GetContentRegionAvail()
  ImGui.BeginChild("cyberhid_targets", 370, h, true)
  local group
  if ImGui.BeginTable("targets", 3, ImGuiTableFlags.RowBg) then
    ImGui.TableSetupColumn("Gamepad", ImGuiTableColumnFlags.WidthStretch)
    ImGui.TableSetupColumn("Bound to", ImGuiTableColumnFlags.WidthFixed, 160)
    ImGui.TableSetupColumn("Out", ImGuiTableColumnFlags.WidthFixed, 34)
    for _, t in ipairs(model.TARGETS) do
      if t.group ~= group then
        group = t.group
        ImGui.TableNextRow()
        ImGui.TableNextColumn()
        ImGui.TextDisabled(group)
      end
      ImGui.TableNextRow()
      ImGui.TableNextColumn()
      local selected = app.selected_target == t.id
      if ImGui.Selectable(t.label .. "##" .. t.id, selected, ImGuiSelectableFlags.SpanAllColumns) ~= selected then
        app.selected_target = t.id
      end
      ImGui.TableNextColumn()
      local text, active = target_summary(app, t)
      if not text then
        ImGui.TextDisabled("-")
      elseif active then
        colored(OK, text)
      else
        colored(WARN, text)
      end
      ImGui.TableNextColumn()
      local out = model.target_output(app.state, t.id)
      if t.kind == "stick" and (text or out ~= 0) then
        ImGui.Text(string.format("%+.0f", out * 100))
      elseif out > 0 then
        colored(OK, t.kind == "trigger" and string.format("%.0f", out * 100) or icon("RadioboxMarked"))
      end
    end
    ImGui.EndTable()
  end
  ImGui.EndChild()
  ImGui.SameLine()
  ImGui.BeginChild("cyberhid_target", 0, h, true)
  target_detail(app, model.TARGET[app.selected_target] or model.TARGETS[1])
  ImGui.EndChild()
end

-- ---- Settings tab -----------------------------------------------------------------

local function settings_tab(app)
  local c, s = app.config, app.state
  local v, ch = ImGui.Checkbox("Enabled", c.enabled)
  if ch then c.enabled = v; app:changed() end
  ImGui.SameLine()
  ImGui.TextDisabled("Bound input is added to the gamepad.")
  local slots = { "Slot 1", "Slot 2", "Slot 3", "Slot 4" }
  ImGui.SetNextItemWidth(160)
  local pick, sch = ImGui.Combo("Gamepad slot", c.slot - 1, slots, 4)
  if sch then c.slot = pick + 1; app:changed() end
  ImGui.SameLine()
  ImGui.TextDisabled("XInput slot the game reads; slot 1 unless several gamepads are connected.")
  local r, rch = ImGui.Checkbox("Report a gamepad when none is connected", c.report_pad)
  if rch then c.report_pad = r; app:changed() end
  ImGui.TextDisabled("Off: bound input reaches the game only while a real gamepad is on the slot.")
  ImGui.Spacing()
  ImGui.Separator()
  ImGui.Text("Plugin")
  if not s then
    ImGui.TextDisabled("No state.json.")
    return
  end
  ImGui.TextDisabled(string.format("Version %s%s", s.plugin_version or "?", s.wine and (" · Wine " .. s.wine) or ""))
  -- One line per DLL: "xinput1_4.dll: XInputGetState, ...".
  local dlls, order = {}, {}
  for _, hk in ipairs(s.hooks or {}) do
    local dll, fn = hk:match("^(.-)!(.*)$")
    if dll then
      if not dlls[dll] then dlls[dll] = {}; table.insert(order, dll) end
      table.insert(dlls[dll], fn)
    end
  end
  for _, dll in ipairs(order) do ImGui.TextDisabled("Hooked in " .. dll .. ": " .. table.concat(dlls[dll], ", ")) end
  for _, e in ipairs(s.hook_errors or {}) do colored(BAD, e) end
  ImGui.TextDisabled(string.format("XInputGetState calls by the game: %d · reads with bound input: %d · reported pad: %d",
    s.get_state_calls or 0, s.merged_reads or 0, s.emulated_reads or 0))
  if (s.get_state_calls or 0) == 0 then
    colored(WARN, "The game has not read a gamepad yet.")
  end
  for _, w in ipairs(s.config_warnings or {}) do colored(WARN, "config.json: " .. w) end
  ImGui.TextDisabled("Files: config.json (bindings) and state.json (plugin) in this mod's folder.")
  if s.wine then
    ImGui.Spacing()
    ImGui.TextWrapped(PROTON_NOTE)
  end
end

-- ---- window -----------------------------------------------------------------------

function ui.draw(app)
  ImGui.SetNextWindowSize(940, 660, ImGuiCond.FirstUseEver)
  if ImGui.Begin("CyberHID") then
    status(app)
    ImGui.Spacing()
    if ImGui.BeginTabBar("cyberhid_tabs") then
      if ImGui.BeginTabItem("Controllers") then
        controllers_tab(app)
        ImGui.EndTabItem()
      end
      if ImGui.BeginTabItem("Bindings") then
        bindings_tab(app)
        ImGui.EndTabItem()
      end
      if ImGui.BeginTabItem("Settings") then
        settings_tab(app)
        ImGui.EndTabItem()
      end
      ImGui.EndTabBar()
    end
  end
  ImGui.End()
end

return ui
