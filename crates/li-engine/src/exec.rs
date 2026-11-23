// Commit and Record: runs operations behind a journal and rolls back on failure.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Every change is written to the journal before it is made. The journal lives in
//! `{temp}/liminal-installer/<product>/` as `journal.jsonl` plus one `.bak` file for each file
//! the run replaced or removed. If the run fails or is cancelled, the journal is replayed in
//! reverse. A journal found at the start of the next run (a crash) is replayed the same way.

use std::path::{Path, PathBuf};

use li_model::{Operation, Receipt};
use serde::{Deserialize, Serialize};

use crate::error::{EngineError, Result};
use crate::machine::{Machine, NodeKind, Resolver};
use crate::payload::Payload;
use crate::progress::{Phase, Progress, RunOptions};

/// One thing a lifecycle command does.
#[derive(Debug, Clone)]
pub(crate) enum Step {
	Apply(Operation),
	/// Undoes what a recorded operation did.
	Revert(Operation),
	WriteReceipt(Box<Receipt>),
	RemoveReceipt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "undo", rename_all = "snake_case")]
enum Undo {
	DeleteFile {
		path: PathBuf,
	},
	RemoveDir {
		path: PathBuf,
	},
	CreateDir {
		path: PathBuf,
	},
	RestoreFile {
		path: PathBuf,
		backup: PathBuf,
		executable: bool,
	},
	Registry {
		key: String,
		previous: Option<String>,
	},
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> EngineError + '_ {
	move |e| EngineError::io(path, e)
}

fn journal_dir(m: &dyn Machine, product: &str) -> PathBuf {
	let safe: String = product
		.chars()
		.map(|c| {
			if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
				c
			} else {
				'_'
			}
		})
		.collect();
	m.dirs().temp.join("liminal-installer").join(safe)
}

struct Journal {
	dir: PathBuf,
	entries: Vec<Undo>,
	seq: u64,
}

impl Journal {
	fn file(&self) -> PathBuf {
		self.dir.join("journal.jsonl")
	}

	fn create(m: &mut dyn Machine, product: &str) -> Result<Journal> {
		let dir = journal_dir(m, product);
		crate::machine::create_dir_all(m, &dir).map_err(io_err(&dir))?;
		Ok(Journal {
			dir,
			entries: Vec::new(),
			seq: 0,
		})
	}

	fn load(m: &dyn Machine, product: &str) -> Result<Option<Journal>> {
		let dir = journal_dir(m, product);
		let file = dir.join("journal.jsonl");
		let Ok(bytes) = m.read(&file) else {
			return Ok(None);
		};
		let mut entries = Vec::new();
		for line in String::from_utf8_lossy(&bytes).lines() {
			// A torn last line from a crash mid-write is ignored: nothing ran after it.
			if let Ok(u) = serde_json::from_str::<Undo>(line) {
				entries.push(u);
			}
		}
		Ok(Some(Journal {
			dir,
			seq: entries.len() as u64,
			entries,
		}))
	}

	fn backup(&mut self, m: &mut dyn Machine, data: &[u8]) -> Result<PathBuf> {
		self.seq += 1;
		let path = self.dir.join(format!("{}.bak", self.seq));
		m.write(&path, data, false).map_err(io_err(&path))?;
		Ok(path)
	}

	fn record(&mut self, m: &mut dyn Machine, undo: Undo) -> Result<()> {
		let mut line = serde_json::to_vec(&undo).map_err(|e| EngineError::Other(e.to_string()))?;
		line.push(b'\n');
		let file = self.file();
		m.append(&file, &line).map_err(io_err(&file))?;
		self.entries.push(undo);
		Ok(())
	}

	/// Replays the journal in reverse. Returns what could not be undone.
	fn rollback(&mut self, m: &mut dyn Machine) -> Vec<String> {
		let mut errors = Vec::new();
		while let Some(undo) = self.entries.pop() {
			if let Err(e) = apply_undo(m, &undo) {
				errors.push(e.to_string());
			}
		}
		errors
	}

	/// Removes the journal directory and, if empty, its parent.
	fn discard(self, m: &mut dyn Machine) {
		if let Ok(children) = m.list(&self.dir) {
			for c in children {
				let _ = m.remove_file(&c);
			}
		}
		let _ = m.remove_dir(&self.dir);
		if let Some(parent) = self.dir.parent() {
			let _ = m.remove_dir(parent);
		}
	}
}

