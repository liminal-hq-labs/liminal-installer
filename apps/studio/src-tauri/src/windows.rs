// The Studio's child windows: About, Preferences and confirmation questions.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_studio::{ErrorKind, StudioError};
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_window_state::{Builder as WindowStateBuilder, StateFlags};

/// What a kind of window looks like. Every one loads the Studio page with `?window=<kind>`.
#[derive(Debug, PartialEq)]
pub struct Spec {
	/// The window's label; a kind that may only be open once has a fixed one.
	pub label: String,
	pub title: &'static str,
	pub size: (f64, f64),
	/// The smallest the window may be resized to.
	pub min_size: (f64, f64),
	/// Blocks the window it belongs to until it closes.
	pub modal: bool,
}

/// The spec for a kind, or None for one the Studio does not know. `id` tells apart windows of a kind that can repeat.
pub fn spec(kind: &str, id: Option<&str>) -> Option<Spec> {
	match kind {
		"about" => Some(Spec {
			label: "about".into(),
			title: "About Liminal Installer Studio",
			size: (440.0, 300.0),
			min_size: (440.0, 300.0),
			modal: true,
		}),
		"preferences" => Some(Spec {
			label: "preferences".into(),
			title: "Preferences",
			size: (820.0, 600.0),
			min_size: (420.0, 360.0),
			modal: false,
		}),
		"shortcuts" => Some(Spec {
			label: "shortcuts".into(),
			title: "Keyboard Shortcuts",
			size: (460.0, 520.0),
			min_size: (360.0, 320.0),
			modal: false,
		}),
		"agent" => Some(Spec {
			label: "agent".into(),
			title: "Connect an Agent",
			size: (560.0, 560.0),
			min_size: (420.0, 380.0),
			modal: false,
		}),
		"new-project" => Some(Spec {
			label: "new-project".into(),
			title: "New Project",
			size: (500.0, 420.0),
			min_size: (500.0, 420.0),
			modal: true,
		}),
		"confirm" => {
			let id = id.filter(|s| {
				!s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
			})?;
			Some(Spec {
				label: format!("confirm-{id}"),
				title: "Liminal Installer Studio",
				size: (460.0, 220.0),
				min_size: (460.0, 220.0),
				modal: true,
			})
		}
		_ => None,
	}
}

/// The top-left that centres a child of `child` size over a parent at `origin` with `parent` size.
pub fn centred_on(origin: (f64, f64), parent: (f64, f64), child: (f64, f64)) -> (f64, f64) {
	(
		origin.0 + (parent.0 - child.0) / 2.0,
		origin.1 + (parent.1 - child.1) / 2.0,
	)
}

/// Whether the window labelled `label` has its size remembered: the main window and Preferences. About and
/// confirmations open at a fixed size and cannot be resized.
pub fn remembers_size(label: &str) -> bool {
	matches!(label, "main" | "preferences")
}

/// Keeps each remembered window's size in `.window-state.json` in the app's config directory, restores it when the
/// window opens and writes it when the app exits.
pub fn window_state_plugin<R: Runtime>() -> TauriPlugin<R> {
	WindowStateBuilder::new()
		.with_state_flags(StateFlags::SIZE)
		.with_filter(remembers_size)
		.build()
}

/// A failure of the window system, not of the request.
fn internal(e: impl ToString) -> StudioError {
	StudioError::new(ErrorKind::Internal, e)
}

/// Opens a child window of the one that asked, or focuses it when a window with that label is open.
/// `query` is appended to the page's query string, already encoded.
#[tauri::command]
pub fn open_window(
	app: AppHandle,
	parent: tauri::WebviewWindow,
	kind: String,
	id: Option<String>,
	query: Option<String>,
) -> Result<(), StudioError> {
	let spec = spec(&kind, id.as_deref()).ok_or_else(|| {
		StudioError::new(ErrorKind::InvalidInput, format!("unknown window `{kind}`"))
	})?;
	if let Some(existing) = app.get_webview_window(&spec.label) {
		return existing.set_focus().map_err(internal);
	}
	let url = match query.as_deref() {
		Some(q) if !q.is_empty() => format!("index.html?window={kind}&{q}"),
		_ => format!("index.html?window={kind}"),
	};
	let scale = parent.scale_factor().map_err(internal)?;
	let origin = parent
		.inner_position()
		.map_err(internal)?
		.to_logical::<f64>(scale);
	let size = parent
		.inner_size()
		.map_err(internal)?
		.to_logical::<f64>(scale);
	let (x, y) = centred_on((origin.x, origin.y), (size.width, size.height), spec.size);

	let window = WebviewWindowBuilder::new(&app, &spec.label, WebviewUrl::App(url.into()))
		.title(spec.title)
		.inner_size(spec.size.0, spec.size.1)
		.position(x, y)
		.resizable(!spec.modal)
		.min_inner_size(spec.min_size.0, spec.min_size.1)
		.minimizable(false)
		.maximizable(false)
		.decorations(false)
		.transparent(true)
		.shadow(true)
		.parent(&parent)
		.map_err(internal)?
		.build()
		.map_err(internal)?;

	if spec.modal {
		// A platform that cannot disable a window ignores this, and the child still stays on top of its parent.
		let _ = parent.set_enabled(false);
		let parent = parent.clone();
		window.on_window_event(move |event| {
			if matches!(event, WindowEvent::Destroyed) {
				let _ = parent.set_enabled(true);
				let _ = parent.set_focus();
			}
		});
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn known_kinds_have_specs() {
		assert_eq!(spec("about", None).unwrap().label, "about");
		assert!(spec("preferences", None).is_some_and(|s| !s.modal));
		assert!(spec("about", None).is_some_and(|s| s.modal));
	}

	#[test]
	fn the_shortcut_agent_and_new_project_windows_are_known() {
		assert!(spec("shortcuts", None).is_some_and(|s| !s.modal));
		assert!(spec("agent", None).is_some_and(|s| !s.modal));
		assert!(spec("new-project", None).is_some_and(|s| s.modal));
	}

	#[test]
	fn a_confirmation_needs_a_clean_id_and_is_labelled_by_it() {
		assert_eq!(spec("confirm", Some("7")).unwrap().label, "confirm-7");
		assert!(spec("confirm", None).is_none());
		assert!(spec("confirm", Some("")).is_none());
		assert!(spec("confirm", Some("a/b")).is_none());
	}

	#[test]
	fn unknown_kinds_are_refused() {
		assert!(spec("shell", None).is_none());
	}

	#[test]
	fn only_the_main_and_preferences_windows_remember_their_size() {
		assert!(remembers_size("main"));
		assert!(remembers_size("preferences"));
		assert!(!remembers_size("about"));
		assert!(!remembers_size("confirm-7"));
	}

	#[test]
	fn centres_over_the_parent() {
		assert_eq!(
			centred_on((100.0, 50.0), (1000.0, 600.0), (400.0, 200.0)),
			(400.0, 250.0)
		);
	}
}
