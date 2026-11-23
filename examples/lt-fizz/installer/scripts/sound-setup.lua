-- Sound card detection and the free-space check for the retro targets.
--
-- (c) Copyright 2026 Liminal HQ, Scott Morris
-- SPDX-License-Identifier: Apache-2.0 OR MIT

local env = require("li.env")
local fs = require("li.fs")
local ui = require("li.ui")
local sound = require("li.retro.sound")

local REQUIRED_MB = 12

-- Fills the sound-card page from what the BLASTER variable and the card probe report.
function detect(page)
	local cards = sound.probe()
	local found = "No sound card"
	if #cards > 0 then
		found = cards[1].name
	end
	page.set("sound.detected", found)
	page.set("sound.cards", cards)
	local blaster = env.get("BLASTER")
	if blaster ~= nil then
		page.set("sound.port", string.match(blaster, "A(%x+)") or "220")
		page.set("sound.irq", string.match(blaster, "I(%d+)") or "7")
	end
end

-- Refuses to continue when the install drive cannot hold the game and its sound drivers.
function validate(page)
	local problems = {}
	local free_mb = fs.free_space(page.get("install.path")) / (1024 * 1024)
	if free_mb < REQUIRED_MB then
		table.insert(problems, string.format("Not enough disk space: %d MB free, %d MB needed.", free_mb, REQUIRED_MB))
		ui.note("Choose another drive or free some space.")
	end
	return problems
end
