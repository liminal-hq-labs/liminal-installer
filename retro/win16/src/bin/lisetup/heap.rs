// The global allocator: a first-fit free list over a static array in the data segment.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Each program in this folder uses part of this module, so what one does not use is not dead code.
#![allow(dead_code)]
//! A Win16 program owns one 64 KB data segment (DS = SS), so the heap is a fixed array in it: the
//! loader zero-fills up to the segment's minimum size. Blocks are 4-byte aligned and start with a
//! four-byte header holding the block's total size, header included (a multiple of four, so bit 0
//! is free for a flag). A free block holds its size and the arena offset of the next free block, so
//! the smallest block is eight bytes. The free list is sorted by address so neighbours merge when
//! one is freed. A block for an alignment over four keeps its start in the word before the header
//! and sets bit 0 of the header. `realloc` shrinks in place, grows into a free neighbour, and only
//! then moves. The allocator also counts live blocks and the bytes asked for, so the log can say
//! what the headers cost. Single task, no threads, not reentrant: nothing may allocate from a
//! window procedure that can run during an allocation.
use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr::null_mut;

/// Bytes of heap.
pub const HEAP_SIZE: usize = 36 * 1024;
const HEADER: usize = 4;
const MIN_BLOCK: usize = 8;
/// The end of the free list. Offsets are into the arena, so a link is four bytes on every target.
const NIL: u32 = u32::MAX;

#[repr(C, align(8))]
struct Arena([u8; HEAP_SIZE]);

struct State {
	arena: Arena,
	free: u32,
	ready: bool,
	/// Bytes in use now and the most at once, headers included.
	used: usize,
	peak: usize,
	/// Live blocks, the bytes their owners asked for, and both at the moment `used` peaked.
	blocks: usize,
	asked: usize,
	peak_blocks: usize,
	peak_asked: usize,
	most_blocks: usize,
}

pub struct Heap(UnsafeCell<State>);

// SAFETY: the program is single-threaded; the type exists to be a `static`.
unsafe impl Sync for Heap {}

#[cfg(not(test))]
#[global_allocator]
pub static HEAP: Heap = Heap::new();

fn round_up(x: usize, a: usize) -> usize {
	(x + a - 1) & !(a - 1)
}

impl State {
	fn base(&mut self) -> *mut u8 {
		self.arena.0.as_mut_ptr()
	}

	unsafe fn rd(&mut self, off: usize) -> usize {
		unsafe { (self.base().add(off) as *const u32).read() as usize }
	}

	unsafe fn wr(&mut self, off: usize, v: usize) {
		unsafe { (self.base().add(off) as *mut u32).write(v as u32) }
	}

	/// Notes a rise in use, for the peak and the census.
	fn note(&mut self) {
		if self.used > self.peak {
			self.peak = self.used;
			self.peak_blocks = self.blocks;
			self.peak_asked = self.asked;
		}
		self.most_blocks = self.most_blocks.max(self.blocks);
	}

	unsafe fn init(&mut self) {
		unsafe {
			self.wr(0, HEAP_SIZE);
			self.wr(4, NIL as usize);
		}
		self.free = 0;
		self.ready = true;
	}

	/// Puts `[start, start + size)` back in the list, merging with the blocks either side.
	unsafe fn insert(&mut self, start: usize, mut size: usize) {
		let mut prev = NIL as usize;
		let mut cur = self.free as usize;
		unsafe {
			while cur != NIL as usize && cur < start {
				prev = cur;
				cur = self.rd(cur + 4);
			}
			let mut next = cur;
			if cur != NIL as usize && start + size == cur {
				size += self.rd(cur);
				next = self.rd(cur + 4);
			}
			if prev != NIL as usize && prev + self.rd(prev) == start {
				let merged = self.rd(prev) + size;
				self.wr(prev, merged);
				self.wr(prev + 4, next);
			} else {
				self.wr(start, size);
				self.wr(start + 4, next);
				if prev == NIL as usize {
					self.free = start as u32;
				} else {
					self.wr(prev + 4, start);
				}
			}
		}
	}

	/// Takes at least `want` bytes off the first free block that holds them: the block's offset and
	/// the size taken (the whole block when the rest would be under the smallest block).
	unsafe fn take(&mut self, want: usize) -> Option<(usize, usize)> {
		let mut prev = NIL as usize;
		let mut cur = self.free as usize;
		unsafe {
			while cur != NIL as usize {
				let size = self.rd(cur);
				let next = self.rd(cur + 4);
				if size >= want {
					let take = if size - want >= MIN_BLOCK { want } else { size };
					let after = if take < size {
						self.wr(cur + take, size - take);
						self.wr(cur + take + 4, next);
						cur + take
					} else {
						next
					};
					if prev == NIL as usize {
						self.free = after as u32;
					} else {
						self.wr(prev + 4, after);
					}
					return Some((cur, take));
				}
				prev = cur;
				cur = next;
			}
		}
		None
	}