fn apply_undo(m: &mut dyn Machine, undo: &Undo) -> Result<()> {
	match undo {
		Undo::DeleteFile { path } => {
			if m.kind(path) == Some(NodeKind::File) {
				m.remove_file(path).map_err(io_err(path))?;
			}
		}
		Undo::RemoveDir { path } => {
			if m.kind(path) == Some(NodeKind::Dir) {
				m.remove_dir(path).map_err(io_err(path))?;
			}
		}
		Undo::CreateDir { path } => {
			if m.kind(path).is_none() {
				m.create_dir(path).map_err(io_err(path))?;
			}
		}
		Undo::RestoreFile {
			path,
			backup,
			executable,
		} => {
			let data = m.read(backup).map_err(io_err(backup))?;
			m.write(path, &data, *executable).map_err(io_err(path))?;
		}
		Undo::Registry { key, previous } => {
			m.registry_set(key, previous.as_deref())
				.map_err(|e| EngineError::Other(format!("registry `{key}`: {e}")))?;
		}
	}
	Ok(())
}

/// Replays and removes a journal left by a crashed run. Returns true if there was one.
pub fn recover(m: &mut dyn Machine, product: &str) -> Result<bool> {
	let Some(mut journal) = Journal::load(m, product)? else {
		return Ok(false);
	};
	let errors = journal.rollback(m);
	if !errors.is_empty() {
		return Err(EngineError::RollbackFailed {
			cause: Box::new(EngineError::Other("an earlier run did not finish".into())),
			rollback: errors,
		});
	}
	journal.discard(m);
	Ok(true)
}

fn slug(name: &str) -> String {
	let s: String = name
		.chars()
		.map(|c| {
			if c.is_ascii_alphanumeric() {
				c.to_ascii_lowercase()
			} else {
				'-'
			}
		})
		.collect();
	s.trim_matches('-').to_owned()
}

fn desktop_entry(name: &str, target: &Path) -> String {
	let exec = target
		.to_string_lossy()
		.replace('\\', "\\\\")
		.replace('"', "\\\"");
	format!(
		"[Desktop Entry]\nType=Application\nVersion=1.0\nName={name}\nExec=\"{exec}\"\nTerminal=false\n"
	)
}

fn ini_set(text: &str, section: &str, key: &str, value: &str) -> String {
	let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
	let header = format!("[{section}]");
	let line = format!("{key}={value}");
	let start = if section.is_empty() {
		Some(0)
	} else {
		lines.iter().position(|l| l.trim() == header).map(|i| i + 1)
	};
	let Some(start) = start else {
		if !lines.is_empty() && !lines.last().is_some_and(String::is_empty) {
			lines.push(String::new());
		}
		lines.push(header);
		lines.push(line);
		return lines.join("\n") + "\n";
	};
	let mut end = lines.len();
	for (i, l) in lines.iter().enumerate().skip(start) {
		let t = l.trim();
		if t.starts_with('[') && t.ends_with(']') {
			end = i;
			break;
		}
	}
	for l in lines.iter_mut().take(end).skip(start) {
		if l.split_once('=').is_some_and(|(k, _)| k.trim() == key) {
			*l = line;
			return lines.join("\n") + "\n";
		}
	}
	let mut at = end;
	while at > start && lines[at - 1].trim().is_empty() {
		at -= 1;
	}
	lines.insert(at, line);
	lines.join("\n") + "\n"
}

fn ini_unset(text: &str, section: &str, key: &str) -> String {
	let header = format!("[{section}]");
	let mut out = Vec::new();
	let mut in_section = section.is_empty();
	for l in text.lines() {
		let t = l.trim();
		if t.starts_with('[') && t.ends_with(']') {
			in_section = t == header;
		} else if in_section && l.split_once('=').is_some_and(|(k, _)| k.trim() == key) {
			continue;
		}
		out.push(l);
	}
	// Drop a section header left with nothing under it.
	let mut cleaned: Vec<&str> = Vec::new();
	for (i, l) in out.iter().enumerate() {
		let t = l.trim();
		let is_header = t.starts_with('[') && t.ends_with(']');
		if is_header {
			let next = out.get(i + 1).map(|n| n.trim());
			if next.is_none_or(|n| n.is_empty() || (n.starts_with('[') && n.ends_with(']')))
				&& t == header
			{
				continue;
			}
		}
		cleaned.push(l);
	}
	let joined = cleaned.join("\n");
	if joined.trim().is_empty() {
		String::new()
	} else {
		joined + "\n"
	}
}

