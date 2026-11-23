// The Studio's MCP server: JSON-RPC over HTTP on 127.0.0.1, guarded by a token that can be replaced.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::net::{SocketAddr, TcpListener};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;

use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use li_model::{Flow, Layout, Node};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::error::StudioError;
use crate::history::Source;
use crate::studio::Studio;

const PROTOCOL_VERSION: &str = "2025-03-26";

fn schema(props: Value, required: &[&str]) -> Value {
	json!({"type": "object", "properties": props, "required": required})
}

/// The tools of the spec's Table 8, with their input schemas.
pub fn tool_list() -> Value {
	let id = json!({"type": "string"});
	let label = json!({"type": "string", "description": "Name shown in the undo history."});
	json!([
		{"name": "get_project", "description": "Read the project: product, targets, components, themes, flows, layouts and scripts.", "inputSchema": schema(json!({}), &[])},
		{"name": "list_targets", "description": "List the project's targets (variations) with theme, outputs and the components each gets.", "inputSchema": schema(json!({}), &[])},
		{"name": "get_flow", "description": "Read a flow graph and its validation problems.", "inputSchema": schema(json!({"id": id}), &["id"])},
		{"name": "update_flow", "description": "Replace a flow (`flow`) or patch it with a JSON merge patch (`patch`). Returns the flow and its validation problems.", "inputSchema": schema(json!({"id": id, "flow": {"type": "object"}, "patch": {"type": "object"}, "label": label}), &["id"])},
		{"name": "get_layout", "description": "Read a layout, optionally resolved for one renderer (overrides applied).", "inputSchema": schema(json!({"id": id, "renderer": {"type": "string"}}), &["id"])},
		{"name": "update_layout", "description": "Replace a layout (`layout`), patch it (`patch`), or replace the node at `path` (child indexes) with `node`. With `renderer`, `node` becomes that renderer's override of the node; with `renderer` and no `node`, the override is removed.", "inputSchema": schema(json!({"id": id, "layout": {"type": "object"}, "patch": {"type": "object"}, "path": {"type": "array", "items": {"type": "integer"}}, "renderer": {"type": "string"}, "node": {"type": "object"}, "label": label}), &["id"])},
		{"name": "read_script", "description": "Read a Lua script by its path under installer/, such as scripts/sound-setup.lua.", "inputSchema": schema(json!({"path": {"type": "string"}}), &["path"])},
		{"name": "write_script", "description": "Create or replace a Lua script.", "inputSchema": schema(json!({"path": {"type": "string"}, "source": {"type": "string"}, "label": label}), &["path", "source"])},
		{"name": "test_script", "description": "Run a script's tests; `source` tests unsaved text instead of the file, and `target` (os-arch:renderer) runs them against that target's machine and components.", "inputSchema": schema(json!({"path": {"type": "string"}, "source": {"type": "string"}, "target": {"type": "string"}}), &["path"])},
		{"name": "check_script_syntax", "description": "Check Lua source without running it: syntax errors with line and column, and warnings for capabilities used but not declared and for features the retro Lua subset lacks.", "inputSchema": schema(json!({"source": {"type": "string"}}), &["source"])},
		{"name": "run_preview", "description": "Dry-run a target (os-arch:renderer): the plan, problems and an image of each step.", "inputSchema": schema(json!({"target": {"type": "string"}}), &["target"])},
		{"name": "get_preview", "description": "The last preview, or null if the project changed since it ran.", "inputSchema": schema(json!({}), &[])},
		{"name": "check_project", "description": "The same checks as `liminst check`.", "inputSchema": schema(json!({}), &[])}
	])
}

fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, StudioError> {
	args.get(key)
		.and_then(Value::as_str)
		.ok_or_else(|| StudioError::invalid("arguments", format!("`{key}` is required")))
}

fn to_value<T: serde::Serialize>(v: &T) -> Result<Value, StudioError> {
	serde_json::to_value(v).map_err(|e| StudioError::invalid("result", e))
}

fn parse<T: for<'de> Deserialize<'de>>(v: &Value, what: &str) -> Result<T, StudioError> {
	serde_json::from_value(v.clone()).map_err(|e| StudioError::invalid(what, e))
}

