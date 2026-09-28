"""Inlines the app's variable fonts (OFL-1.1, from its node_modules) into
fonts.css, which every mock embeds. Run once: python3 make_fonts.py <repo>."""
import base64
import pathlib
import sys

REPO = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "../..")
FONTS = REPO / "node_modules/@fontsource-variable"
faces = [
    ("Big Shoulders Display Variable", "big-shoulders-display/files/big-shoulders-display-latin-wght-normal.woff2", "100 900"),
    ("Atkinson Hyperlegible Next Variable", "atkinson-hyperlegible-next/files/atkinson-hyperlegible-next-latin-wght-normal.woff2", "200 800"),
]
css = "".join(
    f"@font-face{{font-family:'{name}';font-weight:{weight};"
    f"src:url(data:font/woff2;base64,{base64.b64encode((FONTS / path).read_bytes()).decode()}) format('woff2');}}\n"
    for name, path, weight in faces
)
(pathlib.Path(__file__).parent / "fonts.css").write_text(css)
