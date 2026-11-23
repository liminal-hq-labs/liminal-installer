// Progress events for the three meters: current file, total progress and free space.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
	Plan,
	Stage,
	Commit,
	Record,
}

/// One event. A renderer's `meter` nodes read: `file` from `current_file`, `total` from
/// `fraction()` and `free_space` from `free_space` (with `low_space` for the "Low" warning).
#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
	pub phase: Phase,
	/// The destination of the file being copied right now, as a placeholder path.
	pub current_file: Option<String>,
	pub files_done: u64,
	pub files_total: u64,
	pub bytes_done: u64,
	pub bytes_total: u64,
	/// Free bytes on the install volume.
	pub free_space: u64,
	/// True when finishing would leave less than 5% of the volume's current free space, or less
	/// than 1 MiB.
	pub low_space: bool,
}

impl Progress {
	/// Overall progress from 0.0 to 1.0.
	pub fn fraction(&self) -> f64 {
		if self.bytes_total > 0 {
			self.bytes_done as f64 / self.bytes_total as f64
		} else if self.files_total > 0 {
			self.files_done as f64 / self.files_total as f64
		} else {
			1.0
		}
	}
}

/// Controls for a running lifecycle operation.
#[derive(Default)]
pub struct RunOptions<'a> {
	/// Set to cancel; the commit stops at the next operation and rolls back.
	pub cancel: Option<Arc<AtomicBool>>,
	/// Called for every event. Return `false` to cancel.
	pub on_progress: Option<&'a mut dyn FnMut(&Progress) -> bool>,
}

impl RunOptions<'_> {
	pub(crate) fn cancelled(&self) -> bool {
		self.cancel
			.as_ref()
			.is_some_and(|c| c.load(Ordering::Relaxed))
	}

	pub(crate) fn emit(&mut self, p: &Progress) -> bool {
		match self.on_progress.as_mut() {
			Some(f) => f(p),
			None => true,
		}
	}
}
