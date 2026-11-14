//! Frontend-neutral types shared by the terminal and GUI frontends.
//!
//! Holds styled text ([`styled`]), input events ([`input_event`]), geometry
//! ([`geometry`]) and ratatui-free leaf modules moved from `codemason-tui`
//! (logo, tips, clipboard, open-in, branch names, deck progress, `app::*`).

#![deny(unsafe_code)]

pub mod app;
pub mod branch_names;
pub mod clipboard;
pub mod deck_progress;
pub mod geometry;
pub mod input_event;
pub mod logo;
pub mod open_in;
pub mod styled;
pub mod tips;