fn json_set(root: &mut serde_json::Value, pointer: &str, value: serde_json::Value) -> Result<()> {
	let parts: Vec<String> = pointer
		.split('/')
		.skip(1)
		.map(|p| p.replace("~1", "/").replace("~0", "~"))
		.collect();
	if parts.is_empty() {
		*root = value;
		return Ok(());
	}
	let mut cur = root;
	for part in &parts[..parts.len() - 1] {
		if !cur.is_object() {
			*cur = serde_json::json!({});
		}
		cur = cur
			.as_object_mut()
			.expect("object")
			.entry(part.clone())
			.or_insert_with(|| serde_json::json!({}));
	}
	if !cur.is_object() {
		*cur = serde_json::json!({});
	}
	cur.as_object_mut()
		.expect("object")
		.insert(parts[parts.len() - 1].clone(), value);
	Ok(())
}

fn json_unset(root: &mut serde_json::Value, pointer: &str) {
	let parts: Vec<String> = pointer
		.split('/')
		.skip(1)
		.map(|p| p.replace("~1", "/").replace("~0", "~"))
		.collect();
	let Some((last, init)) = parts.split_last() else {
		return;
	};
	let mut cur = root;
	for part in init {
		match cur.get_mut(part.as_str()) {
			Some(next) => cur = next,
			None => return,
		}
	}
	if let Some(o) = cur.as_object_mut() {
		o.remove(last);
	}
}

/// A transaction: steps run one at a time against a journal, then it is committed or aborted.
pub(crate) struct Session<'a, 'o> {
	m: &'a mut dyn Machine,
	res: &'a Resolver,
	payload: Option<&'a dyn Payload>,
	journal: Journal,
	opts: &'a mut RunOptions<'o>,
	progress: Progress,
}

pub(crate) struct Totals {
	pub files: u64,
	pub bytes: u64,
}

impl<'a, 'o> Session<'a, 'o> {
	pub(crate) fn begin(
		m: &'a mut dyn Machine,
		res: &'a Resolver,
		payload: Option<&'a dyn Payload>,
		product: &str,
		totals: Totals,
		opts: &'a mut RunOptions<'o>,
	) -> Result<Self> {
		recover(m, product)?;
		let journal = Journal::create(m, product)?;
		let free = m.free_space(&res.install);
		Ok(Session {
			m,
			res,
			payload,
			journal,
			opts,
			progress: Progress {
				phase: Phase::Commit,
				current_file: None,
				files_done: 0,
				files_total: totals.files,
				bytes_done: 0,
				bytes_total: totals.bytes,
				free_space: free,
				low_space: false,
			},
		})
	}

	fn emit(&mut self) -> Result<()> {
		self.progress.free_space = self.m.free_space(&self.res.install);
		let remaining = self
			.progress
			.bytes_total
			.saturating_sub(self.progress.bytes_done);
		let left = self.progress.free_space.saturating_sub(remaining);
		self.progress.low_space = left < 1 << 20;
		let p = self.progress.clone();
		if !self.opts.emit(&p) {
			return Err(EngineError::Cancelled);
		}
		Ok(())
	}

	/// Runs one step and returns the operations it effectively performed, in placeholder form.
	pub(crate) fn step(&mut self, step: &Step) -> Result<Vec<Operation>> {
		if self.opts.cancelled() {
			return Err(EngineError::Cancelled);
		}
		self.progress.phase = match step {
			Step::WriteReceipt(_) => Phase::Record,
			_ => Phase::Commit,
		};
		let mut done = Vec::new();
		match step {
			Step::Apply(op) => self.apply(op, &mut done)?,
			Step::Revert(op) => self.revert(op)?,
			Step::WriteReceipt(r) => {
				let path = self.res.resolve(&crate::plan::receipt_path())?;
				if let Some(dir) = path.parent() {
					self.ensure_dir(dir, &mut Vec::new())?;
				}
				let json = serde_json::to_vec_pretty(&**r)
					.map_err(|e| EngineError::Other(e.to_string()))?;
				self.put(&path, &json, false)?;
				self.progress.current_file = None;
				self.emit()?;
			}
			Step::RemoveReceipt => {
				let path = self.res.resolve(&crate::plan::receipt_path())?;
				self.delete(&path)?;
				if let Some(dir) = path.parent() {
					self.rmdir(dir)?;
				}
			}
		}
		Ok(done)
	}