	/// The block's offset, its total size and its user pointer's header word for `ptr`.
	unsafe fn locate(&mut self, ptr: *mut u8) -> (usize, usize) {
		unsafe {
			let header = (ptr as *const u32).sub(1).read() as usize;
			let size = header & !3;
			let start = if header & 1 != 0 {
				(ptr as *const u32).sub(2).read() as usize
			} else {
				ptr as usize - HEADER - self.base() as usize
			};
			(start, size)
		}
	}

	unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
		if !self.ready {
			unsafe { self.init() };
		}
		let align = layout.align();
		let size = layout.size();
		if size > HEAP_SIZE {
			return null_mut();
		}
		let want = if align <= 4 {
			size.max(4) + HEADER
		} else {
			size + 8 + align
		};
		let want = (want + 3) & !3;
		let Some((start, taken)) = (unsafe { self.take(want) }) else {
			return null_mut();
		};
		let base = self.base();
		let user = unsafe {
			if align <= 4 {
				self.wr(start, taken);
				base.add(start + HEADER)
			} else {
				let at = base as usize + start;
				let user = round_up(at + 8, align) as *mut u8;
				(user as *mut u32).sub(2).write(start as u32);
				(user as *mut u32).sub(1).write(taken as u32 | 1);
				user
			}
		};
		self.used += taken;
		self.blocks += 1;
		self.asked += size;
		self.note();
		user
	}

	unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
		unsafe {
			let (start, size) = self.locate(ptr);
			self.used -= size;
			self.blocks -= 1;
			self.asked -= layout.size();
			self.insert(start, size);
		}
	}

	unsafe fn realloc(&mut self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
		let old_size = layout.size();
		if layout.align() <= 4 && new_size <= HEAP_SIZE {
			let wanted = (new_size.max(4) + HEADER + 3) & !3;
			unsafe {
				let (start, have) = self.locate(ptr);
				if wanted <= have {
					if have - wanted >= MIN_BLOCK {
						self.wr(start, wanted);
						self.used -= have - wanted;
						self.insert(start + wanted, have - wanted);
					}
					self.asked = self.asked - old_size + new_size;
					return ptr;
				}
				// Grow into the free block that follows, when there is one and it is big enough.
				let after = start + have;
				let mut prev = NIL as usize;
				let mut cur = self.free as usize;
				while cur != NIL as usize && cur < after {
					prev = cur;
					cur = self.rd(cur + 4);
				}
				if cur == after && have + self.rd(cur) >= wanted {
					let size = self.rd(cur);
					let next = self.rd(cur + 4);
					let extra = wanted - have;
					let (grown, link) = if size - extra >= MIN_BLOCK {
						self.wr(after + extra, size - extra);
						self.wr(after + extra + 4, next);
						(wanted, after + extra)
					} else {
						(have + size, next)
					};
					if prev == NIL as usize {
						self.free = link as u32;
					} else {
						self.wr(prev + 4, link);
					}
					self.wr(start, grown);
					self.used += grown - have;
					self.asked = self.asked - old_size + new_size;
					self.note();
					return ptr;
				}
			}
		}
		// Move: a new block, a copy, then the old block back.
		unsafe {
			let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
			let moved = self.alloc(new_layout);
			if !moved.is_null() {
				core::ptr::copy_nonoverlapping(ptr, moved, old_size.min(new_size));
				self.dealloc(ptr, layout);
			}
			moved
		}
	}
}

impl Heap {
	pub const fn new() -> Heap {
		Heap(UnsafeCell::new(State {
			arena: Arena([0; HEAP_SIZE]),
			free: NIL,
			ready: false,
			used: 0,
			peak: 0,
			blocks: 0,
			asked: 0,
			peak_blocks: 0,
			peak_asked: 0,
			most_blocks: 0,
		}))
	}

	/// The most bytes the program has held at once, headers included.
	pub fn peak(&self) -> usize {
		unsafe { (*self.0.get()).peak }
	}

	/// The live blocks and the bytes asked for when the heap peaked, and the most blocks at once.
	pub fn census(&self) -> (usize, usize, usize) {
		let st = unsafe { &*self.0.get() };
		(st.peak_blocks, st.peak_asked, st.most_blocks)
	}
}

