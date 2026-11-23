// The Tauri shell: commands over `li-studio`, the change event and the MCP server.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod settings;
mod windows;

use std::path::PathBuf;
use std::sync::Arc;

use li_model::{Flow, Layout};
use li_studio::studio::{FlowView, HistoryView, ProjectSummary, TargetInfo};
use li_studio::{
	EngineBackend, Preview, Problem, ScriptReport, Source, Studio, StudioError, mcp::McpServer,
};
use serde::Serialize;
use tauri::{Emitter, Manager, State};

/// Where the UI listens for changes from any source.
const CHANGE_EVENT: &str = "studio://changed";

struct AppState {
	studio: Arc<Studio>,
	mcp: Option<McpServer>,
}

#[derive(Serialize)]
struct McpInfo {
	url: String,
	token: String,
}

type Res<T> = Result<T, StudioError>;

#[tauri::command]
fn open_project(state: State<AppState>, path: String) -> Res<ProjectSummary> {
	state.studio.open(&PathBuf::from(path))
}

#[tauri::command]
fn project_summary(state: State<AppState>) -> Res<ProjectSummary> {
	state.studio.project_summary()
}

#[tauri::command]
fn list_targets(state: State<AppState>) -> Res<Vec<TargetInfo>> {
	state.studio.list_targets()
}

#[tauri::command]
fn get_flow(state: State<AppState>, id: String) -> Res<FlowView> {
	state.studio.get_flow(&id)
}

#[tauri::command]
fn set_flow(state: State<AppState>, id: String, flow: Flow, label: String) -> Res<FlowView> {
	state.studio.set_flow(&id, flow, &label, Source::Ui)
}

#[tauri::command]
fn get_layout(state: State<AppState>, id: String, renderer: Option<String>) -> Res<Layout> {
	state.studio.get_layout(&id, renderer.as_deref())
}

#[tauri::command]
fn set_layout(state: State<AppState>, id: String, layout: Layout, label: String) -> Res<Layout> {
	state.studio.set_layout(&id, layout, &label, Source::Ui)
}

#[tauri::command]
fn read_script(state: State<AppState>, path: String) -> Res<String> {
	state.studio.read_script(&path)
}

#[tauri::command]
fn write_script(state: State<AppState>, path: String, source: String, label: String) -> Res<()> {
	state
		.studio
		.write_script(&path, &source, &label, Source::Ui)
}

#[tauri::command]
fn test_script(state: State<AppState>, path: String, source: Option<String>) -> Res<ScriptReport> {
	state.studio.test_script(&path, source.as_deref())
}

#[tauri::command]
fn run_preview(state: State<AppState>, target: String) -> Res<Preview> {
	state.studio.run_preview(&target)
}

#[tauri::command]
fn get_preview(state: State<AppState>) -> Res<Option<Preview>> {
	state.studio.get_preview()
}

#[tauri::command]
fn check_project(state: State<AppState>) -> Res<Vec<Problem>> {
	state.studio.check_project()
}

#[tauri::command]
fn history(state: State<AppState>) -> HistoryView {
	state.studio.history()
}

#[tauri::command]
fn undo(state: State<AppState>) -> Res<HistoryView> {
	state.studio.undo(Source::Ui)
}

#[tauri::command]
fn redo(state: State<AppState>) -> Res<HistoryView> {
	state.studio.redo(Source::Ui)
}

#[tauri::command]
fn mcp_info(state: State<AppState>) -> Option<McpInfo> {
	state.mcp.as_ref().map(|m| McpInfo {
		url: format!("http://{}/mcp", m.addr),
		token: m.token.clone(),
	})
}

pub fn run() {
	let studio = Arc::new(Studio::new(Box::new(EngineBackend)));
	let mcp = match McpServer::start(studio.clone(), 0) {
		Ok(server) => {
			println!("{}", server.banner());
			Some(server)
		}
		Err(e) => {
			eprintln!("Studio MCP server did not start: {e}");
			None
		}
	};
	let initial = std::env::args()
		.nth(1)
		.or_else(|| std::env::var("LIMINAL_STUDIO_PROJECT").ok());

	#[cfg_attr(any(not(debug_assertions), windows), allow(unused_mut))]
	let mut builder = tauri::Builder::default();
	// The debug-only bridge that lets an automation client drive the running app.
	#[cfg(all(debug_assertions, not(windows)))]
	{
		builder = builder.plugin(tauri_plugin_mcp_bridge::init());
	}
	builder
		.plugin(tauri_plugin_dialog::init())
		.manage(AppState {
			studio: studio.clone(),
			mcp,
		})
		.setup(move |app| {
			let handle = app.handle().clone();
			studio.on_change(move |event| {
				let _ = handle.emit(CHANGE_EVENT, event);
			});
			if let Some(path) = initial
				&& let Err(e) = studio.open(&PathBuf::from(&path))
			{
				eprintln!("could not open {path}: {e}");
			}
			if let Some(window) = app.get_webview_window("main") {
				let _ = window.set_focus();
			}
			Ok(())
		})
		.invoke_handler(tauri::generate_handler![
			open_project,
			project_summary,
			list_targets,
			get_flow,
			set_flow,
			get_layout,
			set_layout,
			read_script,
			write_script,
			test_script,
			run_preview,
			get_preview,
			check_project,
			history,
			undo,
			redo,
			mcp_info,
			settings::read_settings,
			settings::write_settings,
			windows::open_window,
		])
		.run(tauri::generate_context!())
		.expect("error while running the Studio");
}
