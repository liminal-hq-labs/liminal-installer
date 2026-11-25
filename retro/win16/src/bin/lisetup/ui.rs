// The wizard window: a page's widgets drawn as Windows 3.1 controls.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Each program in this folder uses part of this module, so what one does not use is not dead code.
#![allow(dead_code)]
//! One top-level window of a class whose default procedure is `DefDlgProc`, so Windows gives it a
//! dialog's keyboard (Tab, arrows in a radio group, Enter for the default button, Esc, access keys
//! through `IsDialogMessage`) and its grey background. The page's `Widget`s become child controls
//! in reading order, stacked down the window; the buttons go in a row at the bottom. The same
//! `Page` and `Values` the DOS runtime draws from drive it, so the flow does not know the
//! difference.
use alloc::string::String;
use alloc::vec::Vec;
use core::ptr::addr_of_mut;
use li_retro::gui::{Gui, GuiEvent, GuiPage};
use li_retro::install::Progress;
use li_retro::model::{Item, Rect, Source, Widget};
use li_retro::ui::{Value, Values};
use w16::*;

export_proc!(WizProc, wiz_proc);

const WS_CHILD: u32 = 0x4000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_TABSTOP: u32 = 0x0001_0000;
const WS_GROUP: u32 = 0x0002_0000;
const WS_BORDER: u32 = 0x0080_0000;
const WS_VSCROLL: u32 = 0x0020_0000;
const WS_FRAME: u32 = 0x80C8_0000; // POPUP | CAPTION | SYSMENU
const BS_DEFPUSHBUTTON: u32 = 1;
const BS_AUTOCHECKBOX: u32 = 3;
const BS_AUTORADIOBUTTON: u32 = 9;
const ES_AUTOHSCROLL: u32 = 0x80;
const ES_MULTILINE: u32 = 4;
const ES_AUTOVSCROLL: u32 = 0x40;
const ES_READONLY: u32 = 0x800;
const SS_BLACKRECT: u32 = 4;
const SS_WHITERECT: u32 = 6;
const SS_BLACKFRAME: u32 = 7;
const WM_CLOSE: u32 = 0x0010;
const BM_GETCHECK: u32 = 0x0400;
const BM_SETCHECK: u32 = 0x0401;
const DM_SETDEFID: u32 = WM_USER + 1;
const EN_CHANGE: u32 = 0x0300;
const SM_CXSCREEN: u32 = 0;
const SM_CYSCREEN: u32 = 1;
const SM_CYCAPTION: u32 = 4;
const SM_CXDLGFRAME: u32 = 7;
const SM_CYDLGFRAME: u32 = 8;
const COLOR_BTNFACE_BRUSH: u16 = 16;
const MB_ICONEXCLAMATION: u32 = 0x30;

/// The id of the first button; item and field controls follow.
const ID_BTN: u32 = 100;
const ID_CTL: u32 = 200;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
	Text,
	Check,
	Radio,
	Edit,
	MeterLabel,
	MeterFill,
	Billboard,
	Button,
}

struct Ctl {
	id: u32,
	hwnd: u32,
	/// The widget this control belongs to, and the item within it.
	widget: usize,
	item: usize,
	kind: Kind,
	/// The text last set, so a change is a change.
	last: String,
	enabled: bool,
	/// Where the control sits (the gauge's fill is moved along its track).
	pos: (u32, u32),
}

/// One step of a `/PLAY` script.
enum Op {
	Set(String, String),
	Toggle(String),
	Choose(String, String),
	Click(String),
}

pub struct Ui {
	hwnd: u32,
	/// The widgets of the page on show, with the commit page's default meters added.
	ws: Vec<Widget>,
	/// The `/PLAY` script: per step, what to do on its page, in order. A page with nothing left presses its default button.
	script: Option<Vec<(String, Op)>>,
	/// What `/DUMP` writes: the text of every page shown.
	dump: String,
	guard: u32,
	/// `/PAUSE=ms`: how long each page stays up before the script acts, so a screenshot can be taken.
	pause: u32,
	ctls: Vec<Ctl>,
	started: u32,
	/// The gauge's inner width in pixels.
	gauge_w: u32,
	registered: bool,
}

