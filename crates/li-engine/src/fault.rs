// A machine wrapper that fails on demand, for testing rollback.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use li_script::ScriptFs;

use crate::machine::{Dirs, Machine, NodeKind};

/// Wraps a machine and makes one mutating call fail. Calls under the temp directory (the journal)
/// are never failed or counted, so rollback itself still works.
pub struct FaultMachine<M: Machine> {
	pub inner: M,
	fail_at: Option<usize>,
	count: AtomicUsize,
}

impl<M: Machine> FaultMachine<M> {
	/// Fails the `n`th (zero-based) counted mutating call.
	pub fn failing_at(inner: M, n: usize) -> Self {
		FaultMachine {
			inner,
			fail_at: Some(n),
			count: AtomicUsize::new(0),
		}
	}

	/// Never fails; counts calls so a test can learn how many a run makes.
	pub fn counting(inner: M) -> Self {
		FaultMachine {
			inner,
			fail_at: None,
			count: AtomicUsize::new(0),
		}
	}

	pub fn calls(&self) -> usize {
		self.count.load(Ordering::Relaxed)
	}

	fn tick(&self, path: &Path) -> io::Result<()> {
		if path.starts_with(&self.inner.dirs().temp) {
			return Ok(());
		}
		let n = self.count.fetch_add(1, Ordering::Relaxed);
		if self.fail_at == Some(n) {
			return Err(io::Error::other(format!("injected fault at call {n}")));
		}
		Ok(())
	}
}

impl<M: Machine> Machine for FaultMachine<M> {
	fn dirs(&self) -> &Dirs {
		self.inner.dirs()
	}
	fn os(&self) -> &str {
		self.inner.os()
	}
	fn arch(&self) -> &str {
		self.inner.arch()
	}
	fn kind(&self, path: &Path) -> Option<NodeKind> {
		self.inner.kind(path)
	}
	fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
		self.inner.read(path)
	}
	fn list(&self, path: &Path) -> io::Result<Vec<PathBuf>> {
		self.inner.list(path)
	}
	fn is_executable(&self, path: &Path) -> bool {
		self.inner.is_executable(path)
	}
	fn write(&mut self, path: &Path, data: &[u8], executable: bool) -> io::Result<()> {
		self.tick(path)?;
		self.inner.write(path, data, executable)
	}
	fn append(&mut self, path: &Path, data: &[u8]) -> io::Result<()> {
		self.tick(path)?;
		self.inner.append(path, data)
	}
	fn create_dir(&mut self, path: &Path) -> io::Result<()> {
		self.tick(path)?;
		self.inner.create_dir(path)
	}
	fn remove_file(&mut self, path: &Path) -> io::Result<()> {
		self.tick(path)?;
		self.inner.remove_file(path)
	}
	fn remove_dir(&mut self, path: &Path) -> io::Result<()> {
		self.tick(path)?;
		self.inner.remove_dir(path)
	}
	fn free_space(&self, path: &Path) -> u64 {
		self.inner.free_space(path)
	}
	fn registry_get(&self, key: &str) -> Option<String> {
		self.inner.registry_get(key)
	}
	fn registry_set(&mut self, key: &str, value: Option<&str>) -> io::Result<()> {
		self.tick(Path::new("/registry"))?;
		self.inner.registry_set(key, value)
	}
	fn launch(&mut self, path: &Path, args: &[String]) -> io::Result<()> {
		self.inner.launch(path, args)
	}
	fn run(&mut self, program: &Path, args: &[String]) -> io::Result<i32> {
		self.tick(Path::new("/run"))?;
		self.inner.run(program, args)
	}
	fn script_fs(&self) -> Arc<dyn ScriptFs> {
		self.inner.script_fs()
	}
}
