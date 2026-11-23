// Which look a target gets and how large its window is.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// The five looks `@liminal-hq/installer-renderer` draws, named as it names them.
pub const LOOKS: [&str; 5] = ["dos", "win95", "afterglow", "neutral", "win11"];

/// Picks the look for a target, as the renderer's `lookFor` does: text-mode and Windows renderers
/// have one look each, and the canvas and web renderers draw Neutral for the Neutral theme (or a
/// layout whose root says `style: "neutral"`), Windows 11 for `fizz-modern` (or `style: "win11"`)
/// and Afterglow otherwise.
pub fn look_for(renderer: &str, theme: Option<&str>, root_style: Option<&str>) -> &'static str {
	match renderer {
		"tui-retro" | "tui" => "dos",
		"win32-classic" | "win16" => "win95",
		_ if theme == Some("neutral") || root_style == Some("neutral") => "neutral",
		_ if theme == Some("fizz-modern") || root_style == Some("win11") => "win11",
		_ => "afterglow",
	}
}

/// The look a project's target is drawn in: the target's renderer and theme, and the `style` of the
/// theme's stage (else of the first page's layout), which can pick a look a theme name does not.
pub fn look_of(project: &li_engine::Project, target: &li_model::Target) -> &'static str {
	let theme = project.theme_id_for(target);
	let layout = project.stage_for(target).or_else(|| {
		project
			.flows
			.values()
			.flat_map(|f| f.steps.iter())
			.find_map(|s| s.layout.as_deref())
			.and_then(|id| project.layout_for(target, id))
	});
	let style = layout
		.and_then(|l| l.root.props.get("style"))
		.and_then(serde_json::Value::as_str);
	look_for(&target.renderer, theme, style)
}

/// The size, in CSS pixels, a look is composed at: the window opens at this size, and the renderer
/// scales the stage to whatever the window becomes. The same table as the renderer's `STAGE`.
pub fn window_size(look: &str) -> (u32, u32) {
	match look {
		"dos" => (960, 600),
		"win95" => (1024, 768),
		"win11" => (1100, 720),
		_ => (1024, 720),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn the_theme_picks_the_look() {
		assert_eq!(look_for("web", Some("afterglow"), None), "afterglow");
		assert_eq!(look_for("web", Some("fizz-modern"), None), "win11");
		assert_eq!(look_for("web", Some("neutral"), None), "neutral");
		assert_eq!(look_for("web", None, Some("win11")), "win11");
		assert_eq!(look_for("tui", Some("afterglow"), None), "dos");
		assert_eq!(look_for("win16", Some("fizz-win95"), None), "win95");
	}

	#[test]
	fn every_look_has_a_window() {
		for look in LOOKS {
			let (w, h) = window_size(look);
			assert!(w >= 800 && h >= 500, "{look}");
		}
		assert_eq!(window_size("win11"), (1100, 720));
	}
}
