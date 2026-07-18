//! Library surface for ripmusic, so the loudness/encode logic is unit-testable
//! and reusable from examples (the binary is a thin CLI over these).

pub mod cdrom;
pub mod encode;
pub mod image;
pub mod loudness;
