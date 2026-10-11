// macOS: crops a PNG to a rectangle, in pixels, without changing a pixel's
// value (#88):
//
//   osascript -l JavaScript e2e/scripts/png-crop.js <in> <out> <left> <top> <width> <height>
//
// `sips -c` would do, but it ignores a crop that ends exactly at the bottom
// of the picture, which is where the page area ends in a window capture.

ObjC.import("AppKit");

function run(argv) {
  const [input, output, left, top, width, height] = argv;
  const source = $.NSBitmapImageRep.imageRepWithContentsOfFile(input);
  if (!source) throw new Error("not a readable image: " + input);
  const cropped = $.CGImageCreateWithImageInRect(source.CGImage, {
    origin: { x: Number(left), y: Number(top) },
    size: { width: Number(width), height: Number(height) },
  });
  if (!cropped) throw new Error("the crop is outside the image");
  const png = $.NSBitmapImageRep.alloc
    .initWithCGImage(cropped)
    .representationUsingTypeProperties($.NSBitmapImageFileTypePNG, $({}));
  if (!png.writeToFileAtomically(output, true)) throw new Error("could not write " + output);
}
