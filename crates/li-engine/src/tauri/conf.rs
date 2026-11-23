// Reading a Tauri 2 `tauri.conf.json` into a normalized snapshot.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::path::Path;

use li_model::tauri::{
	FileAssociation, NsisInstallMode, ResourceRule, SourceHash, TauriSnapshot, UpdaterSnapshot,
	Webview2Mode, Webview2Setting, WindowsSnapshot,
};
use serde_json::Value;

use crate::error::{EngineError, Result};

/// The platform overlays Tauri merges over the base file for one platform.
pub const OVERLAYS: &[&str] = &[
	"tauri.windows.conf.json",
	"tauri.macos.conf.json",
	"tauri.linux.conf.json",
];

fn parse_err(path: &Path, message: impl Into<String>) -> EngineError {
	EngineError::Parse {
		path: path.to_owned(),
		message: message.into(),
	}
}

/// RFC 7396 JSON Merge Patch, which is how Tauri applies a platform overlay.
pub fn merge_patch(base: &mut Value, patch: &Value) {
	match patch {
		Value::Object(p) => {
			if !base.is_object() {
				*base = Value::Object(Default::default());
			}
			let obj = base.as_object_mut().expect("object");
			for (k, v) in p {
				if v.is_null() {
					obj.remove(k);
				} else {
					merge_patch(obj.entry(k.clone()).or_insert(Value::Null), v);
				}
			}
		}
		other => *base = other.clone(),
	}
}

/// The files a snapshot is read from: the config and any overlay beside it, with their hashes.
pub fn source_hashes(config: &Path) -> Result<Vec<SourceHash>> {
	let dir = config.parent().unwrap_or(Path::new("."));
	let mut out = Vec::new();
	let name = config
		.file_name()
		.map(|n| n.to_string_lossy().into_owned())
		.unwrap_or_default();
	let bytes = std::fs::read(config).map_err(|e| EngineError::io(config, e))?;
	out.push(SourceHash {
		path: name,
		sha256: li_pack::sha256_hex(&bytes),
	});
	for overlay in OVERLAYS {
		let path = dir.join(overlay);
		if let Ok(bytes) = std::fs::read(&path) {
			out.push(SourceHash {
				path: (*overlay).to_owned(),
				sha256: li_pack::sha256_hex(&bytes),
			});
		}
	}
	Ok(out)
}

fn read_json(path: &Path) -> Result<Value> {
	let text = std::fs::read_to_string(path).map_err(|e| EngineError::io(path, e))?;
	serde_json::from_str(&text).map_err(|e| parse_err(path, e.to_string()))
}

fn s(v: &Value, pointer: &str) -> Option<String> {
	v.pointer(pointer)
		.and_then(Value::as_str)
		.filter(|t| !t.is_empty())
		.map(str::to_owned)
}

fn strings(v: Option<&Value>) -> Vec<String> {
	match v {
		Some(Value::String(t)) => vec![t.clone()],
		Some(Value::Array(a)) => a
			.iter()
			.filter_map(|x| x.as_str().map(str::to_owned))
			.collect(),
		_ => Vec::new(),
	}
}

fn has_glob(p: &str) -> bool {
	p.contains(['*', '?'])
}

fn last_segment(p: &str) -> &str {
	p.trim_end_matches('/').rsplit('/').next().unwrap_or(p)
}

/// The install-relative form of a resource path: Tauri turns `..` into `_up_`.
fn install_relative(p: &str) -> String {
	let mut out = Vec::new();
	for part in p.split('/') {
		match part {
			"" | "." => {}
			".." => out.push("_up_"),
			other => out.push(other),
		}
	}
	out.join("/")
}

fn dir_of(p: &str) -> String {
	match p.trim_end_matches('/').rsplit_once('/') {
		Some((d, _)) => format!("{d}/"),
		None => String::new(),
	}
}

fn is_dir_like(p: &str) -> bool {
	p.ends_with('/') || !last_segment(p).contains('.')
}

