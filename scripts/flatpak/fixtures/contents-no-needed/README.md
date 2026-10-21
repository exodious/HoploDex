`readelf -d` printed no NEEDED library, which a dynamically linked HoploDex always has.
`checkContents` must fail rather than pass on output it did not understand (011 FR-018).
