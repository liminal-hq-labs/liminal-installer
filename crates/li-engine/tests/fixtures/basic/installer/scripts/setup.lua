-- Fixture hooks.
--
-- (c) Copyright 2026 Liminal HQ, Scott Morris
-- SPDX-License-Identifier: Apache-2.0 OR MIT
-- capabilities: env, plan, fs.write

function detect(page)
	page.fields.detected_os = li.env.get("os")
end

function validate(page)
	if page.fields.name == "bad" then
		return { "the name `bad` is not allowed" }
	end
end

function on_commit(plan)
	plan:add({ op = "write_file", path = "{install}/INSTALLED.txt", contents = "installed by script" })
end
