// Condition bytecode: the `when` expression language of li-engine, compiled to a small stack machine.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The compiler walks li-engine's parsed expression and emits [`Asm`] code; the runtime runs it
//! with [`eval`] against a [`Scope`]. Semantics follow `li_engine::cond`: a missing path is
//! `null`, `in` is list membership, substring or never for other values, and `and`, `or` and
//! `not` use truthiness. Numbers are whole numbers on DOS.

use alloc::string::String;
use alloc::vec::Vec;

use crate::Error;

pub const NULL: u8 = 0;
pub const TRUE: u8 = 1;
pub const FALSE: u8 = 2;
/// Followed by a varint string index.
pub const STR: u8 = 3;
/// Followed by a zig-zag varint.
pub const NUM: u8 = 4;
/// Followed by a varint string index naming a dotted path such as `target.renderer`.
pub const PATH: u8 = 5;
/// Followed by a varint count of the values to pop.
pub const LIST: u8 = 6;
pub const NOT: u8 = 7;
pub const AND: u8 = 8;
pub const OR: u8 = 9;
pub const EQ: u8 = 10;
pub const NE: u8 = 11;
pub const IN: u8 = 12;
pub const NOT_IN: u8 = 13;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Val {
	Null,
	Bool(bool),
	Num(i64),
	Str(String),
	List(Vec<Val>),
}

impl Val {
	pub fn truthy(&self) -> bool {
		match self {
			Val::Null => false,
			Val::Bool(b) => *b,
			Val::Num(n) => *n != 0,
			Val::Str(s) => !s.is_empty(),
			Val::List(l) => !l.is_empty(),
		}
	}
}

/// What a path such as `state.sound.card` reads.
pub trait Scope {
	fn lookup(&self, path: &str) -> Val;
}

/// An assembler for the compiler.
#[derive(Debug, Default, Clone)]
pub struct Asm {
	pub code: Vec<u8>,
}

fn varint(out: &mut Vec<u8>, mut n: u64) {
	loop {
		let b = (n & 0x7f) as u8;
		n >>= 7;
		if n == 0 {
			out.push(b);
			return;
		}
		out.push(b | 0x80);
	}
}

fn read_varint(code: &[u8], at: &mut usize) -> Option<u64> {
	let mut n = 0u64;
	let mut shift = 0;
	loop {
		let b = *code.get(*at)?;
		*at += 1;
		n |= u64::from(b & 0x7f) << shift;
		if b & 0x80 == 0 {
			return Some(n);
		}
		shift += 7;
		if shift > 63 {
			return None;
		}
	}
}

impl Asm {
	pub fn op(&mut self, op: u8) {
		self.code.push(op);
	}

	pub fn str(&mut self, index: usize) {
		self.code.push(STR);
		varint(&mut self.code, index as u64);
	}

	pub fn path(&mut self, index: usize) {
		self.code.push(PATH);
		varint(&mut self.code, index as u64);
	}

	pub fn num(&mut self, n: i64) {
		self.code.push(NUM);
		varint(&mut self.code, ((n << 1) ^ (n >> 63)) as u64);
	}

	pub fn list(&mut self, count: usize) {
		self.code.push(LIST);
		varint(&mut self.code, count as u64);
	}
}

/// Checks that `code` is well formed for a table of `strings` strings: known opcodes, string
/// indices in range, and exactly one value left at the end. An empty program is valid (always).
pub fn check(code: &[u8], strings: usize) -> Result<(), Error> {
	if code.is_empty() {
		return Ok(());
	}
	let bad = || Error::Format("a condition's bytecode is damaged".into());
	let mut at = 0;
	let mut depth = 0usize;
	while at < code.len() {
		let op = code[at];
		at += 1;
		match op {
			NULL | TRUE | FALSE => depth += 1,
			STR | PATH => {
				let i = read_varint(code, &mut at).ok_or_else(bad)?;
				if i >= strings as u64 {
					return Err(bad());
				}
				depth += 1;
			}
			NUM => {
				read_varint(code, &mut at).ok_or_else(bad)?;
				depth += 1;
			}
			LIST => {
				let n = read_varint(code, &mut at).ok_or_else(bad)? as usize;
				if n > depth {
					return Err(bad());
				}
				depth = depth - n + 1;
			}
			NOT => {
				if depth < 1 {
					return Err(bad());
				}
			}
			AND | OR | EQ | NE | IN | NOT_IN => {
				if depth < 2 {
					return Err(bad());
				}
				depth -= 1;
			}
			_ => return Err(bad()),
		}
	}
	if depth == 1 { Ok(()) } else { Err(bad()) }
}

fn same(a: &Val, b: &Val) -> bool {
	a == b
}