// What the window procedure saw: set there, taken by the loop.
static mut EV_ID: u32 = 0;
static mut EV_NOTIFY: u32 = 0;
static mut EV_CLOSE: bool = false;
static mut MSG: Msg = Msg {
	hwnd: 0,
	message: 0,
	wparam: 0,
	lparam: 0,
	time: 0,
	pt: [0; 2],
};
static mut CLASS: WndClass = WndClass {
	style: 0,
	wndproc: 0,
	cls_extra: 0,
	wnd_extra: 0,
	instance: 0,
	icon: 0,
	cursor: 0,
	background: 0,
	menu: 0,
	class_name: 0,
};

extern "C" fn wiz_proc(hwnd: u32, msg: u32, wparam: u32, lparam: u32) -> u32 {
	unsafe {
		match msg {
			WM_COMMAND => {
				EV_ID = wparam;
				EV_NOTIFY = lparam >> 16;
				0
			}
			WM_CLOSE => {
				EV_CLOSE = true;
				0
			}
			_ => DefDlgProc(hwnd, msg, wparam, lparam),
		}
	}
}

/// Windows ANSI bytes with a NUL for `s`: the Latin-1 range as it is and the common typographic
/// marks as plain ones (Windows 3.1's character set has nothing in 0x80 to 0x9F: its System font
/// draws a bar there), anything else a question mark.
pub fn ansi(s: &str) -> Vec<u8> {
	let mut v = Vec::with_capacity(s.len() + 1);
	for c in s.chars() {
		match c {
			'\u{2014}' | '\u{2013}' => v.push(b'-'),
			'\u{2026}' => v.extend_from_slice(b"..."),
			'\u{2018}' | '\u{2019}' => v.push(b'\''),
			'\u{201c}' | '\u{201d}' => v.push(b'"'),
			'\u{2022}' => v.push(b'*'),
			c if (c as u32) < 0x100 => v.push(c as u8),
			_ => v.push(b'?'),
		}
	}
	v.push(0);
	v
}

/// The reverse of [`ansi`] for text the person typed.
fn from_ansi(b: &[u8]) -> String {
	b.iter().map(|&c| c as char).collect()
}

/// Text for a multi-line edit control: CR LF line ends.
fn crlf(s: &str) -> Vec<u8> {
	let mut v = Vec::with_capacity(s.len() + 16);
	for b in ansi(s) {
		if b == b'\n' {
			v.push(b'\r');
		}
		v.push(b);
	}
	v
}

fn near(v: &[u8]) -> u32 {
	v.as_ptr() as u32
}

pub fn take_event() -> Option<(u32, u32)> {
	unsafe {
		let id = EV_ID;
		if id == 0 {
			return None;
		}
		EV_ID = 0;
		Some((id, EV_NOTIFY))
	}
}

fn take_close() -> bool {
	unsafe {
		let c = EV_CLOSE;
		EV_CLOSE = false;
		c
	}
}

/// Lets Windows run: hands every queued message on (to the dialog manager first), then yields.
fn pump(hwnd: u32) {
	unsafe {
		let m = addr_of_mut!(MSG) as u32;
		while PeekMessage(m, 0, 0, 0, PM_REMOVE) != 0 {
			if hwnd == 0 || IsDialogMessage(hwnd, m) == 0 {
				TranslateMessage(m);
				DispatchMessage(m);
			}
		}
		Yield();
	}
}

/// Blocks for one message (or none, if the queue is dry and nothing will come) and dispatches it.
fn pump_one(hwnd: u32) -> bool {
	unsafe {
		let m = addr_of_mut!(MSG) as u32;
		if GetMessage(m, 0, 0, 0) == 0 {
			return false;
		}
		if IsDialogMessage(hwnd, m) == 0 {
			TranslateMessage(m);
			DispatchMessage(m);
		}
		true
	}
}

fn lines_for(len: usize, width_px: u32, bx: u32) -> u32 {
	let per = (width_px / bx.max(1)).max(8) as usize;
	len.div_ceil(per).max(1) as u32
}

impl Ui {
	pub const fn new() -> Ui {
		Ui {
			hwnd: 0,
			ws: Vec::new(),
			script: None,
			dump: String::new(),
			guard: 0,
			pause: 0,
			ctls: Vec::new(),
			started: 0,
			gauge_w: 0,
			registered: false,
		}
	}