/// Runs one tool against the studio. Changes are recorded with source [`Source::Mcp`].
pub fn call_tool(studio: &Studio, name: &str, args: &Value) -> Result<Value, StudioError> {
	let label = |default: String| {
		args.get("label")
			.and_then(Value::as_str)
			.map(str::to_owned)
			.unwrap_or(default)
	};
	match name {
		"get_project" => to_value(&studio.project_summary()?),
		"list_targets" => to_value(&studio.list_targets()?),
		"get_flow" => to_value(&studio.get_flow(str_arg(args, "id")?)?),
		"update_flow" => {
			let id = str_arg(args, "id")?;
			let label = label(format!("Agent: update flow `{id}`"));
			let view = match (args.get("flow"), args.get("patch")) {
				(Some(f), None) => {
					studio.set_flow(id, parse::<Flow>(f, "flow")?, &label, Source::Mcp)?
				}
				(None, Some(p)) => studio.patch_flow(id, p, &label, Source::Mcp)?,
				_ => {
					return Err(StudioError::invalid(
						"arguments",
						"give exactly one of `flow` and `patch`",
					));
				}
			};
			to_value(&view)
		}
		"get_layout" => to_value(&studio.get_layout(
			str_arg(args, "id")?,
			args.get("renderer").and_then(Value::as_str),
		)?),
		"update_layout" => {
			let id = str_arg(args, "id")?;
			let label = label(format!("Agent: update layout `{id}`"));
			let renderer = args.get("renderer").and_then(Value::as_str);
			let layout = if args.get("path").is_some() || renderer.is_some() {
				let path: Vec<usize> = match args.get("path") {
					Some(p) => parse(p, "path")?,
					None => Vec::new(),
				};
				let node: Option<Node> = match args.get("node") {
					Some(n) if !n.is_null() => Some(parse(n, "node")?),
					_ => None,
				};
				studio.set_layout_node(id, &path, renderer, node, &label, Source::Mcp)?
			} else {
				match (args.get("layout"), args.get("patch")) {
					(Some(l), None) => {
						studio.set_layout(id, parse::<Layout>(l, "layout")?, &label, Source::Mcp)?
					}
					(None, Some(p)) => studio.patch_layout(id, p, &label, Source::Mcp)?,
					_ => {
						return Err(StudioError::invalid(
							"arguments",
							"give `layout`, `patch`, or `path` with `node`",
						));
					}
				}
			};
			to_value(&layout)
		}
		"read_script" => Ok(
			json!({"path": str_arg(args, "path")?, "source": studio.read_script(str_arg(args, "path")?)?}),
		),
		"write_script" => {
			let path = str_arg(args, "path")?;
			studio.write_script(
				path,
				str_arg(args, "source")?,
				&label(format!("Agent: write `{path}`")),
				Source::Mcp,
			)?;
			Ok(json!({"path": path, "written": true}))
		}
		"test_script" => to_value(&studio.test_script(
			str_arg(args, "path")?,
			args.get("source").and_then(Value::as_str),
			args.get("target").and_then(Value::as_str),
		)?),
		"check_script_syntax" => to_value(&studio.check_script_syntax(str_arg(args, "source")?)),
		"run_preview" => to_value(&studio.run_preview(str_arg(args, "target")?)?),
		"get_preview" => to_value(&studio.get_preview()?),
		"check_project" => to_value(&studio.check_project()?),
		other => Err(StudioError::NotFound(format!("tool `{other}`"))),
	}
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
	json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Handles one JSON-RPC message. Returns `None` for notifications, which get no reply.
pub fn handle_rpc(studio: &Studio, request: &Value) -> Option<Value> {
	let id = request.get("id").cloned()?;
	let method = request.get("method").and_then(Value::as_str).unwrap_or("");
	let params = request.get("params").cloned().unwrap_or(Value::Null);
	Some(match method {
		"initialize" => json!({"jsonrpc": "2.0", "id": id, "result": {
			"protocolVersion": PROTOCOL_VERSION,
			"capabilities": {"tools": {"listChanged": false}},
			"serverInfo": {"name": "liminal-installer-studio", "version": env!("CARGO_PKG_VERSION")}
		}}),
		"ping" => json!({"jsonrpc": "2.0", "id": id, "result": {}}),
		"tools/list" => json!({"jsonrpc": "2.0", "id": id, "result": {"tools": tool_list()}}),
		"tools/call" => {
			let Some(name) = params.get("name").and_then(Value::as_str) else {
				return Some(rpc_error(&id, -32602, "`name` is required"));
			};
			let args = params
				.get("arguments")
				.cloned()
				.unwrap_or_else(|| json!({}));
			let (text, is_error) = match call_tool(studio, name, &args) {
				Ok(v) => (serde_json::to_string_pretty(&v).unwrap_or_default(), false),
				Err(e) => (e.to_string(), true),
			};
			json!({"jsonrpc": "2.0", "id": id, "result": {
				"content": [{"type": "text", "text": text}],
				"isError": is_error
			}})
		}
		_ => rpc_error(&id, -32601, "method not found"),
	})
}

#[derive(Clone)]
struct AppState {
	studio: Arc<Studio>,
	token: Arc<RwLock<String>>,
}

fn token_matches(given: &str, expected: &str) -> bool {
	given.len() == expected.len()
		&& given
			.bytes()
			.zip(expected.bytes())
			.fold(0u8, |acc, (a, b)| acc | (a ^ b))
			== 0
}

fn origin_is_local(headers: &HeaderMap) -> bool {
	match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
		None => true,
		Some(origin) => {
			let host = origin.split("://").nth(1).unwrap_or(origin);
			let host = host.split(['/', ':']).next().unwrap_or("");
			matches!(
				host,
				"127.0.0.1" | "localhost" | "tauri.localhost" | "[::1]"
			)
		}
	}
}

