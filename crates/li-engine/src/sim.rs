// An in-memory machine for dry runs and tests.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use li_script::ScriptFs;

use crate::machine::{Dirs, Machine, NodeKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimNode {
	Dir,
	File { data: Vec<u8>, executable: bool },
}

/// Answers for a program the simulated machine is asked to run. Returning `None` leaves the answer
/// to the machine's built-ins (`curl` and the WebView2 installers).
pub type RunHandler = Arc<dyn Fn(&SimMachine, &Path, &[String]) -> Option<i32> + Send + Sync>;

#[derive(Default)]
struct SimState {
	nodes: BTreeMap<PathBuf, SimNode>,
	registry: BTreeMap<String, String>,
	launched: Vec<(PathBuf, Vec<String>)>,
	runs: Vec<(PathBuf, Vec<String>)>,
	handler: Option<RunHandler>,
	offline: bool,
	elevated: bool,
}

fn not_found(path: &Path) -> io::Error {
	io::Error::new(
		io::ErrorKind::NotFound,
		format!("{} not found", path.display()),
	)
}

impl SimState {
	fn parent_is_dir(&self, path: &Path) -> bool {
		match path.parent() {
			Some(p) if p.as_os_str().is_empty() || p == Path::new("/") => true,
			Some(p) => matches!(self.nodes.get(p), Some(SimNode::Dir)),
			None => true,
		}
	}
}

/// A machine that lives in memory. Cloning the handle shares the same state.
#[derive(Clone)]
pub struct SimMachine {
	state: Arc<Mutex<SimState>>,
	dirs: Dirs,
	capacity: u64,
	os: &'static str,
}

impl Default for SimMachine {
	fn default() -> Self {
		Self::new()
	}
}

impl SimMachine {
	/// A machine with `/sim/...` home, data, desktop and temp directories and 1 GiB of space.
	pub fn new() -> Self {
		let dirs = Dirs {
			user_data: "/sim/home/.local/share".into(),
			system_data: "/sim/usr/share".into(),
			start_menu: "/sim/home/.local/share/applications".into(),
			desktop: "/sim/home/Desktop".into(),
			temp: "/sim/tmp".into(),
		};
		let m = SimMachine {
			state: Arc::default(),
			dirs,
			capacity: 1 << 30,
			os: "linux",
		};
		for d in [
			&m.dirs.user_data,
			&m.dirs.system_data,
			&m.dirs.desktop,
			&m.dirs.temp,
		] {
			m.mkdir_all(d);
		}
		m
	}

	/// Reports another operating system (`windows`, `macos`), so a dry run of that target reads
	/// the machine as it would be there.
	pub fn with_os(mut self, os: &'static str) -> Self {
		self.os = os;
		self
	}

	/// A Windows machine that already has the WebView2 runtime, machine-wide, at `version`.
	pub fn with_webview2(self, version: &str) -> Self {
		self.set_registry_value(
			crate::tauri::webview2::HKLM_CLIENT_KEY,
			crate::tauri::webview2::VERSION_VALUE,
			version,
		);
		self
	}

	/// A machine with no network: `curl` and the WebView2 bootstrappers fail.
	pub fn offline(self) -> Self {
		self.state.lock().expect("sim lock").offline = true;
		self
	}

	/// A machine whose programs run elevated: installers write `HKLM` and machine-wide folders.
	pub fn elevated(self) -> Self {
		self.state.lock().expect("sim lock").elevated = true;
		self
	}

	/// Whether the machine has no network.
	pub fn is_offline(&self) -> bool {
		self.state.lock().expect("sim lock").offline
	}

	/// Removes every file and folder under `dir` except the file named `keep` (an uninstaller
	/// that runs in place cannot delete itself). The folder stays.
	pub fn remove_tree_except(&self, dir: &Path, keep: &str) {
		let mut st = self.state.lock().expect("sim lock");
		st.nodes.retain(|p, _| {
			!(p.starts_with(dir) && p != dir && p.file_name().is_none_or(|n| n != keep))
		});
	}

	/// Whether programs run elevated.
	pub fn is_elevated(&self) -> bool {
		self.state.lock().expect("sim lock").elevated
	}

	/// Answers for programs the machine is asked to run, ahead of its built-ins.
	pub fn on_run(
		&self,
		f: impl Fn(&SimMachine, &Path, &[String]) -> Option<i32> + Send + Sync + 'static,
	) {
		self.state.lock().expect("sim lock").handler = Some(Arc::new(f));
	}

	/// A registry value as the platform would have it (not an operation the engine wrote).
	pub fn set_registry_value(&self, key: &str, name: &str, value: &str) {
		self.state.lock().expect("sim lock").registry.insert(
			crate::machine::registry_entry_key(key, name),
			value.to_owned(),
		);
	}

	/// Deletes one registry value.
	pub fn remove_registry_value(&self, key: &str, name: &str) {
		self.state
			.lock()
			.expect("sim lock")
			.registry
			.remove(&crate::machine::registry_entry_key(key, name));
	}

	/// Every program the machine was asked to run, in order, with its arguments.
	pub fn runs(&self) -> Vec<(PathBuf, Vec<String>)> {
		self.state.lock().expect("sim lock").runs.clone()
	}

	pub fn with_capacity(mut self, bytes: u64) -> Self {
		self.capacity = bytes;
		self
	}

	/// Creates a directory and its parents.
	pub fn mkdir_all(&self, path: &Path) {
		let mut st = self.state.lock().expect("sim lock");
		let mut cur = PathBuf::new();
		for c in path.components() {
			cur.push(c);
			if cur != Path::new("/") {
				st.nodes.entry(cur.clone()).or_insert(SimNode::Dir);
			}
		}
	}

	/// Puts a file in place, creating parents.
	pub fn add_file(&self, path: &Path, data: &[u8], executable: bool) {
		if let Some(p) = path.parent() {
			self.mkdir_all(p);
		}
		self.state.lock().expect("sim lock").nodes.insert(
			path.to_owned(),
			SimNode::File {
				data: data.to_vec(),
				executable,
			},
		);
	}

	/// Every node, for comparing trees.
	pub fn snapshot(&self) -> BTreeMap<PathBuf, SimNode> {
		self.state.lock().expect("sim lock").nodes.clone()
	}

	pub fn registry(&self) -> BTreeMap<String, String> {
		self.state.lock().expect("sim lock").registry.clone()
	}

	/// What `launch` was asked to start.
	pub fn launched(&self) -> Vec<(PathBuf, Vec<String>)> {
		self.state.lock().expect("sim lock").launched.clone()
	}

	fn used(&self) -> u64 {
		self.state
			.lock()
			.expect("sim lock")
			.nodes
			.values()
			.map(|n| match n {
				SimNode::File { data, .. } => data.len() as u64,
				SimNode::Dir => 0,
			})
			.sum()
	}
}

impl Machine for SimMachine {
	fn dirs(&self) -> &Dirs {
		&self.dirs
	}

	fn os(&self) -> &str {
		self.os
	}

	fn arch(&self) -> &str {
		"x86_64"
	}

	fn kind(&self, path: &Path) -> Option<NodeKind> {
		match self.state.lock().expect("sim lock").nodes.get(path)? {
			SimNode::Dir => Some(NodeKind::Dir),
			SimNode::File { .. } => Some(NodeKind::File),
		}
	}

	fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
		match self.state.lock().expect("sim lock").nodes.get(path) {
			Some(SimNode::File { data, .. }) => Ok(data.clone()),
			Some(SimNode::Dir) => Err(io::Error::other("is a directory")),
			None => Err(not_found(path)),
		}
	}

	fn list(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
		let st = self.state.lock().expect("sim lock");
		if !matches!(st.nodes.get(path), Some(SimNode::Dir)) {
			return Err(not_found(path));
		}
		Ok(st
			.nodes
			.keys()
			.filter(|p| p.parent() == Some(path))
			.cloned()
			.collect())
	}

	fn is_executable(&self, path: &Path) -> bool {
		matches!(
			self.state.lock().expect("sim lock").nodes.get(path),
			Some(SimNode::File {
				executable: true,
				..
			})
		)
	}

	fn write(&mut self, path: &Path, data: &[u8], executable: bool) -> io::Result<()> {
		let mut st = self.state.lock().expect("sim lock");
		if !st.parent_is_dir(path) {
			return Err(not_found(path));
		}
		if matches!(st.nodes.get(path), Some(SimNode::Dir)) {
			return Err(io::Error::other("is a directory"));
		}
		st.nodes.insert(
			path.to_owned(),
			SimNode::File {
				data: data.to_vec(),
				executable,
			},
		);
		Ok(())
	}

	fn append(&mut self, path: &Path, data: &[u8]) -> io::Result<()> {
		let mut st = self.state.lock().expect("sim lock");
		match st.nodes.get_mut(path) {
			Some(SimNode::File { data: d, .. }) => {
				d.extend_from_slice(data);
				Ok(())
			}
			Some(SimNode::Dir) => Err(io::Error::other("is a directory")),
			None => {
				if !st.parent_is_dir(path) {
					return Err(not_found(path));
				}
				st.nodes.insert(
					path.to_owned(),
					SimNode::File {
						data: data.to_vec(),
						executable: false,
					},
				);
				Ok(())
			}
		}
	}

	fn create_dir(&mut self, path: &Path) -> io::Result<()> {
		let mut st = self.state.lock().expect("sim lock");
		if st.nodes.contains_key(path) {
			return Err(io::Error::from(io::ErrorKind::AlreadyExists));
		}
		if !st.parent_is_dir(path) {
			return Err(not_found(path));
		}
		st.nodes.insert(path.to_owned(), SimNode::Dir);
		Ok(())
	}

	fn remove_file(&mut self, path: &Path) -> io::Result<()> {
		let mut st = self.state.lock().expect("sim lock");
		match st.nodes.get(path) {
			Some(SimNode::File { .. }) => {
				st.nodes.remove(path);
				Ok(())
			}
			Some(SimNode::Dir) => Err(io::Error::other("is a directory")),
			None => Err(not_found(path)),
		}
	}

	fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
		let mut st = self.state.lock().expect("sim lock");
		match st.nodes.get(path) {
			Some(SimNode::Dir) => {
				if st.nodes.keys().any(|p| p.parent() == Some(path)) {
					return Err(io::Error::other("directory not empty"));
				}
				st.nodes.remove(path);
				Ok(())
			}
			Some(SimNode::File { .. }) => Err(io::Error::other("not a directory")),
			None => Err(not_found(path)),
		}
	}

	fn free_space(&self, _path: &Path) -> u64 {
		self.capacity.saturating_sub(self.used())
	}

	fn registry_get(&self, key: &str) -> Option<String> {
		self.state
			.lock()
			.expect("sim lock")
			.registry
			.get(key)
			.cloned()
	}

	fn registry_set(&mut self, key: &str, value: Option<&str>) -> io::Result<()> {
		let mut st = self.state.lock().expect("sim lock");
		match value {
			Some(v) => {
				st.registry.insert(key.to_owned(), v.to_owned());
			}
			None => {
				st.registry.remove(key);
			}
		}
		Ok(())
	}

	fn launch(&mut self, path: &Path, args: &[String]) -> io::Result<()> {
		self.state
			.lock()
			.expect("sim lock")
			.launched
			.push((path.to_owned(), args.to_vec()));
		Ok(())
	}

	fn run(&mut self, program: &Path, args: &[String]) -> io::Result<i32> {
		let handler = {
			let mut st = self.state.lock().expect("sim lock");
			st.runs.push((program.to_owned(), args.to_vec()));
			st.handler.clone()
		};
		if let Some(code) = handler.and_then(|h| h(self, program, args)) {
			return Ok(code);
		}
		Ok(self.builtin_run(program, args))
	}

	fn script_fs(&self) -> Arc<dyn ScriptFs> {
		Arc::new(SimFs(self.state.clone()))
	}
}