	/// The machine as the transaction has left it so far.
	pub(crate) fn machine(&self) -> &dyn Machine {
		&*self.m
	}

	/// Keeps the changes and removes the journal.
	pub(crate) fn finish(self) {
		self.journal.discard(self.m);
	}

	/// Rolls everything back and converts the error.
	pub(crate) fn abort(mut self, cause: EngineError) -> EngineError {
		let errors = self.journal.rollback(self.m);
		if !errors.is_empty() {
			return EngineError::RollbackFailed {
				cause: Box::new(cause),
				rollback: errors,
			};
		}
		self.journal.discard(self.m);
		match cause {
			EngineError::Cancelled => EngineError::Cancelled,
			other => EngineError::RolledBack(Box::new(other)),
		}
	}

	// Journalled primitives.

	fn put(&mut self, path: &Path, data: &[u8], executable: bool) -> Result<()> {
		let undo = match self.m.kind(path) {
			Some(NodeKind::File) => {
				let old = self.m.read(path).map_err(io_err(path))?;
				let backup = self.journal.backup(self.m, &old)?;
				Undo::RestoreFile {
					path: path.to_owned(),
					backup,
					executable: self.m.is_executable(path),
				}
			}
			Some(NodeKind::Dir) => {
				return Err(EngineError::Other(format!(
					"{} is a directory",
					path.display()
				)));
			}
			None => Undo::DeleteFile {
				path: path.to_owned(),
			},
		};
		self.journal.record(self.m, undo)?;
		self.m.write(path, data, executable).map_err(io_err(path))
	}

	fn delete(&mut self, path: &Path) -> Result<()> {
		if self.m.kind(path) != Some(NodeKind::File) {
			return Ok(());
		}
		let old = self.m.read(path).map_err(io_err(path))?;
		let backup = self.journal.backup(self.m, &old)?;
		let executable = self.m.is_executable(path);
		self.journal.record(
			self.m,
			Undo::RestoreFile {
				path: path.to_owned(),
				backup,
				executable,
			},
		)?;
		self.m.remove_file(path).map_err(io_err(path))
	}

	fn mkdir_one(&mut self, path: &Path) -> Result<()> {
		self.journal.record(
			self.m,
			Undo::RemoveDir {
				path: path.to_owned(),
			},
		)?;
		self.m.create_dir(path).map_err(io_err(path))
	}

	/// Removes a directory if it exists and is empty. A directory with other things in it is
	/// not ours to remove.
	fn rmdir(&mut self, path: &Path) -> Result<()> {
		if self.m.kind(path) != Some(NodeKind::Dir) {
			return Ok(());
		}
		if !self.m.list(path).map_err(io_err(path))?.is_empty() {
			return Ok(());
		}
		self.journal.record(
			self.m,
			Undo::CreateDir {
				path: path.to_owned(),
			},
		)?;
		self.m.remove_dir(path).map_err(io_err(path))
	}

	fn set_registry(&mut self, key: &str, value: Option<&str>) -> Result<()> {
		let previous = self.m.registry_get(key);
		self.journal.record(
			self.m,
			Undo::Registry {
				key: key.to_owned(),
				previous,
			},
		)?;
		self.m
			.registry_set(key, value)
			.map_err(|e| EngineError::Other(format!("registry `{key}`: {e}")))
	}

	/// Creates `dir` and any missing parents, journalling each. Pushes the placeholder form of
	/// what it created onto `created`.
	fn ensure_dir(&mut self, dir: &Path, created: &mut Vec<String>) -> Result<()> {
		let mut missing = Vec::new();
		let mut cur = dir.to_owned();
		loop {
			match self.m.kind(&cur) {
				Some(NodeKind::Dir) => break,
				Some(NodeKind::File) => {
					return Err(EngineError::Other(format!("{} is a file", cur.display())));
				}
				None => {}
			}
			missing.push(cur.clone());
			match cur.parent() {
				Some(p) if !p.as_os_str().is_empty() && p != Path::new("/") => cur = p.to_owned(),
				_ => break,
			}
		}
		for d in missing.into_iter().rev() {
			self.mkdir_one(&d)?;
			created.push(self.res.unresolve(&d));
		}
		Ok(())
	}