fn contains(haystack: &Val, needle: &Val) -> bool {
	match (haystack, needle) {
		(Val::List(items), _) => items.iter().any(|i| same(i, needle)),
		(Val::Str(h), Val::Str(n)) => h.contains(n.as_str()),
		_ => false,
	}
}

/// Runs `code` and returns the value it leaves. Code already passed by [`check`], or a failure,
/// gives `null`.
pub fn eval(code: &[u8], strings: &[String], scope: &dyn Scope) -> Val {
	let mut stack: Vec<Val> = Vec::new();
	let mut at = 0;
	let s = |i: u64| strings.get(i as usize).cloned().unwrap_or_default();
	while at < code.len() {
		let op = code[at];
		at += 1;
		match op {
			NULL => stack.push(Val::Null),
			TRUE => stack.push(Val::Bool(true)),
			FALSE => stack.push(Val::Bool(false)),
			STR => {
				let i = read_varint(code, &mut at).unwrap_or(0);
				stack.push(Val::Str(s(i)));
			}
			PATH => {
				let i = read_varint(code, &mut at).unwrap_or(0);
				stack.push(scope.lookup(&s(i)));
			}
			NUM => {
				let z = read_varint(code, &mut at).unwrap_or(0);
				stack.push(Val::Num(((z >> 1) as i64) ^ -((z & 1) as i64)));
			}
			LIST => {
				let n = read_varint(code, &mut at).unwrap_or(0) as usize;
				let from = stack.len().saturating_sub(n);
				let items = stack.split_off(from);
				stack.push(Val::List(items));
			}
			NOT => {
				let v = stack.pop().unwrap_or(Val::Null);
				stack.push(Val::Bool(!v.truthy()));
			}
			AND | OR | EQ | NE | IN | NOT_IN => {
				let r = stack.pop().unwrap_or(Val::Null);
				let l = stack.pop().unwrap_or(Val::Null);
				stack.push(Val::Bool(match op {
					AND => l.truthy() && r.truthy(),
					OR => l.truthy() || r.truthy(),
					EQ => same(&l, &r),
					NE => !same(&l, &r),
					IN => contains(&r, &l),
					_ => !contains(&r, &l),
				}));
			}
			_ => return Val::Null,
		}
	}
	stack.pop().unwrap_or(Val::Null)
}

/// Whether `code` holds; empty code always holds.
pub fn holds(code: &[u8], strings: &[String], scope: &dyn Scope) -> bool {
	code.is_empty() || eval(code, strings, scope).truthy()
}

#[cfg(test)]
mod tests {
	use alloc::string::ToString;
	use alloc::vec;

	use super::*;

	struct Fixed;
	impl Scope for Fixed {
		fn lookup(&self, path: &str) -> Val {
			match path {
				"target.renderer" => Val::Str("tui-retro".to_string()),
				"components" => Val::List(vec![Val::Str("game".to_string())]),
				"state.n" => Val::Num(3),
				_ => Val::Null,
			}
		}
	}

	fn strings() -> Vec<String> {
		[
			"target.renderer",
			"tui-retro",
			"win16",
			"components",
			"game",
			"levels",
			"state.n",
			"state.missing",
		]
		.iter()
		.map(ToString::to_string)
		.collect()
	}

	#[test]
	fn runs_comparisons_membership_and_logic() {
		// target.renderer in ['tui-retro', 'win16'] and not ('levels' in components)
		let mut a = Asm::default();
		a.path(0);
		a.str(1);
		a.str(2);
		a.list(2);
		a.op(IN);
		a.str(5);
		a.path(3);
		a.op(IN);
		a.op(NOT);
		a.op(AND);
		check(&a.code, 8).unwrap();
		assert!(eval(&a.code, &strings(), &Fixed).truthy());
		// state.n == 3, state.missing == null, 'game' not in components
		let mut b = Asm::default();
		b.path(6);
		b.num(3);
		b.op(EQ);
		b.path(7);
		b.op(NULL);
		b.op(EQ);
		b.op(AND);
		b.str(4);
		b.path(3);
		b.op(NOT_IN);
		b.op(OR);
		check(&b.code, 8).unwrap();
		assert!(eval(&b.code, &strings(), &Fixed).truthy());
		assert!(holds(&[], &strings(), &Fixed));
	}

	#[test]
	fn negative_numbers_round_trip() {
		let mut a = Asm::default();
		a.num(-7);
		assert_eq!(eval(&a.code, &[], &Fixed), Val::Num(-7));
	}

	#[test]
	fn rejects_damaged_code() {
		assert!(check(&[STR, 9], 2).is_err());
		assert!(check(&[AND], 2).is_err());
		assert!(check(&[TRUE, TRUE], 2).is_err());
		assert!(check(&[99], 2).is_err());
		assert!(check(&[STR], 2).is_err());
	}
}
