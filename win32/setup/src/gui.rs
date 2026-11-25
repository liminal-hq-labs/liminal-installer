// The wizard: a classic Windows 95 setup window built from a page's widgets, with real controls.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! One window for the whole run. Each page destroys the last page's controls and makes its own
//! (push buttons, check boxes, radio buttons, edit controls, a read-only multi-line edit for the
//! licence, a `msctls_progress32` bar for each meter, `SHBrowseForFolder` behind a Browse button),
//! and the Back, Next and Cancel buttons sit in a bar at the foot. A teal banner on the left names the
//! product and the step. The window is a plain overlapped window with `IsDialogMessage` doing the keys,
//! so Tab, the arrows in a radio group, Enter on the default button and Esc work as in a dialog.
//!
//! `/PLAY` drives the real controls from a script (`BM_CLICK` on a button, text into an edit
//! control) and `/DUMP` writes the page text read back from them, which is how the Wine and Windows ME
//! runs are checked against the host's.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ptr::{null, null_mut};
use core::sync::atomic::{AtomicPtr, Ordering};

use li_retro::gui::{Gui, GuiEvent, GuiPage, meter_caption};
use li_retro::install::Progress;
use li_retro::model::{Kind, Source, Widget};

use crate::shell;
use crate::sys::*;
use crate::text::{ansi, utf8};

const WIN_W: i32 = 520;
const BANNER_W: i32 = 150;
const PAD: i32 = 14;
const BAR_H: i32 = 52;
const BTN_W: i32 = 78;
const BTN_H: i32 = 23;
/// The height of the page area, the same on every page so the window never changes size: tall
/// enough for the page with the most on it (the copying page of a product with a billboard and a
/// list of steps), or the most a 640 by 480 screen holds.
const CONTENT_H: i32 = 450;
const CONTENT_H_SMALL: i32 = 340;
const BAR_ID: i32 = 1000;
const TIMER: usize = 1;
/// The widget index of the meters a page gets that its layout did not name.
const EXTRA: usize = usize::MAX - 1;
const CLASS: &[u8] = b"LiminalSetupWizard\0";
/// The teal of the Windows 95 setup banner.
const TEAL: u32 = 0x0080_8000;

static WIZARD: AtomicPtr<Wizard> = AtomicPtr::new(null_mut());

