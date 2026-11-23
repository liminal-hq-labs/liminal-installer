// The Tauri shell: commands over `li-studio`, the change event and the MCP server.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

mod agent;
#[cfg(test)]
mod api_contract;
mod settings;
mod tools;
mod windows;

use std::path::PathBuf;
use std::sync::Arc;

use agent::Agent;
use li_model::{Flow, Layout};
use li_studio::mcp::McpInfo;
use li_studio::studio::{FlowView, HistoryView, ProjectSummary, TargetInfo};
use li_studio::{
	EngineBackend, ErrorKind, Preview, Problem, ScriptDiagnostic, ScriptReport, Source, Studio,
	StudioError, scaffold,
};
use serde::Serialize;
use tauri::{Emitter, Manager, State};

/// Where the UI listens for changes from any source.
const CHANGE_EVENT: &str = "studio://changed";

struct AppState {
	studio: Arc<Studio>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
	version: String,
	settings_file: String,
	log_dir: String,
}

type Res<T> = Result<T, StudioError>;

/// A failure of the shell itself, such as a path Tauri cannot resolve.
fn internal(e: impl ToString) -> StudioError {
	StudioError::new(ErrorKind::Internal, e)
}

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
fn test_script(
	state: State<AppState>,
	path: String,
	source: Option<String>,
	target: Option<String>,
) -> Res<ScriptReport> {
	state
		.studio
		.test_script(&path, source.as_deref(), target.as_deref())
}

#[tauri::command]
fn check_script_syntax(state: State<AppState>, source: String) -> Vec<ScriptDiagnostic> {
	state.studio.check_script_syntax(&source)
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

/// Where the agent server listens, or nothing while it is off. The token is a separate request.
#[tauri::command]
fn mcp_info(agent: State<Agent>) -> Option<McpInfo> {
	agent.info()
}

/// Why the agent server is not running although it is switched on, such as a taken port.
#[tauri::command]
fn mcp_error(agent: State<Agent>) -> Option<String> {
	agent.error()
}

/// The token, shown only when a person asks for it. It is never logged.
#[tauri::command]
fn mcp_token(agent: State<Agent>) -> Option<String> {
	agent.token()
}

#[tauri::command]
fn mcp_regenerate_token(agent: State<Agent>) -> Res<String> {
	agent.regenerate_token()
}

/// Writes a new project's files into `folder` with the scaffold `liminst init` uses. The folder is made when it
/// is missing and must not already hold a project. Returns the folder, for the window to open.
#[tauri::command]
fn new_project(folder: String, name: String, id: String, publisher: String) -> Res<String> {
	let opts = scaffold::InitOptions {
		id,
		name,
		publisher,
		force: false,
	};
	scaffold::validate(&opts)?;
	scaffold::init(&PathBuf::from(&folder), &opts)?;
	Ok(folder)
}

#[tauri::command]
fn app_info(app: tauri::AppHandle) -> Res<AppInfo> {
	let log_dir = app.path().app_log_dir().map_err(internal)?;
	Ok(AppInfo {
		version: app.package_info().version.to_string(),
		settings_file: settings::file(&app)
			.map_err(internal)?
			.to_string_lossy()
			.into_owned(),
		log_dir: log_dir.to_string_lossy().into_owned(),
	})
}

/// Shows the settings file or the log folder in the file manager. Only these two places can be asked for.
#[tauri::command]
fn reveal(app: tauri::AppHandle, what: String) -> Res<()> {
	let path = match what.as_str() {
		"settings" => {
			let file = settings::file(&app).map_err(internal)?;
			if let Some(dir) = file.parent() {
				std::fs::create_dir_all(dir).map_err(|e| StudioError::io(dir, e))?;
			}
			file
		}
		"logs" => {
			let dir = app.path().app_log_dir().map_err(internal)?;
			std::fs::create_dir_all(&dir).map_err(|e| StudioError::io(&dir, e))?;
			dir
		}
		other => {
			return Err(StudioError::new(
				ErrorKind::InvalidInput,
				format!("cannot show `{other}`"),
			));
		}
	};
	tools::reveal(&path).map_err(|e| StudioError::new(ErrorKind::Io, e))
}

pub fn run() {
	let studio = Arc::new(Studio::new(Box::new(EngineBackend)));
	let agent = Agent::new(studio.clone());
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
		.plugin(
			tauri_plugin_log::Builder::new()
				.level(if cfg!(debug_assertions) {
					tauri_plugin_log::log::LevelFilter::Trace
				} else {
					tauri_plugin_log::log::LevelFilter::Info
				})
				.build(),
		)
		.plugin(tauri_plugin_dialog::init())
		.plugin(tauri_plugin_system_appearance::init())
		.plugin(windows::window_state_plugin())
		.manage(AppState {
			studio: studio.clone(),
		})
		.manage(agent)
		.setup(move |app| {
			let handle = app.handle().clone();
			settings::migrate_legacy_for(&handle);
			// The agent server follows the saved settings; its address is logged, its token never is.
			let saved = settings::file(&handle)
				.map(|path| settings::load(&path))
				.unwrap_or_default();
			let agent = app.state::<Agent>();
			match agent.apply(saved.mcp_enabled, saved.mcp_port) {
				Ok(()) => {
					if let Some(line) = agent.banner() {
						println!("{line}");
					}
				}
				// The agent card and Preferences show the reason too.
				Err(e) => eprintln!("Studio MCP server did not start: {e}"),
			}
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
			new_project,
			project_summary,
			list_targets,
			get_flow,
			set_flow,
			get_layout,
			set_layout,
			read_script,
			write_script,
			test_script,
			check_script_syntax,
			run_preview,
			get_preview,
			check_project,
			history,
			undo,
			redo,
			mcp_info,
			mcp_error,
			mcp_token,
			mcp_regenerate_token,
			app_info,
			reveal,
			settings::read_settings,
			settings::write_settings,
			settings::reset_settings,
			windows::open_window,
		])
		.run(tauri::generate_context!())
		.expect("error while running the Studio");
}
