//! The length-prefixed frames between the app and a confined helper, shared
//! by both ends and by both modes (007 research.md §11; 008 research.md §8).
//!
//! A frame is a big-endian `u32` length, then that many bytes: a one-byte tag
//! and the payload. What the tags and payloads mean is the mode's
//! (`preview::helper_protocol` for TIFF, `import_reader::frames` for the
//! import). Each mode gives the largest frame it accepts as `max`
//! ([`MAX_FRAME`] for TIFF); a reader refuses a longer frame before it
//! allocates for it, and reads a large frame in pieces, so a hostile length
//! costs nothing until the bytes arrive.

use std::io::{self, Read, Write};

/// The largest frame the TIFF mode accepts: 768 MiB, above the largest document
/// the parent loads. The app holds each response to a smaller limit for its kind
/// (`preview::helper_protocol`'s `max_response`, through [`read_frame_by_tag`]).
pub const MAX_FRAME: usize = 768 * 1024 * 1024;

pub fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

/// Writes one frame, refusing one whose length (the tag and the payload)
/// is over `max`.
pub fn write_frame(w: &mut impl Write, tag: u8, payload: &[u8], max: usize) -> io::Result<()> {
    let length = payload.len() + 1;
    if length > max {
        return Err(invalid("frame too large"));
    }
    w.write_all(&(length as u32).to_be_bytes())?;
    w.write_all(&[tag])?;
    w.write_all(payload)?;
    w.flush()
}

/// Reads one frame's tag and payload, refusing a length over `max` before
/// reading any of it. `None` at a clean end of input (nothing at all before
/// the end).
pub fn read_frame(r: &mut impl Read, max: usize) -> io::Result<Option<(u8, Vec<u8>)>> {
    read_frame_by_tag(r, max, |_| max)
}

/// [`read_frame`] for a mode whose frames differ in size: once the tag is read,
/// a length over `max_for(tag)` is refused before the payload is read.
pub fn read_frame_by_tag(
    r: &mut impl Read,
    max: usize,
    max_for: impl Fn(u8) -> usize,
) -> io::Result<Option<(u8, Vec<u8>)>> {
    let mut prefix = [0u8; 4];
    let mut filled = 0;
    while filled < prefix.len() {
        match r.read(&mut prefix[filled..]) {
            Ok(0) if filled == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => filled += n,
            Err(err) if err.kind() == io::ErrorKind::Interrupted => {}
            Err(err) => return Err(err),
        }
    }
    let length = u32::from_be_bytes(prefix) as usize;
    if length == 0 || length > max {
        return Err(invalid("frame length out of range"));
    }
    let mut tag = [0u8];
    r.read_exact(&mut tag)?;
    if length > max_for(tag[0]) {
        return Err(invalid("frame length out of range"));
    }
    let mut payload = Vec::new();
    let read = r.take(length as u64 - 1).read_to_end(&mut payload)?;
    if read != length - 1 {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    Ok(Some((tag[0], payload)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_follow_one_another() {
        let mut wire = Vec::new();
        write_frame(&mut wire, 1, &[1, 2, 3, 0, 255], MAX_FRAME).unwrap();
        write_frame(&mut wire, 2, &[], MAX_FRAME).unwrap();
        let mut reader = &wire[..];
        assert_eq!(read_frame(&mut reader, MAX_FRAME).unwrap(), Some((1, vec![1, 2, 3, 0, 255])));
        assert_eq!(read_frame(&mut reader, MAX_FRAME).unwrap(), Some((2, Vec::new())));
        assert_eq!(read_frame(&mut reader, MAX_FRAME).unwrap(), None, "a clean end");
    }

    #[test]
    fn a_truncated_frame_is_an_error_and_not_a_clean_end() {
        let mut wire = Vec::new();
        write_frame(&mut wire, 1, &[1; 100], MAX_FRAME).unwrap();
        for cut in [1, 3, 4, 5, 50, wire.len() - 1] {
            let mut reader = &wire[..cut];
            let err = read_frame(&mut reader, MAX_FRAME).unwrap_err();
            assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof, "cut at {cut}");
        }
    }

    #[test]
    fn an_oversized_or_empty_frame_is_refused_before_any_allocation() {
        let mut huge = (MAX_FRAME as u32 + 1).to_be_bytes().to_vec();
        huge.push(1);
        let err = read_frame(&mut &huge[..], MAX_FRAME).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        let err = read_frame(&mut &u32::MAX.to_be_bytes()[..], MAX_FRAME).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);

        let err = read_frame(&mut &0u32.to_be_bytes()[..], MAX_FRAME).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn each_mode_gives_its_own_largest_frame() {
        let mut wire = Vec::new();
        write_frame(&mut wire, 1, &[0; 99], 100).unwrap();
        assert_eq!(read_frame(&mut &wire[..], 100).unwrap().unwrap().1.len(), 99);
        assert!(write_frame(&mut Vec::new(), 1, &[0; 100], 100).is_err());
        let err = read_frame(&mut &wire[..], 99).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData, "refused before its bytes are read");
    }

    #[test]
    fn a_frame_longer_than_its_tag_allows_is_refused_before_its_payload_is_read() {
        // A reader that counts what is taken from it, with no end.
        struct Endless(usize);
        impl Read for Endless {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                buf.fill(7);
                self.0 += buf.len();
                Ok(buf.len())
            }
        }
        let max_for = |tag| if tag == 1 { 100 } else { MAX_FRAME };
        let mut wire = Endless(0);
        let mut framed = 101u32.to_be_bytes().to_vec();
        framed.push(1);
        let mut reader = (&framed[..]).chain(&mut wire);
        let err = read_frame_by_tag(&mut reader, MAX_FRAME, max_for).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        assert_eq!(wire.0, 0, "nothing past the tag was read");

        let mut framed = 100u32.to_be_bytes().to_vec();
        framed.push(1);
        framed.extend_from_slice(&[0; 99]);
        assert_eq!(
            read_frame_by_tag(&mut &framed[..], MAX_FRAME, max_for).unwrap().unwrap().1.len(),
            99
        );
        framed[4] = 2;
        assert!(
            read_frame_by_tag(&mut &framed[..], MAX_FRAME, max_for).is_ok(),
            "another tag's limit"
        );
    }

    #[test]
    fn a_frame_too_large_to_send_is_refused() {
        // Only the length is checked, so no 768 MiB buffer is needed to see
        // that the writer enforces the limit: one byte over the limit.
        struct Sink;
        impl Write for Sink {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let payload = vec![0u8; MAX_FRAME];
        assert!(write_frame(&mut Sink, 1, &payload, MAX_FRAME).is_err());
        assert!(write_frame(&mut Sink, 1, &payload[..MAX_FRAME - 1], MAX_FRAME).is_ok());
    }
}
