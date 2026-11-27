# Plans

Planner write-ups kept for reference. Each was accurate when it was written, and none is updated afterwards. Where a plan disagrees with [SPEC.md](../../SPEC.md) or [decisions.md](../decisions.md), the specification and the decision win.

| Plan | What it covers | Status |
| --- | --- | --- |
| [release-media-plan.md](release-media-plan.md) | The release media model: what a medium is, how targets and disks are described and how sizes are checked | Reference. Partly built: the `li-media` crate holds the media wire types and rules |
| [studio-media-screen-plan.md](studio-media-screen-plan.md) | The Studio Media screen: the list of media, the capacity meter, the disk map and the media commands | Reference. Partly built: the Media screen lists media over demo data and edits a medium's targets, label, switches and previews |
| [system-folders.md](system-folders.md) | Installing a file into the Windows folder or the system folder (`{windows}`, `{system}`) on the Windows 3.x and Windows 95 targets, for Quill's Ghostwriter screen saver: backups, receipts, checks, budgets and the work packages | Plan. Not built |
| [win16-headroom.md](win16-headroom.md) | Room in the Windows 3.1 runtime's 64 KB data segment and its code segments: what fills them (measured), the ranked ways to win back at least 4 KB, the headroom rule and the `WP-W16-*` packages | Plan. Not built |