unsafe impl GlobalAlloc for Heap {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		unsafe { (*self.0.get()).alloc(layout) }
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		unsafe { (*self.0.get()).dealloc(ptr, layout) }
	}

	unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
		unsafe { (*self.0.get()).realloc(ptr, layout, new_size) }
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::vec::Vec;

	struct Rng(u64);

	impl Rng {
		fn next(&mut self) -> u64 {
			self.0 ^= self.0 << 13;
			self.0 ^= self.0 >> 7;
			self.0 ^= self.0 << 17;
			self.0
		}

		fn below(&mut self, n: usize) -> usize {
			(self.next() % n as u64) as usize
		}
	}

	struct Live {
		ptr: *mut u8,
		layout: Layout,
		tag: u8,
	}

	fn fill(live: &Live) {
		for i in 0..live.layout.size() {
			unsafe { live.ptr.add(i).write(live.tag.wrapping_add(i as u8)) };
		}
	}

	fn check(live: &Live, len: usize) {
		for i in 0..len {
			let got = unsafe { live.ptr.add(i).read() };
			assert_eq!(
				got,
				live.tag.wrapping_add(i as u8),
				"contents of a live block changed"
			);
		}
	}

	fn no_overlap(all: &[Live], at: usize) {
		let (a0, a1) = (
			all[at].ptr as usize,
			all[at].ptr as usize + all[at].layout.size(),
		);
		for (j, other) in all.iter().enumerate() {
			if j != at {
				let (b0, b1) = (other.ptr as usize, other.ptr as usize + other.layout.size());
				assert!(a1 <= b0 || b1 <= a0, "two live blocks overlap");
			}
		}
	}

	fn whole_arena_free(heap: &Heap) {
		let st = unsafe { &mut *heap.0.get() };
		assert_eq!(st.used, 0);
		assert_eq!(st.blocks, 0);
		assert_eq!(st.asked, 0);
		assert_eq!(st.free, 0, "the free list does not start at the arena");
		unsafe {
			assert_eq!(st.rd(0), HEAP_SIZE, "the arena is not one free block");
			assert_eq!(st.rd(4), NIL as usize);
		}
	}

	#[test]
	fn random_use_keeps_the_rules() {
		let heap = Box::new(Heap::new());
		let mut rng = Rng(0x2545_F491_4F6C_DD1D);
		let mut all: Vec<Live> = Vec::new();
		let mut refused = 0;
		for step in 0..10_000 {
			let action = rng.below(10);
			if action < 5 && all.len() < 120 || all.is_empty() {
				let size = if step % 997 == 0 {
					40_000
				} else {
					1 + rng.below(600)
				};
				let align = [1, 2, 4, 8, 16][rng.below(5)];
				let layout = Layout::from_size_align(size, align).unwrap();
				let ptr = unsafe { heap.alloc(layout) };
				if ptr.is_null() {
					refused += 1;
					continue;
				}
				assert_eq!(ptr as usize % align, 0, "misaligned block");
				all.push(Live {
					ptr,
					layout,
					tag: rng.next() as u8,
				});
				let last = all.len() - 1;
				fill(&all[last]);
				no_overlap(&all, last);
			} else if action < 8 {
				let at = rng.below(all.len());
				let live = all.swap_remove(at);
				check(&live, live.layout.size());
				unsafe { heap.dealloc(live.ptr, live.layout) };
			} else {
				let at = rng.below(all.len());
				let new_size = 1 + rng.below(600);
				let old =
					Layout::from_size_align(all[at].layout.size(), all[at].layout.align()).unwrap();
				let ptr = unsafe { heap.realloc(all[at].ptr, old, new_size) };
				if ptr.is_null() {
					refused += 1;
					check(&all[at], old.size());
					continue;
				}
				assert_eq!(ptr as usize % old.align(), 0, "misaligned after realloc");
				let keep = old.size().min(new_size);
				all[at].ptr = ptr;
				all[at].layout = Layout::from_size_align(new_size, old.align()).unwrap();
				check(&all[at], keep);
				fill(&all[at]);
				no_overlap(&all, at);
			}
		}
		assert!(refused > 0, "the test never ran the heap out");
		for live in all.drain(..) {
			check(&live, live.layout.size());
			unsafe { heap.dealloc(live.ptr, live.layout) };
		}
		whole_arena_free(&heap);
	}

	#[test]
	fn exhaustion_leaves_the_heap_usable() {
		let heap = Box::new(Heap::new());
		let layout = Layout::from_size_align(100, 4).unwrap();
		let mut ptrs = Vec::new();
		loop {
			let ptr = unsafe { heap.alloc(layout) };
			if ptr.is_null() {
				break;
			}
			ptrs.push(ptr);
		}
		assert!(ptrs.len() > 300);
		let huge = Layout::from_size_align(HEAP_SIZE, 4).unwrap();
		assert!(unsafe { heap.alloc(huge) }.is_null());
		let one = ptrs.pop().unwrap();
		unsafe { heap.dealloc(one, layout) };
		let again = unsafe { heap.alloc(layout) };
		assert!(!again.is_null());
		ptrs.push(again);
		for ptr in ptrs {
			unsafe { heap.dealloc(ptr, layout) };
		}
		whole_arena_free(&heap);
	}

	#[test]
	fn costs_are_four_bytes_a_block() {
		let heap = Box::new(Heap::new());
		let layout = Layout::from_size_align(10, 1).unwrap();
		let a = unsafe { heap.alloc(layout) };
		assert_eq!(heap.peak(), 16);
		let tiny = Layout::from_size_align(1, 1).unwrap();
		let b = unsafe { heap.alloc(tiny) };
		assert_eq!(heap.peak(), 24);
		assert_eq!(heap.census(), (2, 11, 2));
		// A block grows into the free space after it without moving.
		let grown = unsafe { heap.realloc(b, tiny, 200) };
		assert_eq!(grown, b);
		unsafe { heap.dealloc(a, layout) };
		unsafe { heap.dealloc(grown, Layout::from_size_align(200, 1).unwrap()) };
		whole_arena_free(&heap);
	}
}