	pub fn open(&self) -> bool {
		self.hwnd != 0
	}

	pub fn hwnd(&self) -> u32 {
		self.hwnd
	}

	fn register(&mut self) -> bool {
		if self.registered {
			return true;
		}
		unsafe {
			let c = addr_of_mut!(CLASS);
			(*c).wndproc = far(code_selector(), WizProc as *const () as usize as u32);
			(*c).wnd_extra = 30; // DLGWINDOWEXTRA
			(*c).instance = HINST as u16;
			(*c).cursor = LoadCursor(0, int_resource(32512)) as u16;
			(*c).background = COLOR_BTNFACE_BRUSH;
			(*c).class_name = far(data_selector(), cs!("LiminalSetup"));
			self.registered = RegisterClass(c as u32) != 0;
		}
		self.registered
	}

	fn child(
		&self,
		class: u32,
		text: &[u8],
		style: u32,
		(x, y, w, h): (u32, u32, u32, u32),
		id: u32,
	) -> u32 {
		unsafe {
			CreateWindow(
				class,
				near(text),
				WS_CHILD | WS_VISIBLE | style,
				x,
				y,
				w,
				h,
				self.hwnd,
				id,
				HINST,
				0,
			)
		}
	}

	/// Takes the window down.
	pub fn close(&mut self) {
		if self.hwnd != 0 {
			unsafe { DestroyWindow(self.hwnd) };
			self.hwnd = 0;
		}
		self.ctls.clear();
		self.ws.clear();
		let _ = take_event();
		let _ = take_close();
	}

