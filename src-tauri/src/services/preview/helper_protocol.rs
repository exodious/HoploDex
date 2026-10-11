//! The TIFF mode's requests and responses between the app and the render
//! helper, shared by both ends (007's research.md §11), in the frames of
//! `services::helper::frames`.
//!
//! The parent sends one [`Request::Load`] and then any number of
//! [`Request::Render`]; the helper answers each with one [`Response`].

use std::io::{self, Read, Write};

use super::PageSize;
use crate::services::helper::frames::{MAX_FRAME, invalid, read_frame, write_frame};

const LOAD: u8 = 1;
const RENDER: u8 = 2;
const LOADED: u8 = 11;
const FAILED: u8 = 12;
const PAGE: u8 = 13;
const PAGE_FAILED: u8 = 14;

/// Parent to helper.
#[derive(Debug, PartialEq)]
pub enum Request {
    /// The document's bytes; the helper answers with the pages' sizes.
    Load(Vec<u8>),
    /// One page (0-based) as a PNG about `width_px` wide.
    Render { page: u32, width_px: u32 },
}

/// Helper to parent.
#[derive(Debug, PartialEq)]
pub enum Response {
    Loaded {
        pages: Vec<PageSize>,
    },
    /// The document couldn't be read at all. `reason` is for a developer and
    /// never shown or logged (it could hold content).
    Failed {
        reason: String,
    },
    Page {
        png: Vec<u8>,
    },
    /// This page couldn't be rendered; the others may.
    PageFailed,
}

/// Sends a `Load` without copying the document into a request first.
pub fn write_load(w: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    write_frame(w, LOAD, bytes, MAX_FRAME)
}

pub fn write_request(w: &mut impl Write, request: &Request) -> io::Result<()> {
    match request {
        Request::Load(bytes) => write_load(w, bytes),
        Request::Render { page, width_px } => {
            let mut payload = [0u8; 8];
            payload[..4].copy_from_slice(&page.to_be_bytes());
            payload[4..].copy_from_slice(&width_px.to_be_bytes());
            write_frame(w, RENDER, &payload, MAX_FRAME)
        }
    }
}

/// The next request, or `None` when the parent has closed the pipe.
pub fn read_request(r: &mut impl Read) -> io::Result<Option<Request>> {
    let Some((tag, payload)) = read_frame(r, MAX_FRAME)? else { return Ok(None) };
    match tag {
        LOAD => Ok(Some(Request::Load(payload))),
        RENDER => {
            let payload: [u8; 8] = payload.try_into().map_err(|_| invalid("bad render frame"))?;
            Ok(Some(Request::Render {
                page: u32::from_be_bytes(payload[..4].try_into().expect("4 bytes")),
                width_px: u32::from_be_bytes(payload[4..].try_into().expect("4 bytes")),
            }))
        }
        _ => Err(invalid("unknown request")),
    }
}

pub fn write_response(w: &mut impl Write, response: &Response) -> io::Result<()> {
    match response {
        Response::Loaded { pages } => {
            let mut payload = Vec::with_capacity(4 + pages.len() * 16);
            payload.extend_from_slice(&(pages.len() as u32).to_be_bytes());
            for page in pages {
                payload.extend_from_slice(&page.width.to_be_bytes());
                payload.extend_from_slice(&page.height.to_be_bytes());
            }
            write_frame(w, LOADED, &payload, MAX_FRAME)
        }
        Response::Failed { reason } => write_frame(w, FAILED, reason.as_bytes(), MAX_FRAME),
        Response::Page { png } => write_frame(w, PAGE, png, MAX_FRAME),
        Response::PageFailed => write_frame(w, PAGE_FAILED, &[], MAX_FRAME),
    }
}

