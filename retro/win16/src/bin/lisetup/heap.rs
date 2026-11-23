// The global allocator: a first-fit free list over a static array in the data segment.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Each program in this folder uses part of this module, so what one does not use is not dead code.
#![allow(dead_code)]
//! A Win16 program owns one 64 KB data segment (DS = SS), so the heap is a fixed array in it: the
//! packer leaves the zero tail out of the file (the loader zero-fills up to the segment's minimum
//! size). Each block has an eight-byte header holding its length; free blocks form a list sorted by
//! address so neighbours merge when one is freed. Single task, no threads, not reentrant: nothing
//! may allocate from a window procedure that can run during an allocation.
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::null_mut;

/// Bytes of heap. `tools/size_report.py` prints how much of the segment is left.
pub const HEAP_SIZE: usize = 40 * 1024;
const HEADER: usize = 8;
const MIN_NODE: usize = 16;

#[repr(C)]
struct Node {
	size: usize,
	next: *mut Node,
}

#[repr(C, align(8))]
struct Arena([u8; HEAP_SIZE]);

struct State {
	arena: Arena,
	free: *mut Node,
	ready: bool,
	/// The most bytes in use at once, for the size report.
	used: usize,
	peak: usize,
}

pub struct Heap(UnsafeCell<State>);

// SAFETY: the program is single-threaded; the type exists to be a `static`.
unsafe impl Sync for Heap {}

#[global_allocator]
pub static HEAP: Heap = Heap(UnsafeCell::new(State {
	arena: Arena([0; HEAP_SIZE]),
	free: null_mut(),
	ready: false,
	used: 0,
	peak: 0,
}));

fn round_up(x: usize, a: usize) -> usize {
	(x + a - 1) & !(a - 1)
}

impl Heap {
	/// The most bytes the program has held at once, headers included.
	pub fn peak(&self) -> usize {
		unsafe { (*self.0.get()).peak }
	}

	unsafe fn init(st: &mut State) {
		let n = st.arena.0.as_mut_ptr() as *mut Node;
		unsafe {
			(*n).size = HEAP_SIZE;
			(*n).next = null_mut();
		}
		st.free = n;
		st.ready = true;
	}

	/// Puts `[start, start + size)` back in the list, merging with the blocks either side.
	unsafe fn insert(st: &mut State, start: usize, size: usize) {
		let mut prev: *mut Node = null_mut();
		let mut cur = st.free;
		unsafe {
			while !cur.is_null() && (cur as usize) < start {
				prev = cur;
				cur = (*cur).next;
			}
			let node = start as *mut Node;
			(*node).size = size;
			(*node).next = cur;
			if !cur.is_null() && start + size == cur as usize {
				(*node).size += (*cur).size;
				(*node).next = (*cur).next;
			}
			if prev.is_null() {
				st.free = node;
			} else if prev as usize + (*prev).size == start {
				(*prev).size += (*node).size;
				(*prev).next = (*node).next;
			} else {
				(*prev).next = node;
			}
		}
	}
}

unsafe impl GlobalAlloc for Heap {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		let st = unsafe { &mut *self.0.get() };
		if !st.ready {
			unsafe { Heap::init(st) };
		}
		let align = layout.align().max(8);
		let want = round_up(layout.size().max(8) + HEADER + align, 8);
		let mut prev: *mut Node = null_mut();
		let mut cur = st.free;
		unsafe {
			while !cur.is_null() {
				if (*cur).size >= want {
					let start = cur as usize;
					let take = if (*cur).size - want >= MIN_NODE {
						want
					} else {
						(*cur).size
					};
					let next = if (*cur).size - take >= MIN_NODE {
						let rest = (start + take) as *mut Node;
						(*rest).size = (*cur).size - take;
						(*rest).next = (*cur).next;
						rest
					} else {
						(*cur).next
					};
					if prev.is_null() {
						st.free = next;
					} else {
						(*prev).next = next;
					}
					let user = round_up(start + HEADER, align);
					*((user - 8) as *mut usize) = start;
					*((user - 4) as *mut usize) = take;
					st.used += take;
					st.peak = st.peak.max(st.used);
					return user as *mut u8;
				}
				prev = cur;
				cur = (*cur).next;
			}
		}
		null_mut()
	}

	unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
		let st = unsafe { &mut *self.0.get() };
		unsafe {
			let start = *((ptr as usize - 8) as *const usize);
			let size = *((ptr as usize - 4) as *const usize);
			st.used -= size;
			Heap::insert(st, start, size);
		}
	}
}