	/// Builds the window for `page`.
	fn build(&mut self, page: &GuiPage<'_>, now: u32) -> bool {
		if !self.register() {
			return false;
		}
		self.started = now;
		let (bx, by) = unsafe {
			let u = GetDialogBaseUnits();
			((u & 0xFFFF).max(4), (u >> 16).max(8))
		};
		let (sw, sh) = unsafe { (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN)) };
		let margin = by.max(8);
		let width = (58 * bx).min(sw.saturating_sub(40)).max(280);
		let inner = width - 2 * margin;
		let v: &Values = &*page.values;
		let mut wsv: Vec<Widget> = page.screen.widgets.clone();
		// A commit page with no meters of its own gets the file and overall bars.
		for (label, source) in page.extra_meters() {
			wsv.push(Widget::Meter {
				rect: Rect {
					x: 0,
					y: 0,
					w: 0,
					h: 0,
				},
				attr: 0,
				label: String::from(*label),
				source: *source,
				low_below: 0,
			});
		}
		let ws = &wsv;
		let mut caption = page.title.clone();
		if caption.is_empty() {
			caption = page.product.into();
		}
		let cap = ansi(&caption);
		// Measure the stack first so the window is the right height.
		let line = by + 2;
		let mut heights = 0u32;
		for w in ws {
			heights += match w {
				Widget::Text { template, .. } => {
					lines_for(v.interpolate(template).chars().count(), inner, bx) * line + by / 2
				}
				Widget::Doc { .. } => 8 * line + by,
				Widget::List { items, label, .. } => {
					items.len() as u32 * (by + 6) + if label.is_empty() { 0 } else { line } + by / 2
				}
				Widget::Field { .. } => line + by + 8 + by / 2,
				Widget::Meter { .. } => line + 14 + by / 2,
				Widget::Billboard { slides, .. } => {
					let longest = slides.iter().map(|s| s.chars().count()).max().unwrap_or(1);
					lines_for(longest, inner, bx) * line + by
				}
				Widget::Button { .. } => 0,
			};
		}
		let btn_h = by * 3 / 2 + 6;
		let has_buttons = ws.iter().any(|w| matches!(w, Widget::Button { .. }));
		let client_h = margin + heights + margin + if has_buttons { btn_h + margin } else { 0 };
		// Every page has the same window: this is tall enough for the licence and for a page of
		// seven components, and a page that needs more grows it.
		let client_h = client_h.max(22 * line);
		let (fx, fy, fc) = unsafe {
			(
				GetSystemMetrics(SM_CXDLGFRAME),
				GetSystemMetrics(SM_CYDLGFRAME),
				GetSystemMetrics(SM_CYCAPTION),
			)
		};
		let (ow, oh) = (width + 2 * fx, client_h + 2 * fy + fc + 2);
		let (ox, oy) = (sw.saturating_sub(ow) / 2, sh.saturating_sub(oh) / 3);
		self.hwnd = unsafe {
			CreateWindow(
				cs!("LiminalSetup"),
				near(&cap),
				WS_FRAME,
				ox,
				oy,
				ow,
				oh,
				0,
				0,
				HINST,
				0,
			)
		};
		if self.hwnd == 0 {
			return false;
		}
		let mut y = margin;
		let mut next_id = ID_CTL;
		let mut first_tab = 0u32;
		let static_cls = cs!("STATIC");
		let button_cls = cs!("BUTTON");
		let edit_cls = cs!("EDIT");
		let mut default_id = 0;
		for (wi, w) in ws.iter().enumerate() {
			match w {
				Widget::Button { .. } => {}
				Widget::Text { template, attr, .. } => {
					let t = v.interpolate(template);
					let n = lines_for(t.chars().count(), inner, bx);
					let h = n * line;
					let txt = ansi(&t);
					let id = next_id;
					next_id += 1;
					let hw = self.child(static_cls, &txt, 0, (margin, y, inner, h), id);
					let _ = attr;
					self.ctls.push(Ctl {
						id,
						hwnd: hw,
						widget: wi,
						item: 0,
						kind: Kind::Text,
						last: t,
						enabled: true,
						pos: (0, 0),
					});
					y += h + by / 2;
				}
				Widget::Doc { text, .. } => {
					let body = crlf(text);
					let h = 8 * line + by;
					let id = next_id;
					next_id += 1;
					let hw = self.child(
						edit_cls,
						&body,
						WS_BORDER
							| WS_VSCROLL | WS_TABSTOP
							| ES_MULTILINE | ES_READONLY
							| ES_AUTOVSCROLL,
						(margin, y, inner, h),
						id,
					);
					self.ctls.push(Ctl {
						id,
						hwnd: hw,
						widget: wi,
						item: 0,
						kind: Kind::Text,
						last: String::new(),
						enabled: true,
						pos: (0, 0),
					});
					y += h + by / 2;
				}
				Widget::List {
					check,
					label,
					bind,
					items,
					default,
					..
				} => {
					if !label.is_empty() {
						let l = ansi(label);
						let id = next_id;
						next_id += 1;
						self.child(static_cls, &l, 0, (margin, y, inner, line), id);
						y += line;
					}
					for (ii, it) in items.iter().enumerate() {
						let on = list_on(v, *check, bind, default, it);
						let t = ansi(&v.interpolate(&it.label));
						let id = next_id;
						next_id += 1;
						let mut style = if *check {
							BS_AUTOCHECKBOX
						} else {
							BS_AUTORADIOBUTTON
						};
						if ii == 0 || *check {
							style |= WS_GROUP | WS_TABSTOP;
						}
						let hw = self.child(
							button_cls,
							&t,
							style,
							(margin + bx, y, inner - bx, by + 4),
							id,
						);
						let dead = *check && it.required;
						if first_tab == 0 && !dead {
							first_tab = hw;
						}
						unsafe {
							SendMessage(hw, BM_SETCHECK, u32::from(on), 0);
						}
						if dead {
							unsafe { EnableWindow(hw, 0) };
						}
						self.ctls.push(Ctl {
							id,
							hwnd: hw,
							widget: wi,
							item: ii,
							kind: if *check { Kind::Check } else { Kind::Radio },
							last: String::new(),
							enabled: !dead,
							pos: (0, 0),
						});
						y += by + 6;
					}
					y += by / 2;
				}
				Widget::Field {
					label,
					bind,
					width: cols,
					..
				} => {
					let l = ansi(label);
					let id = next_id;
					next_id += 1;
					self.child(static_cls, &l, 0, (margin, y, inner, line), id);
					y += line;
					let text = ansi(v.str(bind).unwrap_or(""));
					let id = next_id;
					next_id += 1;
					let ew = ((u32::from(*cols).max(4) + 2) * bx).min(inner);
					let ew = if *cols >= 24 { inner } else { ew };
					let hw = self.child(
						edit_cls,
						&text,
						WS_BORDER | WS_TABSTOP | ES_AUTOHSCROLL,
						(margin, y, ew, by + 8),
						id,
					);
					if first_tab == 0 {
						first_tab = hw;
					}
					self.ctls.push(Ctl {
						id,
						hwnd: hw,
						widget: wi,
						item: 0,
						kind: Kind::Edit,
						last: String::new(),
						enabled: true,
						pos: (0, 0),
					});
					y += by + 8 + by / 2;
				}
				Widget::Meter { label, source, .. } => {
					let cap = meter_caption(label, *source);
					let t = ansi(&cap);
					let id = next_id;
					next_id += 1;
					let hw = self.child(static_cls, &t, 0, (margin, y, inner, line), id);
					self.ctls.push(Ctl {
						id,
						hwnd: hw,
						widget: wi,
						item: 0,
						kind: Kind::MeterLabel,
						last: cap,
						enabled: true,
						pos: (0, 0),
					});
					y += line;
					let id = next_id;
					next_id += 1;
					self.child(static_cls, b"\0", SS_WHITERECT, (margin, y, inner, 14), id);
					let id = next_id;
					next_id += 1;
					let fill = self.child(
						static_cls,
						b"\0",
						SS_BLACKRECT,
						(margin + 1, y + 1, 0, 12),
						id,
					);
					let id = next_id;
					next_id += 1;
					self.child(static_cls, b"\0", SS_BLACKFRAME, (margin, y, inner, 14), id);
					self.gauge_w = inner - 2;
					self.ctls.push(Ctl {
						id,
						hwnd: fill,
						widget: wi,
						item: 0,
						kind: Kind::MeterFill,
						last: String::new(),
						enabled: true,
						pos: (margin + 1, y + 1),
					});
					y += 14 + by / 2;
				}
				Widget::Billboard { slides, .. } => {
					let longest = slides.iter().map(|s| s.chars().count()).max().unwrap_or(1);
					let h = lines_for(longest, inner, bx) * line;
					let t = ansi(slides.first().map_or("", String::as_str));
					let id = next_id;
					next_id += 1;
					let hw = self.child(static_cls, &t, 0, (margin, y, inner, h), id);
					self.ctls.push(Ctl {
						id,
						hwnd: hw,
						widget: wi,
						item: 0,
						kind: Kind::Billboard,
						last: String::new(),
						enabled: true,
						pos: (0, 0),
					});
					y += h + by;
				}
			}
		}
		// The buttons, right-aligned along the bottom.
		let nbtn = ws
			.iter()
			.filter(|w| matches!(w, Widget::Button { .. }))
			.count() as u32;
		let bw = (11 * bx).max(72);
		let mut x = width.saturating_sub(margin + nbtn * bw + nbtn.saturating_sub(1) * 8);
		let mut bi = 0;
		for (wi, w) in ws.iter().enumerate() {
			if let Widget::Button {
				label,
				default,
				hotkey,
				..
			} = w
			{
				let id = ID_BTN + bi;
				bi += 1;
				let text = ansi(&with_mnemonic(label, *hotkey));
				let mut style = WS_TABSTOP;
				if *default {
					style |= BS_DEFPUSHBUTTON;
					default_id = id;
				}
				let hw = self.child(
					button_cls,
					&text,
					style,
					(x, client_h - margin - btn_h, bw, btn_h),
					id,
				);
				let on = page.enabled(w);
				if !on {
					unsafe { EnableWindow(hw, 0) };
				}
				self.ctls.push(Ctl {
					id,
					hwnd: hw,
					widget: wi,
					item: 0,
					kind: Kind::Button,
					last: String::new(),
					enabled: on,
					pos: (0, 0),
				});
				x += bw + 8;
			}
		}
		unsafe {
			if default_id != 0 {
				SendMessage(self.hwnd, DM_SETDEFID, default_id, 0);
			}
			ShowWindow(self.hwnd, SW_SHOW);
			UpdateWindow(self.hwnd);
			let focus = if first_tab != 0 {
				first_tab
			} else {
				self.ctls
					.iter()
					.find(|c| c.kind == Kind::Button && c.enabled)
					.map_or(0, |c| c.hwnd)
			};
			if focus != 0 {
				SetFocus(focus);
			}
		}
		let _ = take_event();
		let _ = take_close();
		self.ws = wsv;
		true
	}

	/// Brings the texts, the meters and the buttons up to date with the values.
	fn sync(&mut self, page: &GuiPage<'_>, now: u32) {
		let v: &Values = &*page.values;
		let ws = core::mem::take(&mut self.ws);
		for c in &mut self.ctls {
			match (&ws[c.widget], c.kind) {
				(Widget::Text { template, .. }, Kind::Text) => {
					let t = v.interpolate(template);
					if t != c.last {
						unsafe { SetWindowText(c.hwnd, near(&ansi(&t))) };
						c.last = t;
					}
				}
				(
					Widget::Meter {
						label,
						source,
						low_below,
						..
					},
					Kind::MeterLabel,
				) => {
					let pct = match source {
						Source::File => v.file_pct,
						Source::Total => v.total_pct,
						Source::FreeSpace => v.free_pct,
					};
					let mut t = meter_caption(label, *source);
					t.push_str("  ");
					push_dec(&mut t, pct);
					t.push('%');
					if *source == Source::FreeSpace && pct < u32::from(*low_below) {
						t.push_str("  LOW");
					}
					if t != c.last {
						unsafe { SetWindowText(c.hwnd, near(&ansi(&t))) };
						c.last = t;
					}
				}
				(Widget::Meter { source, .. }, Kind::MeterFill) => {
					let pct = match source {
						Source::File => v.file_pct,
						Source::Total => v.total_pct,
						Source::FreeSpace => v.free_pct,
					}
					.min(100);
					let w = self.gauge_w * pct / 100;
					let mut key = String::new();
					push_dec(&mut key, w);
					if key != c.last {
						unsafe { MoveWindow(c.hwnd, c.pos.0, c.pos.1, w, 12, 1) };
						c.last = key;
					}
				}
				(
					Widget::Billboard {
						slides, interval, ..
					},
					Kind::Billboard,
				) if !slides.is_empty() => {
					let n = (now.wrapping_sub(self.started) / (u32::from(*interval).max(1) * 1000))
						as usize % slides.len();
					if slides[n] != c.last {
						unsafe { SetWindowText(c.hwnd, near(&ansi(&slides[n]))) };
						c.last = slides[n].clone();
					}
				}
				(w @ Widget::Button { .. }, Kind::Button) => {
					let on = page.enabled(w);
					if on != c.enabled {
						unsafe { EnableWindow(c.hwnd, u32::from(on)) };
						c.enabled = on;
					}
				}
				_ => {}
			}
		}
		self.ws = ws;
	}
}
fn push_dec(s: &mut String, mut n: u32) {
	let mut tmp = [0u8; 10];
	let mut i = 0;
	loop {
		tmp[i] = b'0' + (n % 10) as u8;
		n /= 10;
		i += 1;
		if n == 0 {
			break;
		}
	}
	while i > 0 {
		i -= 1;
		s.push(tmp[i] as char);
	}
}

