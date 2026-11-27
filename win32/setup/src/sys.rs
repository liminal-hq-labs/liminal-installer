// The Win32 calls the installer makes: ANSI entry points only, imported by name (raw-dylib), nothing newer than Windows 95.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![allow(non_snake_case, dead_code, clippy::too_many_arguments)]

use core::ffi::c_void;

/// A Win32 handle.
pub type Handle = *mut c_void;
pub type WndProc = unsafe extern "system" fn(Handle, u32, usize, isize) -> isize;

pub const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
pub const NULL: Handle = core::ptr::null_mut();

// Window messages.
pub const WM_DESTROY: u32 = 0x0002;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_SETTEXT: u32 = 0x000C;
pub const WM_PAINT: u32 = 0x000F;
pub const WM_ERASEBKGND: u32 = 0x0014;
pub const WM_CTLCOLORSTATIC: u32 = 0x0138;
pub const WM_SETFONT: u32 = 0x0030;
pub const WM_KEYDOWN: u32 = 0x0100;
pub const WM_COMMAND: u32 = 0x0111;
pub const VK_TAB: usize = 0x09;
pub const VK_SHIFT: i32 = 0x10;
pub const VK_SPACE: usize = 0x20;
pub const WM_TIMER: u32 = 0x0113;
pub const WM_USER: u32 = 0x0400;
pub const DM_GETDEFID: u32 = WM_USER;
pub const DC_HASDEFID: isize = 0x534B;
pub const BM_GETCHECK: u32 = 0x00F0;
pub const BM_SETCHECK: u32 = 0x00F1;
pub const BM_CLICK: u32 = 0x00F5;
pub const BM_SETSTYLE: u32 = 0x00F4;
pub const PBM_SETRANGE: u32 = WM_USER + 1;
pub const PBM_SETPOS: u32 = WM_USER + 2;
pub const EM_SETSEL: u32 = 0x00B1;
pub const EM_SETLIMITTEXT: u32 = 0x00C5;
pub const EN_CHANGE: u32 = 0x0300;
pub const BN_CLICKED: u32 = 0;
pub const IDOK: usize = 1;
pub const IDCANCEL: usize = 2;
pub const IDYES: i32 = 6;

// Window styles.
pub const WS_CAPTION: u32 = 0x00C0_0000;
pub const WS_SYSMENU: u32 = 0x0008_0000;
pub const WS_MINIMIZEBOX: u32 = 0x0002_0000;
pub const WS_VISIBLE: u32 = 0x1000_0000;
pub const WS_CHILD: u32 = 0x4000_0000;
pub const WS_TABSTOP: u32 = 0x0001_0000;
pub const WS_GROUP: u32 = 0x0002_0000;
pub const WS_VSCROLL: u32 = 0x0020_0000;
pub const WS_DISABLED: u32 = 0x0800_0000;
pub const WS_CLIPCHILDREN: u32 = 0x0200_0000;
pub const WS_EX_CLIENTEDGE: u32 = 0x0000_0200;
pub const WS_EX_STATICEDGE: u32 = 0x0002_0000;
pub const WS_EX_CONTROLPARENT: u32 = 0x0001_0000;
pub const CS_HREDRAW: u32 = 0x0002;
pub const CS_VREDRAW: u32 = 0x0001;
pub const CW_USEDEFAULT: i32 = i32::MIN;
pub const COLOR_BTNFACE: isize = 15;
pub const COLOR_WINDOW: i32 = 5;
pub const COLOR_BTNSHADOW: i32 = 16;
pub const COLOR_BTNHIGHLIGHT: i32 = 20;
pub const IDC_ARROW: usize = 32512;
pub const DEFAULT_GUI_FONT: i32 = 17;
pub const SW_SHOW: i32 = 5;
pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;
pub const SWP_NOZORDER: u32 = 0x0004;
pub const SWP_NOACTIVATE: u32 = 0x0010;
pub const SWP_NOSIZE: u32 = 0x0001;
pub const PM_REMOVE: u32 = 1;
pub const GWL_ID: i32 = -12;
pub const GWL_STYLE: i32 = -16;