	fn shortcut_path(&self, name: &str, place: &str) -> Result<PathBuf> {
		let place = if place.starts_with('{') {
			place.to_owned()
		} else {
			format!("{{{place}}}")
		};
		Ok(self
			.res
			.resolve(&place)?
			.join(format!("{}.desktop", slug(name))))
	}

	fn registry_entry(op: &Operation) -> Option<String> {
		match op {
			Operation::FileType { .. }
			| Operation::UrlScheme { .. }
			| Operation::RegistrySet { .. }
			| Operation::PathAdd { .. }
			| Operation::ServiceInstall { .. }
			| Operation::OsRegister { .. } => Some(crate::plan::op_key(op)),
			_ => None,
		}
	}

	fn apply(&mut self, op: &Operation, done: &mut Vec<Operation>) -> Result<()> {
		match op {
			Operation::Mkdir { path } => {
				let dir = self.res.resolve(path)?;
				let mut created = Vec::new();
				self.ensure_dir(&dir, &mut created)?;
				done.extend(created.into_iter().map(|path| Operation::Mkdir { path }));
			}
			Operation::Copy { from, to, sha256 } => {
				let payload = self
					.payload
					.ok_or_else(|| EngineError::MissingSource(from.clone()))?;
				let dest = self.res.resolve(to)?;
				self.progress.current_file = Some(to.clone());
				self.emit()?;
				let data = payload.read(from)?;
				let actual = li_pack::sha256_hex(&data);
				if let Some(expected) = sha256
					&& !actual.eq_ignore_ascii_case(expected)
				{
					return Err(EngineError::HashMismatch {
						what: from.clone(),
						expected: expected.clone(),
						actual,
					});
				}
				self.put(&dest, &data, payload.executable(from))?;
				self.progress.files_done += 1;
				self.progress.bytes_done += data.len() as u64;
				self.emit()?;
				done.push(Operation::Copy {
					from: from.clone(),
					to: to.clone(),
					sha256: Some(actual),
				});
			}
			Operation::WriteFile { path, contents } => {
				let dest = self.res.resolve(path)?;
				self.put(&dest, contents.as_bytes(), false)?;
				done.push(op.clone());
			}
			Operation::WriteIni {
				path,
				section,
				key,
				value,
			} => {
				let dest = self.res.resolve(path)?;
				let old = self
					.m
					.read(&dest)
					.map(|b| String::from_utf8_lossy(&b).into_owned())
					.unwrap_or_default();
				self.put(&dest, ini_set(&old, section, key, value).as_bytes(), false)?;
				done.push(op.clone());
			}
			Operation::WriteJson {
				path,
				pointer,
				value,
			} => {
				let dest = self.res.resolve(path)?;
				let mut doc = match self.m.read(&dest) {
					Ok(b) => serde_json::from_slice(&b)
						.map_err(|e| EngineError::Other(format!("{}: {e}", dest.display())))?,
					Err(_) => serde_json::json!({}),
				};
				json_set(&mut doc, pointer, value.clone())?;
				let out = serde_json::to_vec_pretty(&doc)
					.map_err(|e| EngineError::Other(e.to_string()))?;
				self.put(&dest, &out, false)?;
				done.push(op.clone());
			}
			Operation::Shortcut {
				name,
				target,
				place,
			} => {
				let file = self.shortcut_path(name, place)?;
				if let Some(dir) = file.parent() {
					let mut created = Vec::new();
					self.ensure_dir(dir, &mut created)?;
					done.extend(created.into_iter().map(|path| Operation::Mkdir { path }));
				}
				let target = self.res.resolve(target)?;
				self.put(&file, desktop_entry(name, &target).as_bytes(), true)?;
				done.push(op.clone());
			}
			Operation::ReceiptWrite { .. } => {
				return Err(EngineError::Other(
					"receipt.write is handled by the Record phase".into(),
				));
			}
			other => {
				let key = Self::registry_entry(other).expect("registry operation");
				let value =
					serde_json::to_string(other).map_err(|e| EngineError::Other(e.to_string()))?;
				self.set_registry(&key, Some(&value))?;
				done.push(other.clone());
			}
		}
		Ok(())
	}

