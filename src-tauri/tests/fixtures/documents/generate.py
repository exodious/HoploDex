#!/usr/bin/env python3
"""Regenerates the document fixtures in this folder (see SOURCE.md).

Needs Python 3 (standard library only), ImageMagick (`magick`) and libtiff's
`tiffcp`. The output is deterministic. Run it from anywhere:

    python3 generate.py

`python3 generate.py scan-pdf OUT [PAGES [DPI]]` instead writes one extra,
uncommitted document for performance runs (see `make_scan_pdf`).

Written for HoploDex (feature 007, T004); the output is covered by the
project's own license, there is no third-party content.
"""
import os
import struct
import subprocess
import sys
import tempfile
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
PDF_PASSWORD = "hoplodex-test"


def out(name, data):
    with open(os.path.join(HERE, name), "wb") as f:
        f.write(data)
    print(f"{name}: {len(data)} bytes")


# ---------------------------------------------------------------- PDF

PAD = bytes.fromhex("28BF4E5E4E758A4164004E56FFFA01082E2E00B6D0683E802F0CA9FE6453697A")
FILE_ID = bytes(range(16))


def rc4(key, data):
    s = list(range(256))
    j = 0
    for i in range(256):
        j = (j + s[i] + key[i % len(key)]) & 255
        s[i], s[j] = s[j], s[i]
    i = j = 0
    out_ = bytearray()
    for b in data:
        i = (i + 1) & 255
        j = (j + s[i]) & 255
        s[i], s[j] = s[j], s[i]
        out_.append(b ^ s[(s[i] + s[j]) & 255])
    return bytes(out_)


def standard_handler(user, owner):
    """The standard security handler, V 2 / R 3 (RC4, 128-bit), ISO 32000-1
    7.6.3 algorithms 2, 3 and 5. Returns (file key, /O, /U, /P)."""
    from hashlib import md5
    p = -4
    pad = lambda pw: (pw.encode() + PAD)[:32]
    h = md5(pad(owner)).digest()
    for _ in range(50):
        h = md5(h).digest()
    okey = h[:16]
    o = rc4(okey, pad(user))
    for i in range(1, 20):
        o = rc4(bytes(b ^ i for b in okey), o)
    h = md5(pad(user) + o + struct.pack("<i", p) + FILE_ID).digest()
    for _ in range(50):
        h = md5(h[:16]).digest()
    key = h[:16]
    u = rc4(key, md5(PAD + FILE_ID).digest())
    for i in range(1, 20):
        u = rc4(bytes(b ^ i for b in key), u)
    return key, o, u + b"\0" * 16, p


def pdf(pages, password=None):
    """A minimal PDF 1.4 with one page of text per entry of `pages`; with
    `password`, RC4-128 encrypted, `password` being the user password."""
    from hashlib import md5
    n = len(pages)
    if password:
        key, o, u, p = standard_handler(password, password + "-owner")
    objs = [None] * (3 + 2 * n)  # 1 catalog, 2 pages, 3 font, then page+content
    objs[0] = b"<< /Type /Catalog /Pages 2 0 R >>"
    kids = " ".join(f"{4 + 2 * i} 0 R" for i in range(n))
    objs[1] = f"<< /Type /Pages /Kids [{kids}] /Count {n} >>".encode()
    objs[2] = b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"
    for i, lines in enumerate(pages):
        content = "BT /F1 18 Tf 72 720 Td 26 TL\n"
        for line in lines:
            esc = line.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")
            content += f"({esc}) Tj T*\n"
        content += "ET"
        c = content.encode()
        objs[3 + 2 * i] = (
            f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
            f"/Contents {5 + 2 * i} 0 R /Resources << /Font << /F1 3 0 R >> >> >>"
        ).encode()
        if password:
            objnum = 5 + 2 * i
            okey = md5(key + struct.pack("<I", objnum)[:3] + b"\0\0").digest()[:16]
            c = rc4(okey, c)
        objs[4 + 2 * i] = b"<< /Length %d >>\nstream\n" % len(c) + c + b"\nendstream"
    trailer_extra = b""
    if password:
        objs.append(
            b"<< /Filter /Standard /V 2 /R 3 /Length 128 /P %d /O <%s> /U <%s> >>"
            % (p, o.hex().encode(), u.hex().encode()))
        trailer_extra = b" /Encrypt %d 0 R /ID [<%s> <%s>]" % (len(objs), FILE_ID.hex().encode(), FILE_ID.hex().encode())
    body = b"%PDF-1.4\n"
    offsets = []
    for i, o in enumerate(objs):
        offsets.append(len(body))
        body += b"%d 0 obj\n" % (i + 1) + o + b"\nendobj\n"
    xref = len(body)
    body += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    for off in offsets:
        body += b"%010d 00000 n \n" % off
    body += b"trailer\n<< /Size %d /Root 1 0 R%s >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, trailer_extra, xref)
    return body


