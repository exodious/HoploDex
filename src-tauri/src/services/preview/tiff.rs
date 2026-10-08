//! TIFF page counting and rendering, run inside the render helper
//! (research.md §11).
//!
//! [`load`] walks the file's directories itself, reading only the few tags a
//! page's size needs, so the page list is cheap and a hostile chain (a
//! directory that points back at itself, a page claiming 100,000 pixels
//! square) can't make it decode anything. [`render_page`] decodes one page
//! with the `tiff` crate (which also reads CCITT Group 3 and 4 through its
//! `fax` support, and LZW, Deflate and JPEG), brings it down to the width
//! asked and encodes a PNG. A page it can't decode fails alone.

use std::borrow::Cow;
use std::collections::HashSet;
use std::io::Cursor;

use tiff::ColorType;
use tiff::decoder::{Decoder, DecodingResult, Limits};

use super::PageSize;

/// The DPI of a page that doesn't say (research.md §11).
const DEFAULT_DPI: f64 = 200.0;

/// The most directories one file may have. A scan of more pages than this is
/// reported as damaged rather than shown partly.
const MAX_PAGES: usize = 10_000;

/// The widest page the helper makes, and the most pixels in it
/// (research.md §14).
pub const MAX_WIDTH_PX: u32 = 4096;
pub const MAX_PIXELS: u64 = 24 * 1024 * 1024;

/// The most decoded bytes of one page. The decoder refuses a page that would
/// need more, before it allocates.
const MAX_DECODED_BYTES: usize = 640 * 1024 * 1024;

const TAG_WIDTH: u16 = 256;
const TAG_HEIGHT: u16 = 257;
const TAG_X_RESOLUTION: u16 = 282;
const TAG_Y_RESOLUTION: u16 = 283;
const TAG_RESOLUTION_UNIT: u16 = 296;

/// How the file lays out its directories.
#[derive(Clone, Copy)]
struct Layout {
    little: bool,
    big: bool,
}

#[derive(Clone, Copy)]
struct Reader<'a> {
    bytes: &'a [u8],
    layout: Layout,
}

impl Reader<'_> {
    fn get<const N: usize>(&self, at: u64) -> Option<[u8; N]> {
        let at = usize::try_from(at).ok()?;
        self.bytes.get(at..at.checked_add(N)?)?.try_into().ok()
    }

    fn u16(&self, at: u64) -> Option<u16> {
        let b = self.get::<2>(at)?;
        Some(if self.layout.little { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) })
    }

    fn u32(&self, at: u64) -> Option<u32> {
        let b = self.get::<4>(at)?;
        Some(if self.layout.little { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) })
    }

    fn u64(&self, at: u64) -> Option<u64> {
        let b = self.get::<8>(at)?;
        Some(if self.layout.little { u64::from_le_bytes(b) } else { u64::from_be_bytes(b) })
    }

    /// An offset: 4 bytes in a classic TIFF, 8 in a BigTIFF.
    fn offset(&self, at: u64) -> Option<u64> {
        if self.layout.big { self.u64(at) } else { self.u32(at).map(u64::from) }
    }
}

/// One directory's tags that matter, and where the next directory is.
struct Directory {
    width: Option<u64>,
    height: Option<u64>,
    x_resolution: Option<f64>,
    y_resolution: Option<f64>,
    resolution_unit: u16,
    next: u64,
}