impl SimMachine {
	/// What the simulated machine does for `curl` and the WebView2 installers; any other program
	/// exits 0 and changes nothing.
	fn builtin_run(&mut self, program: &Path, args: &[String]) -> i32 {
		use crate::tauri::webview2;
		let name = program
			.file_name()
			.map(|n| n.to_string_lossy().to_ascii_lowercase())
			.unwrap_or_default();
		let (offline, elevated) = {
			let st = self.state.lock().expect("sim lock");
			(st.offline, st.elevated)
		};
		if name == "curl" || name == "curl.exe" {
			if offline {
				return 6;
			}
			let dest = args
				.iter()
				.position(|a| a == "-o")
				.and_then(|i| args.get(i + 1));
			if let Some(dest) = dest {
				self.add_file(Path::new(dest), b"MZ simulated download", true);
			}
			return 0;
		}
		let is_bootstrapper = name.contains("microsoftedgewebview2setup");
		let is_offline_installer = name.contains("webview2runtimeinstaller");
		if is_bootstrapper || is_offline_installer {
			if is_bootstrapper && offline {
				return 1;
			}
			// An existing machine-wide runtime is updated where it is; otherwise the installer
			// puts the runtime where its own elevation says.
			let machine_wide = self
				.registry_value(webview2::HKLM_CLIENT_KEY, webview2::VERSION_VALUE)
				.is_some();
			let key = if elevated || machine_wide {
				webview2::HKLM_CLIENT_KEY
			} else {
				webview2::HKCU_CLIENT_KEY
			};
			self.set_registry_value(
				key,
				webview2::VERSION_VALUE,
				webview2::SIM_INSTALLED_VERSION,
			);
			return 0;
		}
		0
	}
}

struct SimFs(Arc<Mutex<SimState>>);

impl ScriptFs for SimFs {
	fn read(&self, path: &str) -> Result<Vec<u8>, String> {
		match self.0.lock().expect("sim lock").nodes.get(Path::new(path)) {
			Some(SimNode::File { data, .. }) => Ok(data.clone()),
			_ => Err(format!("cannot read `{path}`")),
		}
	}

	fn exists(&self, path: &str) -> bool {
		self.0
			.lock()
			.expect("sim lock")
			.nodes
			.contains_key(Path::new(path))
	}

	fn list(&self, path: &str) -> Result<Vec<String>, String> {
		let st = self.0.lock().expect("sim lock");
		let dir = Path::new(path);
		if !matches!(st.nodes.get(dir), Some(SimNode::Dir)) {
			return Err(format!("cannot list `{path}`"));
		}
		Ok(st
			.nodes
			.keys()
			.filter(|p| p.parent() == Some(dir))
			.filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
			.collect())
	}
}