// Control styles.
pub const BS_PUSHBUTTON: u32 = 0x0000;
pub const BS_DEFPUSHBUTTON: u32 = 0x0001;
pub const BS_AUTOCHECKBOX: u32 = 0x0003;
pub const BS_AUTORADIOBUTTON: u32 = 0x0009;
pub const BS_GROUPBOX: u32 = 0x0007;
pub const SS_LEFT: u32 = 0x0000;
pub const SS_NOPREFIX: u32 = 0x0080;
pub const SS_ETCHEDHORZ: u32 = 0x0010;
pub const SS_SUNKEN: u32 = 0x1000;
pub const ES_AUTOHSCROLL: u32 = 0x0080;
pub const ES_MULTILINE: u32 = 0x0004;
pub const ES_READONLY: u32 = 0x0800;
pub const PBS_SMOOTH: u32 = 0x01;

// Drawing.
pub const DT_WORDBREAK: u32 = 0x0010;
pub const DT_CALCRECT: u32 = 0x0400;
pub const DT_NOPREFIX: u32 = 0x0800;
pub const DT_LEFT: u32 = 0x0000;
pub const TRANSPARENT: i32 = 1;
pub const SRCCOPY: u32 = 0x00CC_0020;
pub const DIB_RGB_COLORS: u32 = 0;
pub const FW_NORMAL: i32 = 400;
pub const FW_BOLD: i32 = 700;

// Files and the system.
pub const GENERIC_READ: u32 = 0x8000_0000;
pub const GENERIC_WRITE: u32 = 0x4000_0000;
pub const FILE_SHARE_READ: u32 = 1;
pub const CREATE_ALWAYS: u32 = 2;
pub const OPEN_EXISTING: u32 = 3;
pub const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
pub const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
pub const ERROR_ALREADY_EXISTS: u32 = 183;
pub const ERROR_SHARING_VIOLATION: u32 = 32;
pub const ERROR_ACCESS_DENIED: u32 = 5;
pub const INVALID_FILE_ATTRIBUTES: u32 = 0xFFFF_FFFF;
pub const INVALID_SET_FILE_POINTER: u32 = 0xFFFF_FFFF;
pub const FILE_BEGIN: u32 = 0;
pub const MOVEFILE_DELAY_UNTIL_REBOOT: u32 = 4;
pub const SEM_FAILCRITICALERRORS: u32 = 1;
pub const MB_OK: u32 = 0;
pub const MB_YESNO: u32 = 4;
pub const MB_ICONQUESTION: u32 = 0x20;
pub const MB_ICONERROR: u32 = 0x10;
pub const MB_ICONINFORMATION: u32 = 0x40;

// The shell, the registry and COM.
pub const CSIDL_PROGRAMS: i32 = 2;
pub const SHCNE_ASSOCCHANGED: i32 = 0x0800_0000;
pub const SHCNF_IDLIST: u32 = 0;
pub const BIF_RETURNONLYFSDIRS: u32 = 1;
pub const BIF_STATUSTEXT: u32 = 4;
pub const HKEY_CLASSES_ROOT: usize = 0x8000_0000;
pub const HKEY_CURRENT_USER: usize = 0x8000_0001;
pub const HKEY_LOCAL_MACHINE: usize = 0x8000_0002;
pub const KEY_ALL_ACCESS: u32 = 0xF003F;
pub const KEY_READ: u32 = 0x20019;
pub const REG_SZ: u32 = 1;
pub const CLSCTX_INPROC_SERVER: u32 = 1;