fn read_header(bytes: &[u8]) -> Option<(Reader<'_>, u64)> {
    let little = match bytes.get(..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    let mut reader = Reader { bytes, layout: Layout { little, big: false } };
    match reader.u16(2)? {
        42 => Some((reader, u64::from(reader.u32(4)?))),
        43 => {
            // BigTIFF: the offset size (8) and a zero, then the first offset.
            if reader.u16(4)? != 8 || reader.u16(6)? != 0 {
                return None;
            }
            reader.layout.big = true;
            Some((reader, reader.u64(8)?))
        }
        _ => None,
    }
}

/// A tag's one number, if it has exactly one and is a BYTE, SHORT, LONG or
/// LONG8, stored in the entry itself.
fn inline_number(reader: &Reader<'_>, entry: u64, count: u64) -> Option<u64> {
    let value_at = entry + if reader.layout.big { 12 } else { 8 };
    if count != 1 {
        return None;
    }
    match reader.u16(entry + 2)? {
        1 => reader.get::<1>(value_at).map(|b| u64::from(b[0])),
        3 => reader.u16(value_at).map(u64::from),
        4 => reader.u32(value_at).map(u64::from),
        16 => reader.u64(value_at),
        _ => None,
    }
}

/// A RATIONAL tag's value, a pair of LONGs stored where the entry points.
fn rational(reader: &Reader<'_>, entry: u64, count: u64) -> Option<f64> {
    let value_at = entry + if reader.layout.big { 12 } else { 8 };
    if count != 1 || reader.u16(entry + 2)? != 5 {
        return None;
    }
    let at = reader.offset(value_at)?;
    let (numerator, denominator) = (reader.u32(at)?, reader.u32(at + 4)?);
    (denominator != 0).then(|| f64::from(numerator) / f64::from(denominator))
}

fn read_directory(reader: &Reader<'_>, at: u64) -> Option<Directory> {
    let big = reader.layout.big;
    let (count, entries_at, entry_size) =
        if big { (reader.u64(at)?, at + 8, 20) } else { (u64::from(reader.u16(at)?), at + 2, 12) };
    let next_at = entries_at.checked_add(count.checked_mul(entry_size)?)?;
    let next = reader.offset(next_at)?;
    let mut directory = Directory {
        width: None,
        height: None,
        x_resolution: None,
        y_resolution: None,
        resolution_unit: 2,
        next,
    };
    for i in 0..count {
        let entry = entries_at + i * entry_size;
        let tag = reader.u16(entry)?;
        let entry_count =
            if big { reader.u64(entry + 4)? } else { u64::from(reader.u32(entry + 4)?) };
        match tag {
            TAG_WIDTH => directory.width = inline_number(reader, entry, entry_count),
            TAG_HEIGHT => directory.height = inline_number(reader, entry, entry_count),
            TAG_X_RESOLUTION => directory.x_resolution = rational(reader, entry, entry_count),
            TAG_Y_RESOLUTION => directory.y_resolution = rational(reader, entry, entry_count),
            TAG_RESOLUTION_UNIT => {
                if let Some(unit) = inline_number(reader, entry, entry_count) {
                    directory.resolution_unit = unit as u16;
                }
            }
            _ => {}
        }
    }
    Some(directory)
}

/// A page's size in points: its pixels over its DPI, 200 when it has none
/// (or its unit is "none").
fn page_size(directory: &Directory) -> Option<PageSize> {
    let per_inch = |resolution: Option<f64>| match (resolution, directory.resolution_unit) {
        (Some(r), 2) if r > 0.0 => r,
        (Some(r), 3) if r > 0.0 => r * 2.54,
        _ => DEFAULT_DPI,
    };
    let (width, height) = (directory.width?, directory.height?);
    if width == 0 || height == 0 {
        return None;
    }
    Some(PageSize {
        width: width as f64 * 72.0 / per_inch(directory.x_resolution),
        height: height as f64 * 72.0 / per_inch(directory.y_resolution),
    })
}

/// The size of every page, from a walk of the file's directories. A first
/// directory that can't be read is an error (the file is damaged); a later
/// one that can't ends the list there, and a directory met twice (a chain
/// that loops) ends it too.
pub fn load(bytes: &[u8]) -> Result<Vec<PageSize>, String> {
    let (reader, first) = read_header(bytes).ok_or("not a TIFF header")?;
    let mut pages = Vec::new();
    let mut seen = HashSet::new();
    let mut at = first;
    while at != 0 && seen.insert(at) {
        let size = read_directory(&reader, at).map(|d| (page_size(&d), d.next));
        let Some((Some(size), next)) = size else {
            if pages.is_empty() {
                return Err("the first directory can't be read".into());
            }
            break;
        };
        if pages.len() >= MAX_PAGES {
            return Err("too many pages".into());
        }
        pages.push(size);
        at = next;
    }
    if pages.is_empty() {
        return Err("no pages".into());
    }
    Ok(pages)
}

/// The size a page is rendered at: `requested` wide at most, never wider than
/// 4096 px or the page itself, and no more than 24 megapixels, in the page's
/// own aspect ratio.
pub fn output_size(source: (u32, u32), requested: u32) -> (u32, u32) {
    let (source_width, source_height) = (u64::from(source.0.max(1)), u64::from(source.1.max(1)));
    let mut width = u64::from(requested.clamp(1, MAX_WIDTH_PX)).min(source_width);
    let height_for = |width: u64| (source_height * width).div_ceil(source_width).max(1);
    while width > 1 && width * height_for(width) > MAX_PIXELS {
        let scaled =
            (width as f64 * (MAX_PIXELS as f64 / (width * height_for(width)) as f64).sqrt()) as u64;
        width = scaled.min(width - 1).max(1);
    }
    (width as u32, height_for(width) as u32)
}

/// What the decoder gave, brought to 8-bit samples: gray (1 channel) or RGB
/// (3), alpha flattened onto white.
struct Raster<'a> {
    samples: Cow<'a, [u8]>,
    channels: usize,
}

