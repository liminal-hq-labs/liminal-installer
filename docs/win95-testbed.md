# Windows 95 test bed

A real Windows 95 OSR2.5 under DOSBox-X, built from the owner's CD image, for checking the Win32 builds of Liminal Installer and Quill on the oldest Win32 floor we claim. It runs headless, from a read-only base image, with a throwaway copy per run. The scripts are in [`scripts/win95/`](../scripts/win95/).

## Table of contents

- [What you get](#what-you-get)
- [Building the base image](#building-the-base-image)
- [Running a program](#running-a-program)
- [The Internet Explorer 4 base](#the-internet-explorer-4-base)
- [The mouse](#the-mouse)
- [Screenshots and keys](#screenshots-and-keys)
- [How it works](#how-it-works)
- [What this Windows 95 is](#what-this-windows-95-is)
- [What was hit on the way](#what-was-hit-on-the-way)
- [Limits](#limits)
- [VirtualBox as a second option](#virtualbox-as-a-second-option)

## What you get

- `win95-base.img`: a 1 GB raw disk with one FAT32 partition, Windows 95 OSR2.5 (4.00.950 C) installed, never booted again after its verification boot, mode 0444.
- `run-win95.sh`: copies the base to a working disk, puts your program on a small FAT16 disk that Windows 95 sees as `D:`, boots, lets the shell start the program from `WIN.INI`'s `run=` line, takes screenshots, shuts Windows down from the Start menu, and copies everything the program wrote to `D:` back to the host.
- `build-ie4.sh`: a second base, `win95-ie4-base.img`, with Internet Explorer 4 and the Windows Desktop Update on top (see below). `run-win95.sh -b ie4` picks it.
- `screenshot.sh`, `type-keys.sh` and `mouse.sh`: helpers for driving a guest that is already up; `mouse.sh` also backs `run-win95.sh -m`.

Everything lives under `$W95_HOME` (default `$HOME/win95-testbed`). Keep it outside the repository: the images, the logs and the runs are not committed.

## Building the base image

```sh
W95_HOME=/mnt/games/media/win95-testbed \
	scripts/win95/build-base.sh "/path/Win95 OSR2.ISO" "/path/key.txt"
```

The ISO is only read. The key file's last matching line must look like `XXXXX-OEM-XXXXXXX-XXXXX`; it is split into the three boxes of the Certificate of Authenticity page and typed with `xdotool` from standard input. It is not written to any image, answer file, log or command line, and the scripts hold no key. The install needs Docker (the DOSBox-X image is built from `scripts/win95/dosbox-x.Dockerfile` on first use), Xvfb, `xdotool`, ImageMagick, 7-Zip, `sfdisk`, `mkfs.fat` and mtools.

The stages are `prep`, `boot`, `setup` and `verify`; each leaves a marker in `$W95_HOME/state`, so an interrupted run resumes at the stage that did not finish (`setup` always restarts from the snapshot `hd-prepared.img`). `--force` starts again. Measured on this machine, with `nice -n 19`: prep and boot about 50 seconds, Setup 492 seconds (from the first DOS prompt to a shut-down desktop), the verification boot about 20 seconds.

## Running a program

```sh
W95_HOME=/mnt/games/media/win95-testbed scripts/win95/run-win95.sh [options] PROGRAM.EXE [ARGS...]
```

| Option | Meaning |
| --- | --- |
| `-w SECONDS` | How long the program runs after the desktop appears (default 30). |
| `-i SECONDS` | Seconds between screenshots (default 5). |
| `-k "KEYS"` | After the wait, press these xdotool key names, one screenshot each; repeatable (`-k alt+h -k a` opens Quill's About box). |
| `-d DIR` | Also put the contents of a host directory on `D:` (a payload, say). |
| `-g COMMAND` | Run a program already in Windows 95, such as `C:\WINDOWS\WINVER.EXE`, instead of a host file. |
| `-K` | Keep `work.img` and `exchange.img` in the run folder. |
| `-b plain\|ie4` | Which base to copy: plain OSR2.5 (default) or the Internet Explorer 4 one. |
| `-m SCRIPT` | A mouse script run once the desktop is up, for example `"click 24 466; wait 1; shot start.png"`; see [The mouse](#the-mouse). |
| `-n NAME` | Label for the run folder. |

The run folder is `$W95_HOME/runs/<time>-<name>/` and its path is printed last: `shot-NN.png`, `final.png`, and `out/` holding what ended up on `D:`. A run takes about a minute. Windows 95 boots to the desktop in 7 to 20 seconds, DOSBox-X being fast.

A program that needs a different working directory or a closing key can be steered with `-k`; a longer script can source `lib.sh` and call `w95_key`, `w95_type` and `w95_shot` directly.

## The Internet Explorer 4 base

```sh
W95_HOME=/mnt/games/media/win95-testbed scripts/win95/build-ie4.sh
```

The CD's `WIN95` folder, copied to `C:\WIN95` on the plain base, already holds `IE4SETUP.EXE` and its cabinets, so this needs neither the ISO nor the key. The script copies the plain base, starts Setup from the Start menu's Run box, and drives the wizard by keys: licence accepted, Standard Installation, the Windows Desktop Update (the IE4 shell) on, the default channel region, the default folder. Nothing connects to a network. Setup restarts Windows once; the script then merges a `.reg` file that sets the Explorer policies `NoActiveDesktop`, `NoActiveDesktopChanges` and `NoChannelUI`. That removes the Active Desktop wallpaper (the desktop is plain teal again). The floating channel bar still shows on the right of the screen, because the policy does not remove it; Setup offers no option to leave it out.

It judges success by the version resources of the installed files and by a boot to the desktop, then freezes the disk as `win95-ie4-base.img` (mode 0444). Measured: 462 seconds for the install, plus a verification boot.

| File | Version |
| --- | --- |
| `COMCTL32.DLL` | 4.72.2106.4 |
| `SHDOCVW.DLL` | 4.72.2106.7 |
| `WININET.DLL` | 4.72.2106.5 |
| `SHELL32.DLL` | 4.72.2106.4 |

This CD carries Internet Explorer 4.01 SP2, so `COMCTL32.DLL` is 4.72, a little newer than the 4.71 of the first Internet Explorer 4.0 release. It is the IE4 shell and common controls level; there is no 4.71 on this disc. The stages are `install` and `verify`; `--force` starts again.

## The mouse

Windows 95 detects DOSBox-X's emulated PS/2 mouse during Setup (`DETLOG.TXT` records "Standard PS/2 Port Mouse") and uses it; the pointer follows and clicks land, shown by the screenshots from `run-win95.sh -m`.

DOSBox-X gives a PS/2 mouse only relative motion, and only while it holds the pointer grab, so the host pointer cannot be placed at a guest position. `mouse.sh` therefore does three things:

- **Grab.** `ctrl+F10` toggles DOSBox-X's pointer grab. `mouse.sh capture` moves the pointer, compares screenshots, and toggles if nothing moved. The profile sets `autolock=false` and `mouse_emulation=always`; a plain click on the window does not grab under Xvfb.
- **Mapping.** The guest screen is the 640 by 480 window, so a guest pixel is a window pixel. Windows 95 accelerates fast motion: steps of one pixel arrive one for one (checked by differencing screenshots at several targets), steps of two or three come out about 1.4 and 2 times too far, and one-pixel steps sent faster than every 20 ms are coalesced into larger packets and overshoot. So the pointer is homed against the top-left corner and walked in one-pixel steps every 20 ms, remembering where it is in `$W95_HOME/run/mouse.pos`. A move across the whole screen takes about ten seconds.
- **Buttons.** `xdotool click` sends the buttons; a double click is two clicks 90 ms apart.

```sh
scripts/win95/mouse.sh capture
scripts/win95/mouse.sh click 24 466                 # the Start button
scripts/win95/mouse.sh dblclick 50 77
scripts/win95/mouse.sh rclick 300 200
scripts/win95/mouse.sh drag 10 10 200 100
scripts/win95/mouse.sh script "click 24 466; wait 1; key Escape; shot start.png"
```

With `-m`, the script runs after the desktop is up and the grab is made; `shot NAME.png` writes into the run folder. Proofs from a run on this machine: Start menu, Programs submenu, and Quill's File and Help menus opened by mouse on the plain base; a double click in `me-files.exe` that opened a folder on the IE4 base.

## Screenshots and keys

```sh
W95_DISPLAY=:95 scripts/win95/screenshot.sh out.png
scripts/win95/type-keys.sh ctrl+Escape u Return
scripts/win95/type-keys.sh --text 'C:\TEST.EXE' --key Return
```

Screenshots capture the DOSBox-X window, so Windows 95's 640 by 480 screen comes out at exactly that size.

## How it works

- **DOSBox-X.** 2025.02.01 in Docker (`debian:trixie-slim` and the `dosbox-x` package), `machine=svga_s3`, `cputype=pentium`, `core=normal`, `cycles=max`, 64 MB, no sound card, `ver=7.1`. A Pentium-class SVGA PC, the same engine as the Windows 3.11 harness.
- **Disk preparation on the host.** The CD's El Torito floppy is the boot disk. The system disk is partitioned with `sfdisk` (type 0x0C) and formatted FAT32 with `mkfs.fat`; the `WIN95` folder and an answer file are copied in with mtools. DOSBox-X cannot read the CD from the guest's real-mode drivers (the Oak driver finds no drive), so Setup runs from `C:\WIN95` instead of the CD. That is also why later installs of Windows components never ask for a disc.
- **Boot code.** The floppy runs `fdisk /mbr` and `sys c:`; without the master boot record code DOSBox-X refuses to boot the disk.
- **Setup.** `AUTOEXEC.BAT` on the disk runs `SETUP.EXE MSBATCH.INF /IS /IQ` once. The answer file skips every wizard page except the licence and the Certificate of Authenticity, which the script answers with `alt+y` and the typed key. A printer wizard appears after the first restart and is dismissed by an Escape the script sends when a page has sat still for a while. The desktop is recognised by the teal background filling most of the screen.
- **Per-run copy.** `cp --sparse=always` of the 1 GB base to a working image (about 280 MB on disk), which is deleted after the run unless `-K` is given.
- **`D:`.** A second 32 MB disk image, FAT16 with a partition table, attached as the IDE slave. Windows 95 needs a partition table to give a hard disk a letter, so a bare FAT image would not do. DOSBox-X's host-folder mounts are not visible to a Windows 95 guest, so the disk is built per run with mtools and read back the same way once Windows has shut down and flushed.
- **Shutdown.** `ctrl+Escape`, `u`, `Return`, then 25 seconds, then the container is removed. Pulling the plug would lose the program's output to the lazy writer.

## What this Windows 95 is

The CD is OSR2.5 (4.00.950 C). What matters for what we test:

- **It is not bare retail Windows 95.** It has FAT32, the OSR2 kernel and shell (`KERNEL32.DLL` and `SHELL32.DLL` 4.00.1111, newer than retail's 4.00.950), USB support and a newer `COMCTL32.DLL`. Programs built against the retail floor can behave differently here.
- **The plain base is not the IE4 floor.** The disc carries `IE4SETUP.EXE` (Internet Explorer 4.01 and the Desktop Update shell), but Setup does not install it, and `build-ie4.sh` does that into a second base: the installed system has `COMCTL32.DLL` 4.70 (the Internet Explorer 3 level, dated 1996-08-24), `EXPLORER.EXE` 4.00.950, and no `SHDOCVW.DLL` or `WININET.DLL`. So the plain base is the OSR2 floor, below IE4's comctl32 and the integrated shell; `-b ie4` is the IE4 level (comctl32 4.72).
- **It is not Windows 95 with Winsock 2 or DCOM.** Nothing beyond the CD is installed.

## What was hit on the way

- The Oak CD driver on the boot floppy finds no drive under DOSBox-X's emulated IDE CD (with or without `ide, secondary` enabled), so a CD install in the usual way did not work; the source is copied to the disk instead.
- `boot -l c` on a DOSBox-X-mounted FAT32 image needs `[dos] ver=7.1` (otherwise it waits on a y/n prompt in the console) and needs master boot record code on the disk (`fdisk /mbr`); `sfdisk` writes none.
- Setup's licence page has a blinking caret, so "screen unchanged" is a pixel-count comparison, not a checksum.
- The Certificate of Authenticity page raises a "not valid" box over itself; its Re-enter button is what puts the caret in the first box. A wrong first attempt typed the key into the box instead.
- Typing the key with `xdotool type --file -` typed nothing; `--file /dev/stdin` works.
- The installer's illustrations contain the same teal as the desktop, so a one-third threshold ended the install at 49 per cent of the file copy. The detector now wants 55 per cent.
- A `PRODUCTKEY` line in the answer file is ignored for an OEM key, so the key is typed.

## Limits

- Windows 95 runs at 640 by 480 in 16 colours on the standard VGA driver. Setup did not pick the S3 driver; getting 256 colours or 800 by 600 needs the S3 Trio64 driver set up by hand (not done).
- No sound, no network, no CD in the guest.
- Mouse moves are slow (see above) and the pointer must be grabbed first; positions are only as good as the last `home`, so a script that clicks many places should `home` now and then.
- One guest at a time (a lock in `$W95_HOME/run`), and the display is the fixed Xvfb `:95`.
- A program that quits Windows 95 itself leaves the shutdown keys typing into a black screen; the run still returns.

## VirtualBox as a second option

Not built, only notes, taken from the VirtualBox manual and the long-standing community guides; none of it was run here. No VM exists for it.

Settings that Windows 95 wants:

- **Type** Windows 95, 64 MB of RAM (under 480 MB at the most; more than 512 MB breaks Windows 9x), 1 CPU, PAE off.
- **Chipset** PIIX3, **ACPI** and the **I/O APIC** off (Windows 95 has no ACPI and fails with an APIC), **EFI** off.
- **Storage** IDE (PIIX4 controller), a 1 to 2 GB disk, the CD on the secondary IDE.
- **Display** VBoxVGA, 8 to 16 MB; **audio** none or Sound Blaster 16; **network** PCnet-PCI II, or none.
- **Acceleration** nested paging and VT-x off if Setup stalls; Windows 95 does not use them.

The known crash is the "Windows protection error. You need to restart your computer" at the first or second boot, on CPUs faster than about 2.1 GHz (Microsoft knowledge base article Q312108 describes a timing-loop overflow in the disk driver stack, `ESDI_506.PDR`). Fixes, in order of preference: cap the CPU execution under the VM's System, Processor setting to a low value during Setup and the first boots; or patch the driver with `FIX95CPU`, an unofficial community utility (other community patchers for the same driver exist). **None of these was downloaded or run, and none should be without your approval**: they are third-party binaries, listed here only so you can decide.
