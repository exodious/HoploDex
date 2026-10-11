//! Reading an import file in a confined child, so a hostile spreadsheet can't
//! exhaust the app's memory or time (research.md §8, §9). The parent holds the
//! `client`; the child runs the `reader` under the `limits`, and they talk in
//! `frames`.

pub mod client;
pub mod frames;
pub mod limits;
pub mod reader;