/// `bundle.resources` as source and destination rules. A glob keeps its pattern in `from` and its
/// destination folder in `to` (ending in `/`); a folder ends both in `/`.
fn resource_rules(v: Option<&Value>) -> Vec<ResourceRule> {
	let mut out = Vec::new();
	let mut add = |src: &str, dest: Option<&str>| {
		let src_norm = src.trim_start_matches("./").to_owned();
		let rule = if has_glob(&src_norm) {
			let to = match dest {
				Some(d) => format!("{}/", install_relative(d)).replace("//", "/"),
				None => dir_of(&install_relative(&src_norm)),
			};
			ResourceRule {
				from: src_norm,
				to: to.trim_start_matches('/').to_owned(),
			}
		} else if is_dir_like(&src_norm) {
			let to = install_relative(dest.unwrap_or(&src_norm));
			ResourceRule {
				from: format!("{}/", src_norm.trim_end_matches('/')),
				to: if to.is_empty() {
					String::new()
				} else {
					format!("{to}/")
				},
			}
		} else {
			let to = match dest {
				Some(d) if d.ends_with('/') => {
					format!("{}{}", install_relative(d).trim_end_matches('/'), "/")
						+ last_segment(&src_norm)
				}
				Some(d) => install_relative(d),
				None => install_relative(&src_norm),
			};
			ResourceRule { from: src_norm, to }
		};
		out.push(rule);
	};
	match v {
		Some(Value::Array(a)) => {
			for item in a.iter().filter_map(Value::as_str) {
				add(item, None);
			}
		}
		Some(Value::Object(o)) => {
			for (src, dest) in o {
				if let Some(d) = dest.as_str() {
					add(src, Some(d));
				}
			}
		}
		_ => {}
	}
	out
}

fn file_associations(v: Option<&Value>) -> Vec<FileAssociation> {
	let Some(Value::Array(items)) = v else {
		return Vec::new();
	};
	items
		.iter()
		.filter_map(|item| {
			let ext = strings(item.get("ext"));
			if ext.is_empty() {
				return None;
			}
			let name = item
				.get("name")
				.and_then(Value::as_str)
				.map(str::to_owned)
				.unwrap_or_else(|| ext[0].clone());
			Some(FileAssociation {
				ext,
				name,
				description: item
					.get("description")
					.and_then(Value::as_str)
					.map(str::to_owned),
				mime_type: item
					.get("mimeType")
					.and_then(Value::as_str)
					.map(str::to_owned),
			})
		})
		.collect()
}

/// `plugins.deep-link.desktop.schemes`; `desktop` is one object or a list of them.
fn deep_link_schemes(v: &Value) -> Vec<String> {
	let Some(desktop) = v.pointer("/plugins/deep-link/desktop") else {
		return Vec::new();
	};
	let mut out = Vec::new();
	match desktop {
		Value::Array(items) => {
			for item in items {
				out.extend(strings(item.get("schemes")));
			}
		}
		other => out.extend(strings(other.get("schemes"))),
	}
	out.sort();
	out.dedup();
	out
}

fn webview2(win: &Value) -> Webview2Setting {
	let mode_value = win.pointer("/bundle/windows/webviewInstallMode");
	let min_version = s(win, "/bundle/windows/minimumWebview2Version");
	let Some(m) = mode_value else {
		return Webview2Setting {
			min_version,
			..Default::default()
		};
	};
	let name = match m {
		Value::String(t) => Some(t.as_str()),
		other => other.get("type").and_then(Value::as_str),
	};
	let mode = match name {
		Some("embedBootstrapper") => Webview2Mode::EmbedBootstrapper,
		Some("offlineInstaller") => Webview2Mode::OfflineInstaller,
		Some("fixedRuntime") => Webview2Mode::FixedRuntime,
		Some("skip") => Webview2Mode::Skip,
		_ => Webview2Mode::DownloadBootstrapper,
	};
	Webview2Setting {
		mode,
		silent: m.get("silent").and_then(Value::as_bool).unwrap_or(true),
		path: m.get("path").and_then(Value::as_str).map(str::to_owned),
		min_version,
	}
}

fn updater(v: &Value) -> Option<UpdaterSnapshot> {
	let u = v.pointer("/plugins/updater")?;
	let create = match v.pointer("/bundle/createUpdaterArtifacts") {
		Some(Value::Bool(b)) => *b,
		Some(Value::String(_)) => true,
		_ => false,
	};
	Some(UpdaterSnapshot {
		endpoints: strings(u.get("endpoints")),
		pubkey: u.get("pubkey").and_then(Value::as_str).map(str::to_owned),
		windows_install_mode: u
			.pointer("/windows/installMode")
			.and_then(Value::as_str)
			.map(str::to_owned),
		create_artifacts: create,
	})
}

