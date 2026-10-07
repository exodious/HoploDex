# Document fixtures

Small documents for feature 007's tests and for `examples/human_seed.rs`
(`include_bytes!`). Every file here was made for HoploDex by
`generate.py` in this folder (Python 3 standard library, plus ImageMagick
and libtiff's `tiffcp` for the TIFFs), has no third-party content, and is
covered by the project's own license. `python3 generate.py` rewrites them
all, byte for byte (the output is deterministic).

The generators for hostile documents are not here: they are code, in
`tests/support/hostile_documents.rs`, and return bytes.

| File | How it was made |
|---|---|
| `three-pages.pdf` | Written by `generate.py` as plain PDF 1.4: three pages in Helvetica, each with its own text ("Page 1 of 3", ...). |
| `password-protected.pdf` | The same writer with the standard security handler, V 2 / R 3 (RC4, 128-bit), implemented in `generate.py`. **User password: `hoplodex-test`** (owner password `hoplodex-test-owner`). One page: "Password-protected test document". Ghostscript reads it with that password and not without. RC4-128 was chosen over AES-256 because the tool in the container (Ghostscript) can't write R6 and `generate.py` has no AES; every web view's PDF viewer reads RC4-128. |
| `three-pages-truncated.pdf` | `three-pages.pdf` cut to its first 60%: it still starts `%PDF-`, but has no cross-reference table or trailer. |
| `three-pages-bitflipped.pdf` | `three-pages.pdf` with one bit changed in every 37th byte from offset 64, so the header still starts `%PDF-`. |
| `one-page.tif` | A 200 x 272 RGB page, LZW. Made with ImageMagick, compressed by `tiffcp`. |
| `four-pages-mixed.tif` | Four pages, one compression each: 1 LZW (RGB), 2 Deflate (RGB, `AdobeDeflate`), 3 CCITT Group 4 (1-bit, 264 x 200), 4 JPEG (RGB, YCbCr). Checked with `tiffinfo`. |
| `sample.docx`, `sample.xlsx`, `sample.odt`, `sample.ods` | Written by `generate.py` with `zipfile`: the minimum parts each format needs, one line of text or one row ("Rounds", 150). `calamine` reads the `.xlsx` and `.ods`. ODF's `mimetype` is the first entry, stored. |
| `sample.xls` | A BIFF8 workbook (one sheet, "Rounds": a label and 150) inside a compound file, both written by `generate.py`. `calamine` opens it and reads the number. It is not checked in Excel itself. |
| `sample.doc` | **Not a complete Word document.** No free tool in the container writes DOC, so it is a compound file written by `generate.py` with a `WordDocument` stream that starts with the Word 97 file signature and a line of text, and no piece table. It passes a content check (a compound file with a `WordDocument` stream, no `Macros`); Word may call it damaged. Only classification and attach/open tests may rely on it. |

## Notes

- The compound-file writer in `generate.py` makes version 3 files whose
  streams are all padded to 4096 bytes, so none uses the mini stream.
- `three-pages.pdf`, the truncated and the bit-flipped copy are meant for the
  PDF viewer, which reports the damaged ones in its own words (spec FR-006).