#[derive(Deserialize)]
struct TokenQuery {
	token: Option<String>,
}

async fn mcp_post(
	State(state): State<AppState>,
	Query(q): Query<TokenQuery>,
	headers: HeaderMap,
	body: String,
) -> Response {
	if !origin_is_local(&headers) {
		return (StatusCode::FORBIDDEN, "origin not allowed").into_response();
	}
	let bearer = headers
		.get(header::AUTHORIZATION)
		.and_then(|v| v.to_str().ok())
		.and_then(|v| v.strip_prefix("Bearer "))
		.map(str::to_owned);
	let given = bearer.or(q.token).unwrap_or_default();
	let expected = state.token.read().map(|t| t.clone()).unwrap_or_default();
	if !token_matches(&given, &expected) {
		return (StatusCode::UNAUTHORIZED, "missing or wrong token").into_response();
	}
	let Ok(request) = serde_json::from_str::<Value>(&body) else {
		let err = rpc_error(&Value::Null, -32700, "parse error");
		return (StatusCode::OK, axum::Json(err)).into_response();
	};
	let studio = state.studio.clone();
	let reply = tokio::task::spawn_blocking(move || match &request {
		Value::Array(batch) => {
			let replies: Vec<Value> = batch
				.iter()
				.filter_map(|r| handle_rpc(&studio, r))
				.collect();
			(!replies.is_empty()).then_some(Value::Array(replies))
		}
		single => handle_rpc(&studio, single),
	})
	.await
	.ok()
	.flatten();
	match reply {
		Some(v) => axum::Json(v).into_response(),
		None => StatusCode::ACCEPTED.into_response(),
	}
}

/// A running server. Dropping it stops it.
pub struct McpServer {
	pub addr: SocketAddr,
	token: Arc<RwLock<String>>,
	shutdown: Option<tokio::sync::oneshot::Sender<()>>,
	thread: Option<JoinHandle<()>>,
}

