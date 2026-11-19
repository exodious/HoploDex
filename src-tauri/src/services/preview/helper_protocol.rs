//! The TIFF mode's requests and responses between the app and the render
//! helper, shared by both ends (007 research.md §11), in the frames of
//! `services::helper::frames`.
//!
//! The parent sends one [`Request::Load`] and then any number of
//! [`Request::Render`]; the helper answers each with one [`Response`].

use std::io::{self, Read, Write};

use super::PageSize;
use super::tiff::{MAX_PAGES, MAX_PIXELS};
use crate::services::helper::frames::{
    MAX_FRAME, invalid, read_frame, read_frame_by_tag, write_frame,
};

const LOAD: u8 = 1;
const RENDER: u8 = 2;
const LOADED: u8 = 11;
const FAILED: u8 = 12;
const PAGE: u8 = 13;
const PAGE_FAILED: u8 = 14;

/// A `Page` frame's width, height and channels, before its samples.
const PAGE_HEADER: usize = 9;

/// The longest `Failed` reason a helper sends.
const MAX_REASON: usize = 4096;

/// The largest response of each kind (its tag and payload), which the app
/// checks before it reads one: a page holds at most [`MAX_PIXELS`] pixels of
/// three samples each, a page list only what [`MAX_PAGES`] pages need.
fn max_response(tag: u8) -> usize {
    match tag {
        LOADED => 1 + 4 + MAX_PAGES * 16,
        FAILED => 1 + MAX_REASON,
        PAGE => 1 + PAGE_HEADER + MAX_PIXELS as usize * 3,
        PAGE_FAILED => 1,
        _ => 0,
    }
}

/// Parent to helper.
#[derive(Debug, PartialEq)]
pub enum Request {
    /// The document's bytes; the helper answers with the pages' sizes.
    Load(Vec<u8>),
    /// One page (0-based) as pixels about `width_px` wide.
    Render { page: u32, width_px: u32 },
}

/// A page's pixels, row after row of 8-bit samples: never a file format, so
/// what reaches the web view is what the app itself encoded
/// (`helper_handle`, which trusts none of these fields).
#[derive(Debug, PartialEq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    /// 1 for gray, 3 for RGB.
    pub channels: u8,
    pub samples: Vec<u8>,
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
    Page(Pixels),
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
            write_frame(w, LOADED, &payload, max_response(LOADED))
        }
        Response::Failed { reason } => {
            write_frame(w, FAILED, reason.as_bytes(), max_response(FAILED))
        }
        Response::Page(pixels) => {
            let mut payload = Vec::with_capacity(PAGE_HEADER + pixels.samples.len());
            payload.extend_from_slice(&pixels.width.to_be_bytes());
            payload.extend_from_slice(&pixels.height.to_be_bytes());
            payload.push(pixels.channels);
            payload.extend_from_slice(&pixels.samples);
            write_frame(w, PAGE, &payload, max_response(PAGE))
        }
        Response::PageFailed => write_frame(w, PAGE_FAILED, &[], max_response(PAGE_FAILED)),
    }
}

/// The next response, or `None` when the helper has closed the pipe.
pub fn read_response(r: &mut impl Read) -> io::Result<Option<Response>> {
    let Some((tag, mut payload)) = read_frame_by_tag(r, MAX_FRAME, max_response)? else {
        return Ok(None);
    };
    match tag {
        LOADED => {
            let count = payload
                .get(..4)
                .map(|b| u32::from_be_bytes(b.try_into().expect("4 bytes")) as usize)
                .ok_or_else(|| invalid("bad loaded frame"))?;
            if count > MAX_PAGES || payload.len() != 4 + count * 16 {
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
        PAGE => {
            if payload.len() < PAGE_HEADER {
                return Err(invalid("bad page frame"));
            }
            let width = u32::from_be_bytes(payload[..4].try_into().expect("4 bytes"));
            let height = u32::from_be_bytes(payload[4..8].try_into().expect("4 bytes"));
            let channels = payload[8];
            payload.drain(..PAGE_HEADER);
            Ok(Some(Response::Page(Pixels { width, height, channels, samples: payload })))
        }
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
        round_trip_response(Response::Page(Pixels {
            width: 2,
            height: 1,
            channels: 3,
            samples: vec![1, 2, 3, 4, 5, 6],
        }));
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

        // A well-formed loaded frame with one page more than the helper's
        // limit is refused (and can't be sent); one at the limit is not.
        let list = |count: usize| {
            let mut wire = ((1 + 4 + count * 16) as u32).to_be_bytes().to_vec();
            wire.push(LOADED);
            wire.extend_from_slice(&(count as u32).to_be_bytes());
            wire.extend(1.0f64.to_be_bytes().repeat(2 * count));
            wire
        };
        let pages = vec![PageSize { width: 1.0, height: 1.0 }; MAX_PAGES + 1];
        assert!(write_response(&mut Vec::new(), &Response::Loaded { pages }).is_err());
        assert!(read_response(&mut &list(MAX_PAGES + 1)[..]).is_err());
        assert!(matches!(
            read_response(&mut &list(MAX_PAGES)[..]).unwrap(),
            Some(Response::Loaded { pages }) if pages.len() == MAX_PAGES
        ));

        // A frame longer than its kind's largest is refused from its length
        // and tag alone, before its payload arrives.
        let page_limit = 1 + 9 + 3 * 24 * 1024 * 1024;
        for (tag, max) in [
            (LOADED, 1 + 4 + MAX_PAGES * 16),
            (FAILED, 1 + MAX_REASON),
            (PAGE, page_limit),
            (PAGE_FAILED, 1),
        ] {
            let mut wire = (max as u32 + 1).to_be_bytes().to_vec();
            wire.push(tag);
            let err = read_response(&mut &wire[..]).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::InvalidData, "tag {tag}");
        }
        let mut wire = (MAX_FRAME as u32).to_be_bytes().to_vec();
        wire.push(99);
        assert_eq!(read_response(&mut &wire[..]).unwrap_err().kind(), io::ErrorKind::InvalidData);
        // The largest page passes that check: this frame is only cut short.
        let mut wire = (page_limit as u32).to_be_bytes().to_vec();
        wire.push(PAGE);
        assert_eq!(read_response(&mut &wire[..]).unwrap_err().kind(), io::ErrorKind::UnexpectedEof);

        // A page frame too short to say its size.
        let mut wire = 9u32.to_be_bytes().to_vec();
        wire.extend_from_slice(&[PAGE, 0, 0, 0, 1, 0, 0, 0, 1]);
        assert!(read_response(&mut &wire[..]).is_err());
    }
}