/// The next response, or `None` when the helper has closed the pipe.
pub fn read_response(r: &mut impl Read) -> io::Result<Option<Response>> {
    let Some((tag, payload)) = read_frame(r, MAX_FRAME)? else { return Ok(None) };
    match tag {
        LOADED => {
            let count = payload
                .get(..4)
                .map(|b| u32::from_be_bytes(b.try_into().expect("4 bytes")) as usize)
                .ok_or_else(|| invalid("bad loaded frame"))?;
            if payload.len() != 4 + count.checked_mul(16).ok_or_else(|| invalid("bad count"))? {
                return Err(invalid("bad loaded frame"));
            }
            let pages = payload[4..]
                .as_chunks::<16>()
                .0
                .iter()
                .map(|c| PageSize {
                    width: f64::from_be_bytes(c[..8].try_into().expect("8 bytes")),
                    height: f64::from_be_bytes(c[8..].try_into().expect("8 bytes")),
                })
                .collect();
            Ok(Some(Response::Loaded { pages }))
        }
        FAILED => Ok(Some(Response::Failed { reason: String::from_utf8_lossy(&payload).into() })),
        PAGE => Ok(Some(Response::Page { png: payload })),
        PAGE_FAILED if payload.is_empty() => Ok(Some(Response::PageFailed)),
        _ => Err(invalid("unknown response")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip_request(request: Request) {
        let mut wire = Vec::new();
        write_request(&mut wire, &request).unwrap();
        let mut reader = &wire[..];
        assert_eq!(read_request(&mut reader).unwrap(), Some(request));
        assert_eq!(read_request(&mut reader).unwrap(), None, "a clean end");
    }

    fn round_trip_response(response: Response) {
        let mut wire = Vec::new();
        write_response(&mut wire, &response).unwrap();
        let mut reader = &wire[..];
        assert_eq!(read_response(&mut reader).unwrap(), Some(response));
        assert_eq!(read_response(&mut reader).unwrap(), None);
    }

    #[test]
    fn every_frame_round_trips() {
        round_trip_request(Request::Load(vec![1, 2, 3, 0, 255]));
        round_trip_request(Request::Load(Vec::new()));
        round_trip_request(Request::Render { page: 7, width_px: 1024 });
        round_trip_response(Response::Loaded {
            pages: vec![
                PageSize { width: 612.0, height: 792.0 },
                PageSize { width: 17.28, height: 0.5 },
            ],
        });
        round_trip_response(Response::Loaded { pages: Vec::new() });
        round_trip_response(Response::Failed { reason: "no first IFD".into() });
        round_trip_response(Response::Page { png: vec![0x89, b'P', b'N', b'G'] });
        round_trip_response(Response::PageFailed);
    }

    #[test]
    fn frames_follow_one_another() {
        let mut wire = Vec::new();
        write_request(&mut wire, &Request::Load(vec![9; 10])).unwrap();
        write_request(&mut wire, &Request::Render { page: 1, width_px: 2 }).unwrap();
        let mut reader = &wire[..];
        assert!(matches!(read_request(&mut reader).unwrap(), Some(Request::Load(_))));
        assert_eq!(
            read_request(&mut reader).unwrap(),
            Some(Request::Render { page: 1, width_px: 2 })
        );
    }

    #[test]
    fn unknown_tags_and_malformed_payloads_are_refused() {
        let mut wire = 2u32.to_be_bytes().to_vec();
        wire.extend_from_slice(&[99, 0]);
        assert!(read_request(&mut &wire[..]).is_err());
        assert!(read_response(&mut &wire[..]).is_err());

        // A render frame of the wrong length.
        let mut wire = 4u32.to_be_bytes().to_vec();
        wire.extend_from_slice(&[RENDER, 0, 0, 0]);
        assert!(read_request(&mut &wire[..]).is_err());

        // A loaded frame whose count doesn't match its length.
        let mut wire = 5u32.to_be_bytes().to_vec();
        wire.push(LOADED);
        wire.extend_from_slice(&1000u32.to_be_bytes());
        assert!(read_response(&mut &wire[..]).is_err());
    }
}