fn meter_caption(label: &str, source: Source) -> String {
	String::from(if !label.is_empty() {
		label
	} else {
		match source {
			Source::File => "File",
			Source::FreeSpace => "Free space",
			Source::Total => "Progress",
		}
	})
}

/// `Next` with its access key `N` becomes `&Next`.
fn with_mnemonic(label: &str, hotkey: u8) -> String {
	if hotkey != 0
		&& let Some(i) = label.bytes().position(|b| b.to_ascii_uppercase() == hotkey)
	{
		let mut s = String::from(&label[..i]);
		s.push('&');
		s.push_str(&label[i..]);
		return s;
	}
	String::from(label)
}

fn list_on(v: &Values, check: bool, bind: &str, default: &str, it: &Item) -> bool {
	if check {
		it.required || v.list(bind).is_some_and(|l| l.contains(&it.id))
	} else {
		v.str(bind).map(|s| s == it.id).unwrap_or(default == it.id)
	}
}

impl Ui {
	/// Shows the page and returns when the person does something the flow must know about.
	fn run_page(&mut self, page: &mut GuiPage<'_>) -> GuiEvent {
		let now = unsafe { GetTickCount() };
		if page.fresh {
			self.dump.push_str(&page.describe());
			self.close();
			if !self.build(page, now) {
				return GuiEvent::Action(String::from("cancel"));
			}
		}
		self.sync(page, now);
		self.play_step(page);
		loop {
			if take_close() {
				return GuiEvent::Action(String::from("cancel"));
			}
			if let Some((id, notify)) = take_event() {
				if let Some(ev) = self.handle(page, id, notify) {
					return ev;
				}
				continue;
			}
			if !pump_one(self.hwnd) {
				return GuiEvent::Action(String::from("cancel"));
			}
		}
	}

