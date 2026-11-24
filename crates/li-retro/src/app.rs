// The flow VM: walks a compiled flow, shows its pages and runs install or uninstall at its commit step.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! [`run`] is what `SETUP.EXE` does. It mirrors `li_engine::FlowRunner`: a step whose `when` is
//! false is skipped, a page waits for Next, Back or Cancel, a branch picks the first condition
//! that holds, the commit step does the work and the finish step ends the run. The `builtin:`
//! actions replace the Lua hooks the host runner has.
//!
//! The exit codes are the runner's: 0 done, 1 failed (everything rolled back), 2 bad arguments or
//! a damaged `SETUP.LPK`, 3 the person cancelled.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use crate::Error;
use crate::expr;
use crate::gui::{GuiEvent, GuiPage};
use crate::install::{self, Progress, Receipt};
use crate::model::{Builtin, Flow, Kind, Project, Step};
use crate::pack::Pack;
use crate::platform::{Key, Platform};
use crate::text::human;
use crate::ui::{Cells, Outcome, Page, Value, Values};

/// What `SETUP.EXE` was asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mode {
	Install,
	/// Remove an install; the directory is the one the receipt lives in, or the default.
	Uninstall(Option<String>),
	/// Print the version and leave.
	Version,
}

impl Mode {
	/// Reads the command tail: `/U [dir]` uninstalls, `/V` prints the version. The install switches
	/// (`/S`, `/COMPONENTS=`) may stand anywhere and are read by [`Switches::of`].
	pub fn from_args(args: &str) -> Result<Mode, String> {
		let words = words_of(args);
		let mut words = words
			.iter()
			.map(String::as_str)
			.filter(|w| Switches::word(w).is_none());
		match words.next() {
			None => Ok(Mode::Install),
			Some(w) if w.eq_ignore_ascii_case("/U") || w.eq_ignore_ascii_case("-U") => {
				Ok(Mode::Uninstall(words.next().map(str::to_string)))
			}
			Some(w) if w.eq_ignore_ascii_case("/V") => Ok(Mode::Version),
			Some(w) => Err(format!(
				"unknown option {w}; use SETUP, SETUP /U [folder] or SETUP /V"
			)),
		}
	}
}

/// The install switches of a command tail: `/S` installs without pages, and `/COMPONENTS=a,b`
/// installs exactly the components named, besides the required ones. Without it an install takes the
/// components that start ticked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Switches {
	pub silent: bool,
	pub components: Option<Vec<String>>,
}

impl Switches {
	/// What one word says, when it is a switch: `Some(None)` for `/S`, `Some(Some(list))` for
	/// `/COMPONENTS=`.
	fn word(w: &str) -> Option<Option<&str>> {
		if w.eq_ignore_ascii_case("/S") {
			return Some(None);
		}
		let (name, list) = w.split_at_checked(12)?;
		name.eq_ignore_ascii_case("/COMPONENTS=")
			.then_some(Some(list))
	}

	pub fn of(args: &str) -> Switches {
		let mut out = Switches::default();
		for w in words_of(args) {
			match Switches::word(&w) {
				Some(None) => out.silent = true,
				Some(Some(list)) => {
					out.components = Some(
						list.split(',')
							.map(str::trim)
							.filter(|c| !c.is_empty())
							.map(str::to_string)
							.collect(),
					);
				}
				None => {}
			}
		}
		out
	}
}

/// The words of a command tail; a word in double quotes may hold spaces.
pub fn words_of(args: &str) -> Vec<String> {
	let mut out = Vec::new();
	let mut cur = String::new();
	let mut quoted = false;
	let mut any = false;
	for c in args.chars() {
		match c {
			'"' => {
				quoted = !quoted;
				any = true;
			}
			c if c.is_whitespace() && !quoted => {
				if any || !cur.is_empty() {
					out.push(core::mem::take(&mut cur));
					any = false;
				}
			}
			c => cur.push(c),
		}
	}
	if any || !cur.is_empty() {
		out.push(cur);
	}
	out
}