fn to_raster(
    data: &DecodingResult,
    color: ColorType,
    width: usize,
    height: usize,
) -> Result<Raster<'_>, String> {
    let unsupported = || "an unsupported kind of page".to_owned();
    // Every sample as a byte, high byte of a 16-bit one.
    enum Samples<'a> {
        Bytes(&'a [u8]),
        Words(&'a [u16]),
    }
    let samples = match data {
        DecodingResult::U8(v) => Samples::Bytes(v),
        DecodingResult::U16(v) => Samples::Words(v),
        _ => return Err(unsupported()),
    };
    let at = |i: usize| -> u8 {
        match &samples {
            Samples::Bytes(v) => v[i],
            Samples::Words(v) => (v[i] >> 8) as u8,
        }
    };
    let per_pixel = match color {
        ColorType::Gray(_) => 1,
        ColorType::GrayA(_) => 2,
        ColorType::RGB(_) | ColorType::YCbCr(_) => 3,
        ColorType::RGBA(_) => 4,
        _ => return Err(unsupported()),
    };
    let count = width.checked_mul(height).ok_or_else(unsupported)?;
    match (color, &samples) {
        // 1, 2 and 4 bits: packed, each row padded to a byte.
        (ColorType::Gray(b @ (1 | 2 | 4)), Samples::Bytes(v)) => {
            let b = usize::from(b);
            let stride = (width * b).div_ceil(8);
            if v.len() < stride * height {
                return Err("short data".into());
            }
            let max = (1u32 << b) - 1;
            let mut out = Vec::with_capacity(count);
            for row in v.chunks_exact(stride).take(height) {
                for x in 0..width {
                    let bit = x * b;
                    let value = u32::from(row[bit / 8] >> (8 - b - bit % 8)) & max;
                    out.push((value * 255 / max) as u8);
                }
            }
            Ok(Raster { samples: Cow::Owned(out), channels: 1 })
        }
        (ColorType::Gray(8) | ColorType::RGB(8), Samples::Bytes(v)) => {
            if v.len() < count * per_pixel {
                return Err("short data".into());
            }
            Ok(Raster { samples: Cow::Borrowed(&v[..count * per_pixel]), channels: per_pixel })
        }
        (ColorType::Gray(8 | 16) | ColorType::RGB(8 | 16), _) => {
            let available = match &samples {
                Samples::Bytes(v) => v.len(),
                Samples::Words(v) => v.len(),
            };
            if available < count * per_pixel {
                return Err("short data".into());
            }
            Ok(Raster {
                samples: Cow::Owned((0..count * per_pixel).map(at).collect()),
                channels: per_pixel,
            })
        }
        (ColorType::GrayA(8 | 16) | ColorType::RGBA(8 | 16), _) => {
            let available = match &samples {
                Samples::Bytes(v) => v.len(),
                Samples::Words(v) => v.len(),
            };
            if available < count * per_pixel {
                return Err("short data".into());
            }
            let channels = per_pixel - 1;
            let mut out = Vec::with_capacity(count * channels);
            for pixel in 0..count {
                let alpha = u32::from(at(pixel * per_pixel + channels));
                for c in 0..channels {
                    let value = u32::from(at(pixel * per_pixel + c));
                    out.push(((value * alpha + 255 * (255 - alpha)) / 255) as u8);
                }
            }
            Ok(Raster { samples: Cow::Owned(out), channels })
        }
        (ColorType::YCbCr(8), Samples::Bytes(v)) => {
            if v.len() < count * 3 {
                return Err("short data".into());
            }
            let mut out = Vec::with_capacity(count * 3);
            for p in v[..count * 3].as_chunks::<3>().0 {
                let (y, cb, cr) =
                    (f32::from(p[0]), f32::from(p[1]) - 128.0, f32::from(p[2]) - 128.0);
                let clamp = |v: f32| v.round().clamp(0.0, 255.0) as u8;
                out.push(clamp(y + 1.402 * cr));
                out.push(clamp(y - 0.344_136 * cb - 0.714_136 * cr));
                out.push(clamp(y + 1.772 * cb));
            }
            Ok(Raster { samples: Cow::Owned(out), channels: 3 })
        }
        _ => Err(unsupported()),
    }
}

