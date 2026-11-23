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

#[derive(Debug, Default)]
struct SimState {
	nodes: BTreeMap<PathBuf, SimNode>,
	registry: BTreeMap<String, String>,
	launched: Vec<(PathBuf, Vec<String>)>,
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
		"linux"
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

	fn script_fs(&self) -> Arc<dyn ScriptFs> {
		Arc::new(SimFs(self.state.clone()))
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
