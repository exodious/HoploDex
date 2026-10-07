//! Document preview (feature 007, research.md §20): the `hdpreview`
//! protocol, the PDF surface, the TIFF render helper, text, and the
//! availability rules that decide which of them a document gets.

pub mod availability;
pub mod confine;
pub mod helper;
pub mod helper_protocol;
pub mod protocol_handler;
#[cfg(target_os = "linux")]
pub mod sandbox_probe;
pub mod surface;
pub mod text;
pub mod tiff;
pub mod tripwire;