/// The result of a flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome2 {
	Finished,
	Cancelled,
	Failed(String),
}

struct Session<'a, P: Platform> {
	p: &'a mut P,
	project: &'a Project,
	/// `None` for an uninstall run from the install folder, which has no payload.
	pack: Option<Pack>,
	v: Values,
	/// `/S`: no page is shown.
	silent: bool,
}

/// Opens `SETUP.LPK` through the platform and reads the compiled project, its first entry.
pub fn load_project<P: Platform>(p: &mut P) -> Result<(Project, Pack), Error> {
	let mut pack = Pack::open(p)?;
	let bytes = pack.project_bytes(p)?;
	Ok((Project::decode(&bytes)?, pack))
}

/// Runs `SETUP.EXE`'s work and returns the exit code. `args` is the command tail.
pub fn run<P: Platform>(p: &mut P, args: &str) -> i32 {
	let mode = match Mode::from_args(args) {
		Ok(m) => m,
		Err(m) => {
			p.report(&format!("{m}\n"));
			return 2;
		}
	};
	if mode == Mode::Version {
		let host = match p.shell() {
			Some(s) if s.long_names() => "Windows",
			Some(_) => "Windows 3.1",
			None => "DOS",
		};
		p.report(&format!(
			"Liminal Installer for {host} {}\n",
			env!("CARGO_PKG_VERSION")
		));
		if let Ok((project, _)) = load_project(p) {
			p.report(&format!("{} {}\n", project.name, project.version));
		}
		return 0;
	}
	let kept = if matches!(mode, Mode::Uninstall(_)) {
		p.shell().and_then(|s| s.uninstall_project())
	} else {
		None
	};
	let loaded = match kept {
		Some(bytes) => Project::decode(&bytes).map(|pr| (pr, None)),
		None => load_project(p).map(|(pr, pack)| (pr, Some(pack))),
	};
	let (project, pack) = match loaded {
		Ok(x) => x,
		Err(e) => {
			p.report(&format!("SETUP.LPK: {e}\n"));
			return 2;
		}
	};
	let switches = Switches::of(args);
	let mut s = Session {
		p,
		project: &project,
		pack,
		v: Values::default(),
		silent: switches.silent,
	};
	s.seed(&mode);
	if let Some(ids) = &switches.components
		&& let Err(m) = s.choose(ids)
	{
		s.p.report(&format!("{m}\n"));
		return 2;
	}
	let flow_id = if matches!(mode, Mode::Uninstall(_)) {
		"uninstall"
	} else {
		"install"
	};
	let outcome = match s.project.flow(flow_id) {
		Some(flow) => s.run_flow(flow, flow_id == "uninstall"),
		None if flow_id == "uninstall" => s.bare_uninstall(),
		None => Outcome2::Failed("the project has no install flow".to_string()),
	};
	match outcome {
		Outcome2::Finished => 0,
		Outcome2::Cancelled => 3,
		Outcome2::Failed(m) if s.silent => {
			s.p.report(&format!("{m}\n"));
			1
		}
		Outcome2::Failed(m) => {
			s.message(&["Setup could not finish.", &m]);
			1
		}
	}
}

