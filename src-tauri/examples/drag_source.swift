// For scripts/macos/drop-check.sh: a window of its own process holding a file
// to drag, as Finder offers one, the counterpart of `drop_check
// --drag-source` on Linux and drag_source.ps1 on Windows.
//
//   drag_source FILE X Y   a 160 by 160 point window, its top-left corner at
//                          X Y (screen points from the top left), until killed
import AppKit

final class SourceView: NSView, NSDraggingSource {
    let file: URL
    init(frame: NSRect, file: URL) {
        self.file = file
        super.init(frame: frame)
    }
    required init?(coder: NSCoder) { fatalError("not used") }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.systemTeal.setFill()
        bounds.fill()
    }

    override func mouseDragged(with event: NSEvent) {
        let item = NSDraggingItem(pasteboardWriter: file as NSURL)
        item.setDraggingFrame(NSRect(x: 40, y: 40, width: 80, height: 80), contents: nil)
        beginDraggingSession(with: [item], event: event, source: self)
    }

    func draggingSession(_ session: NSDraggingSession,
                         sourceOperationMaskFor context: NSDraggingContext) -> NSDragOperation {
        context == .outsideApplication ? .copy : []
    }
}

let args = CommandLine.arguments
guard args.count == 4, let x = Double(args[2]), let y = Double(args[3]) else {
    FileHandle.standardError.write("usage: drag_source FILE X Y\n".data(using: .utf8)!)
    exit(3)
}
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let screen = NSScreen.screens[0].frame
let frame = NSRect(x: x, y: screen.height - y - 160, width: 160, height: 160)
let window = NSWindow(contentRect: frame, styleMask: [.borderless], backing: .buffered, defer: false)
window.contentView = SourceView(frame: NSRect(x: 0, y: 0, width: 160, height: 160),
                                file: URL(fileURLWithPath: args[1]))
window.level = .floating
window.makeKeyAndOrderFront(nil)
app.run()
