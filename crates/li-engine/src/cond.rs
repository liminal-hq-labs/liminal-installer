// The condition expression language used by `when` and `branches`.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A small expression language with no function calls, so every renderer can evaluate it.
//!
//! ```text
//! expr    = or
//! or      = and { "or" and }
//! and     = not { "and" not }
//! not     = "not" not | cmp
//! cmp     = primary [ ("==" | "!=" | "in" | "not" "in") primary ]
//! primary = string | number | "true" | "false" | "null" | list | path | "(" expr ")"
//! list    = "[" [ expr { "," expr } ] "]"
//! path    = root { "." name }       root is target, components, state or system
//! ```
//!
//! Strings use single or double quotes. A path that does not exist is `null`. `a in b` is list
//! membership, substring test on strings, or key test on objects. A bare path or value in an
//! `and`/`or`/`not` is true unless it is `null`, `false`, `0`, `""`, `[]` or `{}`.

use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CondError {
	#[error("unexpected character `{0}`")]
	BadChar(char),
	#[error("unterminated string")]
	Unterminated,
	#[error("unexpected `{0}`")]
	Unexpected(String),
	#[error("unexpected end of expression")]
	End,
	#[error("function calls are not allowed (`{0}(`)")]
	FunctionCall(String),
	#[error("unknown name `{0}`; expected target, components, state or system")]
	UnknownRoot(String),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
	Str(String),
	Num(f64),
	Ident(String),
	Eq,
	Ne,
	LParen,
	RParen,
	LBracket,
	RBracket,
	Comma,
	Dot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
	Eq,
	Ne,
	In,
	NotIn,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
	Lit(Value),
	List(Vec<Expr>),
	Path(Vec<String>),
	Not(Box<Expr>),
	And(Box<Expr>, Box<Expr>),
	Or(Box<Expr>, Box<Expr>),
	Cmp(CmpOp, Box<Expr>, Box<Expr>),
}

/// What an expression can see.
#[derive(Debug, Clone, Default)]
pub struct CondContext {
	pub target: Value,
	pub components: Value,
	pub state: Value,
	pub system: Value,
}

fn lex(src: &str) -> Result<Vec<Tok>, CondError> {
	let chars: Vec<char> = src.chars().collect();
	let mut i = 0;
	let mut out = Vec::new();
	while i < chars.len() {
		let c = chars[i];
		match c {
			c if c.is_whitespace() => i += 1,
			'(' => {
				out.push(Tok::LParen);
				i += 1;
			}
			')' => {
				out.push(Tok::RParen);
				i += 1;
			}
			'[' => {
				out.push(Tok::LBracket);
				i += 1;
			}
			']' => {
				out.push(Tok::RBracket);
				i += 1;
			}
			',' => {
				out.push(Tok::Comma);
				i += 1;
			}
			'.' => {
				out.push(Tok::Dot);
				i += 1;
			}
			'=' if chars.get(i + 1) == Some(&'=') => {
				out.push(Tok::Eq);
				i += 2;
			}
			'!' if chars.get(i + 1) == Some(&'=') => {
				out.push(Tok::Ne);
				i += 2;
			}
			'\'' | '"' => {
				let quote = c;
				let mut s = String::new();
				i += 1;
				loop {
					match chars.get(i) {
						None => return Err(CondError::Unterminated),
						Some(&q) if q == quote => break,
						Some(&ch) => s.push(ch),
					}
					i += 1;
				}
				i += 1;
				out.push(Tok::Str(s));
			}
			c if c.is_ascii_digit()
				|| (c == '-' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) =>
			{
				let start = i;
				i += 1;
				while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
					i += 1;
				}
				let text: String = chars[start..i].iter().collect();
				let n = text.parse().map_err(|_| CondError::Unexpected(text))?;
				out.push(Tok::Num(n));
			}
			c if c.is_ascii_alphabetic() || c == '_' => {
				let start = i;
				while i < chars.len()
					&& (chars[i].is_ascii_alphanumeric() || chars[i] == '_' || chars[i] == '-')
				{
					i += 1;
				}
				out.push(Tok::Ident(chars[start..i].iter().collect()));
			}
			other => return Err(CondError::BadChar(other)),
		}
	}
	Ok(out)
}

struct Parser {
	toks: Vec<Tok>,
	at: usize,
}

impl Parser {
	fn peek(&self) -> Option<&Tok> {
		self.toks.get(self.at)
	}

	fn peek_at(&self, n: usize) -> Option<&Tok> {
		self.toks.get(self.at + n)
	}

	fn bump(&mut self) -> Option<Tok> {
		let t = self.toks.get(self.at).cloned();
		self.at += 1;
		t
	}

	fn is_kw(&self, n: usize, kw: &str) -> bool {
		matches!(self.peek_at(n), Some(Tok::Ident(s)) if s == kw)
	}