	fn revert(&mut self, op: &Operation) -> Result<()> {
		match op {
			Operation::Mkdir { path } => {
				let dir = self.res.resolve(path)?;
				self.rmdir(&dir)?;
			}
			Operation::Copy { to, .. } => {
				let p = self.res.resolve(to)?;
				self.delete(&p)?;
			}
			Operation::WriteFile { path, .. } => {
				let p = self.res.resolve(path)?;
				self.delete(&p)?;
			}
			Operation::WriteIni {
				path, section, key, ..
			} => {
				let p = self.res.resolve(path)?;
				if let Ok(b) = self.m.read(&p) {
					let next = ini_unset(&String::from_utf8_lossy(&b), section, key);
					if next.trim().is_empty() {
						self.delete(&p)?;
					} else {
						self.put(&p, next.as_bytes(), false)?;
					}
				}
			}
			Operation::WriteJson { path, pointer, .. } => {
				let p = self.res.resolve(path)?;
				if let Ok(b) = self.m.read(&p)
					&& let Ok(mut doc) = serde_json::from_slice::<serde_json::Value>(&b)
				{
					json_unset(&mut doc, pointer);
					let empty = doc.as_object().is_some_and(|o| o.is_empty());
					if empty {
						self.delete(&p)?;
					} else {
						let out = serde_json::to_vec_pretty(&doc)
							.map_err(|e| EngineError::Other(e.to_string()))?;
						self.put(&p, &out, false)?;
					}
				}
			}
			Operation::Shortcut { name, place, .. } => {
				let p = self.shortcut_path(name, place)?;
				self.delete(&p)?;
			}
			Operation::ReceiptWrite { .. } => {}
			other => {
				let key = Self::registry_entry(other).expect("registry operation");
				if self.m.registry_get(&key).is_some() {
					self.set_registry(&key, None)?;
				}
			}
		}
		Ok(())
	}
}

/// Whether the machine already shows what `op` should have produced. Used by repair and by
/// update to skip work that is already done.
pub(crate) fn is_intact(m: &dyn Machine, res: &Resolver, op: &Operation) -> bool {
	let check = || -> Result<bool> {
		Ok(match op {
			Operation::Mkdir { path } => m.kind(&res.resolve(path)?) == Some(NodeKind::Dir),
			Operation::Copy { to, sha256, .. } => {
				let p = res.resolve(to)?;
				match (m.read(&p), sha256) {
					(Ok(data), Some(s)) => li_pack::sha256_hex(&data).eq_ignore_ascii_case(s),
					(Ok(_), None) => true,
					(Err(_), _) => false,
				}
			}
			Operation::WriteFile { path, contents } => m
				.read(&res.resolve(path)?)
				.is_ok_and(|d| d == contents.as_bytes()),
			Operation::WriteIni { path, .. } | Operation::WriteJson { path, .. } => {
				m.kind(&res.resolve(path)?) == Some(NodeKind::File)
			}
			Operation::Shortcut { name, place, .. } => {
				let place = if place.starts_with('{') {
					place.clone()
				} else {
					format!("{{{place}}}")
				};
				m.kind(&res.resolve(&place)?.join(format!("{}.desktop", slug(name))))
					== Some(NodeKind::File)
			}
			Operation::ReceiptWrite { .. } => true,
			other => Session::registry_entry(other).is_some_and(|k| m.registry_get(&k).is_some()),
		})
	};
	check().unwrap_or(false)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn ini_edits() {
		let t = ini_set("", "main", "a", "1");
		assert_eq!(t, "[main]\na=1\n");
		let t = ini_set(&t, "main", "b", "2");
		assert_eq!(t, "[main]\na=1\nb=2\n");
		let t = ini_set(&t, "main", "a", "9");
		assert_eq!(t, "[main]\na=9\nb=2\n");
		let t = ini_set(&t, "other", "c", "3");
		assert_eq!(t, "[main]\na=9\nb=2\n\n[other]\nc=3\n");
		let t = ini_unset(&t, "other", "c");
		assert_eq!(t, "[main]\na=9\nb=2\n\n");
		let t = ini_unset(&ini_unset(&t, "main", "a"), "main", "b");
		assert_eq!(t, "");
	}

	#[test]
	fn json_edits() {
		let mut v = serde_json::json!({});
		json_set(&mut v, "/a/b", serde_json::json!(1)).unwrap();
		json_set(&mut v, "/c", serde_json::json!("x")).unwrap();
		assert_eq!(v, serde_json::json!({"a": {"b": 1}, "c": "x"}));
		json_unset(&mut v, "/a/b");
		assert_eq!(v, serde_json::json!({"a": {}, "c": "x"}));
	}
}
