local mp = require("mp")
local options = { volume = 30 }
require("mp.options").read_options(options, "lofi_volume")

local function apply()
    if mp.get_property("current-ao") ~= "avfoundation" then return end
    -- Renderer gain affects queued audio; software gain waits for the two-second buffer.
    -- Keep mpv's cubic volume curve so the saved level has the same loudness.
    local ok = mp.set_property_number("ao-volume", 100 * (options.volume / 100) ^ 3)
    if ok then mp.set_property_number("volume", 100) end
end

mp.register_script_message("lofi-volume", function(value)
    local volume = tonumber(value)
    if not volume or volume < 0 or volume > 100 then return end
    options.volume = volume
    apply()
end)
mp.register_event("audio-reconfig", apply)
