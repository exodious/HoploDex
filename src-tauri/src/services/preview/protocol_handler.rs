//! The `hdpreview` custom protocol: one-time URLs that serve a PDF from
//! memory to the surface (research.md §4, contracts/tauri-commands.md "The
//! `hdpreview` protocol").

use tauri::http::{Request, Response};

use crate::session::Session;

/// Answers the PDF surface's requests. Everything is `404` until T060 serves
/// the current token's bytes and the Linux hook; a request for anything else
/// is `404` for good (contracts/tauri-commands.md "The `hdpreview`
/// protocol").
pub fn handle(_session: &Session, _request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    Response::builder().status(404).body(Vec::new()).expect("a 404 is a valid response")
}
