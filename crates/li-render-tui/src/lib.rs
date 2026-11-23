// The tui renderer: draws a layout node tree with ratatui and handles keyboard-only input.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The `tui` renderer.
//!
//! A [`Screen`] holds one [`li_model::Layout`] and its focus state. It draws into a ratatui
//! [`Buffer`](ratatui::buffer::Buffer), so it can be tested and dumped as text without a
//! terminal ([`dump`]), and it runs in a real terminal through [`run_page`].
//!
//! The engine plugs in through [`Bindings`]: the renderer reads and writes the values the
//! layout binds to (`bind` on a node), including the live meter values.
//!
//! # Node properties
//!
//! Properties are read from `Node::props`; every one is optional.
//!
//! | Node | Properties |
//! |---|---|
//! | `window` | `title` |
//! | `backdrop` | `title`; `meter` children sit at the bottom, the rest is centred |
//! | `dialog` | `title`, `width` |
//! | `stack` | `direction` (`vertical` or `horizontal`), `gap` |
//! | `grid` | `columns` (default 2), `gap` |
//! | `text` | `text`, `align` (`left`, `centre`, `right`) |
//! | `image` | `alt` |
//! | `billboard` | `items` (strings), `text`; the bound value is the index shown |
//! | `meter` | `meter` (`file`, `total` or `free_space`), `label`, `width`, `low_below` |
//! | `progress` | `label`, `width` |
//! | `button` | `label`, `action` (defaults to `id`, then `bind`, then the label), `default` |
//! | `checklist` | `label`, `items` (`{id, label, checked, required}`); the bound value is a list of ids |
//! | `radio` | `label`, `items` (`{id, label}`), `default`; the bound value is the chosen id |
//! | `field`, `path` | `label`, `width`, `placeholder`; the bound value is the text |
//!
//! Text properties may hold `{name}` placeholders, which a screen fills from its bindings when it draws. A name with no bound value stays as written.
//!
//! A node's `region` property (`side`, `hints`) marks pictorial content a text screen leaves out, and `meter` may be written `source`, `options` may stand for `items`, and a stack's `direction` may be `row` or `column`.
//!
//! Every control's accessible name is its `name`, `label` or `text` property, in that order.

mod artboard;
mod bindings;
mod cells;
mod draw;
mod dump;
mod flatten;
mod glyphs;
mod interp;
mod run;
mod screen;

pub use artboard::{SIDE_WIDTH, flatten_artboard, nearest_vga};
pub use bindings::{Bindings, MapBindings};
pub use cells::{Cell, CellGrid, Colour, Modifiers, render_cells};
pub use dump::{dump, dump_with_controls};
pub use flatten::{DEFAULT_ATTR, Flat, FlatCell, Widget, attr_of};
pub use glyphs::Look;
pub use interp::{interpolate, interpolate_node};
pub use run::{PageResult, Tick, TickResult, run_page};
pub use screen::{ControlKind, ControlSummary, Outcome, Screen};

/// The renderer id used to pick `@renderer` overrides in a layout.
pub const RENDERER_ID: &str = "tui";
