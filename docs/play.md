# Playing with Windows 95 by hand

`scripts/win95/play-win95.sh` starts the Windows 95 OSR2.5 base from the [test bed](win95-testbed.md) in a real DOSBox-X window on your desktop, with a working mouse and keyboard, and with Quill ready on `D:`. It is the interactive twin of `run-win95.sh`: no Xvfb, no screenshots, no automation.

## Table of contents

- [Before you start](#before-you-start)
- [Run it](#run-it)
- [The mouse and the keyboard](#the-mouse-and-the-keyboard)
- [A bigger or sharper window](#a-bigger-or-sharper-window)
- [What is on D](#what-is-on-d)
- [What works and what does not](#what-works-and-what-does-not)
- [Sound](#sound)
- [Stopping, and what is cleaned up](#stopping-and-what-is-cleaned-up)
- [How it reaches your screen](#how-it-reaches-your-screen)
- [DOSBox-X on the host instead of Docker](#dosbox-x-on-the-host-instead-of-docker)

## Before you start

- The base images from [the test bed](win95-testbed.md#building-the-base-image) must exist. On this machine they are in `/mnt/games/media/win95-testbed`, so set `W95_HOME` to that folder (the script otherwise looks in `$HOME/win95-testbed`). They are only read: every session copies the base to a throwaway working disk first.
- Docker, `xdotool` (it reads the screen size), `sfdisk`, `mkfs.fat` and mtools, as for the rest of the test bed. The container image `liminal-win95-dosbox-x:local` is built from `scripts/win95/dosbox-x.Dockerfile` on first use.
- A desktop session: `DISPLAY` must be set. Wayland is fine, through XWayland.
- A built `QUILL32.EXE`. The script looks in a Quill checkout (`-q DIR`, or `QUILL_DIR`, default `~/source/experiments/quill16`) at `win9x/dist/QUILL32.EXE`. If it is missing the script says so and starts the plain desktop. Build it with `(cd ~/source/experiments/quill16/win9x && ./build.sh)`.

## Run it

```sh
export W95_HOME=/mnt/games/media/win95-testbed
scripts/win95/play-win95.sh                      # plain Windows 95 OSR2.5, Quill starts at logon
scripts/win95/play-win95.sh -b ie4               # the Internet Explorer 4 and Desktop Update base
scripts/win95/play-win95.sh -n                   # the desktop only, nothing starts
scripts/win95/play-win95.sh -a 'D:\LETTER.RTF'   # Quill opens a document at logon
scripts/win95/play-win95.sh -d ~/some/files MYAPP.EXE   # your own files on D:, and MYAPP.EXE starts instead of Quill
```

| Option | Meaning |
| --- | --- |
| `-b plain\|ie4` | Which base to copy (default plain). |
| `-d DIR` | Also put the contents of this host directory on `D:`. |
| `-q DIR` | The Quill checkout to take the build outputs and sample documents from. |
| `-n` | Start nothing at logon. |
| `-a ARGS` | Arguments for the program that starts at logon. |
| `-s 1..4` | Window size in multiples of 640 by 480. The default is 2 on a display at least 1080 pixels tall, otherwise 1. |
| `-o opengl\|openglnb\|surface` | The video output; see [A bigger or sharper window](#a-bigger-or-sharper-window). |
| `-x` | Allow the container to reach the X server with `xhost`; see [How it reaches your screen](#how-it-reaches-your-screen). |
| `-N` | Use a DOSBox-X installed on the host; see [DOSBox-X on the host instead of Docker](#dosbox-x-on-the-host-instead-of-docker). |
| `-K` | Keep the working disk and the exchange disk (the run folder is under `$W95_HOME/runs/play-<time>/`). |

Programs named on the command line are copied to `D:`, and the first one starts at logon in place of Quill. The first start copies a 1 GB sparse image and takes a few seconds; Windows 95 then reaches the desktop in about 30 seconds.

## The mouse and the keyboard

DOSBox-X hands Windows 95 a PS/2 mouse that only moves relative to where it was, so the host pointer has to be captured.

- **Click inside the window** to capture the pointer. It disappears from the host and moves Windows 95's pointer.
- **Ctrl+F10** releases it (and captures it again). The window title repeats this.
- The keyboard goes to the window while it has focus; your desktop keeps the shortcuts it grabs first (Super, and Alt+Tab on most setups).
- Fast flicks of the mouse travel further than slow ones, because Windows 95 accelerates pointer motion. This is the guest's behaviour, not a fault.

## A bigger or sharper window

Windows 95 runs at 640 by 480, so the script opens a 1280 by 960 window (`-s 2`) when the display is tall enough. `-s 3` gives 1920 by 1440.

`-o opengl` (the default) scales smoothly. `-o openglnb` scales with nearest-neighbour, which keeps pixels square and crisp at whole-number sizes. `-o surface` never scales and is always 640 by 480: the fallback if OpenGL misbehaves in the container.

## What is on D

`D:` is a small FAT16 disk built for the session from:

- `QUILL32.EXE` from `win9x/dist/` (the Win32 build, the one that runs on Windows 95);
- `QUILLDOS.EXE` from `dos/dist/` and `QUILL.EXE` from `win16/out/`, when built (they are a DOS and a Windows 3.x program: `QUILLDOS.EXE` runs from the MS-DOS prompt, `QUILL.EXE` does not run on Windows 95);
- `LETTER.RTF`, `LETTER.QLD` and `PLAIN.QLD` from `tests/docs/`;
- whatever `-d` and the command line add.

When you stop, files that were added or changed on `D:` are copied to `$W95_HOME/runs/play-<time>/out/` and the path is printed; unchanged files are not. Everything on `C:` is thrown away.

## What works and what does not

What was checked in an Xvfb session of this script: both bases boot to the desktop; `QUILL32.EXE` starts from `WIN.INI`'s `run=` line and draws its window with the toolbar and ruler; typing reaches it; Ctrl+O raises its prompt; the pointer moves after a click and is released by Ctrl+F10; stopping with a signal removes the container and the working disk.

Known limits:

- Windows 95 here is a plain OSR2.5 install on emulated S3 video at 640 by 480. There is no network, no printer and no sound.
- The OpenGL output may run in software inside the container. If the window stutters, try `-s 1` or `-o surface`.
- The Win16 `QUILL.EXE` is not part of Windows 95. It is for Windows 3.11 (Quill's own `docs/play.md` covers it) and is unfinished: it opens documents on 3.11, it previously faulted, and the vendored runtime fixed the faults in 30 of 30 scripted runs, but it is not done.
- Shutting down from Start shows "It's now safe to turn off your computer". Close the window then.

## Sound

None. The profile sets `sbtype=none` and `gus=false`, and the container uses SDL's dummy audio driver, so no sound is expected and no sound device is touched.

## Stopping, and what is cleaned up

Close the window, press Ctrl+C in the terminal, or run `docker rm -f liminal-win95-play` from another terminal. The script then removes the container, copies back what changed on `D:`, deletes the working disk and the exchange disk (unless `-K`), and takes back any `xhost` permission it added. If nothing was kept it removes the run folder too. The base images are never opened for writing.

## How it reaches your screen

The container runs as your user with the host network, the X11 socket (`/tmp/.X11-unix`), `DISPLAY`, your host name and a read-only copy of your Xauthority file (`$XAUTHORITY`, which XWayland under GNOME keeps in `/run/user/<uid>/`). That is enough on a default GNOME or KDE session and needs no change to the X server's access list.

If DOSBox-X still says it cannot open the display, run with `-x`. That runs `xhost +SI:localuser:$USER` before the container starts, which lets processes of your own user connect without a cookie, and `xhost -SI:localuser:$USER` when the session ends (it does nothing if the permission was already there). Nothing else is opened to the network.

## DOSBox-X on the host instead of Docker

Arch's official repositories carry only `dosbox` 0.74.3 (`pacman -Si dosbox-x` finds nothing), which cannot boot a hard disk image of Windows 95. DOSBox-X is in the AUR:

```sh
paru -S dosbox-x          # the AUR package (2026.10.01 when this was written); dosbox-x-sdl2 and dosbox-x-git also exist
scripts/win95/play-win95.sh -N
```

`-N` writes the same profile with host paths and runs `dosbox-x` directly, with no Docker and no X11 plumbing. That path was written but not run here, since the host has no DOSBox-X. The Flatpak (`flatpak install flathub com.dosbox_x.DOSBox-X`) sandboxes file access, so it needs `--filesystem=$W95_HOME` and a `dosbox-x` wrapper on your `PATH` that calls `flatpak run`; the container is simpler.