	fn handle(&mut self, page: &mut GuiPage<'_>, id: u32, notify: u32) -> Option<GuiEvent> {
		let ws = core::mem::take(&mut self.ws);
		let ev = self.handle_in(&ws, page, id, notify);
		self.ws = ws;
		ev
	}

	fn handle_in(
		&mut self,
		ws: &[Widget],
		page: &mut GuiPage<'_>,
		id: u32,
		notify: u32,
	) -> Option<GuiEvent> {
		let v: &mut Values = &mut *page.values;
		if id == IDCANCEL {
			// Esc: back where there is a Back button, else cancel.
			for c in &self.ctls {
				if let Widget::Button { action, .. } = &ws[c.widget]
					&& c.kind == Kind::Button
					&& action == "back"
					&& c.enabled
				{
					return Some(GuiEvent::Action(String::from("back")));
				}
			}
			return Some(GuiEvent::Action(String::from("cancel")));
		}
		let c = self.ctls.iter().find(|c| c.id == id)?;
		match (c.kind, &ws[c.widget]) {
			(Kind::Button, Widget::Button { action, .. }) if c.enabled => {
				Some(GuiEvent::Action(action.clone()))
			}
			(Kind::Check, Widget::List { bind, items, .. }) => {
				let it = &items[c.item];
				let on = unsafe { SendMessage(c.hwnd, BM_GETCHECK, 0, 0) } != 0;
				if it.required {
					unsafe { SendMessage(c.hwnd, BM_SETCHECK, 1, 0) };
					return None;
				}
				let mut list = v.list(bind).cloned().unwrap_or_default();
				list.retain(|x| *x != it.id);
				if on {
					list.push(it.id.clone());
				}
				v.state.insert(bind.clone(), Value::List(list));
				Some(GuiEvent::Changed)
			}
			(Kind::Radio, Widget::List { bind, items, .. }) => {
				let it = &items[c.item];
				if v.str(bind) == Some(it.id.as_str()) {
					return None;
				}
				v.set_str(bind, &it.id);
				Some(GuiEvent::Changed)
			}
			(Kind::Edit, Widget::Field { bind, .. }) if notify == EN_CHANGE => {
				let mut buf = [0u8; 80];
				let n = unsafe { GetWindowText(c.hwnd, addr_of_mut!(buf) as u32, 79) } as usize;
				let s = from_ansi(&buf[..n.min(79)]);
				if v.str(bind) == Some(s.as_str()) {
					return None;
				}
				v.set_str(bind, &s);
				Some(GuiEvent::Changed)
			}
			_ => None,
		}
	}