#[repr(C)]
pub struct WndClassA {
	pub style: u32,
	pub wnd_proc: WndProc,
	pub cls_extra: i32,
	pub wnd_extra: i32,
	pub instance: Handle,
	pub icon: Handle,
	pub cursor: Handle,
	pub background: Handle,
	pub menu_name: *const u8,
	pub class_name: *const u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Point {
	pub x: i32,
	pub y: i32,
}

#[repr(C)]
pub struct Msg {
	pub hwnd: Handle,
	pub message: u32,
	pub wparam: usize,
	pub lparam: isize,
	pub time: u32,
	pub pt: Point,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Rect {
	pub left: i32,
	pub top: i32,
	pub right: i32,
	pub bottom: i32,
}

#[repr(C)]
pub struct PaintStruct {
	pub hdc: Handle,
	pub erase: i32,
	pub paint: Rect,
	pub restore: i32,
	pub inc_update: i32,
	pub reserved: [u8; 32],
}

#[repr(C)]
pub struct StartupInfoA {
	pub cb: u32,
	pub reserved: *mut u8,
	pub desktop: *mut u8,
	pub title: *mut u8,
	pub x: u32,
	pub y: u32,
	pub x_size: u32,
	pub y_size: u32,
	pub x_chars: u32,
	pub y_chars: u32,
	pub fill: u32,
	pub flags: u32,
	pub show: u16,
	pub reserved2: u16,
	pub reserved3: *mut u8,
	pub std_in: Handle,
	pub std_out: Handle,
	pub std_err: Handle,
}

#[repr(C)]
pub struct ProcessInformation {
	pub process: Handle,
	pub thread: Handle,
	pub process_id: u32,
	pub thread_id: u32,
}

#[repr(C)]
pub struct BrowseInfoA {
	pub owner: Handle,
	pub root: *const c_void,
	pub display_name: *mut u8,
	pub title: *const u8,
	pub flags: u32,
	pub callback: *const c_void,
	pub lparam: isize,
	pub image: i32,
}

#[repr(C)]
pub struct BitmapInfoHeader {
	pub size: u32,
	pub width: i32,
	pub height: i32,
	pub planes: u16,
	pub bit_count: u16,
	pub compression: u32,
	pub size_image: u32,
	pub x_pels: i32,
	pub y_pels: i32,
	pub colours_used: u32,
	pub colours_important: u32,
}

#[repr(C)]
pub struct Guid {
	pub d1: u32,
	pub d2: u16,
	pub d3: u16,
	pub d4: [u8; 8],
}

#[link(name = "kernel32", kind = "raw-dylib", import_name_type = "undecorated")]
unsafe extern "system" {
	pub fn ExitProcess(code: u32) -> !;
	pub fn GetModuleHandleA(name: *const u8) -> Handle;
	pub fn GetModuleFileNameA(module: Handle, name: *mut u8, len: u32) -> u32;
	pub fn GetCommandLineA() -> *const u8;
	pub fn GetProcessHeap() -> Handle;
	pub fn HeapAlloc(heap: Handle, flags: u32, bytes: usize) -> *mut u8;
	pub fn HeapFree(heap: Handle, flags: u32, p: *mut u8) -> i32;
	pub fn CreateFileA(
		name: *const u8,
		access: u32,
		share: u32,
		security: *const c_void,
		disposition: u32,
		flags: u32,
		template: Handle,
	) -> Handle;
	pub fn ReadFile(file: Handle, buf: *mut u8, len: u32, read: *mut u32, overlapped: *mut c_void) -> i32;
	pub fn WriteFile(file: Handle, buf: *const u8, len: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
	pub fn CloseHandle(h: Handle) -> i32;
	pub fn GetFileSize(file: Handle, high: *mut u32) -> u32;
	pub fn SetFilePointer(file: Handle, distance: i32, high: *mut i32, method: u32) -> u32;
	pub fn GetFileAttributesA(name: *const u8) -> u32;
	pub fn SetFileAttributesA(name: *const u8, attributes: u32) -> i32;
	pub fn DeleteFileA(name: *const u8) -> i32;
	pub fn CreateDirectoryA(name: *const u8, security: *const c_void) -> i32;
	pub fn RemoveDirectoryA(name: *const u8) -> i32;
	pub fn CopyFileA(from: *const u8, to: *const u8, fail_if_exists: i32) -> i32;
	pub fn MoveFileExA(from: *const u8, to: *const u8, flags: u32) -> i32;
	pub fn GetLastError() -> u32;
	pub fn GetTickCount() -> u32;
	pub fn Sleep(ms: u32);
	pub fn GetEnvironmentVariableA(name: *const u8, buf: *mut u8, len: u32) -> u32;
	pub fn GetTempPathA(len: u32, buf: *mut u8) -> u32;
	pub fn GetWindowsDirectoryA(buf: *mut u8, len: u32) -> u32;
	pub fn GetSystemDirectoryA(buf: *mut u8, len: u32) -> u32;
	pub fn GetDiskFreeSpaceA(root: *const u8, sectors: *mut u32, bytes: *mut u32, free: *mut u32, total: *mut u32) -> i32;
	pub fn LoadLibraryA(name: *const u8) -> Handle;
	pub fn GetProcAddress(module: Handle, name: *const u8) -> *const c_void;
	pub fn CreateProcessA(
		app: *const u8,
		command_line: *mut u8,
		process_security: *const c_void,
		thread_security: *const c_void,
		inherit: i32,
		flags: u32,
		environment: *const c_void,
		dir: *const u8,
		startup: *const StartupInfoA,
		info: *mut ProcessInformation,
	) -> i32;
	pub fn SetErrorMode(mode: u32) -> u32;
	pub fn MultiByteToWideChar(code_page: u32, flags: u32, src: *const u8, src_len: i32, dst: *mut u16, dst_len: i32) -> i32;
	pub fn WideCharToMultiByte(
		code_page: u32,
		flags: u32,
		src: *const u16,
		src_len: i32,
		dst: *mut u8,
		dst_len: i32,
		default_char: *const u8,
		used_default: *mut i32,
	) -> i32;
}

#[link(name = "user32", kind = "raw-dylib", import_name_type = "undecorated")]
unsafe extern "system" {
	pub fn RegisterClassA(class: *const WndClassA) -> u16;
	pub fn CreateWindowExA(
		ex_style: u32,
		class_name: *const u8,
		title: *const u8,
		style: u32,
		x: i32,
		y: i32,
		width: i32,
		height: i32,
		parent: Handle,
		menu: Handle,
		instance: Handle,
		param: *const c_void,
	) -> Handle;
	pub fn DestroyWindow(hwnd: Handle) -> i32;
	pub fn DefWindowProcA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize;
	pub fn GetMessageA(msg: *mut Msg, hwnd: Handle, min: u32, max: u32) -> i32;
	pub fn PeekMessageA(msg: *mut Msg, hwnd: Handle, min: u32, max: u32, remove: u32) -> i32;
	pub fn TranslateMessage(msg: *const Msg) -> i32;
	pub fn DispatchMessageA(msg: *const Msg) -> isize;
	pub fn IsDialogMessageA(dialog: Handle, msg: *const Msg) -> i32;
	pub fn PostQuitMessage(code: i32);
	pub fn SendMessageA(hwnd: Handle, msg: u32, wparam: usize, lparam: isize) -> isize;
	pub fn SetWindowTextA(hwnd: Handle, text: *const u8) -> i32;
	pub fn GetWindowTextA(hwnd: Handle, buf: *mut u8, max: i32) -> i32;
	pub fn GetWindowTextLengthA(hwnd: Handle) -> i32;
	pub fn EnableWindow(hwnd: Handle, enable: i32) -> i32;
	pub fn IsWindowEnabled(hwnd: Handle) -> i32;
	pub fn ShowWindow(hwnd: Handle, cmd: i32) -> i32;
	pub fn UpdateWindow(hwnd: Handle) -> i32;
	pub fn SetWindowPos(hwnd: Handle, after: Handle, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
	pub fn GetClientRect(hwnd: Handle, rect: *mut Rect) -> i32;
	pub fn GetWindowRect(hwnd: Handle, rect: *mut Rect) -> i32;
	pub fn AdjustWindowRect(rect: *mut Rect, style: u32, menu: i32) -> i32;
	pub fn GetSystemMetrics(index: i32) -> i32;
	pub fn LoadCursorA(instance: Handle, name: *const u8) -> Handle;
	pub fn MessageBoxA(hwnd: Handle, text: *const u8, caption: *const u8, flags: u32) -> i32;
	pub fn SetTimer(hwnd: Handle, id: usize, ms: u32, proc_: *const c_void) -> usize;
	pub fn KillTimer(hwnd: Handle, id: usize) -> i32;
	pub fn GetDC(hwnd: Handle) -> Handle;
	pub fn ReleaseDC(hwnd: Handle, dc: Handle) -> i32;
	pub fn DrawTextA(dc: Handle, text: *const u8, len: i32, rect: *mut Rect, format: u32) -> i32;
	pub fn FillRect(dc: Handle, rect: *const Rect, brush: Handle) -> i32;
	pub fn BeginPaint(hwnd: Handle, ps: *mut PaintStruct) -> Handle;
	pub fn EndPaint(hwnd: Handle, ps: *const PaintStruct) -> i32;
	pub fn InvalidateRect(hwnd: Handle, rect: *const Rect, erase: i32) -> i32;
	pub fn SetFocus(hwnd: Handle) -> Handle;
	pub fn SetForegroundWindow(hwnd: Handle) -> i32;
	pub fn GetSysColor(index: i32) -> u32;
	pub fn GetDlgItem(hwnd: Handle, id: i32) -> Handle;
	pub fn SetWindowLongA(hwnd: Handle, index: i32, value: i32) -> i32;
	pub fn GetWindowLongA(hwnd: Handle, index: i32) -> i32;
	pub fn GetFocus() -> Handle;
	pub fn GetNextDlgTabItem(dialog: Handle, control: Handle, previous: i32) -> Handle;
	pub fn GetKeyState(key: i32) -> i16;
}

#[link(name = "gdi32", kind = "raw-dylib", import_name_type = "undecorated")]
unsafe extern "system" {
	pub fn GetStockObject(object: i32) -> Handle;
	pub fn SelectObject(dc: Handle, object: Handle) -> Handle;
	pub fn DeleteObject(object: Handle) -> i32;
	pub fn CreateSolidBrush(colour: u32) -> Handle;
	pub fn CreateFontA(
		height: i32,
		width: i32,
		escapement: i32,
		orientation: i32,
		weight: i32,
		italic: u32,
		underline: u32,
		strike: u32,
		charset: u32,
		precision: u32,
		clip: u32,
		quality: u32,
		pitch: u32,
		face: *const u8,
	) -> Handle;
	pub fn SetBkMode(dc: Handle, mode: i32) -> i32;
	pub fn SetTextColor(dc: Handle, colour: u32) -> u32;
	pub fn TextOutA(dc: Handle, x: i32, y: i32, text: *const u8, len: i32) -> i32;
	pub fn CreateCompatibleDC(dc: Handle) -> Handle;
	pub fn CreateCompatibleBitmap(dc: Handle, w: i32, h: i32) -> Handle;
	pub fn DeleteDC(dc: Handle) -> i32;
	pub fn BitBlt(dst: Handle, x: i32, y: i32, w: i32, h: i32, src: Handle, sx: i32, sy: i32, rop: u32) -> i32;
	pub fn GetDIBits(
		dc: Handle,
		bitmap: Handle,
		start: u32,
		lines: u32,
		bits: *mut u8,
		info: *mut BitmapInfoHeader,
		usage: u32,
	) -> i32;
}

#[link(name = "comctl32", kind = "raw-dylib", import_name_type = "undecorated")]
unsafe extern "system" {
	pub fn InitCommonControls();
}

#[link(name = "shell32", kind = "raw-dylib", import_name_type = "undecorated")]
unsafe extern "system" {
	pub fn SHBrowseForFolderA(info: *mut BrowseInfoA) -> *mut c_void;
	pub fn SHGetPathFromIDListA(pidl: *const c_void, path: *mut u8) -> i32;
	pub fn SHGetSpecialFolderLocation(owner: Handle, csidl: i32, pidl: *mut *mut c_void) -> i32;
	pub fn SHChangeNotify(event: i32, flags: u32, item1: *const c_void, item2: *const c_void);
}

#[link(name = "advapi32", kind = "raw-dylib", import_name_type = "undecorated")]
unsafe extern "system" {
	pub fn RegOpenKeyExA(key: usize, sub: *const u8, options: u32, access: u32, out: *mut usize) -> i32;
	pub fn RegCreateKeyExA(
		key: usize,
		sub: *const u8,
		reserved: u32,
		class: *const u8,
		options: u32,
		access: u32,
		security: *const c_void,
		out: *mut usize,
		disposition: *mut u32,
	) -> i32;
	pub fn RegSetValueExA(key: usize, name: *const u8, reserved: u32, kind: u32, data: *const u8, len: u32) -> i32;
	pub fn RegQueryValueExA(key: usize, name: *const u8, reserved: *mut u32, kind: *mut u32, data: *mut u8, len: *mut u32)
	-> i32;
	pub fn RegCloseKey(key: usize) -> i32;
	pub fn RegDeleteKeyA(key: usize, sub: *const u8) -> i32;
}

#[link(name = "ole32", kind = "raw-dylib", import_name_type = "undecorated")]
unsafe extern "system" {
	pub fn CoInitialize(reserved: *const c_void) -> i32;
	pub fn CoUninitialize();
	pub fn CoCreateInstance(clsid: *const Guid, outer: *mut c_void, context: u32, iid: *const Guid, out: *mut *mut c_void)
	-> i32;
	pub fn CoTaskMemFree(p: *mut c_void);
}
