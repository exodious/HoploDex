//! The shared core of the confined child processes (research.md §8): the
//! confinement, the Windows job object, the frame protocol's framing, and the
//! spawn, time limits and kill-on-exit plumbing that feature 007's TIFF render
//! helper (`services::preview`) and the import helper (`services::import_reader`)
//! both use. What a helper does with its frames is its own mode's.

pub mod confine;
pub mod frames;
pub mod handle;
#[cfg(windows)]
pub mod job;