	/// The commit page: updates the meters from the values, lets Windows run and says whether to go on.
	fn run_working(&mut self, page: &mut GuiPage<'_>) -> bool {
		let now = unsafe { GetTickCount() };
		if page.fresh {
			self.dump.push_str(&page.describe());
			self.close();
			if !self.build(page, now) {
				return true;
			}
		}
		self.sync(page, now);
		if page.fresh && self.script.is_some() && self.pause > 0 {
			self.hold(self.pause);
		}
		pump(self.hwnd);
		let mut go = !take_close();
		while let Some((id, _)) = take_event() {
			let cancel = id == IDCANCEL
				|| self.ctls.iter().any(|c| {
					c.id == id
						&& matches!(&self.ws[c.widget], Widget::Button { action, .. } if action == "cancel")
				});
			go &= !cancel;
		}
		go
	}
}

impl Ui {
	/// Reads a `/PLAY` script: `step op args`, with `set bind value`, `toggle id`, `choose bind id`, `click action` and `#`
	/// comments, as the Win32 runtime does. A page with nothing left in its script presses its default button.
	pub fn play(&mut self, text: &str) {
		let mut v = Vec::new();
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
					.map(|(b, x)| Op::Set(b.into(), x.trim().into())),
				"toggle" => Some(Op::Toggle(rest.into())),
				"choose" => rest
					.split_once(' ')
					.map(|(b, x)| Op::Choose(b.into(), x.trim().into())),
				"click" => Some(Op::Click(rest.into())),
				_ => None,
			};
			if let Some(op) = op {
				v.push((String::from(step), op));
			}
		}
		self.script = Some(v);
	}

	pub fn set_pause(&mut self, ms: u32) {
		self.pause = ms;
	}

	/// Lets Windows run for `ms` milliseconds with the page on show.
	fn hold(&self, ms: u32) {
		let t0 = unsafe { GetTickCount() };
		while unsafe { GetTickCount() }.wrapping_sub(t0) < ms {
			pump(self.hwnd);
		}
	}

	/// The text of every page shown, as `GuiPage::describe` wrote it, for `/DUMP`.
	pub fn dump(&self) -> &str {
		&self.dump
	}

	/// Presses a button as a click would: the mouse going down and up on it.
	fn click(&self, hwnd: u32) {
		unsafe {
			SendMessage(hwnd, 0x0201, 1, 0x0002_0002);
			SendMessage(hwnd, 0x0202, 0, 0x0002_0002);
		}
	}

	fn click_action(&self, ws: &[Widget], action: &str) {
		for c in &self.ctls {
			if c.kind == Kind::Button
				&& c.enabled && matches!(&ws[c.widget], Widget::Button { action: a, .. } if a == action)
			{
				self.click(c.hwnd);
				return;
			}
		}
		// Nothing to press: leave with a code a test can see.
		unsafe { w16_exit(9) };
	}

	/// Plays the next scripted step on the real controls (when there is a script): one call, one step.
	fn play_step(&mut self, page: &GuiPage<'_>) {
		if self.script.is_some() && self.pause > 0 {
			self.hold(self.pause);
		}
		let Some(script) = self.script.as_mut() else {
			return;
		};
		self.guard += 1;
		if self.guard > 400 {
			unsafe { w16_exit(9) };
		}
		let pos = script.iter().position(|(st, _)| st == page.step);
		let op = pos.map(|i| script.remove(i).1);
		let ws = core::mem::take(&mut self.ws);
		match op {
			Some(Op::Set(bind, text)) => {
				for c in &self.ctls {
					if c.kind == Kind::Edit
						&& matches!(&ws[c.widget], Widget::Field { bind: b, .. } if *b == bind)
					{
						unsafe { SetWindowText(c.hwnd, near(&ansi(&text))) };
					}
				}
			}
			Some(Op::Toggle(id)) => {
				for c in &self.ctls {
					if c.kind == Kind::Check
						&& matches!(&ws[c.widget], Widget::List { items, .. } if items[c.item].id == id)
					{
						self.click(c.hwnd);
					}
				}
			}
			Some(Op::Choose(bind, id)) => {
				for c in &self.ctls {
					if c.kind == Kind::Radio
						&& matches!(&ws[c.widget], Widget::List { bind: b, items, .. } if *b == bind && items[c.item].id == id)
					{
						self.click(c.hwnd);
					}
				}
			}
			Some(Op::Click(action)) => self.click_action(&ws, &action),
			None => {
				let action = ws
					.iter()
					.find_map(|w| match w {
						Widget::Button {
							default: true,
							action,
							..
						} => Some(action.clone()),
						_ => None,
					})
					.unwrap_or_else(|| String::from("next"));
				self.click_action(&ws, &action);
			}
		}
		self.ws = ws;
	}
}

impl Gui for Ui {
	fn page(&mut self, page: &mut GuiPage<'_>) -> GuiEvent {
		self.run_page(page)
	}

	fn working(&mut self, page: &mut GuiPage<'_>, _progress: &Progress) -> bool {
		self.run_working(page)
	}

	fn message(&mut self, text: &str) {
		unsafe {
			MessageBox(
				self.hwnd,
				near(&ansi(text)),
				cs!("Setup"),
				MB_ICONEXCLAMATION,
			);
		}
	}
}
