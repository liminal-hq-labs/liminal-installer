# Decisions

Each decision is numbered and stays here even when superseded, with a note on what replaced it. Open questions move here once they are answered.

| # | Decision | Choice | Why |
|---|---|---|---|
| D1 | Home | A Liminal HQ Labs experiment in `liminal-hq-labs/liminal-installer`, public from the first commit | Try the idea in the open before it earns a place in Liminal HQ |
| D2 | Licence | `Apache-2.0 OR MIT` | House licence, compatible with both ecosystems |
| D3 | Dependency updates | No Dependabot configuration until the project opts in | Labs repositories stay quiet by default. See the [Labs charter](https://github.com/liminal-hq-labs/.github/blob/main/docs/labs-charter.md#dependabot-is-opt-in) |
| D4 | DOS floor | A 386 or later with a DOS extender | Keeps the retro runtime small enough for a floppy without the cost of real-mode 8086 support |
| D5 | `canvas` controls | The renderer draws its own controls, with native accessibility hooks added later; only the scaffold exists now | One look on every platform from a small binary, without waiting on per-toolkit controls |
| D6 | Hub feeds | The Hub reads multiple feeds, each with its own pinned key | A publisher's key only ever vouches for that publisher's feed |
| D7 | Delta updates | Deferred | Whole-file updates are enough to ship the first version, and the receipt format leaves room for chunks later |
| D8 | Theme sharing | Themes are shared through a theme section in any feed | Reuses the signing and pinning that feeds already have, with no separate gallery to run |
| D9 | Final name | The working name stays `liminst` until one is chosen from the Names board | The name and CLI command change together, once, rather than twice |
