// Compiles li-engine's condition expressions to the runtime's bytecode.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use li_engine::cond::{CmpOp, Expr, parse};
use li_retro::expr::{AND, Asm, EQ, FALSE, IN, NE, NOT, NOT_IN, NULL, OR, TRUE};
use serde_json::Value;

use crate::{CompileError, err};

/// The string table the bytecode refers to.
#[derive(Default)]
pub struct Strings {
	table: Vec<String>,
}

impl Strings {
	fn intern(&mut self, s: &str) -> usize {
		match self.table.iter().position(|t| t == s) {
			Some(i) => i,
			None => {
				self.table.push(s.to_owned());
				self.table.len() - 1
			}
		}
	}

	pub fn into_table(self) -> Vec<String> {
		self.table
	}

	/// Parses and compiles `src`; `location` names it in errors.
	pub fn compile(&mut self, src: &str, location: &str) -> Result<Vec<u8>, CompileError> {
		let e =
			parse(src).map_err(|e| CompileError::Message(format!("{location}: `{src}`: {e}")))?;
		let mut asm = Asm::default();
		self.emit(&e, &mut asm, location)?;
		Ok(asm.code)
	}

	fn emit(&mut self, e: &Expr, a: &mut Asm, loc: &str) -> Result<(), CompileError> {
		match e {
			Expr::Lit(Value::Null) => a.op(NULL),
			Expr::Lit(Value::Bool(true)) => a.op(TRUE),
			Expr::Lit(Value::Bool(false)) => a.op(FALSE),
			Expr::Lit(Value::String(s)) => {
				let i = self.intern(s);
				a.str(i);
			}
			Expr::Lit(Value::Number(n)) => match n.as_i64() {
				Some(i) => a.num(i),
				None => match n.as_f64() {
					Some(f) if f.fract() == 0.0 => a.num(f as i64),
					_ => {
						return err(format!(
							"{loc}: the DOS runtime compares whole numbers only, not {n}"
						));
					}
				},
			},
			Expr::Lit(other) => return err(format!("{loc}: cannot compile the literal {other}")),
			Expr::List(items) => {
				for i in items {
					self.emit(i, a, loc)?;
				}
				a.list(items.len());
			}
			Expr::Path(p) => {
				let i = self.intern(&p.join("."));
				a.path(i);
			}
			Expr::Not(x) => {
				self.emit(x, a, loc)?;
				a.op(NOT);
			}
			Expr::And(l, r) | Expr::Or(l, r) => {
				self.emit(l, a, loc)?;
				self.emit(r, a, loc)?;
				a.op(if matches!(e, Expr::And(..)) { AND } else { OR });
			}
			Expr::Cmp(op, l, r) => {
				self.emit(l, a, loc)?;
				self.emit(r, a, loc)?;
				a.op(match op {
					CmpOp::Eq => EQ,
					CmpOp::Ne => NE,
					CmpOp::In => IN,
					CmpOp::NotIn => NOT_IN,
				});
			}
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use li_retro::expr::{Scope, Val, eval};

	use super::*;

	struct S;
	impl Scope for S {
		fn lookup(&self, path: &str) -> Val {
			match path {
				"target.renderer" => Val::Str("tui-retro".into()),
				"components" => Val::List(vec![Val::Str("game".into()), Val::Str("sound".into())]),
				"state.scope" => Val::Str("user".into()),
				"system.free_space" => Val::Num(1000),
				_ => Val::Null,
			}
		}
	}

	#[test]
	fn matches_the_engine_on_its_own_table() {
		// The same cases as li_engine::cond's table, over the same values.
		let cases: &[(&str, bool)] = &[
			("target.renderer == 'tui-retro'", true),
			("target.renderer != 'tui-retro'", false),
			("target.renderer in ['tui-retro', 'win16']", true),
			("target.renderer in ['canvas']", false),
			("target.renderer not in ['canvas']", true),
			("'sound' in components", true),
			("'levels' in components", false),
			("'levels' not in components", true),
			("state.missing", false),
			("state.missing == null", true),
			("system.free_space == 1000 and 'game' in components", true),
			("not system.installed and state.scope", true),
			(
				"(system.installed or state.scope) and state.scope == 'user'",
				true,
			),
			("true and not false", true),
			("\"tui-retro\" == target.renderer", true),
			("'tui' in target.renderer", true),
			("[] == []", true),
			("not not state.scope", true),
		];
		for (src, want) in cases {
			let mut strings = Strings::default();
			let code = strings.compile(src, "test").unwrap();
			li_retro::expr::check(&code, strings.table.len()).unwrap();
			let got = eval(&code, &strings.table, &S).truthy();
			assert_eq!(got, *want, "{src}");
		}
	}

	#[test]
	fn rejects_fractions() {
		assert!(
			Strings::default()
				.compile("system.free_space == 1.5", "t")
				.is_err()
		);
	}
}