	fn or(&mut self) -> Result<Expr, CondError> {
		let mut left = self.and()?;
		while self.is_kw(0, "or") {
			self.bump();
			left = Expr::Or(Box::new(left), Box::new(self.and()?));
		}
		Ok(left)
	}

	fn and(&mut self) -> Result<Expr, CondError> {
		let mut left = self.not()?;
		while self.is_kw(0, "and") {
			self.bump();
			left = Expr::And(Box::new(left), Box::new(self.not()?));
		}
		Ok(left)
	}

	fn not(&mut self) -> Result<Expr, CondError> {
		if self.is_kw(0, "not") {
			self.bump();
			return Ok(Expr::Not(Box::new(self.not()?)));
		}
		self.cmp()
	}

	fn cmp(&mut self) -> Result<Expr, CondError> {
		let left = self.primary()?;
		let op = match self.peek() {
			Some(Tok::Eq) => CmpOp::Eq,
			Some(Tok::Ne) => CmpOp::Ne,
			Some(Tok::Ident(s)) if s == "in" => CmpOp::In,
			Some(Tok::Ident(s)) if s == "not" && self.is_kw(1, "in") => {
				self.bump();
				CmpOp::NotIn
			}
			_ => return Ok(left),
		};
		self.bump();
		let right = self.primary()?;
		Ok(Expr::Cmp(op, Box::new(left), Box::new(right)))
	}

	fn primary(&mut self) -> Result<Expr, CondError> {
		match self.bump().ok_or(CondError::End)? {
			Tok::Str(s) => Ok(Expr::Lit(Value::String(s))),
			Tok::Num(n) => Ok(Expr::Lit(serde_json::json!(n))),
			Tok::LParen => {
				let e = self.or()?;
				match self.bump() {
					Some(Tok::RParen) => Ok(e),
					Some(t) => Err(CondError::Unexpected(format!("{t:?}"))),
					None => Err(CondError::End),
				}
			}
			Tok::LBracket => {
				let mut items = Vec::new();
				if self.peek() == Some(&Tok::RBracket) {
					self.bump();
					return Ok(Expr::List(items));
				}
				loop {
					items.push(self.or()?);
					match self.bump() {
						Some(Tok::Comma) => {}
						Some(Tok::RBracket) => break,
						Some(t) => return Err(CondError::Unexpected(format!("{t:?}"))),
						None => return Err(CondError::End),
					}
				}
				Ok(Expr::List(items))
			}
			Tok::Ident(name) => match name.as_str() {
				"true" => Ok(Expr::Lit(Value::Bool(true))),
				"false" => Ok(Expr::Lit(Value::Bool(false))),
				"null" => Ok(Expr::Lit(Value::Null)),
				_ => {
					if self.peek() == Some(&Tok::LParen) {
						return Err(CondError::FunctionCall(name));
					}
					if !matches!(name.as_str(), "target" | "components" | "state" | "system") {
						return Err(CondError::UnknownRoot(name));
					}
					let mut path = vec![name];
					while self.peek() == Some(&Tok::Dot) {
						self.bump();
						match self.bump() {
							Some(Tok::Ident(seg)) => {
								if self.peek() == Some(&Tok::LParen) {
									return Err(CondError::FunctionCall(seg));
								}
								path.push(seg);
							}
							Some(t) => return Err(CondError::Unexpected(format!("{t:?}"))),
							None => return Err(CondError::End),
						}
					}
					Ok(Expr::Path(path))
				}
			},
			t => Err(CondError::Unexpected(format!("{t:?}"))),
		}
	}
}

/// Parses an expression.
pub fn parse(src: &str) -> Result<Expr, CondError> {
	let mut p = Parser {
		toks: lex(src)?,
		at: 0,
	};
	let e = p.or()?;
	match p.peek() {
		None => Ok(e),
		Some(t) => Err(CondError::Unexpected(format!("{t:?}"))),
	}
}

fn truthy(v: &Value) -> bool {
	match v {
		Value::Null => false,
		Value::Bool(b) => *b,
		Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
		Value::String(s) => !s.is_empty(),
		Value::Array(a) => !a.is_empty(),
		Value::Object(o) => !o.is_empty(),
	}
}