fn random_token() -> std::io::Result<String> {
	let mut bytes = [0u8; 24];
	getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
	Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

impl McpServer {
	/// Binds `127.0.0.1:port` (0 picks a free port) and serves in a background thread.
	pub fn start(studio: Arc<Studio>, port: u16) -> std::io::Result<McpServer> {
		let listener = TcpListener::bind(("127.0.0.1", port))?;
		listener.set_nonblocking(true)?;
		let addr = listener.local_addr()?;
		let token = Arc::new(RwLock::new(random_token()?));
		let state = AppState {
			studio,
			token: token.clone(),
		};
		let (tx, rx) = tokio::sync::oneshot::channel::<()>();
		let thread = std::thread::Builder::new()
			.name("studio-mcp".into())
			.spawn(move || {
				let rt = tokio::runtime::Builder::new_multi_thread()
					.worker_threads(2)
					.enable_all()
					.build()
					.expect("tokio runtime");
				rt.block_on(async move {
					let listener = tokio::net::TcpListener::from_std(listener).expect("listener");
					let app = Router::new()
						.route("/mcp", post(mcp_post))
						.with_state(state);
					let _ = axum::serve(listener, app)
						.with_graceful_shutdown(async {
							let _ = rx.await;
						})
						.await;
				});
			})?;
		Ok(McpServer {
			addr,
			token,
			shutdown: Some(tx),
			thread: Some(thread),
		})
	}

	/// The token a client must send. Shown only when a person asks for it; it is never logged.
	pub fn token(&self) -> String {
		self.token.read().map(|t| t.clone()).unwrap_or_default()
	}

	/// Replaces the token. Clients holding the old one are refused from the next request on.
	pub fn regenerate_token(&self) -> std::io::Result<String> {
		let fresh = random_token()?;
		let mut guard = self
			.token
			.write()
			.map_err(|_| std::io::Error::other("token lock poisoned"))?;
		guard.clone_from(&fresh);
		Ok(fresh)
	}

	/// The line printed at start: where the server listens. It leaves the token out, because a log is not a place for a secret.
	pub fn banner(&self) -> String {
		format!("Studio MCP server: http://{}/mcp", self.addr)
	}
}

/// Blocks the calling thread until the process is interrupted (Ctrl+C, or SIGINT).
pub fn wait_for_interrupt() -> std::io::Result<()> {
	tokio::runtime::Builder::new_current_thread()
		.enable_all()
		.build()?
		.block_on(tokio::signal::ctrl_c())
}

impl Drop for McpServer {
	fn drop(&mut self) {
		if let Some(tx) = self.shutdown.take() {
			let _ = tx.send(());
		}
		if let Some(t) = self.thread.take() {
			let _ = t.join();
		}
	}
}

#[cfg(test)]
mod tests {
	use std::io::{Read, Write};
	use std::net::TcpStream;

	use super::*;
	use crate::studio::testutil::open_example;

	fn call(s: &Studio, name: &str, args: Value) -> (bool, Value) {
		let r = handle_rpc(
			s,
			&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}}),
		)
		.unwrap();
		let res = &r["result"];
		let text = res["content"][0]["text"].as_str().unwrap();
		let value = serde_json::from_str(text).unwrap_or(Value::String(text.into()));
		(res["isError"].as_bool().unwrap(), value)
	}

	#[test]
	fn lists_every_tool_in_table_8() {
		let (_d, s, _r) = open_example("neutral");
		let r = handle_rpc(
			&s,
			&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
		)
		.unwrap();
		let names: Vec<&str> = r["result"]["tools"]
			.as_array()
			.unwrap()
			.iter()
			.map(|t| t["name"].as_str().unwrap())
			.collect();
		assert_eq!(
			names,
			[
				"get_project",
				"list_targets",
				"get_flow",
				"update_flow",
				"get_layout",
				"update_layout",
				"read_script",
				"write_script",
				"test_script",
				"check_script_syntax",
				"run_preview",
				"get_preview",
				"check_project"
			]
		);
	}

	#[test]
	fn notifications_get_no_reply_and_unknown_methods_error() {
		let (_d, s, _r) = open_example("neutral");
		assert!(
			handle_rpc(
				&s,
				&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
			)
			.is_none()
		);
		let r = handle_rpc(&s, &json!({"jsonrpc": "2.0", "id": 4, "method": "nope"})).unwrap();
		assert_eq!(r["error"]["code"], -32601);
		let init = handle_rpc(
			&s,
			&json!({"jsonrpc": "2.0", "id": 5, "method": "initialize", "params": {}}),
		)
		.unwrap();
		assert_eq!(init["result"]["protocolVersion"], PROTOCOL_VERSION);
	}

	#[test]
	fn read_tools_return_project_data() {
		let (_d, s, _r) = open_example("lt-fizz");
		let (err, project) = call(&s, "get_project", json!({}));
		assert!(!err);
		assert_eq!(project["id"], "hq.liminal.lt-fizz");
		let (_, targets) = call(&s, "list_targets", json!({}));
		assert_eq!(targets.as_array().unwrap().len(), 4);
		let (_, flow) = call(&s, "get_flow", json!({"id": "install"}));
		assert_eq!(flow["errors"].as_array().unwrap().len(), 0);
		let (_, layout) = call(
			&s,
			"get_layout",
			json!({"id": "fizz-dos/welcome", "renderer": "tui-retro"}),
		);
		assert_eq!(layout["root"]["props"]["style"], "text-mode");
		assert!(layout["root"].get("overrides").is_none());
		let (_, script) = call(
			&s,
			"read_script",
			json!({"path": "scripts/sound-setup.lua"}),
		);
		assert!(
			script["source"]
				.as_str()
				.unwrap()
				.contains("function detect(")
		);
		let (_, check) = call(&s, "check_project", json!({}));
		assert!(
			check
				.as_array()
				.unwrap()
				.iter()
				.all(|p| p["severity"] != "error")
		);
	}

	#[test]
	fn update_flow_goes_through_the_history_and_reports_errors() {
		let (_d, s, _r) = open_example("neutral");
		let (err, view) = call(
			&s,
			"update_flow",
			json!({"id": "install", "patch": {"steps": []}, "label": "Agent empties flow"}),
		);
		assert!(!err);
		assert_eq!(view["errors"].as_array().unwrap().len(), 1);
		let h = s.history();
		assert_eq!(h.entries.len(), 1);
		assert_eq!(h.entries[0].source, Source::Mcp);
		assert_eq!(h.entries[0].label, "Agent empties flow");
		s.undo(Source::Ui).unwrap();
		assert!(s.get_flow("install").unwrap().errors.is_empty());
	}

	#[test]
	fn update_layout_sets_a_renderer_override_at_a_path() {
		let (_d, s, _r) = open_example("neutral");
		let (err, _) = call(
			&s,
			"update_layout",
			json!({"id": "welcome", "path": [], "renderer": "web", "node": {"kind": "dialog"}}),
		);
		assert!(!err);
		let (_, web) = call(
			&s,
			"get_layout",
			json!({"id": "welcome", "renderer": "web"}),
		);
		assert_eq!(web["root"]["kind"], "dialog");
		let (_, canvas) = call(
			&s,
			"get_layout",
			json!({"id": "welcome", "renderer": "canvas"}),
		);
		assert_eq!(canvas["root"]["kind"], "window");
		let (_, patched) = call(
			&s,
			"update_layout",
			json!({"id": "welcome", "patch": {"root": {"props": {"title": "Patched"}}}}),
		);
		assert_eq!(patched["root"]["props"]["title"], "Patched");
		assert_eq!(s.history().entries.len(), 2);
	}

	#[test]
	fn scripts_can_be_written_and_tested() {
		let (_d, s, _r) = open_example("neutral");
		let src = "local env = require(\"li.env\")\nfunction detect(page) end\n";
		let (err, _) = call(
			&s,
			"write_script",
			json!({"path": "scripts/hello.lua", "source": src}),
		);
		assert!(!err);
		let (_, back) = call(&s, "read_script", json!({"path": "scripts/hello.lua"}));
		assert_eq!(back["source"], src);
		let (_, report) = call(&s, "test_script", json!({"path": "scripts/hello.lua"}));
		assert_eq!(report["ok"], true);
		assert_eq!(report["capabilities"], json!(["li.env"]));
		let (_, bad) = call(
			&s,
			"test_script",
			json!({"path": "scripts/hello.lua", "source": "-- empty"}),
		);
		assert_eq!(bad["ok"], false);
	}

	#[test]
	fn script_syntax_is_checked_without_running_it() {
		let (_d, s, _r) = open_example("neutral");
		let (err, found) = call(
			&s,
			"check_script_syntax",
			json!({"source": "function detect(page)\n\tlocal a = li.env.get('x')\n"}),
		);
		assert!(!err);
		assert_eq!(found[0]["code"], "syntax");
		assert_eq!(found[0]["severity"], "error");
		assert_eq!(found[0]["line"], 2);
		let (_, warned) = call(
			&s,
			"check_script_syntax",
			json!({"source": "function detect(page)\n\tlocal a = li.env.get('x')\nend\n"}),
		);
		assert_eq!(warned[0]["code"], "undeclared-capability");
		assert_eq!(warned[0]["severity"], "warning");
		assert_eq!(
			(warned[0]["line"].clone(), warned[0]["column"].clone()),
			(json!(2), json!(12))
		);
	}

	#[test]
	fn previews_are_served_until_the_project_changes() {
		let (_d, s, _r) = open_example("lt-fizz");
		let (_, none) = call(&s, "get_preview", json!({}));
		assert!(none.is_null());
		let (err, pv) = call(&s, "run_preview", json!({"target": "dos-x86:tui-retro"}));
		assert!(!err);
		assert_eq!(pv["target"], "dos-x86:tui-retro");
		let (_, again) = call(&s, "get_preview", json!({}));
		assert_eq!(again["target"], "dos-x86:tui-retro");
		let (err, e) = call(&s, "run_preview", json!({"target": "nope"}));
		assert!(err);
		assert!(e.as_str().unwrap().contains("unknown target"));
	}

	#[test]
	fn bad_arguments_are_tool_errors_not_crashes() {
		let (_d, s, _r) = open_example("neutral");
		assert!(call(&s, "get_flow", json!({})).0);
		assert!(call(&s, "get_flow", json!({"id": "nope"})).0);
		assert!(call(&s, "update_flow", json!({"id": "install"})).0);
		assert!(
			call(
				&s,
				"write_script",
				json!({"path": "../x.lua", "source": ""})
			)
			.0
		);
		assert!(call(&s, "no_such_tool", json!({})).0);
	}

	fn post(addr: SocketAddr, auth: Option<&str>, origin: Option<&str>, body: &str) -> String {
		let mut stream = TcpStream::connect(addr).unwrap();
		let mut req = format!(
			"POST /mcp HTTP/1.1\r\nHost: {addr}\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n",
			body.len()
		);
		if let Some(a) = auth {
			req.push_str(&format!("Authorization: Bearer {a}\r\n"));
		}
		if let Some(o) = origin {
			req.push_str(&format!("Origin: {o}\r\n"));
		}
		req.push_str("\r\n");
		req.push_str(body);
		stream.write_all(req.as_bytes()).unwrap();
		let mut out = String::new();
		stream.read_to_string(&mut out).unwrap();
		out
	}

	#[test]
	fn the_http_server_requires_the_token_and_serves_tools() {
		let (_d, s, _r) = open_example("neutral");
		let server = McpServer::start(Arc::new(s), 0).unwrap();
		assert!(server.addr.ip().is_loopback());
		assert!(!server.banner().contains(&server.token()));
		let token = server.token();
		let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;

		let no_token = post(server.addr, None, None, body);
		assert!(no_token.starts_with("HTTP/1.1 401"), "{no_token}");
		let wrong = post(server.addr, Some("nope"), None, body);
		assert!(wrong.starts_with("HTTP/1.1 401"));
		let foreign = post(
			server.addr,
			Some(&token),
			Some("https://evil.example"),
			body,
		);
		assert!(foreign.starts_with("HTTP/1.1 403"));

		let ok = post(server.addr, Some(&token), None, body);
		assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
		assert!(ok.contains("check_project"));
		let note = post(
			server.addr,
			Some(&token),
			None,
			r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
		);
		assert!(note.starts_with("HTTP/1.1 202"));
	}

	#[test]
	fn tokens_are_long_random_hex_and_regeneration_refuses_the_old_one() {
		let (_d, s, _r) = open_example("neutral");
		let server = McpServer::start(Arc::new(s), 0).unwrap();
		let first = server.token();
		assert_eq!(first.len(), 48);
		assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
		let body = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
		assert!(post(server.addr, Some(&first), None, body).starts_with("HTTP/1.1 200"));

		let second = server.regenerate_token().unwrap();
		assert_ne!(first, second);
		assert_eq!(server.token(), second);
		assert!(post(server.addr, Some(&first), None, body).starts_with("HTTP/1.1 401"));
		assert!(post(server.addr, Some(&second), None, body).starts_with("HTTP/1.1 200"));
	}
}
