-- Sound card detection and the install-folder check for the retro targets.
--
-- (c) Copyright 2026 Liminal HQ, Scott Morris
-- SPDX-License-Identifier: Apache-2.0 OR MIT
--
-- capabilities: env, ui

local CARDS = {
	{ id = "none", name = "No sound" },
	{ id = "speaker", name = "PC speaker" },
	{ id = "adlib", name = "AdLib music card" },
	{ id = "sbpro", name = "Sound Blaster / Pro / 16" },
	{ id = "gus", name = "Gravis UltraSound" },
}

-- Fills the sound-card page from the BLASTER variable, as the DOS sound drivers read it.
function detect(page)
	local blaster = li.env.get("BLASTER")
	local fields = page.fields
	fields["sound.cards"] = CARDS
	if blaster ~= nil then
		fields["sound.detected"] = "Sound Blaster"
		fields["sound.card"] = "sbpro"
		fields["sound.port"] = string.match(blaster, "A(%x+)") or "220"
		fields["sound.irq"] = string.match(blaster, "I(%d+)") or "5"
		fields["sound.dma"] = string.match(blaster, "D(%d+)") or "1"
	else
		fields["sound.detected"] = "No sound card"
		fields["sound.card"] = "none"
	end
end

-- Refuses to continue without an install folder.
function validate(page)
	local problems = {}
	local path = page.fields["install.path"]
	if path == nil or path == "" then
		table.insert(problems, "Choose a folder to install into.")
		li.ui.note("The game needs a folder for its files and sound drivers.")
	end
	return problems
end
