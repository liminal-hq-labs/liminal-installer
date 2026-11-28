# Startup helper example

Startup helper is a small product for a program that stays running: it starts at logon, has a Start menu item that stops it, and has to be told to stop before its files can be removed. It exercises four setup features together:

- **A `startup` shortcut place.** `in = "startup"` puts the shortcut in the person's Startup folder on Windows 95 and later, and in `~/.config/autostart` on Linux (the engine's tests cover that).
- **Shortcut arguments.** `args = "/quit"` is the command line after the target.
- **A program to run before an uninstall** (`[[uninstall.pre]]`, with `capabilities = ["run"]`): it runs before an uninstall removes files and before an install over an earlier one replaces them, waits up to `wait_ms` and, unless `ignore_failure`, stops the work when it fails.
- **Delete at restart.** A file still in use when the uninstall reaches it is removed when Windows next starts, and the person is told.

`win32/tools/wine-e2e.sh` installs it with the real Windows 95 `SETUP.EXE` under Wine, holds `HELPER.LOG` open, uninstalls, and checks the four. `STOP.BAT` stands in for `HELPER.EXE /quit`. A Run key value is the alternative to the Startup folder on the engine's own targets: a `registry.set` operation writes any value (see SPEC section 8), so no second mechanism exists.