/// One thing a script does on a page.
#[derive(Debug, Clone)]
enum Op {
	Set(String, String),
	Toggle(String),
	Choose(String, String),
	Click(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Role {
	/// Text that follows the values: its template.
	Text(String),
	Doc,
	Heading,
	CheckItem {
		bind: String,
		id: String,
		label: String,
	},
	RadioItem {
		bind: String,
		id: String,
		label: String,
		default: bool,
	},
	/// The label of a list or a field, with its template.
	Caption(String),
	Edit(String),
	Browse(String),
	/// A progress bar and its source.
	Meter(Source),
	Percent(Source),
	Slide {
		slides: Vec<String>,
		interval: u8,
	},
	/// A push button inside the page.
	Button(String),
}

struct Ctl {
	hwnd: Handle,
	id: i32,
	role: Role,
	/// The widget this control came from.
	widget: usize,
	/// The text last put in the control, so an unchanged value is not written again.
	shown: String,
}

struct BarButton {
	hwnd: Handle,
	action: String,
	widget: usize,
}

pub struct Wizard {
	hwnd: Handle,
	font: Handle,
	bold: Handle,
	big: Handle,
	ctls: Vec<Ctl>,
	bar: Vec<BarButton>,
	next_id: i32,
	event: Option<GuiEvent>,
	/// True while the program changes controls itself, so their notifications are ignored.
	quiet: bool,
	kind: Kind,
	banner: String,
	step_caption: String,
	client_h: i32,
	/// The height the page's own controls asked for, for the geometry log.
	need_h: i32,
	/// The person (or the script) cancelled while the work ran.
	cancelled: bool,
	started: u32,
	/// The script from `/PLAY`, by step.
	script: Option<BTreeMap<String, VecDeque<Op>>>,
	pub dump: Handle,
	/// A folder to save a bitmap of each page in (`/SHOT`).
	pub shots: Option<String>,
	shot_count: u32,
	guard: u32,
	step: String,
	title: String,
}

impl Wizard {
	pub fn new() -> Wizard {
		Wizard {
			hwnd: NULL,
			font: NULL,
			bold: NULL,
			big: NULL,
			ctls: Vec::new(),
			bar: Vec::new(),
			next_id: 100,
			event: None,
			quiet: false,
			kind: Kind::Page,
			banner: String::new(),
			step_caption: String::new(),
			client_h: 0,
			need_h: 0,
			cancelled: false,
			started: 0,
			script: None,
			dump: NULL,
			shots: None,
			shot_count: 0,
			guard: 0,
			step: String::new(),
			title: String::new(),
		}
	}

	/// Reads a `/PLAY` script: `step op args`, with `set bind value`, `toggle id`, `choose bind id`,
	/// `click action` and `#` comments. A page with nothing left in its script presses its default button.
	pub fn play(&mut self, text: &str) {
		let mut map: BTreeMap<String, VecDeque<Op>> = BTreeMap::new();
		for line in text.lines() {
			let line = line.trim();
			if line.is_empty() || line.starts_with('#') {
				continue;
			}
			let mut it = line.splitn(3, ' ');
			let (Some(step), Some(op)) = (it.next(), it.next()) else {
				continue;
			};
			let rest = it.next().unwrap_or("").trim();
			let op = match op {
				"set" => rest
					.split_once(' ')
					.map(|(b, v)| Op::Set(b.to_string(), v.trim().to_string())),
				"toggle" => Some(Op::Toggle(rest.to_string())),
				"choose" => rest
					.split_once(' ')
					.map(|(b, v)| Op::Choose(b.to_string(), v.trim().to_string())),
				"click" => Some(Op::Click(rest.to_string())),
				_ => None,
			};
			if let Some(op) = op {
				map.entry(step.to_string()).or_default().push_back(op);
			}
		}
		self.script = Some(map);
	}

	/// The window's rectangle and the height its page asked for, one `geometry:` line per page, so
	/// a run can check that the window stays put and keeps its size.
	fn log_geometry(&self) {
		let mut r = Rect {
			left: 0,
			top: 0,
			right: 0,
			bottom: 0,
		};
		// SAFETY: reads the window's rectangle into a local.
		unsafe { GetWindowRect(self.hwnd, &mut r) };
		self.log(&format!(
			"geometry: {},{} {}x{} need {}\n",
			r.left,
			r.top,
			r.right - r.left,
			r.bottom - r.top,
			self.need_h
		));
	}

	pub fn log(&self, text: &str) {
		if self.dump.is_null() {
			return;
		}
		let mut n = 0u32;
		// SAFETY: a write of the string's bytes to the dump file.
		unsafe { WriteFile(self.dump, text.as_ptr(), text.len() as u32, &mut n, null_mut()) };
	}

	fn ensure_window(&mut self) {
		if !self.hwnd.is_null() {
			return;
		}
		WIZARD.store(self as *mut Wizard, Ordering::Relaxed);
		// SAFETY: standard window class and window creation with NUL-ended names.
		unsafe {
			InitCommonControls();
			let instance = GetModuleHandleA(null());
			let class = WndClassA {
				style: CS_HREDRAW | CS_VREDRAW,
				wnd_proc,
				cls_extra: 0,
				wnd_extra: 0,
				instance,
				icon: NULL,
				cursor: LoadCursorA(NULL, IDC_ARROW as *const u8),
				background: (COLOR_BTNFACE + 1) as Handle,
				menu_name: null(),
				class_name: CLASS.as_ptr(),
			};
			RegisterClassA(&class);
			self.font = GetStockObject(DEFAULT_GUI_FONT);
			let face = b"MS Sans Serif\0";
			self.bold = CreateFontA(-11, 0, 0, 0, FW_BOLD, 0, 0, 0, 0, 0, 0, 0, 0, face.as_ptr());
			self.big = CreateFontA(-19, 0, 0, 0, FW_BOLD, 0, 0, 0, 0, 0, 0, 0, 0, face.as_ptr());
			if self.bold.is_null() {
				self.bold = self.font;
			}
			if self.big.is_null() {
				self.big = self.bold;
			}
			self.hwnd = CreateWindowExA(
				WS_EX_CONTROLPARENT,
				CLASS.as_ptr(),
				b"Setup\0".as_ptr(),
				WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN,
				100,
				100,
				WIN_W,
				400,
				NULL,
				NULL,
				instance,
				null(),
			);
			self.started = GetTickCount();
		}
	}

	fn make(&mut self, class: &str, text: &str, style: u32, ex: u32, r: (i32, i32, i32, i32)) -> (Handle, i32) {
		let id = self.next_id;
		self.next_id += 1;
		let (c, t) = (ansi(class), ansi(text));
		// SAFETY: a child window with NUL-ended class and text, owned by the wizard window.
		let hwnd = unsafe {
			let h = CreateWindowExA(
				ex,
				c.as_ptr(),
				t.as_ptr(),
				WS_CHILD | WS_VISIBLE | style,
				r.0,
				r.1,
				r.2,
				r.3,
				self.hwnd,
				id as isize as Handle,
				GetModuleHandleA(null()),
				null(),
			);
			SendMessageA(h, WM_SETFONT, self.font as usize, 1);
			h
		};
		(hwnd, id)
	}

	/// A captioned progress bar with its percentage.
	fn meter(&mut self, x0: i32, w: i32, y: &mut i32, caption: &str, source: Source, widget: usize) {
		let (hw, id) = self.make("STATIC", caption, SS_LEFT | SS_NOPREFIX, 0, (x0, *y, w - 44, 15));
		self.push(hw, id, Role::Caption(caption.to_string()), widget, caption);
		let (pw, pid) = self.make(
			"msctls_progress32",
			"",
			PBS_SMOOTH,
			WS_EX_CLIENTEDGE,
			(x0, *y + 17, w - 44, 18),
		);
		// SAFETY: sets the range 0 to 100 of the progress bar just made (PBM_SETRANGE, which comctl32 4.0 on Windows 95 has).
		unsafe { SendMessageA(pw, PBM_SETRANGE, 0, 100 << 16) };
		self.push(pw, pid, Role::Meter(source), widget, "");
		let (tw, tid) = self.make("STATIC", "0%", SS_LEFT | SS_NOPREFIX, 0, (x0 + w - 40, *y + 19, 40, 15));
		self.push(tw, tid, Role::Percent(source), widget, "0%");
		*y += 44;
	}

	fn clear(&mut self) {
		self.quiet = true;
		for c in self.ctls.drain(..) {
			// SAFETY: destroys a child this wizard made.
			unsafe { DestroyWindow(c.hwnd) };
		}
		for b in self.bar.drain(..) {
			// SAFETY: as above.
			unsafe { DestroyWindow(b.hwnd) };
		}
		self.quiet = false;
	}

	/// The height of `text` wrapped to `width`, in the page font.
	fn measure(&self, text: &str, width: i32, font: Handle) -> i32 {
		let t = ansi(text);
		let mut r = Rect {
			left: 0,
			top: 0,
			right: width,
			bottom: 0,
		};
		// SAFETY: measures into a rectangle with a screen DC that is released.
		unsafe {
			let dc = GetDC(self.hwnd);
			let old = SelectObject(dc, font);
			DrawTextA(dc, t.as_ptr(), -1, &mut r, DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX);
			SelectObject(dc, old);
			ReleaseDC(self.hwnd, dc);
		}
		r.bottom.max(13)
	}

	fn push(&mut self, hwnd: Handle, id: i32, role: Role, widget: usize, shown: &str) {
		self.ctls.push(Ctl {
			hwnd,
			id,
			role,
			widget,
			shown: shown.to_string(),
		});
	}

	/// Makes the controls for a page and sizes the window to them.
	fn build(&mut self, page: &mut GuiPage<'_>) {
		self.clear();
		self.quiet = true;
		self.kind = page.kind;
		self.step = page.step.to_string();
		self.title = page.title.clone();
		self.banner = page.product.to_string();
		self.step_caption = if page.total > 0 {
			format!("Step {} of {}", page.number, page.total)
		} else {
			String::new()
		};
		self.cancelled = false;
		self.guard = 0;
		let x0 = BANNER_W + PAD;
		let w = WIN_W - 2 * 3 - x0 - PAD;
		let mut y = PAD;
		// The heading.
		if !page.title.is_empty() {
			let (h, id) = self.make("STATIC", &page.title, SS_LEFT | SS_NOPREFIX, 0, (x0, y, w, 20));
			// SAFETY: sets the font of a control just made.
			unsafe { SendMessageA(h, WM_SETFONT, self.bold as usize, 1) };
			self.push(h, id, Role::Heading, usize::MAX, &page.title);
			y += 28;
		}
		let mut bar_actions: Vec<(usize, String, String, u8)> = Vec::new();
		let mut tab_first = true;
		for (wi, widget) in page.screen.widgets.iter().enumerate() {
			match widget {
				Widget::Text { template, .. } => {
					let body = page.text(template);
					let slack = if template.contains('{') { 13 } else { 0 };
					let h = self.measure(&body, w, self.font) + slack;
					let (hw, id) = self.make("STATIC", &body, SS_LEFT | SS_NOPREFIX, 0, (x0, y, w, h));
					self.push(hw, id, Role::Text(template.clone()), wi, &body);
					y += h + 8;
				}
				Widget::Doc { text, .. } => {
					let crlf = text.replace('\n', "\r\n");
					let (hw, id) = self.make(
						"EDIT",
						&crlf,
						ES_MULTILINE | ES_READONLY | WS_VSCROLL | WS_TABSTOP,
						WS_EX_CLIENTEDGE,
						(x0, y, w, 140),
					);
					self.push(hw, id, Role::Doc, wi, &crlf);
					y += 148;
				}
				Widget::Field { label, bind, .. } => {
					let cap = page.text(label);
					if !cap.is_empty() {
						let (hw, id) = self.make("STATIC", &cap, SS_LEFT | SS_NOPREFIX, 0, (x0, y, w, 15));
						self.push(hw, id, Role::Caption(label.clone()), wi, &cap);
						y += 17;
					}
					let browse = bind == "install.path";
					let ew = if browse { w - BTN_W - 8 } else { w };
					let value = page.values.str(bind).unwrap_or("").to_string();
					let (hw, id) = self.make("EDIT", &value, ES_AUTOHSCROLL | WS_TABSTOP, WS_EX_CLIENTEDGE, (x0, y, ew, 23));
					// SAFETY: limits the text of the edit control just made.
					unsafe { SendMessageA(hw, EM_SETLIMITTEXT, 259, 0) };
					self.push(hw, id, Role::Edit(bind.clone()), wi, &value);
					if browse {
						let (bh, bid) = self.make(
							"BUTTON",
							"B&rowse...",
							BS_PUSHBUTTON | WS_TABSTOP,
							0,
							(x0 + ew + 8, y, BTN_W, BTN_H),
						);
						self.push(bh, bid, Role::Browse(bind.clone()), wi, "Browse...");
					}
					y += 31;
				}
				Widget::List {
					check,
					label,
					bind,
					items,
					default,
					..
				} => {
					let cap = page.text(label);
					if !cap.is_empty() {
						let (hw, id) = self.make("STATIC", &cap, SS_LEFT | SS_NOPREFIX, 0, (x0, y, w, 15));
						self.push(hw, id, Role::Caption(label.clone()), wi, &cap);
						y += 18;
					}
					for (n, item) in items.iter().enumerate() {
						let text = page.text(&item.label);
						let mut style = WS_TABSTOP | if *check { BS_AUTOCHECKBOX } else { BS_AUTORADIOBUTTON };
						if n == 0 && !*check {
							style |= WS_GROUP;
						}
						if item.required {
							style |= WS_DISABLED;
						}
						let (hw, id) = self.make("BUTTON", &text, style, 0, (x0 + 8, y, w - 8, 18));
						let on = if *check {
							page.values.list(bind).is_some_and(|l| l.contains(&item.id))
						} else {
							match page.values.str(bind) {
								Some(s) => s == item.id,
								None => *default == item.id,
							}
						};
						// SAFETY: sets the state of the button just made.
						unsafe { SendMessageA(hw, BM_SETCHECK, usize::from(on), 0) };
						let role = if *check {
							Role::CheckItem {
								bind: bind.clone(),
								id: item.id.clone(),
								label: item.label.clone(),
							}
						} else {
							Role::RadioItem {
								bind: bind.clone(),
								id: item.id.clone(),
								label: item.label.clone(),
								default: *default == item.id,
							}
						};
						self.push(hw, id, role, wi, &text);
						y += 21;
						tab_first = false;
					}
					y += 6;
				}
				Widget::Meter { label, source, .. } => {
					let cap = meter_caption(label, *source).to_string();
					self.meter(x0, w, &mut y, &cap, *source, wi);
				}
				Widget::Billboard { slides, interval, .. } => {
					let first = slides.first().map_or(String::new(), |s| page.text(s));
					let (hw, id) = self.make("STATIC", &first, SS_LEFT | SS_NOPREFIX, WS_EX_STATICEDGE, (x0, y, w, 64));
					self.push(
						hw,
						id,
						Role::Slide {
							slides: slides.iter().map(|s| page.text(s)).collect(),
							interval: *interval,
						},
						wi,
						&first,
					);
					y += 72;
				}
				Widget::Button {
					label, action, hotkey, ..
				} => {
					let wizard_button = matches!(action.as_str(), "back" | "next" | "commit" | "finish" | "ok" | "cancel");
					if wizard_button {
						bar_actions.push((wi, action.clone(), page.text(label), *hotkey));
					} else {
						let text = with_hotkey(&page.text(label), *hotkey);
						let (hw, id) = self.make("BUTTON", &text, BS_PUSHBUTTON | WS_TABSTOP, 0, (x0, y, BTN_W + 20, BTN_H));
						self.push(hw, id, Role::Button(action.clone()), wi, &page.text(label));
						y += BTN_H + 8;
					}
				}
			}
		}
		for (label, src) in page.extra_meters() {
			self.meter(x0, w, &mut y, label, *src, EXTRA);
		}
		let _ = tab_first;
		self.need_h = y + PAD;
		// SAFETY: reads the screen's height.
		let floor = if unsafe { GetSystemMetrics(SM_CYSCREEN) } >= 600 {
			CONTENT_H
		} else {
			CONTENT_H_SMALL
		};
		let content_h = floor.max(y + PAD);
		self.client_h = content_h + BAR_H;
		// The separator above the bar and the bar's buttons: Back and Next together, Cancel after a gap.
		let (sep, sid) = self.make("STATIC", "", SS_ETCHEDHORZ, 0, (0, content_h, WIN_W - 6, 2));
		self.push(sep, sid, Role::Heading, usize::MAX, "");
		let total: i32 = bar_actions.len() as i32 * (BTN_W + 6) + 10;
		let mut bx = WIN_W - 6 - PAD - total.max(0) + 6;
		let by = content_h + 14;
		let default_action = page
			.screen
			.widgets
			.iter()
			.find_map(|w| match w {
				Widget::Button {
					default: true, action, ..
				} => Some(action.clone()),
				_ => None,
			})
			.unwrap_or_default();
		for (n, (wi, action, label, hotkey)) in bar_actions.iter().enumerate() {
			if action == "cancel" && n > 0 {
				bx += 10;
			}
			let is_default = *action == default_action;
			let style = WS_TABSTOP | if is_default { BS_DEFPUSHBUTTON } else { BS_PUSHBUTTON };
			let (hw, _) = self.make("BUTTON", &with_hotkey(label, *hotkey), style, 0, (bx, by, BTN_W, BTN_H));
			// A bar button's id is its index, so a click finds its action.
			// SAFETY: changes the control id of the button just made.
			unsafe { SetWindowLongA(hw, GWL_ID, BAR_ID + n as i32) };
			self.bar.push(BarButton {
				hwnd: hw,
				action: action.clone(),
				widget: *wi,
			});
			bx += BTN_W + 6;
		}
		self.update_states(page);
		// SAFETY: sizes and shows the window; focus goes to the default button or the first control.
		unsafe {
			let mut r = Rect {
				left: 0,
				top: 0,
				right: WIN_W - 6,
				bottom: self.client_h,
			};
			AdjustWindowRect(&mut r, WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX, 0);
			let (ww, wh) = (r.right - r.left, r.bottom - r.top);
			let (sw, sh) = (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN));
			SetWindowPos(
				self.hwnd,
				NULL,
				((sw - ww) / 2).max(0),
				((sh - wh) / 3).max(0),
				ww,
				wh,
				SWP_NOZORDER,
			);
			let caption = ansi(&format!("{} Setup", page.product));
			SetWindowTextA(self.hwnd, caption.as_ptr());
			ShowWindow(self.hwnd, SW_SHOW);
			InvalidateRect(self.hwnd, null(), 1);
			UpdateWindow(self.hwnd);
			SetForegroundWindow(self.hwnd);
			KillTimer(self.hwnd, TIMER);
			if self.ctls.iter().any(|c| matches!(c.role, Role::Slide { .. })) {
				SetTimer(self.hwnd, TIMER, 500, null());
			}
			let focus = self
				.bar
				.iter()
				.find(|b| b.action == default_action && IsWindowEnabled(b.hwnd) != 0)
				.map(|b| b.hwnd)
				.or_else(|| {
					self.ctls
						.iter()
						.find(|c| {
							matches!(
								c.role,
								Role::Edit(_) | Role::CheckItem { .. } | Role::RadioItem { .. } | Role::Doc
							)
						})
						.map(|c| c.hwnd)
				})
				.or_else(|| self.bar.first().map(|b| b.hwnd));
			if let Some(f) = focus {
				SetFocus(f);
			}
		}
		self.quiet = false;
	}

	/// Enabled states, meters and the text that follows the values.
	fn update_states(&mut self, page: &mut GuiPage<'_>) {
		self.quiet = true;
		for i in 0..self.ctls.len() {
			let (hwnd, role, widget) = (self.ctls[i].hwnd, self.ctls[i].role.clone(), self.ctls[i].widget);
			match role {
				Role::Text(template) => {
					let body = page.text(&template);
					self.set_text(i, &body);
				}
				Role::Caption(template) => {
					let body = page.text(&template);
					self.set_text(i, &body);
				}
				Role::Edit(bind) => {
					let value = page.values.str(&bind).unwrap_or("").to_string();
					if self.ctls[i].shown != value {
						self.set_text(i, &value);
					}
				}
				Role::CheckItem { bind, id, .. } => {
					let on = page.values.list(&bind).is_some_and(|l| l.contains(&id));
					// SAFETY: sets the state of a check box.
					unsafe { SendMessageA(hwnd, BM_SETCHECK, usize::from(on), 0) };
				}
				Role::Meter(src) => {
					let pct = match src {
						Source::File => page.values.file_pct,
						Source::Total => page.values.total_pct,
						Source::FreeSpace => page.values.free_pct,
					}
					.min(100);
					// SAFETY: sets the position of a progress bar.
					unsafe { SendMessageA(hwnd, PBM_SETPOS, pct as usize, 0) };
				}
				Role::Percent(src) => {
					let pct = match src {
						Source::File => page.values.file_pct,
						Source::Total => page.values.total_pct,
						Source::FreeSpace => page.values.free_pct,
					}
					.min(100);
					self.set_text(i, &format!("{pct}%"));
				}
				_ => {}
			}
			let _ = widget;
		}
		for b in &self.bar {
			let on = page.screen.widgets.get(b.widget).is_some_and(|w| page.enabled(w));
			// SAFETY: enables or disables a button.
			unsafe { EnableWindow(b.hwnd, i32::from(on)) };
		}
		self.quiet = false;
	}

	fn set_text(&mut self, i: usize, text: &str) {
		if self.ctls[i].shown == text {
			return;
		}
		self.ctls[i].shown = text.to_string();
		let t = ansi(text);
		// SAFETY: sets the text of a control.
		unsafe { SetWindowTextA(self.ctls[i].hwnd, t.as_ptr()) };
	}

	/// Reads the controls back into the page's values.
	fn sync(&mut self, page: &mut GuiPage<'_>) {
		let mut lists: BTreeMap<String, Vec<String>> = BTreeMap::new();
		for c in &mut self.ctls {
			match &c.role {
				Role::Edit(bind) => {
					let s = window_text(c.hwnd);
					c.shown = s.clone();
					page.values.set_str(bind, &s);
				}
				Role::CheckItem { bind, id, .. } => {
					let list = lists.entry(bind.clone()).or_default();
					// SAFETY: reads the state of a check box.
					if unsafe { SendMessageA(c.hwnd, BM_GETCHECK, 0, 0) } != 0 {
						list.push(id.clone());
					}
				}
				// SAFETY: reads the state of a radio button.
				Role::RadioItem { bind, id, .. } if unsafe { SendMessageA(c.hwnd, BM_GETCHECK, 0, 0) } != 0 => {
					page.values.set_str(bind, id);
				}
				_ => {}
			}
		}
		for (bind, list) in lists {
			page.values.state.insert(bind, li_retro::ui::Value::List(list));
		}
	}

	/// The page as the controls show it, in `GuiPage::describe`'s format.
	fn describe(&self, page: &GuiPage<'_>) -> String {
		let mut out = format!("== {} | {}\n", self.step, window_text_of_heading(self));
		let mut ci = 0;
		let mut bi = 0;
		for (wi, w) in page.screen.widgets.iter().enumerate() {
			match w {
				Widget::Text { .. } => {
					if let Some(c) = self.ctls.iter().find(|c| c.widget == wi && matches!(c.role, Role::Text(_))) {
						out.push_str(&format!("text: {}\n", window_text(c.hwnd)));
					}
				}
				Widget::Doc { .. } => {
					if let Some(c) = self.ctls.iter().find(|c| c.widget == wi) {
						out.push_str(&format!("doc: {} chars\n", window_text(c.hwnd).chars().count()));
					}
				}
				Widget::Field { .. } => {
					let cap = self
						.ctls
						.iter()
						.find(|c| c.widget == wi && matches!(c.role, Role::Caption(_)));
					let edit = self.ctls.iter().find(|c| c.widget == wi && matches!(c.role, Role::Edit(_)));
					out.push_str(&format!(
						"field: {} = {}\n",
						cap.map_or(String::new(), |c| window_text(c.hwnd)),
						edit.map_or(String::new(), |c| window_text(c.hwnd))
					));
				}
				Widget::List { check, .. } => {
					let cap = self
						.ctls
						.iter()
						.find(|c| c.widget == wi && matches!(c.role, Role::Caption(_)));
					out.push_str(&format!(
						"{}: {}\n",
						if *check { "check" } else { "radio" },
						cap.map_or(String::new(), |c| window_text(c.hwnd))
					));
					for c in self
						.ctls
						.iter()
						.filter(|c| c.widget == wi && matches!(c.role, Role::CheckItem { .. } | Role::RadioItem { .. }))
					{
						// SAFETY: reads the state of a button.
						let on = unsafe { SendMessageA(c.hwnd, BM_GETCHECK, 0, 0) } != 0;
						let mark = match (*check, on) {
							(true, true) => "[x]",
							(true, false) => "[ ]",
							(false, true) => "(o)",
							(false, false) => "( )",
						};
						out.push_str(&format!("  {mark} {}\n", window_text(c.hwnd)));
					}
				}
				Widget::Meter { .. } => {
					if let Some(c) = self
						.ctls
						.iter()
						.find(|c| c.widget == wi && matches!(c.role, Role::Caption(_)))
					{
						out.push_str(&format!("meter: {}\n", window_text(c.hwnd)));
					}
				}
				Widget::Billboard { .. } => {
					if let Some(c) = self.ctls.iter().find(|c| c.widget == wi) {
						out.push_str(&format!("billboard: {}\n", window_text(c.hwnd)));
					}
				}
				Widget::Button { .. } => {
					let hw = if let Some(b) = self.bar.iter().find(|b| b.widget == wi) {
						bi += 1;
						Some(b.hwnd)
					} else {
						self.ctls
							.iter()
							.find(|c| c.widget == wi && matches!(c.role, Role::Button(_)))
							.map(|c| c.hwnd)
					};
					if let Some(h) = hw {
						// SAFETY: asks whether a button is enabled.
						let off = if unsafe { IsWindowEnabled(h) } == 0 {
							" [disabled]"
						} else {
							""
						};
						out.push_str(&format!("button: {}{off}\n", window_text(h).replace('&', "")));
					}
				}
			}
			ci += 1;
		}
		let _ = (ci, bi);
		for c in self
			.ctls
			.iter()
			.filter(|c| c.widget == EXTRA && matches!(c.role, Role::Caption(_)))
		{
			out.push_str(&format!("meter: {}\n", window_text(c.hwnd)));
		}
		out
	}

	fn pump(&mut self, block: bool) -> bool {
		let mut msg = core::mem::MaybeUninit::<Msg>::uninit();
		// SAFETY: standard message loop; the message is initialised by Get/PeekMessage before use.
		unsafe {
			if block {
				let r = GetMessageA(msg.as_mut_ptr(), NULL, 0, 0);
				if r <= 0 {
					return false;
				}
			} else if PeekMessageA(msg.as_mut_ptr(), NULL, 0, 0, PM_REMOVE) == 0 {
				return false;
			}
			if self.key(&*msg.as_ptr()) {
				return true;
			}
			if IsDialogMessageA(self.hwnd, msg.as_ptr()) == 0 {
				TranslateMessage(msg.as_ptr());
				DispatchMessageA(msg.as_ptr());
			}
		}
		true
	}

	/// Tab moves the focus to the next control (Shift+Tab to the one before) and Space presses
	/// the focused button or box, whatever `IsDialogMessage` makes of the key. Any other key,
	/// and Space in a text field, is left to it.
	///
	/// # Safety
	///
	/// `msg` is a message just taken from the queue.
	unsafe fn key(&self, msg: &Msg) -> bool {
		if msg.message != WM_KEYDOWN || (msg.wparam != VK_TAB && msg.wparam != VK_SPACE) {
			return false;
		}
		// SAFETY: asks Windows which window has the focus and which control follows it.
		unsafe {
			let focus = GetFocus();
			let ours = |h: Handle| self.bar.iter().any(|b| b.hwnd == h) || self.ctls.iter().any(|c| c.hwnd == h);
			if focus.is_null() || !ours(focus) {
				return false;
			}
			if msg.wparam == VK_TAB {
				let back = i32::from(GetKeyState(VK_SHIFT) < 0);
				let next = GetNextDlgTabItem(self.hwnd, focus, back);
				if !next.is_null() {
					SetFocus(next);
				}
				return true;
			}
			let pressable = self.bar.iter().any(|b| b.hwnd == focus)
				|| self.ctls.iter().any(|c| {
					c.hwnd == focus && matches!(c.role, Role::CheckItem { .. } | Role::RadioItem { .. } | Role::Browse(_))
				});
			if pressable {
				SendMessageA(focus, BM_CLICK, 0, 0);
			}
			pressable
		}
	}

	fn click(&self, h: Handle) {
		// SAFETY: presses a button as a click would.
		unsafe { SendMessageA(h, BM_CLICK, 0, 0) };
	}

	/// Plays the next scripted step on the real controls. Returns true when it did something.
	fn play_step(&mut self, page: &GuiPage<'_>) -> bool {
		let Some(script) = self.script.as_mut() else {
			return false;
		};
		self.guard += 1;
		if self.guard > 400 {
			self.log("!! the script is stuck on this page\n");
			// SAFETY: ends the process with a code a test can see.
			unsafe { ExitProcess(9) };
		}
		let op = script.get_mut(page.step).and_then(VecDeque::pop_front);
		match op {
			Some(Op::Set(bind, v)) => {
				if let Some(c) = self.ctls.iter().find(|c| matches!(&c.role, Role::Edit(b) if *b == bind)) {
					let t = ansi(&v);
					// SAFETY: sets the text of an edit control, which tells the window it changed.
					unsafe { SetWindowTextA(c.hwnd, t.as_ptr()) };
				}
			}
			Some(Op::Toggle(id)) => {
				if let Some(c) = self
					.ctls
					.iter()
					.find(|c| matches!(&c.role, Role::CheckItem { id: i, .. } if *i == id))
				{
					self.click(c.hwnd);
				}
			}
			Some(Op::Choose(bind, id)) => {
				if let Some(c) = self
					.ctls
					.iter()
					.find(|c| matches!(&c.role, Role::RadioItem { bind: b, id: i, .. } if *b == bind && *i == id))
				{
					self.click(c.hwnd);
				}
			}
			Some(Op::Click(action)) => self.click_action(&action),
			None => {
				let default = page.screen.widgets.iter().find_map(|w| match w {
					Widget::Button {
						default: true, action, ..
					} => Some(action.clone()),
					_ => None,
				});
				let action = default.unwrap_or_else(|| "next".to_string());
				self.click_action(&action);
			}
		}
		true
	}

	fn click_action(&self, action: &str) {
		let target = self.bar.iter().find(|b| b.action == action).map(|b| b.hwnd).or_else(|| {
			self.ctls
				.iter()
				.find(|c| matches!(&c.role, Role::Button(a) if a == action))
				.map(|c| c.hwnd)
		});
		match target {
			// SAFETY: asks whether a button is enabled.
			Some(h) if unsafe { IsWindowEnabled(h) } != 0 => self.click(h),
			_ => {
				self.log(&format!("!! no enabled button for `{action}`\n"));
				// SAFETY: ends the process with a code a test can see.
				unsafe { ExitProcess(9) };
			}
		}
	}

	/// What a click on a button of the bar does.
	fn bar_clicked(&mut self, n: usize) {
		let Some(b) = self.bar.get(n) else {
			return;
		};
		let action = b.action.clone();
		if action == "cancel" && self.kind != Kind::Finish && self.script.is_none() && !self.confirm_exit() {
			return;
		}
		self.event = Some(GuiEvent::Action(action));
	}

	fn confirm_exit(&self) -> bool {
		let text = ansi("Setup is not complete. If you exit now, the program will not be installed or changed.\n\nExit Setup?");
		let caption = ansi("Exit Setup");
		// SAFETY: a modal question over the wizard.
		unsafe { MessageBoxA(self.hwnd, text.as_ptr(), caption.as_ptr(), MB_YESNO | MB_ICONQUESTION) == IDYES }
	}

	fn command(&mut self, id: i32, notify: u32) {
		if self.quiet {
			return;
		}
		if (BAR_ID..BAR_ID + 16).contains(&id) {
			self.bar_clicked((id - BAR_ID) as usize);
			return;
		}
		match id as usize {
			IDOK => {
				// Enter: the default button.
				if let Some(n) = self.default_bar() {
					self.bar_clicked(n);
				}
				return;
			}
			IDCANCEL => {
				if let Some(n) = self.bar.iter().position(|b| b.action == "cancel") {
					self.bar_clicked(n);
				}
				return;
			}
			_ => {}
		}
		let Some(i) = self.ctls.iter().position(|c| c.id == id) else {
			return;
		};
		let role = self.ctls[i].role.clone();
		match role {
			Role::Edit(_) if notify == EN_CHANGE => {
				self.event.get_or_insert(GuiEvent::Changed);
			}
			Role::CheckItem { .. } | Role::RadioItem { .. } if notify == BN_CLICKED => {
				self.event.get_or_insert(GuiEvent::Changed);
			}
			Role::Browse(bind) if notify == BN_CLICKED => {
				if let Some(path) = shell::browse_folder(self.hwnd, "Select the folder to install into:")
					&& let Some(e) = self.ctls.iter().find(|c| matches!(&c.role, Role::Edit(b) if *b == bind))
				{
					let t = ansi(&path);
					// SAFETY: sets the text of an edit control, which tells the window it changed.
					unsafe { SetWindowTextA(e.hwnd, t.as_ptr()) };
				}
			}
			Role::Button(action) if notify == BN_CLICKED => {
				self.event = Some(GuiEvent::Action(action));
			}
			_ => {}
		}
	}

	fn default_bar(&self) -> Option<usize> {
		let def = self.default_action_id();
		self.bar.iter().position(|b| Some(b.action.as_str()) == def.as_deref())
	}

	fn default_action_id(&self) -> Option<String> {
		// SAFETY: reads the style of each bar button to find the default one.
		self.bar
			.iter()
			.find(|b| unsafe { GetWindowLongA(b.hwnd, GWL_STYLE) } as u32 & 0xF == BS_DEFPUSHBUTTON && unsafe { IsWindowEnabled(b.hwnd) } != 0)
			.map(|b| b.action.clone())
	}

	fn close_requested(&mut self) {
		if self.quiet {
			return;
		}
		if let Some(n) = self.bar.iter().position(|b| b.action == "cancel") {
			self.bar_clicked(n);
		} else if self.kind == Kind::Finish
			&& let Some(b) = self.bar.first()
		{
			self.event = Some(GuiEvent::Action(b.action.clone()));
		}
	}

	fn tick(&mut self) {
		// SAFETY: reads the tick count.
		let now = unsafe { GetTickCount() };
		for i in 0..self.ctls.len() {
			if let Role::Slide { slides, interval } = &self.ctls[i].role
				&& slides.len() > 1
				&& *interval > 0
			{
				let n = (now.wrapping_sub(self.started) / (u32::from(*interval) * 1000)) as usize % slides.len();
				let text = slides[n].clone();
				self.quiet = true;
				self.set_text(i, &text);
				self.quiet = false;
			}
		}
	}

	/// Saves the window's client area (the title bar belongs to the window manager) as a 24-bit bitmap in the `/SHOT` folder, for the screenshots in the docs.
	fn shot(&mut self) {
		let Some(dir) = self.shots.clone() else {
			return;
		};
		// Let the controls paint first.
		while self.pump(false) {}
		self.shot_count += 1;
		let path = ansi(&format!(
			"{}\\{:02}-{}.bmp",
			dir.trim_end_matches('\\'),
			self.shot_count,
			self.step
		));
		// SAFETY: copies the window's pixels into a bitmap of the same size and writes it out; every object made
		// is deleted.
		unsafe {
			let mut r = Rect::default();
			GetClientRect(self.hwnd, &mut r);
			let (w, h) = (r.right - r.left, r.bottom - r.top);
			let wdc = GetDC(self.hwnd);
			let mdc = CreateCompatibleDC(wdc);
			let bmp = CreateCompatibleBitmap(wdc, w, h);
			let old = SelectObject(mdc, bmp);
			BitBlt(mdc, 0, 0, w, h, wdc, 0, 0, SRCCOPY);
			SelectObject(mdc, old);
			let stride = ((w * 3 + 3) / 4 * 4) as usize;
			let mut bits = alloc::vec![0u8; stride * h as usize];
			let mut info = BitmapInfoHeader {
				size: 40,
				width: w,
				height: h,
				planes: 1,
				bit_count: 24,
				compression: 0,
				size_image: bits.len() as u32,
				x_pels: 0,
				y_pels: 0,
				colours_used: 0,
				colours_important: 0,
			};
			GetDIBits(mdc, bmp, 0, h as u32, bits.as_mut_ptr(), &mut info, DIB_RGB_COLORS);
			DeleteObject(bmp);
			DeleteDC(mdc);
			ReleaseDC(self.hwnd, wdc);
			let f = CreateFileA(
				path.as_ptr(),
				GENERIC_WRITE,
				0,
				null(),
				CREATE_ALWAYS,
				FILE_ATTRIBUTE_NORMAL,
				NULL,
			);
			if f == INVALID_HANDLE_VALUE {
				return;
			}
			let total = 14 + 40 + bits.len() as u32;
			let mut head = alloc::vec::Vec::with_capacity(54);
			head.extend_from_slice(b"BM");
			head.extend_from_slice(&total.to_le_bytes());
			head.extend_from_slice(&[0, 0, 0, 0]);
			head.extend_from_slice(&54u32.to_le_bytes());
			for v in [40u32, w as u32, h as u32] {
				head.extend_from_slice(&v.to_le_bytes());
			}
			head.extend_from_slice(&1u16.to_le_bytes());
			head.extend_from_slice(&24u16.to_le_bytes());
			head.extend_from_slice(&[0u8; 24]);
			let mut n = 0u32;
			WriteFile(f, head.as_ptr(), head.len() as u32, &mut n, null_mut());
			WriteFile(f, bits.as_ptr(), bits.len() as u32, &mut n, null_mut());
			CloseHandle(f);
		}
	}

	/// Closes the window.
	pub fn close(&mut self) {
		if !self.hwnd.is_null() {
			// SAFETY: destroys the wizard window.
			unsafe { DestroyWindow(self.hwnd) };
			self.hwnd = NULL;
		}
	}

	/// An error or a note that needs a person, before there is a window or when it has failed.
	pub fn notify(&mut self, text: &str) {
		let t = ansi(text);
		let owner = self.hwnd;
		// SAFETY: a modal message over the wizard, or on its own before one exists.
		unsafe { MessageBoxA(owner, t.as_ptr(), b"Setup\0".as_ptr(), MB_OK | MB_ICONINFORMATION) };
	}
}

impl Default for Wizard {
	fn default() -> Self {
		Wizard::new()
	}
}

fn with_hotkey(label: &str, hotkey: u8) -> String {
	if hotkey != 0
		&& let Some((i, _)) = label
			.char_indices()
			.find(|(_, c)| c.to_ascii_uppercase() as u32 == u32::from(hotkey))
	{
		return format!("{}&{}", &label[..i], &label[i..]);
	}
	label.to_string()
}

fn window_text(h: Handle) -> String {
	// SAFETY: reads a window's text into a buffer sized by its length.
	unsafe {
		let n = GetWindowTextLengthA(h).max(0) as usize;
		let mut buf = alloc::vec![0u8; n + 1];
		let got = GetWindowTextA(h, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
		utf8(&buf[..got]).replace("\r\n", "\n")
	}
}

fn window_text_of_heading(w: &Wizard) -> String {
	w.title.clone()
}

impl Gui for Wizard {
	fn page(&mut self, page: &mut GuiPage<'_>) -> GuiEvent {
		self.ensure_window();
		if page.fresh {
			self.build(page);
			let d = self.describe(page);
			self.log(&d);
			self.log_geometry();
			self.shot();
		} else {
			self.update_states(page);
		}
		loop {
			if let Some(e) = self.event.take() {
				self.sync(page);
				return e;
			}
			if self.script.is_some() && self.play_step(page) {
				continue;
			}
			if !self.pump(true) {
				return GuiEvent::Action("cancel".to_string());
			}
		}
	}

	fn working(&mut self, page: &mut GuiPage<'_>, progress: &Progress) -> bool {
		self.ensure_window();
		if page.fresh {
			self.build(page);
			let d = self.describe(page);
			self.log(&d);
			self.log_geometry();
			self.shot();
		}
		page.values.set_var("progress.file", &progress.file);
		self.update_states(page);
		while self.pump(false) {}
		if let Some(GuiEvent::Action(a)) = self.event.take()
			&& a == "cancel"
		{
			self.cancelled = true;
		}
		if !progress.file.is_empty() {
			self.log(&format!("file: {}\n", progress.file));
		}
		!self.cancelled
	}

	fn message(&mut self, text: &str) {
		self.log(&format!("message: {}\n", text.replace('\n', " | ")));
		if self.script.is_some() {
			return;
		}
		self.notify(text);
	}
}

unsafe extern "system" fn wnd_proc(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize {
	let ptr = WIZARD.load(Ordering::Relaxed);
	// SAFETY: the pointer is set by `ensure_window` to the wizard, which lives for the whole run and is
	// only used from this thread.
	let wiz = if ptr.is_null() { None } else { Some(&mut *ptr) };
	match msg {
		WM_COMMAND => {
			if let Some(w) = wiz {
				w.command((wparam & 0xFFFF) as i32, (wparam >> 16) as u32);
			}
			0
		}
		WM_CLOSE => {
			if let Some(w) = wiz {
				w.close_requested();
			}
			0
		}
		WM_TIMER => {
			if let Some(w) = wiz {
				w.tick();
			}
			0
		}
		DM_GETDEFID => {
			if let Some(w) = wiz
				&& let Some(n) = w.default_bar()
			{
				return (DC_HASDEFID << 16) | (BAR_ID + n as i32) as isize;
			}
			0
		}
		WM_PAINT => {
			if let Some(w) = wiz {
				w.paint(hwnd);
				return 0;
			}
			DefWindowProcA(hwnd, msg, wparam, lparam)
		}
		_ => DefWindowProcA(hwnd, msg, wparam, lparam),
	}
}

impl Wizard {
	/// The teal banner on the left: the product's name and the step.
	fn paint(&self, hwnd: Handle) {
		let mut ps = core::mem::MaybeUninit::<PaintStruct>::uninit();
		// SAFETY: standard paint sequence; the brush is deleted and the DC given back.
		unsafe {
			let dc = BeginPaint(hwnd, ps.as_mut_ptr());
			let brush = CreateSolidBrush(TEAL);
			let r = Rect {
				left: 0,
				top: 0,
				right: BANNER_W,
				bottom: self.client_h - BAR_H,
			};
			FillRect(dc, &r, brush);
			DeleteObject(brush);
			SetBkMode(dc, TRANSPARENT);
			SetTextColor(dc, 0x00FF_FFFF);
			let old = SelectObject(dc, self.big);
			let mut t = Rect {
				left: 12,
				top: 18,
				right: BANNER_W - 10,
				bottom: self.client_h - BAR_H - 40,
			};
			let name = ansi(&self.banner);
			DrawTextA(dc, name.as_ptr(), -1, &mut t, DT_WORDBREAK | DT_NOPREFIX | DT_LEFT);
			SelectObject(dc, self.bold);
			let step = ansi(&self.step_caption);
			TextOutA(dc, 12, self.client_h - BAR_H - 28, step.as_ptr(), (step.len() - 1) as i32);
			SelectObject(dc, old);
			EndPaint(hwnd, ps.as_ptr());
		}
	}
}