/// Reads `tauri.conf.json` and the overlays beside it into a snapshot. Windows settings come from
/// the base merged with `tauri.windows.conf.json`, as Tauri applies it on Windows.
pub fn read_snapshot(config: &Path) -> Result<TauriSnapshot> {
	let base = read_json(config)?;
	let dir = config.parent().unwrap_or(Path::new("."));
	let windows_overlay = dir.join("tauri.windows.conf.json");
	let mut win = base.clone();
	if windows_overlay.is_file() {
		merge_patch(&mut win, &read_json(&windows_overlay)?);
	}

	let product_name = s(&base, "/productName")
		.ok_or_else(|| parse_err(config, "`productName` is missing or empty"))?;
	let identifier = s(&base, "/identifier")
		.ok_or_else(|| parse_err(config, "`identifier` is missing or empty"))?;
	let targets = match base.pointer("/bundle/targets") {
		None => vec!["all".to_owned()],
		other => strings(other),
	};
	let install_mode = match s(&win, "/bundle/windows/nsis/installMode").as_deref() {
		Some("perMachine") => NsisInstallMode::PerMachine,
		Some("both") => NsisInstallMode::Both,
		_ => NsisInstallMode::CurrentUser,
	};
	Ok(TauriSnapshot {
		version: TauriSnapshot::VERSION,
		sources: source_hashes(config)?,
		product_name,
		identifier,
		app_version: s(&base, "/version").unwrap_or_default(),
		main_binary_name: s(&base, "/mainBinaryName"),
		publisher: s(&base, "/bundle/publisher"),
		homepage: s(&base, "/bundle/homepage"),
		description: s(&base, "/bundle/shortDescription")
			.or_else(|| s(&base, "/bundle/longDescription")),
		targets,
		windows: WindowsSnapshot {
			webview2: webview2(&win),
			install_mode,
			allow_downgrades: win
				.pointer("/bundle/windows/allowDowngrades")
				.and_then(Value::as_bool)
				.unwrap_or(true),
			start_menu_folder: s(&win, "/bundle/windows/nsis/startMenuFolder"),
			has_installer_hooks: s(&win, "/bundle/windows/nsis/installerHooks").is_some(),
			has_custom_template: s(&win, "/bundle/windows/nsis/template").is_some(),
		},
		resources: resource_rules(base.pointer("/bundle/resources")),
		external_bin: strings(base.pointer("/bundle/externalBin")),
		file_associations: file_associations(base.pointer("/bundle/fileAssociations")),
		deep_link_schemes: deep_link_schemes(&base),
		updater: updater(&base),
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn merge_patch_follows_rfc_7396() {
		let mut base = json!({"a": {"b": 1, "c": 2}, "d": [1]});
		merge_patch(&mut base, &json!({"a": {"b": null, "e": 3}, "d": [2]}));
		assert_eq!(base, json!({"a": {"c": 2, "e": 3}, "d": [2]}));
	}

	#[test]
	fn resources_normalize_both_forms() {
		let array = resource_rules(Some(&json!([
			"assets/*",
			"../shared/data/",
			"licences/NOTICE.txt",
			"templates"
		])));
		assert_eq!(
			array,
			vec![
				ResourceRule {
					from: "assets/*".into(),
					to: "assets/".into()
				},
				ResourceRule {
					from: "../shared/data/".into(),
					to: "_up_/shared/data/".into()
				},
				ResourceRule {
					from: "licences/NOTICE.txt".into(),
					to: "licences/NOTICE.txt".into()
				},
				ResourceRule {
					from: "templates/".into(),
					to: "templates/".into()
				},
			]
		);
		let map = resource_rules(Some(&json!({"src/*.png": "img", "README.md": "docs/"})));
		assert_eq!(
			map,
			vec![
				ResourceRule {
					from: "README.md".into(),
					to: "docs/README.md".into()
				},
				ResourceRule {
					from: "src/*.png".into(),
					to: "img/".into()
				},
			]
		);
	}

	#[test]
	fn webview2_modes_read_tauris_names() {
		let w = webview2(&json!({"bundle": {"windows": {
			"webviewInstallMode": {"type": "embedBootstrapper", "silent": false},
			"minimumWebview2Version": "110.0.1587.40"
		}}}));
		assert_eq!(w.mode, Webview2Mode::EmbedBootstrapper);
		assert!(!w.silent);
		assert_eq!(w.min_version.as_deref(), Some("110.0.1587.40"));
		let d = webview2(&json!({}));
		assert_eq!(d.mode, Webview2Mode::DownloadBootstrapper);
		assert!(d.silent);
		let f = webview2(&json!({"bundle": {"windows": {
			"webviewInstallMode": {"type": "fixedRuntime", "path": "./Microsoft.WebView2.FixedVersionRuntime"}
		}}}));
		assert_eq!(f.mode, Webview2Mode::FixedRuntime);
		assert_eq!(
			f.path.as_deref(),
			Some("./Microsoft.WebView2.FixedVersionRuntime")
		);
	}

	#[test]
	fn deep_links_accept_an_object_or_a_list() {
		let one = json!({"plugins": {"deep-link": {"desktop": {"schemes": ["a", "b"]}}}});
		assert_eq!(deep_link_schemes(&one), vec!["a", "b"]);
		let many = json!({"plugins": {"deep-link": {"desktop": [{"schemes": ["b"]}, {"schemes": ["a"]}]}}});
		assert_eq!(deep_link_schemes(&many), vec!["a", "b"]);
	}
}