/// Averages the source pixels each output pixel covers.
fn downscale(raster: &Raster<'_>, source: (usize, usize), out: (usize, usize)) -> Vec<u8> {
    let (source_width, source_height) = source;
    let (out_width, out_height) = out;
    let channels = raster.channels;
    if source == out {
        return raster.samples.to_vec();
    }
    let mut result = Vec::with_capacity(out_width * out_height * channels);
    let mut column_sums = vec![0u64; source_width * channels];
    for oy in 0..out_height {
        let y0 = oy * source_height / out_height;
        let y1 = ((oy + 1) * source_height / out_height).max(y0 + 1).min(source_height);
        column_sums.iter_mut().for_each(|s| *s = 0);
        for y in y0..y1 {
            let row =
                &raster.samples[y * source_width * channels..(y + 1) * source_width * channels];
            for (sum, &sample) in column_sums.iter_mut().zip(row) {
                *sum += u64::from(sample);
            }
        }
        let rows = (y1 - y0) as u64;
        for ox in 0..out_width {
            let x0 = ox * source_width / out_width;
            let x1 = ((ox + 1) * source_width / out_width).max(x0 + 1).min(source_width);
            let columns = (x1 - x0) as u64;
            for c in 0..channels {
                let mut total = 0u64;
                for x in x0..x1 {
                    total += column_sums[x * channels + c];
                }
                result.push((total / (rows * columns)) as u8);
            }
        }
    }
    result
}

fn encode_png(samples: &[u8], channels: usize, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let mut png_bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut png_bytes, width, height);
    encoder.set_color(if channels == 1 { png::ColorType::Grayscale } else { png::ColorType::Rgb });
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(samples).map_err(|e| e.to_string())?;
    writer.finish().map_err(|e| e.to_string())?;
    Ok(png_bytes)
}