fn same(a: &Value, b: &Value) -> bool {
	match (a, b) {
		(Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
		_ => a == b,
	}
}

fn contains(haystack: &Value, needle: &Value) -> bool {
	match (haystack, needle) {
		(Value::Array(items), _) => items.iter().any(|i| same(i, needle)),
		(Value::String(h), Value::String(n)) => h.contains(n.as_str()),
		(Value::Object(o), Value::String(k)) => o.contains_key(k),
		_ => false,
	}
}

impl Expr {
	/// Evaluates to a JSON value. Comparisons and logic give booleans.
	pub fn eval(&self, ctx: &CondContext) -> Value {
		match self {
			Expr::Lit(v) => v.clone(),
			Expr::List(items) => Value::Array(items.iter().map(|e| e.eval(ctx)).collect()),
			Expr::Path(path) => {
				let mut cur = match path[0].as_str() {
					"target" => &ctx.target,
					"components" => &ctx.components,
					"state" => &ctx.state,
					_ => &ctx.system,
				};
				for seg in &path[1..] {
					match cur.get(seg.as_str()) {
						Some(next) => cur = next,
						None => return Value::Null,
					}
				}
				cur.clone()
			}
			Expr::Not(e) => Value::Bool(!truthy(&e.eval(ctx))),
			Expr::And(a, b) => Value::Bool(truthy(&a.eval(ctx)) && truthy(&b.eval(ctx))),
			Expr::Or(a, b) => Value::Bool(truthy(&a.eval(ctx)) || truthy(&b.eval(ctx))),
			Expr::Cmp(op, a, b) => {
				let (l, r) = (a.eval(ctx), b.eval(ctx));
				Value::Bool(match op {
					CmpOp::Eq => same(&l, &r),
					CmpOp::Ne => !same(&l, &r),
					CmpOp::In => contains(&r, &l),
					CmpOp::NotIn => !contains(&r, &l),
				})
			}
		}
	}

	pub fn eval_bool(&self, ctx: &CondContext) -> bool {
		truthy(&self.eval(ctx))
	}

	/// Every comparison of a string literal against `components`, for the project checks.
	pub fn component_mentions(&self, out: &mut Vec<String>) {
		match self {
			Expr::Cmp(CmpOp::In | CmpOp::NotIn, l, r) => {
				if let (Expr::Lit(Value::String(s)), Expr::Path(p)) = (&**l, &**r)
					&& p.len() == 1 && p[0] == "components"
				{
					out.push(s.clone());
				}
			}
			Expr::Not(e) => e.component_mentions(out),
			Expr::And(a, b) | Expr::Or(a, b) => {
				a.component_mentions(out);
				b.component_mentions(out);
			}
			_ => {}
		}
	}
}

/// Parses and evaluates in one step.
pub fn eval_bool(src: &str, ctx: &CondContext) -> Result<bool, CondError> {
	Ok(parse(src)?.eval_bool(ctx))
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	fn ctx() -> CondContext {
		CondContext {
			target: json!({"id": "dos-x86", "renderer": "tui-retro"}),
			components: json!(["game", "sound"]),
			state: json!({"scope": "user", "accepted": true, "name": "", "n": 3}),
			system: json!({"os": "linux", "free_space": 1000, "installed": false}),
		}
	}

	#[test]
	fn table() {
		let cases: &[(&str, bool)] = &[
			("target.renderer == 'tui-retro'", true),
			("target.renderer != 'tui-retro'", false),
			("target.renderer in ['tui-retro', 'win16']", true),
			("target.renderer in ['canvas']", false),
			("target.renderer not in ['canvas']", true),
			("'sound' in components", true),
			("'levels' in components", false),
			("'levels' not in components", true),
			("state.accepted", true),
			("not state.accepted", false),
			("state.name", false),
			("state.missing", false),
			("state.missing == null", true),
			("state.n == 3", true),
			("state.n == 3.0", true),
			("system.free_space == 1000 and 'game' in components", true),
			("system.installed or state.accepted", true),
			("system.installed or not state.accepted", false),
			("not system.installed and state.accepted", true),
			(
				"(system.installed or state.accepted) and state.name == ''",
				true,
			),
			("true and not false", true),
			("\"linux\" == system.os", true),
			("'tui' in target.renderer", true),
			("'renderer' in target", true),
			("[] == []", true),
			(
				"state.scope in ['user','machine'] and not (state.n == 4)",
				true,
			),
			("not not state.accepted", true),
		];
		for (src, want) in cases {
			assert_eq!(eval_bool(src, &ctx()).unwrap(), *want, "{src}");
		}
	}

	#[test]
	fn errors() {
		assert!(matches!(
			parse("len(components)"),
			Err(CondError::FunctionCall(_))
		));
		assert!(matches!(
			parse("state.name.lower()"),
			Err(CondError::FunctionCall(_))
		));
		assert!(matches!(
			parse("bogus == 1"),
			Err(CondError::UnknownRoot(_))
		));
		assert!(matches!(parse("'a"), Err(CondError::Unterminated)));
		assert!(matches!(parse("state.a =="), Err(CondError::End)));
		assert!(matches!(
			parse("state.a state.b"),
			Err(CondError::Unexpected(_))
		));
		assert!(parse("a & b").is_err());
		assert!(matches!(parse("[1, 2"), Err(CondError::End)));
		assert!(matches!(parse("(state.a"), Err(CondError::End)));
	}

	#[test]
	fn component_mentions_are_found() {
		let e = parse("'a' in components or not ('b' not in components)").unwrap();
		let mut out = Vec::new();
		e.component_mentions(&mut out);
		assert_eq!(out, vec!["a", "b"]);
	}
}