def make_pdfs():
    pages = [
        ["HoploDex test document", "Page 1 of 3", "First page: purchase receipt text."],
        ["HoploDex test document", "Page 2 of 3", "Second page: serial number SN-0002."],
        ["HoploDex test document", "Page 3 of 3", "Third page: end of the document."],
    ]
    data = pdf(pages)
    out("three-pages.pdf", data)
    # Both still start with %PDF- so they pass the content check.
    out("three-pages-truncated.pdf", data[: len(data) * 6 // 10])
    flipped = bytearray(data)
    for pos in range(64, len(flipped) - 64, 37):
        flipped[pos] ^= 0x10 if pos % 2 else 0x01
    out("three-pages-bitflipped.pdf", bytes(flipped))
    out("password-protected.pdf", pdf(
        [["Password-protected test document", "If you can read this, the password worked."]],
        password=PDF_PASSWORD))


# ---------------------------------------------------------------- TIFF

def magick(*args):
    subprocess.run(["magick", *args], check=True)


def make_tiffs():
    with tempfile.TemporaryDirectory() as tmp:
        def p(name):
            return os.path.join(tmp, name)

        sizes = {1: "200x272", 2: "200x272", 3: "264x200", 4: "200x272"}
        colours = {1: "#c0392b", 2: "#2980b9", 3: "#27ae60", 4: "#8e44ad"}
        for n in range(1, 5):
            # Deterministic pictures: a coloured band, a diagonal, and a number.
            magick("-size", sizes[n], "xc:white", "-fill", colours[n],
                   "-draw", "rectangle 0,0 199,40", "-fill", "black",
                   "-draw", "line 0,40 199,259", "-draw", "line 0,259 199,40",
                   "-fill", "black", "-pointsize", "48",
                   "-draw", f"text 70,150 '{n}'",
                   "-alpha", "off", "-depth", "8", "-type", "TrueColor", "-compress", "None", f"tiff:{p(f'raw{n}.tif')}")
        # Page 3 is bi-level for CCITT G4.
        magick(p("raw3.tif"), "-colorspace", "Gray", "-threshold", "50%", "-depth", "1", "-type", "bilevel",
               "-compress", "None", f"tiff:{p('raw3.tif')}")
        for n, comp in ((1, "lzw"), (2, "zip"), (3, "g4"), (4, "jpeg")):
            args = ["tiffcp", "-c", comp]
            if comp == "jpeg":
                args = ["tiffcp", "-c", "jpeg:90", "-r", "16"]
            subprocess.run([*args, p(f"raw{n}.tif"), p(f"page{n}.tif")], check=True)
        first = True
        for n in range(1, 5):
            args = ["tiffcp"] + ([] if first else ["-a"]) + [p(f"page{n}.tif"), p("four.tif")]
            subprocess.run(args, check=True)
            first = False
        with open(p("four.tif"), "rb") as f:
            out("four-pages-mixed.tif", f.read())
        # A single page: LZW, colour.
        with open(p("page1.tif"), "rb") as f:
            out("one-page.tif", f.read())


def make_scan_tiff():
    """`bill-of-sale-scan.tif`: three US Letter pages (1700 x 2200 px at
    200 DPI) of a typed bill of sale, as a scanner's fax-style output: 1-bit,
    CCITT Group 4, a little skew and softness before the threshold. The
    human-testing seed attaches it, so the viewer has a TIFF that looks like
    a real scan, not a thumbnail zoomed to fit."""
    width, height = 1700, 2200
    serif, bold, mono = "DejaVu-Serif", "DejaVu-Serif-Bold", "DejaVu-Sans-Mono"
    oblique = "DejaVu-Serif-Italic"
    with tempfile.TemporaryDirectory() as tmp:
        def page(n, items):
            args = ["-density", "200", "-size", f"{width}x{height}", "xc:white", "-fill", "black"]
            for kind, *a in items:
                if kind == "text":
                    x, y, text, size, font = a
                    args += ["-font", font, "-pointsize", str(size), "-annotate", f"+{x}+{y}", text]
                elif kind == "rule":
                    x1, y1, x2, y2, w = a
                    args += ["-stroke", "black", "-strokewidth", str(w), "-draw",
                             f"line {x1},{y1} {x2},{y2}", "-stroke", "none"]
                elif kind == "box":
                    x1, y1, x2, y2 = a
                    args += ["-fill", "none", "-stroke", "black", "-strokewidth", "2", "-draw",
                             f"rectangle {x1},{y1} {x2},{y2}", "-stroke", "none", "-fill", "black"]
                elif kind == "ink":  # a handwritten signature: a loose stroke
                    x, y = a
                    d = (f"M {x},{y} C {x+30},{y-70} {x+60},{y-80} {x+75},{y-20} "
                         f"S {x+110},{y-90} {x+130},{y-30} S {x+175},{y-60} {x+215},{y-18} "
                         f"S {x+290},{y-35} {x+380},{y-24}")
                    args += ["-fill", "none", "-stroke", "black", "-strokewidth", "4",
                             "-draw", f"path '{d}'", "-stroke", "none", "-fill", "black"]
            args += ["-font", serif, "-pointsize", "9", "-annotate", "+120+2090",
                     "Ridgeline Arms  -  Bill of sale  -  Sale of 12 September 2025"]
            args += ["-annotate", "+1450+2090", f"Page {n} of 3"]
            # What a flatbed does: a slight skew, soft edges, then 1-bit.
            args += ["-background", "white", "-rotate", f"{0.25 * (1 if n % 2 else -1)}",
                     "-gravity", "center", "-crop", f"{width}x{height}+0+0", "+repage",
                     "-blur", "0x0.7", "-threshold", "62%", "-type", "bilevel",
                     "-units", "PixelsPerInch", "-density", "200", "-compress", "Group4",
                     f"tiff:{os.path.join(tmp, f'p{n}.tif')}"]
            magick(*args)

        rows = [
            ("Make", "Dead Air"), ("Model", "Sandman-K"), ("Type", "Suppressor"),
            ("Serial number", "SMK-51207"), ("Bore", ".30 caliber"), ("Price", "$1,100.00"),
        ]
        p1 = [
            ("text", 120, 230, "RIDGELINE ARMS", 26, bold),
            ("text", 120, 285, "Federal firearms licensee  -  retail and NFA sales", 11, serif),
            ("text", 120, 325, "1200 Example Street, Springfield, ST 00000   (555) 010-0142", 11, serif),
            ("rule", 120, 360, 1580, 360, 4),
            ("text", 120, 480, "BILL OF SALE", 30, bold),
            ("text", 1150, 480, "No. 25-0912-031", 13, mono),
            ("text", 120, 570, "Date of sale: September 12, 2025", 13, serif),
            ("text", 120, 640, "The seller named below sells, and the buyer named below buys, the", 13, serif),
            ("text", 120, 690, "following item, on the terms printed on the next page.", 13, serif),
            ("box", 120, 760, 1580, 1160),
        ]
        for i, (label, value) in enumerate(rows):
            y = 760 + 67 * i
            if i:
                p1.append(("rule", 120, y, 1580, y, 1))
            p1 += [("text", 150, y + 46, label, 12, serif), ("text", 560, y + 46, value, 13, mono)]
        p1 += [
            ("rule", 540, 760, 540, 1160, 1),
            ("text", 120, 1290, "SELLER", 11, bold),
            ("text", 120, 1340, "Ridgeline Arms, by Dana Whitfield, Sales manager", 13, serif),
            ("text", 120, 1400, "FFL on file with the buyer's transfer paperwork.", 11, oblique),
            ("text", 120, 1530, "BUYER", 11, bold),
            ("text", 120, 1580, "Smith Family Trust, by Alex Smith, Trustee", 13, serif),
            ("text", 120, 1640, "To be registered on ATF Form 4 (transfer approved by the ATF).", 11, oblique),
            ("ink", 150, 1850),
            ("rule", 120, 1860, 760, 1860, 2),
            ("text", 120, 1900, "Seller's signature", 10, serif),
            ("text", 120, 1935, "Date: 09/12/2025", 10, serif),
            ("ink", 960, 1850),
            ("rule", 930, 1860, 1580, 1860, 2),
            ("text", 930, 1900, "Buyer's signature", 10, serif),
            ("text", 930, 1935, "Date: 09/12/2025", 10, serif),
        ]
        terms = [
            "1.  The buyer has inspected the item and accepts it as described on page 1.",
            "2.  Title passes to the buyer when the ATF approves the transfer and the",
            "    seller delivers the item. Until then the seller keeps it in its vault.",
            "3.  The buyer certifies that he or she, or the trust, may lawfully possess the",
            "    item where it will be kept, and will not move it across a state line",
            "    without the permission the law requires.",
            "4.  All sales are final once the ATF approves the transfer. The seller may",
            "    repair or replace a defective item within thirty days at its choice.",
            "5.  The tax stamp, if any, is paid by the buyer and is not part of the price.",
            "6.  This bill of sale and the approved form are the whole agreement.",
        ]
        p2 = [
            ("text", 120, 230, "TERMS OF SALE", 26, bold),
            ("rule", 120, 275, 1580, 275, 4),
            ("text", 120, 360, "Bill of sale no. 25-0912-031 (Dead Air Sandman-K, SMK-51207)", 11, mono),
        ]
        for i, line in enumerate(terms):
            p2.append(("text", 120 if line[0] != " " else 180, 480 + 55 * i, line.strip(), 13, serif))
        p2 += [
            ("text", 120, 1180, "Notes", 12, bold),
            ("rule", 120, 1260, 1580, 1260, 1),
            ("text", 130, 1245, "Silencer cleaned and test-fired at the shop, 3 rounds, no issues.", 12, oblique),
            ("rule", 120, 1340, 1580, 1340, 1),
            ("text", 130, 1325, "Buyer received the mount, the manual and a hard case.", 12, oblique),
            ("rule", 120, 1420, 1580, 1420, 1),
            ("rule", 120, 1500, 1580, 1500, 1),
            ("rule", 120, 1580, 1580, 1580, 1),
            ("text", 120, 1900, "Initials  ", 11, serif),
            ("rule", 260, 1910, 560, 1910, 2),
            ("ink", 290, 1895),
        ]
        p3 = [
            ("text", 120, 230, "RECEIPT OF TRANSFER", 26, bold),
            ("rule", 120, 275, 1580, 275, 4),
            ("text", 120, 380, "Smith Family Trust acknowledges receipt of the item below.", 13, serif),
            ("text", 120, 470, "Dead Air Sandman-K suppressor, serial SMK-51207", 13, mono),
            ("text", 120, 570, "Approved on ATF Form 4:", 13, serif),
            ("text", 820, 570, "February 10, 2026", 13, mono),
            ("text", 120, 650, "Date received:", 13, serif),
            ("text", 820, 650, "February 14, 2026", 13, mono),
        ]
        checks = ["The item matches page 1, serial number included.",
                  "The Form 4 approval was shown to me and a copy given.",
                  "The shop's return terms on page 2 were explained.",
                  "The item arrived with the mount, manual and case."]
        for i, line in enumerate(checks):
            y = 800 + 80 * i
            p3 += [("box", 120, y, 160, y + 40), ("text", 190, y + 32, line, 13, serif)]
        p3 += [
            ("text", 120, 1280, "Received by", 11, bold),
            ("ink", 150, 1480),
            ("rule", 120, 1490, 760, 1490, 2),
            ("text", 120, 1530, "Alex Smith, Trustee", 11, serif),
            ("text", 930, 1530, "Released by Dana Whitfield", 11, serif),
            ("ink", 960, 1480),
            ("rule", 930, 1490, 1580, 1490, 2),
        ]
        for n, items in enumerate((p1, p2, p3), 1):
            page(n, items)
        subprocess.run(["tiffcp", os.path.join(tmp, "p1.tif"), os.path.join(tmp, "p2.tif"),
                        os.path.join(tmp, "p3.tif"), os.path.join(tmp, "all.tif")], check=True)
        with open(os.path.join(tmp, "all.tif"), "rb") as f:
            out("bill-of-sale-scan.tif", f.read())


# ----------------------------------------------------- ZIP-based formats

FIXED = (2020, 1, 1, 0, 0, 0)


def zipfile_bytes(entries, first_stored=None):
    import io
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w") as z:
        for name, data in entries:
            info = zipfile.ZipInfo(name, FIXED)
            info.compress_type = zipfile.ZIP_STORED if name == first_stored else zipfile.ZIP_DEFLATED
            info.external_attr = 0o644 << 16
            z.writestr(info, data)
    return buf.getvalue()


def make_ooxml_odf():
    ct_word = ("application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml")
    docx = zipfile_bytes([
        ("[Content_Types].xml",
         '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
         '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
         '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
         '<Default Extension="xml" ContentType="application/xml"/>'
         f'<Override PartName="/word/document.xml" ContentType="{ct_word}"/>'
         '</Types>'),
        ("_rels/.rels",
         '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
         '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
         '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>'
         '</Relationships>'),
        ("word/document.xml",
         '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
         '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>'
         '<w:p><w:r><w:t>HoploDex test bill of sale</w:t></w:r></w:p>'
         '</w:body></w:document>'),
    ])
    out("sample.docx", docx)

    ct_sheet = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"
    xlsx = zipfile_bytes([
        ("[Content_Types].xml",
         '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
         '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
         '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
         '<Default Extension="xml" ContentType="application/xml"/>'
         f'<Override PartName="/xl/workbook.xml" ContentType="{ct_sheet}"/>'
         '<Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>'
         '</Types>'),
        ("_rels/.rels",
         '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
         '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
         '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>'
         '</Relationships>'),
        ("xl/workbook.xml",
         '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
         '<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" '
         'xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">'
         '<sheets><sheet name="Rounds" sheetId="1" r:id="rId1"/></sheets></workbook>'),
        ("xl/_rels/workbook.xml.rels",
         '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
         '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
         '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>'
         '</Relationships>'),
        ("xl/worksheets/sheet1.xml",
         '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>'
         '<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>'
         '<row r="1"><c r="A1" t="inlineStr"><is><t>Rounds</t></is></c><c r="B1"><v>150</v></c></row>'
         '</sheetData></worksheet>'),
    ])
    out("sample.xlsx", xlsx)

    def odf(kind, mime):
        manifest = (
            '<?xml version="1.0" encoding="UTF-8"?>'
            '<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0" manifest:version="1.2">'
            f'<manifest:file-entry manifest:full-path="/" manifest:media-type="{mime}"/>'
            '<manifest:file-entry manifest:full-path="content.xml" manifest:media-type="text/xml"/>'
            '</manifest:manifest>')
        ns = ('xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" '
              'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" '
              'xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0" office:version="1.2"')
        if kind == "text":
            body = '<office:text><text:p>HoploDex test range note</text:p></office:text>'
        else:
            body = ('<office:spreadsheet><table:table table:name="Rounds"><table:table-row>'
                    '<table:table-cell office:value-type="string"><text:p>Rounds</text:p></table:table-cell>'
                    '<table:table-cell office:value-type="float" office:value="150"><text:p>150</text:p></table:table-cell>'
                    '</table:table-row></table:table></office:spreadsheet>')
        content = f'<?xml version="1.0" encoding="UTF-8"?><office:document-content {ns}><office:body>{body}</office:body></office:document-content>'
        return zipfile_bytes([("mimetype", mime), ("META-INF/manifest.xml", manifest),
                              ("content.xml", content)], first_stored="mimetype")

    out("sample.odt", odf("text", "application/vnd.oasis.opendocument.text"))
    out("sample.ods", odf("spreadsheet", "application/vnd.oasis.opendocument.spreadsheet"))


# ------------------------------------------------- OLE (DOC and XLS)

def ole_file(streams):
    """A minimal version-3 compound file: a flat root holding `streams`
    ({name: bytes}). Every stream is padded to 4096 bytes, so none needs the
    mini stream; one FAT sector, so at most about 60 KB in all."""
    SECT = 512
    FREE, ENDOFCHAIN, FATSECT = 0xFFFFFFFF, 0xFFFFFFFE, 0xFFFFFFFD
    items = []
    for name, b in streams.items():
        b = b + b"\0" * (-len(b) % SECT)
        if len(b) < 4096:
            b = b + b"\0" * (4096 - len(b))
        items.append((name, b))
    ndir = (len(items) + 1 + 3) // 4
    nsect = sum(len(b) // SECT for _, b in items)
    assert 1 + ndir + nsect <= 128
    fat = [FREE] * 128
    fat[0] = FATSECT
    for i in range(ndir):
        fat[1 + i] = 2 + i if i < ndir - 1 else ENDOFCHAIN
    pos = 1 + ndir
    starts = []
    for _, b in items:
        n = len(b) // SECT
        starts.append(pos)
        for i in range(n):
            fat[pos + i] = pos + i + 1 if i < n - 1 else ENDOFCHAIN
        pos += n

    def dirent(name, typ, left, right, child, start, size):
        nm = (name.encode("utf-16-le") + b"\0\0") if name else b""
        e = nm.ljust(64, b"\0") + struct.pack("<H", len(nm))
        e += struct.pack("<BB", typ, 1)
        e += struct.pack("<III", left, right, child)
        e += b"\0" * 16 + struct.pack("<I", 0) + b"\0" * 16
        e += struct.pack("<IQ", start, size)
        assert len(e) == 128
        return e

    # Valid CFB trees are red-black but readers (cfb, calamine, LibreOffice)
    # follow left/right/child links; a right-leaning chain is accepted.
    # Names must sort by (length, then upper-case) down the right links.
    order = sorted(range(len(items)), key=lambda i: (len(items[i][0]), items[i][0].upper()))
    ents = [dirent("", 0, FREE, FREE, FREE, 0, 0) for _ in range(ndir * 4)]
    for rank, i in enumerate(order):
        right = order[rank + 1] + 1 if rank + 1 < len(order) else FREE
        ents[i + 1] = dirent(items[i][0], 2, FREE, right, FREE, starts[i], len(items[i][1]))
    ents[0] = dirent("Root Entry", 5, FREE, FREE, order[0] + 1, ENDOFCHAIN, 0)
    hdr = bytearray(512)
    hdr[0:8] = b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1"
    struct.pack_into("<HHHHH", hdr, 24, 0x3E, 3, 0xFFFE, 9, 6)
    struct.pack_into("<I", hdr, 44, 1)        # FAT sectors
    struct.pack_into("<I", hdr, 48, 1)        # first directory sector
    struct.pack_into("<I", hdr, 56, 4096)     # mini stream cutoff
    struct.pack_into("<I", hdr, 60, ENDOFCHAIN)  # first mini FAT sector
    struct.pack_into("<I", hdr, 68, ENDOFCHAIN)  # first DIFAT sector
    struct.pack_into("<I", hdr, 76, 0)        # DIFAT[0] = FAT sector 0
    for k in range(1, 109):
        struct.pack_into("<I", hdr, 76 + 4 * k, FREE)
    body = bytes(hdr) + struct.pack("<128I", *fat) + b"".join(ents)
    for _, b in items:
        body += b
    return body


def biff(rec, data=b""):
    return struct.pack("<HH", rec, len(data)) + data


def make_ole():
    # XLS: BIFF8 workbook with one sheet "Rounds": A1 = "Rounds", B1 = 150.
    # The directory is laid out so that the sheet's BOF offset is known.
    def build_workbook():
        def boundsheet(offset):
            name = b"Rounds"
            return biff(0x0085, struct.pack("<IBB", offset, 0, 0) + bytes([len(name), 0]) + name)

        sst_str = b"Rounds"
        sst = biff(0x00FC, struct.pack("<II", 1, 1) + struct.pack("<HB", len(sst_str), 0) + sst_str)
        globals_nosheet = (
            biff(0x0809, struct.pack("<HHHHII", 0x0600, 0x0005, 0x0DBB, 0x07CC, 0, 6))
            + biff(0x0042, struct.pack("<H", 1252))
        )
        # BOUNDSHEET offset = length of the globals substream
        tail = sst + biff(0x000A)
        probe = len(globals_nosheet) + len(boundsheet(0)) + len(tail)
        globals_ = globals_nosheet + boundsheet(probe) + tail
        sheet = (
            biff(0x0809, struct.pack("<HHHHII", 0x0600, 0x0010, 0x0DBB, 0x07CC, 0, 6))
            + biff(0x0200, struct.pack("<IIHHH", 0, 1, 0, 2, 0))
            + biff(0x00FD, struct.pack("<HHHI", 0, 0, 0, 0))     # LABELSST A1
            + biff(0x0203, struct.pack("<HHHd", 0, 1, 0, 150.0))  # NUMBER B1
            + biff(0x000A)
        )
        return globals_ + sheet

    out("sample.xls", ole_file({"Workbook": build_workbook()}))
    # DOC: a compound file whose WordDocument stream carries a Word 97 FIB
    # signature (wIdent 0xA5EC, nFib 0x00C1). It passes the content check; it
    # is not guaranteed to open in Word (no piece table), see SOURCE.md.
    fib = struct.pack("<HH", 0xA5EC, 0x00C1) + b"\0" * 28
    text = b"HoploDex test document\r"
    out("sample.doc", ole_file({"WordDocument": fib + text, "1Table": b"\0" * 16}))


def make_scan_pdf(path, pages=10, dpi=300):
    """Opt-in (`python3 generate.py scan-pdf OUT [PAGES] [DPI]`, not part of the
    committed fixtures): a PDF that looks like what a scanner makes of a typed
    ledger, for measuring the viewer's first paint on a document of realistic
    weight (T152, SC-001). `pages` US Letter pages at `dpi`, each a typed
    acquisition record with rules, a box and drawn signatures, in grey with the
    paper's grain and a little skew and softness, JPEG-compressed (quality 85)
    inside the PDF as a scanner's own PDF output is. The defaults come to about
    10 MB. The content is invented. Fonts are DejaVu unless HD_FONT_SERIF,
    HD_FONT_BOLD, HD_FONT_MONO name others (ImageMagick's font names)."""
    serif = os.environ.get("HD_FONT_SERIF", "DejaVu-Serif")
    bold = os.environ.get("HD_FONT_BOLD", "DejaVu-Serif-Bold")
    mono = os.environ.get("HD_FONT_MONO", "DejaVu-Sans-Mono")
    k = dpi / 200.0  # the 200 DPI layout below, scaled
    width, height = round(1700 * k), round(2200 * k)
    makes = [("Dead Air", "Sandman-K", "Suppressor"), ("Ridgeline", "RL-9", "Pistol"),
             ("Harrow", "Model 12", "Shotgun"), ("Lowell", "LA-15", "Rifle"),
             ("Dead Air", "Wolfman", "Suppressor"), ("Stanton", "Scout", "Rifle")]
    with tempfile.TemporaryDirectory() as tmp:
        files = []
        for n in range(1, pages + 1):
            make, model, kind = makes[(n - 1) % len(makes)]
            serial = f"{make[:3].upper()}-{51207 + 137 * n}"
            args = ["-seed", str(n), "-density", str(dpi), "-size", f"{width}x{height}", "xc:white", "-fill", "black"]

            def text(x, y, t, size, font):
                return ["-font", font, "-pointsize", str(round(size * k, 1)),
                        "-annotate", f"+{round(x * k)}+{round(y * k)}", t]

            def rule(x1, y1, x2, y2, w):
                return ["-stroke", "black", "-strokewidth", str(max(1, round(w * k))), "-draw",
                        f"line {round(x1 * k)},{round(y1 * k)} {round(x2 * k)},{round(y2 * k)}", "-stroke", "none"]

            def ink(x, y):
                c = lambda v: round(v * k)
                d = (f"M {c(x)},{c(y)} C {c(x+30)},{c(y-70)} {c(x+60)},{c(y-80)} {c(x+75)},{c(y-20)} "
                     f"S {c(x+110)},{c(y-90)} {c(x+130)},{c(y-30)} S {c(x+175)},{c(y-60)} {c(x+215)},{c(y-18)} "
                     f"S {c(x+290)},{c(y-35)} {c(x+380)},{c(y-24)}")
                return ["-fill", "none", "-stroke", "black", "-strokewidth", str(max(1, round(4 * k))),
                        "-draw", f"path '{d}'", "-stroke", "none", "-fill", "black"]

            args += text(120, 230, "RIDGELINE ARMS", 26, bold)
            args += text(120, 285, "Acquisition record  -  retail and NFA sales", 11, serif)
            args += rule(120, 360, 1580, 360, 4)
            args += text(120, 480, "BILL OF SALE", 30, bold)
            args += text(1150, 480, f"No. 25-0912-{n:03d}", 13, mono)
            args += text(120, 570, f"Date of sale: September {1 + n}, 2025", 13, serif)
            rows = [("Make", make), ("Model", model), ("Type", kind), ("Serial number", serial),
                    ("Bore", ".30 caliber" if n % 2 else "9 mm"), ("Price", f"${900 + 85 * n:,}.00")]
            args += ["-fill", "none", "-stroke", "black", "-strokewidth", str(max(1, round(2 * k))), "-draw",
                     f"rectangle {round(120 * k)},{round(700 * k)} {round(1580 * k)},{round(1100 * k)}",
                     "-stroke", "none", "-fill", "black"]
            for i, (label, value) in enumerate(rows):
                y = 700 + 67 * i
                if i:
                    args += rule(120, y, 1580, y, 1)
                args += text(150, y + 46, label, 12, serif) + text(560, y + 46, value, 13, mono)
            args += rule(540, 700, 540, 1100, 1)
            lines = ["The buyer has inspected the item and accepts it as described above.",
                     "Title passes to the buyer when the transfer is approved and the item",
                     "is delivered. Until then the seller keeps it in its vault.",
                     "The buyer certifies that the item may lawfully be kept where it will be",
                     "stored and will not be moved across a state line without the permission",
                     "the law requires. All sales are final once the transfer is approved."]
            for i, line in enumerate(lines):
                args += text(120, 1230 + 55 * i, line, 12, serif)
            args += text(120, 1700, "Notes", 12, bold)
            for i in range(3):
                args += rule(120, 1790 + 80 * i, 1580, 1790 + 80 * i, 1)
            args += text(130, 1775, f"Cleaned and test-fired at the shop, {3 + n} rounds, no issues.", 12, serif)
            args += ink(150, 2000) + rule(120, 2010, 760, 2010, 2) + text(120, 2050, "Seller's signature", 10, serif)
            args += ink(960, 2000) + rule(930, 2010, 1580, 2010, 2) + text(930, 2050, "Buyer's signature", 10, serif)
            args += text(1450, 2150, f"Page {n} of {pages}", 9, serif)
            # What a flatbed does: a slight skew, soft edges, paper grain, grey.
            args += ["-background", "white", "-rotate", f"{0.25 * (1 if n % 2 else -1)}",
                     "-gravity", "center", "-crop", f"{width}x{height}+0+0", "+repage",
                     "-blur", f"0x{0.9 * k / 1.5:.2f}", "-attenuate", "0.45", "+noise", "Gaussian",
                     "-colorspace", "Gray", "-level", "6%,100%",
                     "-quality", "85", "-units", "PixelsPerInch", "-density", str(dpi),
                     os.path.join(tmp, f"p{n}.jpg")]
            magick(*args)
            files.append(os.path.join(tmp, f"p{n}.jpg"))
        magick(*files, "-compress", "JPEG", "-units", "PixelsPerInch", "-density", str(dpi), path)
    print(f"{path}: {os.path.getsize(path)} bytes, {pages} pages at {dpi} DPI")


if __name__ == "__main__":
    if len(sys.argv) > 1:
        if sys.argv[1] != "scan-pdf" or len(sys.argv) < 3:
            sys.exit("usage: generate.py [scan-pdf OUT [PAGES [DPI]]]")
        make_scan_pdf(sys.argv[2], *(int(a) for a in sys.argv[3:5]))
        sys.exit(0)
    make_pdfs()
    make_tiffs()
    make_scan_tiff()
    make_ooxml_odf()
    make_ole()
    sys.exit(0)