/// Renders page `page` (0-based) as a PNG, at most `width_px` wide
/// ([`output_size`]). `Err` is this page failing: an unsupported compression,
/// photometric or layout, damaged data, or a page too large to decode.
pub fn render_page(bytes: &[u8], page: u32, width_px: u32) -> Result<Vec<u8>, String> {
    let mut limits = Limits::default();
    limits.decoding_buffer_size = MAX_DECODED_BYTES;
    limits.intermediate_buffer_size = MAX_DECODED_BYTES / 4;
    let mut decoder =
        Decoder::new(Cursor::new(bytes)).map_err(|e| e.to_string())?.with_limits(limits);
    decoder.seek_to_image(page as usize).map_err(|e| e.to_string())?;
    let (width, height) = decoder.dimensions().map_err(|e| e.to_string())?;
    let color = decoder.colortype().map_err(|e| e.to_string())?;
    if width == 0 || height == 0 {
        return Err("an empty page".into());
    }
    let data = decoder.read_image().map_err(|e| e.to_string())?;
    let raster = to_raster(&data, color, width as usize, height as usize)?;
    let (out_width, out_height) = output_size((width, height), width_px);
    let scaled = downscale(
        &raster,
        (width as usize, height as usize),
        (out_width as usize, out_height as usize),
    );
    encode_png(&scaled, raster.channels, out_width, out_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_PAGE: &[u8] = include_bytes!("../../../tests/fixtures/documents/one-page.tif");
    const MIXED: &[u8] = include_bytes!("../../../tests/fixtures/documents/four-pages-mixed.tif");

    fn png_dimensions(png: &[u8]) -> (u32, u32) {
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        (
            u32::from_be_bytes(png[16..20].try_into().unwrap()),
            u32::from_be_bytes(png[20..24].try_into().unwrap()),
        )
    }

    fn decode(png: &[u8]) -> (png::OutputInfo, Vec<u8>) {
        let mut reader = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).unwrap();
        buffer.truncate(info.buffer_size());
        (info, buffer)
    }

    #[test]
    fn the_walk_counts_pages_and_sizes_them_at_200_dpi_when_untagged() {
        let pages = load(MIXED).unwrap();
        assert_eq!(pages.len(), 4);
        let first = load(ONE_PAGE).unwrap();
        assert_eq!(first.len(), 1);
        assert!(first[0].width > 0.0 && first[0].height > first[0].width);
    }

    #[test]
    fn a_first_directory_that_cannot_be_read_is_an_error() {
        assert!(load(b"II*\0\x08\0\0\0").is_err());
        assert!(load(b"not a tiff").is_err());
        assert!(load(b"").is_err());
        let mut past = ONE_PAGE.to_vec();
        past[4..8].copy_from_slice(&1_000_000u32.to_le_bytes());
        assert!(load(&past).is_err());
    }

    #[test]
    fn a_chain_that_loops_ends_at_the_first_repeat() {
        let mut looped = ONE_PAGE.to_vec();
        let first = u32::from_le_bytes(looped[4..8].try_into().unwrap()) as usize;
        let count = u16::from_le_bytes([looped[first], looped[first + 1]]) as usize;
        let next = first + 2 + 12 * count;
        looped[next..next + 4].copy_from_slice(&(first as u32).to_le_bytes());
        assert_eq!(load(&looped).unwrap().len(), 1);
    }

    #[test]
    fn every_compression_in_the_mixed_fixture_renders() {
        for page in 0..4 {
            let png = render_page(MIXED, page, 100).unwrap_or_else(|e| panic!("page {page}: {e}"));
            let (width, height) = png_dimensions(&png);
            assert_eq!(width, 100, "page {page}");
            assert!(height > 0);
        }
    }

    #[test]
    fn a_page_past_the_end_fails() {
        assert!(render_page(ONE_PAGE, 1, 100).is_err());
    }

    #[test]
    fn output_sizes_follow_the_limits() {
        assert_eq!(output_size((200, 100), 100), (100, 50));
        assert_eq!(output_size((200, 100), 10_000), (200, 100), "never wider than the page");
        assert_eq!(output_size((8192, 100), 10_000), (4096, 50), "never wider than 4096");
        assert_eq!(output_size((200, 100), 0), (1, 1), "a width under 1 is 1");
        let (w, h) = output_size((8192, 40_000), 4096);
        assert!(u64::from(w) * u64::from(h) <= MAX_PIXELS, "{w} x {h}");
        assert!(u64::from(w) * u64::from(h) > MAX_PIXELS * 9 / 10, "{w} x {h}");
    }

    #[test]
    fn downscaling_averages_each_box() {
        let raster = Raster { samples: Cow::Owned(vec![0, 100, 200, 100]), channels: 1 };
        assert_eq!(downscale(&raster, (2, 2), (1, 1)), vec![100]);
        assert_eq!(downscale(&raster, (2, 2), (2, 1)), vec![100, 100]);
    }

    #[test]
    fn a_page_that_claims_a_huge_size_fails_instead_of_allocating() {
        // 100,000 x 100,000 pixels of 8-bit gray: a 10 GB decode, with one
        // byte of data behind it.
        let mut b = b"II*\0\x08\0\0\0".to_vec();
        let entries: [(u16, u16, u32, u32); 9] = [
            (256, 4, 1, 100_000),
            (257, 4, 1, 100_000),
            (258, 3, 1, 8),
            (259, 3, 1, 1),
            (262, 3, 1, 1),
            (273, 4, 1, 8 + 2 + 9 * 12 + 4),
            (277, 3, 1, 1),
            (278, 4, 1, 100_000),
            (279, 4, 1, 1),
        ];
        b.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        for (tag, kind, count, value) in entries {
            b.extend_from_slice(&tag.to_le_bytes());
            b.extend_from_slice(&kind.to_le_bytes());
            b.extend_from_slice(&count.to_le_bytes());
            b.extend_from_slice(&value.to_le_bytes());
        }
        b.extend_from_slice(&[0, 0, 0, 0, 0]);
        assert_eq!(load(&b).unwrap().len(), 1);
        assert!(render_page(&b, 0, 100).is_err());
    }

    #[test]
    fn colour_pages_come_out_in_colour_and_one_bit_pages_in_gray() {
        let (info, _) = decode(&render_page(MIXED, 0, 50).unwrap());
        assert_eq!(info.color_type, png::ColorType::Rgb);
        let (info, _) = decode(&render_page(MIXED, 2, 50).unwrap());
        assert_eq!(info.color_type, png::ColorType::Grayscale);
    }

    #[test]
    fn the_jpeg_page_has_the_colours_of_the_lzw_page() {
        // `generate.py` makes the pages of the mixed fixture from one image,
        // so a JPEG page (YCbCr) must come out with the same average colour
        // as the LZW one (RGB) within JPEG's loss.
        let mean = |page: u32| {
            let (info, pixels) = decode(&render_page(MIXED, page, 16).unwrap());
            assert_eq!(info.color_type, png::ColorType::Rgb);
            let mut sums = [0u64; 3];
            for p in pixels.as_chunks::<3>().0 {
                for c in 0..3 {
                    sums[c] += u64::from(p[c]);
                }
            }
            let n = (pixels.len() / 3) as u64;
            sums.map(|s| (s / n) as i32)
        };
        let (lzw, jpeg) = (mean(0), mean(3));
        for c in 0..3 {
            assert!((lzw[c] - jpeg[c]).abs() < 40, "channel {c}: {lzw:?} vs {jpeg:?}");
        }
    }
}
