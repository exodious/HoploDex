//! The `hdpreview` custom protocol: one-time URLs that serve a PDF from
//! memory to the surface (research.md §4, contracts/tauri-commands.md "The
//! `hdpreview` protocol").
//!
//! It answers three requests and nothing else:
//! - `GET /<token>/document.pdf`, the current PDF preview's token: `200` with
//!   the whole document;
//! - `GET /hooked/<surface secret>`: `204`, PDF.js's hook has run (Linux,
//!   research.md §8);
//! - anything else, or a token no longer shown: `404`, empty.
//!
//! The session's lock is held only to copy out the bytes or set a flag. When
//! both the document has been served (and on Linux hooked), `preview:pdf-ready`
//! goes to the main web view, once.

use serde_json::json;
use tauri::http::{Method, Request, Response};

use crate::commands::CommandError;
use crate::services::preview::PreviewContent;
use crate::session::Session;

/// The scheme's name, as registered in `main.rs`.
pub const SCHEME: &str = "hdpreview";

/// The path the hook is requested at, before the surface's secret.
const HOOKED_PREFIX: &str = "/hooked/";

/// A token as it appears in a URL: 32 lowercase hex digits.
pub fn token_hex(token: &[u8; 16]) -> String {
    token.iter().map(|b| format!("{b:02x}")).collect()
}

/// The path of the document served under `token`.
pub fn document_path(token: &[u8; 16]) -> String {
    format!("/{}/document.pdf", token_hex(token))
}

/// The URL the surface is sent to for the document under `token`. On Windows
/// wry serves a custom scheme as `http://<scheme>.localhost`, on the others as
/// `<scheme>://localhost`.
pub fn document_url(token: &[u8; 16]) -> String {
    format!("{}{}", base_url(), document_path(token))
}

#[cfg(windows)]
fn base_url() -> String {
    format!("http://{SCHEME}.localhost")
}

#[cfg(not(windows))]
fn base_url() -> String {
    format!("{SCHEME}://localhost")
}

/// Whether `url` is the address of a document served under `token`, in the
/// form the web view reports it (any host form of this scheme).
pub fn is_document_url(url: &tauri::Url, token: &[u8; 16]) -> bool {
    let host_ok = url.scheme() == SCHEME
        || (url.scheme() == "http" && url.host_str() == Some(&format!("{SCHEME}.localhost")));
    host_ok && url.path() == document_path(token)
}

fn not_found() -> Response<Vec<u8>> {
    Response::builder().status(404).body(Vec::new()).expect("a 404 is a valid response")
}

/// Answers the PDF surface's requests.
pub fn handle(session: &Session, request: &Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() != Method::GET {
        return not_found();
    }
    let path = request.uri().path();
    match path.strip_prefix(HOOKED_PREFIX) {
        Some(secret) => hooked(session, secret),
        None => serve(session, path),
    }
}

/// What a request changed: the preview's id when the preview has just become
/// ready.
type Became = Option<u64>;

fn is_ready(served: bool, hooked: bool) -> bool {
    served && (hooked || !cfg!(target_os = "linux"))
}

fn announce_ready(session: &Session, became: Became) {
    if let Some(preview_id) = became {
        session.events().emit_to_main("preview:pdf-ready", json!({ "previewId": preview_id }));
    }
}

fn serve(session: &Session, path: &str) -> Response<Vec<u8>> {
    let found = session.inspect_mut(|open| -> Result<_, CommandError> {
        let Some(preview) = open.preview.as_mut() else { return Ok(None) };
        let id = preview.id;
        let PreviewContent::Pdf { token, bytes, served, hooked, .. } = &mut preview.content else {
            return Ok(None);
        };
        if path != document_path(token) {
            return Ok(None);
        }
        let was_ready = is_ready(*served, *hooked);
        *served = true;
        let became = (!was_ready && is_ready(*served, *hooked)).then_some(id);
        Ok(Some((bytes.to_vec(), became)))
    });
    // A closed session or a replaced preview is a 404 like any other.
    let Ok(Some((body, became))) = found else { return not_found() };
    announce_ready(session, became);
    Response::builder()
        .status(200)
        .header("Content-Type", "application/pdf")
        .header("Cache-Control", "no-store")
        .body(body)
        .expect("a 200 with these headers is a valid response")
}

fn hooked(session: &Session, secret: &str) -> Response<Vec<u8>> {
    let found = session.inspect_mut(|open| -> Result<_, CommandError> {
        let Some(preview) = open.preview.as_mut() else { return Ok(None) };
        let id = preview.id;
        let PreviewContent::Pdf { secret: surface_secret, served, hooked, .. } =
            &mut preview.content
        else {
            return Ok(None);
        };
        if !secrets_match(surface_secret, secret) {
            return Ok(None);
        }
        let was_ready = is_ready(*served, *hooked);
        *hooked = true;
        Ok(Some((!was_ready && is_ready(*served, *hooked)).then_some(id)))
    });
    let Ok(Some(became)) = found else { return not_found() };
    announce_ready(session, became);
    Response::builder().status(204).body(Vec::new()).expect("a 204 is a valid response")
}

/// Compares without stopping at the first difference.
fn secrets_match(expected: &str, given: &str) -> bool {
    let (expected, given) = (expected.as_bytes(), given.as_bytes());
    if expected.len() != given.len() {
        return false;
    }
    expected.iter().zip(given).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_is_32_hex_digits_and_the_path_ends_in_document_pdf() {
        let token = [0xab; 16];
        assert_eq!(token_hex(&token), "ab".repeat(16));
        assert_eq!(document_path(&token), format!("/{}/document.pdf", "ab".repeat(16)));
    }

    #[test]
    fn a_document_url_is_recognised_in_the_form_the_web_view_reports_it() {
        let token = [7; 16];
        let url: tauri::Url = document_url(&token).parse().unwrap();
        assert!(is_document_url(&url, &token));
        assert!(!is_document_url(&url, &[8; 16]));
        let other: tauri::Url =
            format!("https://example.com{}", document_path(&token)).parse().unwrap();
        assert!(!is_document_url(&other, &token));
    }

    #[test]
    fn secrets_are_compared_whole() {
        assert!(secrets_match("abc", "abc"));
        assert!(!secrets_match("abc", "abd"));
        assert!(!secrets_match("abc", "ab"));
        assert!(!secrets_match("", "a"));
    }
}