impl<P: Platform> Session<'_, P> {
	fn now(&mut self) -> u32 {
		self.p.millis()
	}

	/// Starts the state: product and target values, the install folder and the chosen components.
	fn seed(&mut self, mode: &Mode) {
		let pr = self.project;
		let (target, renderer) = pr
			.target
			.split_once(':')
			.unwrap_or((&pr.target, "tui-retro"));
		for (k, val) in [
			("product.id", pr.id.as_str()),
			("product.name", &pr.name),
			("product.publisher", &pr.publisher),
			("product.version", &pr.version),
			("target.id", target),
			("target.renderer", renderer),
			(
				"system.os",
				if renderer == "win32-classic" {
					"windows"
				} else {
					"dos"
				},
			),
			("system.arch", "x86"),
			("validate.problems", ""),
		] {
			self.v.set_var(k, val);
		}
		let dir = match mode {
			Mode::Uninstall(Some(d)) => d.clone(),
			_ => pr.default_dir.clone(),
		};
		let dir = match (
			dir.strip_prefix("%PROGRAMFILES%"),
			self.p.shell().and_then(|s| s.program_files()),
		) {
			(Some(rest), Some(pf)) => format!("{}{rest}", pf.trim_end_matches('\\')),
			(Some(rest), None) => format!("C:\\Program Files{rest}"),
			(None, _) => dir,
		};
		let dir = install::normalize_dir_for(self.p, &dir);
		self.v.set_str("install.path", &dir);
		let on: Vec<String> = pr
			.components
			.iter()
			.filter(|c| c.required || c.default)
			.map(|c| c.id.clone())
			.collect();
		self.v
			.state
			.insert("components.selected".into(), Value::List(on));
		self.refresh();
	}

	/// Takes exactly these components, besides the required ones.
	fn choose(&mut self, ids: &[String]) -> Result<(), String> {
		let known = &self.project.components;
		if let Some(bad) = ids
			.iter()
			.find(|i| !known.iter().any(|c| c.id.eq_ignore_ascii_case(i)))
		{
			return Err(format!("unknown component {bad}"));
		}
		let on = known
			.iter()
			.filter(|c| c.required || ids.iter().any(|i| c.id.eq_ignore_ascii_case(i)))
			.map(|c| c.id.clone())
			.collect();
		self.v
			.state
			.insert("components.selected".into(), Value::List(on));
		self.refresh();
		Ok(())
	}

	fn selected(&self) -> Vec<String> {
		self.v
			.list("components.selected")
			.cloned()
			.unwrap_or_default()
	}

	fn dir(&mut self) -> String {
		install::normalize_dir_for(
			&mut *self.p,
			self.v
				.str("install.path")
				.unwrap_or(&self.project.default_dir),
		)
	}

	/// Recomputes what depends on the choices made so far.
	fn refresh(&mut self) {
		let dir = self.dir();
		let sel = self.selected();
		let need = install::bytes_needed(self.project, &sel);
		let files = install::selected_files(self.project, &sel);
		let free = self.p.free_space(&dir);
		self.v.set_var("install.size", &human(need));
		self.v.set_var(
			"system.free_space",
			&free.map_or_else(|| "unknown".to_string(), human),
		);
		let after = free.map_or_else(|| "unknown".to_string(), |f| human(f.saturating_sub(need)));
		self.v.set_var("system.free_space_after", &after);
		self.v
			.set_var("system.free_bytes", &free.unwrap_or(0).to_string());
		let installed = Receipt::read(self.p, &dir).is_ok();
		self.v
			.set_var("system.installed", if installed { "true" } else { "false" });
		let summary = format!("{} files, {} in {} components", files.len(), human(need), {
			self.project
				.components
				.iter()
				.filter(|c| c.required || sel.contains(&c.id))
				.count()
		});
		self.v.set_var("plan.summary", &summary);
	}

	fn show(&mut self, page: &Page) {
		let mut cells = Cells::default();
		page.draw(&self.project.strings, &self.v, &mut cells);
		self.p.present(&cells);
		self.p.cursor(None);
		let said = page.describe_focus(&self.project.strings, &self.v);
		self.p.announce(&said);
		let t = cells.text();
		self.p.dump(&t);
	}

	/// A small modal box with an OK button, over the current screen.
	fn message(&mut self, lines: &[&str]) {
		if let Some(g) = self.p.gui() {
			g.message(&lines.join("\n"));
			return;
		}
		let mut cells = Cells::default();
		cells.fill(
			crate::model::Rect {
				x: 0,
				y: 0,
				w: 80,
				h: 25,
			},
			0xb0,
			self.project.theme.base,
		);
		let w = lines
			.iter()
			.map(|l| l.chars().count())
			.max()
			.unwrap_or(10)
			.clamp(12, 70)
			+ 4;
		let h = lines.len() + 4;
		let (x, y) = ((80 - w) / 2, (25 - h) / 2);
		let r = crate::model::Rect {
			x: x as u8,
			y: y as u8,
			w: w as u8,
			h: h as u8,
		};
		cells.fill(r, b' ', self.project.theme.dialog);
		for (i, l) in lines.iter().enumerate() {
			for (j, line) in crate::text::wrap(l, w - 4).into_iter().take(1).enumerate() {
				cells.put(x + 2, y + 1 + i + j, &line, self.project.theme.dialog, 80);
			}
		}
		let ok = "[ OK ]";
		cells.put(
			x + (w - ok.len()) / 2,
			y + h - 2,
			ok,
			self.project.theme.focus,
			80,
		);
		self.p.present(&cells);
		let t = cells.text();
		self.p.dump(&t);
		loop {
			match self.p.wait_key(0) {
				Some(Key::Enter | Key::Esc | Key::Char(b' ')) | None => return,
				Some(_) => {}
			}
		}
	}

	fn scope_holds(&self, code: &[u8]) -> bool {
		expr::holds(code, &self.project.strings, &self.v)
	}

	fn run_flow(&mut self, flow: &Flow, uninstall: bool) -> Outcome2 {
		let mut cur = usize::from(flow.start);
		let mut history: Vec<usize> = Vec::new();
		let mut guard = 0;
		'steps: loop {
			guard += 1;
			if guard > 10_000 {
				return Outcome2::Failed("the flow did not finish".to_string());
			}
			let step = &flow.steps[cur];
			if !self.scope_holds(&step.when) {
				match step.next {
					Some(n) => {
						cur = usize::from(n);
						continue;
					}
					None => return Outcome2::Failed(format!("step {} has no next step", step.id)),
				}
			}
			let after = step.next.map(usize::from);
			match step.kind {
				Kind::Page => {
					self.enter(step);
					let action = if self.silent {
						Action::Next
					} else {
						self.page(step)
					};
					match action {
						Action::Cancel => return Outcome2::Cancelled,
						Action::Back => {
							if let Some(prev) = history.pop() {
								cur = prev;
								continue 'steps;
							}
							continue 'steps;
						}
						Action::Next => {}
					}
					history.push(cur);
					match after {
						Some(n) => cur = n,
						None => {
							return Outcome2::Failed(format!("step {} has no next step", step.id));
						}
					}
				}
				Kind::Action => {
					self.enter(step);
					match after {
						Some(n) => cur = n,
						None => {
							return Outcome2::Failed(format!("step {} has no next step", step.id));
						}
					}
				}
				Kind::Branch => {
					let chosen = step
						.branches
						.iter()
						.find(|(c, _)| self.scope_holds(c))
						.map(|(_, n)| usize::from(*n));
					match chosen.or(after) {
						Some(n) => cur = n,
						None => {
							return Outcome2::Failed(format!("step {} has no next step", step.id));
						}
					}
				}
				Kind::Commit => {
					match self.commit(step, uninstall) {
						Ok(()) => {}
						Err(Error::Cancelled) => return Outcome2::Cancelled,
						Err(e) => return Outcome2::Failed(e.to_string()),
					}
					history.clear();
					match after {
						Some(n) => cur = n,
						None => {
							return Outcome2::Failed(format!("step {} has no next step", step.id));
						}
					}
				}
				Kind::Finish => {
					if !self.silent {
						self.page(step);
					}
					return Outcome2::Finished;
				}
			}
		}
	}

	/// Runs the step's built-in action on entry.
	fn enter(&mut self, step: &Step) {
		if step.builtin == Builtin::DetectSound {
			crate::sound::detect(self.p, &mut self.v);
		}
		self.refresh();
	}

	/// Whether the page may be left with Next. Carries out `check-disk-space`.
	fn validate(&mut self, step: &Step) -> bool {
		if step.builtin != Builtin::CheckDiskSpace {
			return true;
		}
		self.refresh();
		let dir = self.dir();
		let need = install::bytes_needed(self.project, &self.selected());
		match self.p.free_space(&dir) {
			Some(free) if free < need => {
				let drive = dir.get(..2).unwrap_or("the drive");
				self.message(&[
					&format!("Not enough disk space on {drive}"),
					&format!("Setup needs {} and {} is free.", human(need), human(free)),
					"Choose another folder or free some space.",
				]);
				false
			}
			_ => true,
		}
	}

	fn page(&mut self, step: &Step) -> Action {
		let Some(screen) = step.screen else {
			return Action::Next;
		};
		self.p.step(&step.id);
		let now = self.now();
		let mut page = Page::new(self.project, usize::from(screen), &mut self.v, now);
		self.refresh();
		if self.p.gui().is_some() {
			return self.gui_page(step, usize::from(screen), &page);
		}
		self.show(&page);
		loop {
			let key = self.p.wait_key(250);
			let now = self.now();
			let mut redraw = page.tick(now);
			if let Some(key) = key {
				match page.key(key, &self.project.strings, &mut self.v) {
					Outcome::None => {}
					Outcome::Redraw => {
						self.refresh();
						redraw = true;
					}
					Outcome::Cancel => {
						return if step.kind == Kind::Finish {
							Action::Next
						} else {
							Action::Cancel
						};
					}
					Outcome::Activate(a) => match a.as_str() {
						"next" | "commit" | "finish" | "ok" => {
							if self.validate(step) {
								return Action::Next;
							}
							redraw = true;
						}
						"back" => return Action::Back,
						"cancel" => {
							return if step.kind == Kind::Finish {
								Action::Next
							} else {
								Action::Cancel
							};
						}
						"test" => {
							self.p.beep(880, 150);
							self.p.beep(1320, 150);
						}
						_ => {}
					},
				}
			}
			if redraw {
				self.show(&page);
			}
		}
	}

	/// The pages a person will see in this flow, for a "Step 2 of 7" caption.
	fn page_count(&self, step: &Step) -> usize {
		let flow = ["install", "uninstall"]
			.iter()
			.filter_map(|id| self.project.flow(id))
			.find(|f| f.steps.iter().any(|s| s.id == step.id));
		flow.map_or(1, |f| {
			f.steps
				.iter()
				.filter(|s| s.screen.is_some() && s.kind != Kind::Finish)
				.count()
				.max(1)
		})
	}

	/// A page on a native window: the window keeps its controls and returns on every change or
	/// button; the values are refreshed between calls.
	fn gui_page(&mut self, step: &Step, screen: usize, _page: &Page) -> Action {
		let project = self.project;
		let scr = &project.screens[screen];
		let total = self.page_count(step);
		let number = project_position(project, step).min(total);
		let mut fresh = true;
		loop {
			self.refresh();
			let title = self.v.interpolate(&scr.title);
			let event = {
				let Some(g) = self.p.gui() else {
					return Action::Next;
				};
				let mut gp = GuiPage {
					screen: scr,
					strings: &project.strings,
					values: &mut self.v,
					step: &step.id,
					kind: step.kind,
					product: &project.name,
					title,
					number,
					total,
					fresh,
				};
				g.page(&mut gp)
			};
			fresh = false;
			match event {
				GuiEvent::Changed => {}
				GuiEvent::Action(a) => match a.as_str() {
					"next" | "commit" | "finish" | "ok" => {
						if self.validate(step) {
							return Action::Next;
						}
						fresh = true;
					}
					"back" => return Action::Back,
					"cancel" => {
						return if step.kind == Kind::Finish {
							Action::Next
						} else {
							Action::Cancel
						};
					}
					_ => {}
				},
			}
		}
	}

	/// The commit step: shows its page and runs the install, or the uninstall in the uninstall flow.
	fn commit(&mut self, step: &Step, uninstall: bool) -> Result<(), Error> {
		let dir = self.dir();
		let sel = self.selected();
		let now = self.now();
		let shown = step.screen.filter(|_| !self.silent);
		let page = shown.map(|s| Page::new(self.project, usize::from(s), &mut self.v, now));
		self.refresh();
		let project = self.project;
		let values = self.v.clone();
		let mut live = Live {
			project,
			v: self.v.clone(),
			page,
			screen: shown.map(usize::from),
			step: step.id.clone(),
			kind: step.kind,
			fresh: true,
			last: 0,
		};
		let p = &mut *self.p;
		let result = if uninstall {
			install::uninstall(p, &dir, &mut |p, pr| live.update(p, pr)).map(|_| ())
		} else {
			let Some(pack) = self.pack.as_mut() else {
				return Err(Error::Format(
					"this run has no payload to install".to_string(),
				));
			};
			install::install(p, project, pack, &dir, &sel, &values, &mut |p, pr| {
				live.update(p, pr)
			})
		};
		self.v.file_pct = 100;
		self.v.total_pct = 100;
		result
	}

	/// An uninstall for a project with no uninstall flow: no pages, just the work.
	fn bare_uninstall(&mut self) -> Outcome2 {
		let dir = self.dir();
		match install::uninstall(self.p, &dir, &mut |_, _| true) {
			Ok(_) => Outcome2::Finished,
			Err(Error::Cancelled) => Outcome2::Cancelled,
			Err(e) => Outcome2::Failed(e.to_string()),
		}
	}
}

