/* The same message-box program as src/bin/msgbox.rs, for Open Watcom: the size and header baseline the Rust build is compared with.
 *
 * (c) Copyright 2026 Liminal HQ, Scott Morris
 * SPDX-License-Identifier: Apache-2.0 OR MIT
 */

#include <windows.h>

int PASCAL WinMain(HINSTANCE inst, HINSTANCE prev, LPSTR cmd, int show)
{
	MessageBox(0, "Hello from Open Watcom", "Win16", MB_OK);
	return 0;
}
