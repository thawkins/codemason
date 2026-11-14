//! Frontend-neutral types shared by the terminal and GUI frontends.
//!
//! Holds styled text ([`styled`]), input events ([`input_event`]) and
//! geometry ([`geometry`]). Has no dependencies; frontends provide adapters.

#![deny(unsafe_code)]

pub mod geometry;
pub mod input_event;
pub mod styled;