/// The commit page while the work runs: it shows the progress and watches for Esc.
struct Live<'a> {
	project: &'a Project,
	v: Values,
	page: Option<Page>,
	screen: Option<usize>,
	step: String,
	kind: Kind,
	fresh: bool,
	last: u32,
}

impl Live<'_> {
	/// Returns false to cancel.
	fn update<P: Platform>(&mut self, p: &mut P, pr: &Progress) -> bool {
		let keep = !p.cancel_requested();
		let now = p.millis();
		let boundary = pr.file_pct == 100 || pr.files_done == 0;
		if !boundary && now.wrapping_sub(self.last) < 60 {
			return keep;
		}
		self.last = now;
		self.v.set_var("progress.file", &pr.file);
		self.v.file_pct = pr.file_pct;
		self.v.total_pct = (pr.bytes_done * 100)
			.checked_div(pr.bytes_total)
			.unwrap_or(100) as u32;
		self.v.free_pct = ((pr.free.min(pr.free_at_start) * 100 + pr.free_at_start / 2)
			.checked_div(pr.free_at_start))
		.unwrap_or(100) as u32;
		let pct = self.v.total_pct.to_string();
		self.v.set_var("progress.percent", &pct);
		self.v.set_var("system.free_space", &human(pr.free));
		if let Some(sc) = self.screen
			&& p.gui().is_some()
		{
			let scr = &self.project.screens[sc];
			let title = self.v.interpolate(&scr.title);
			let fresh = core::mem::replace(&mut self.fresh, false);
			let mut gp = GuiPage {
				screen: scr,
				strings: &self.project.strings,
				values: &mut self.v,
				step: &self.step,
				kind: self.kind,
				product: &self.project.name,
				title,
				number: 0,
				total: 0,
				fresh,
			};
			let cont = p.gui().is_some_and(|g| g.working(&mut gp, pr));
			return keep && cont;
		}
		if let Some(pg) = self.page.as_mut() {
			pg.tick(now);
			let mut cells = Cells::default();
			pg.draw(&self.project.strings, &self.v, &mut cells);
			p.present(&cells);
			let t = cells.text();
			p.dump(&t);
		}
		keep
	}
}

/// The 1-based place of a step among the flow's steps that show a page.
fn project_position(project: &Project, step: &Step) -> usize {
	project
		.flows
		.iter()
		.find(|f| f.steps.iter().any(|s| s.id == step.id))
		.map_or(1, |f| {
			f.steps
				.iter()
				.filter(|s| s.screen.is_some() && s.kind != Kind::Finish)
				.position(|s| s.id == step.id)
				.map_or(1, |i| i + 1)
		})
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Action {
	Next,
	Back,
	Cancel,
}
