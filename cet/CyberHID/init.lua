-- CyberHID controls window for Cyber Engine Tweaks: connected controllers
-- with their live inputs, gamepad bindings and settings. Shown while the
-- CET overlay is open. The CyberHID RED4ext plugin reads config.json from
-- this folder and writes state.json here.

local model = require("model")
local ui = require("ui")

local app = model.new(io, json, function()
  return os.time() * 1000
end)
local overlay = false

registerForEvent("onInit", function()
  app:load()
end)

registerForEvent("onOverlayOpen", function()
  overlay = true
  app:opened()
end)

registerForEvent("onOverlayClose", function()
  overlay = false
  app:closed()
end)

registerForEvent("onUpdate", function(dt)
  if overlay then app:tick(dt) end
end)

registerForEvent("onDraw", function()
  if overlay then ui.draw(app) end
end)

registerForEvent("onShutdown", function()
  app:closed()
end)

return { app = app }
