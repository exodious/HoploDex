// For scripts/macos/pdf-surface-check.sh: real mouse input and window bounds
// on macOS, as pdf_surface_input.py gives the Linux runs. The terminal (or
// sshd) running it needs Accessibility access to post events.
//
//   pdf_surface_input bounds PID      the PID's largest window: x y width height
//   pdf_surface_input move X Y        move the pointer (screen points)
//   pdf_surface_input click X Y       left click
//   pdf_surface_input rclick X Y      right click
//   pdf_surface_input text PID        every text the PID's accessibility tree
//                                   exposes (what VoiceOver could read), with
//                                   each element's role and position
//   pdf_surface_input press PID TITLE press the PID's element titled TITLE
//                                   (a context menu item, a button)
//   pdf_surface_input clickt X Y      left click, and print when (Unix ms,
//                                   just before the button goes up)
//   pdf_surface_input find PID ROLE TITLE
//                                   the centre of the PID's first ROLE
//                                   element whose title is TITLE: x y
//   pdf_surface_input key CODE [cmd]  press a key (a virtual key code), with
//                                   Command held if `cmd` is given
//   pdf_surface_input scroll X Y N    wheel N notches at X Y (negative: down)
//   pdf_surface_input menu PID        the PID's menu bar items, as paths,
//                                   with any disabled ones marked
import ApplicationServices
import CoreGraphics
import Foundation

let args = CommandLine.arguments

func post(_ type: CGEventType, _ x: Double, _ y: Double, _ button: CGMouseButton = .left) {
    CGEvent(mouseEventSource: nil, mouseType: type, mouseCursorPosition: CGPoint(x: x, y: y),
            mouseButton: button)!.post(tap: .cghidEventTap)
    usleep(80_000)
}

func attribute(_ element: AXUIElement, _ name: String) -> CFTypeRef? {
    var value: CFTypeRef?
    return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
}

func position(_ element: AXUIElement) -> String {
    guard let value = attribute(element, kAXPositionAttribute) else { return "" }
    var point = CGPoint.zero
    AXValueGetValue(value as! AXValue, .cgPoint, &point)
    return " @\(Int(point.x)),\(Int(point.y))"
}

/// Every element under `element`, depth first, with the strings it exposes.
func walk(_ element: AXUIElement, _ depth: Int = 0, _ visit: (AXUIElement, String, [String]) -> Void) {
    guard depth < 60 else { return }
    let role = attribute(element, kAXRoleAttribute) as? String ?? "?"
    let strings = [kAXValueAttribute, kAXTitleAttribute, kAXDescriptionAttribute]
        .compactMap { attribute(element, $0) as? String }.filter { !$0.isEmpty }
    visit(element, role, strings)
    for child in attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] {
        walk(child, depth + 1, visit)
    }
}

switch args.count > 1 ? args[1] : "" {
case "bounds":
    let pid = Int32(args[2])!
    let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as! [[String: Any]]
    let mine = windows.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == pid }
        .compactMap { $0[kCGWindowBounds as String] as? [String: Double] }
        .max { ($0["Width"]! * $0["Height"]!) < ($1["Width"]! * $1["Height"]!) }
    if let b = mine { print(Int(b["X"]!), Int(b["Y"]!), Int(b["Width"]!), Int(b["Height"]!)) } else { exit(1) }
case "move":
    post(.mouseMoved, Double(args[2])!, Double(args[3])!)
case "click":
    let (x, y) = (Double(args[2])!, Double(args[3])!)
    post(.mouseMoved, x, y); post(.leftMouseDown, x, y); post(.leftMouseUp, x, y)
case "rclick":
    let (x, y) = (Double(args[2])!, Double(args[3])!)
    post(.mouseMoved, x, y); post(.rightMouseDown, x, y, .right); post(.rightMouseUp, x, y, .right)
case "clickt":
    let (x, y) = (Double(args[2])!, Double(args[3])!)
    post(.mouseMoved, x, y); post(.leftMouseDown, x, y)
    print(Int64(Date().timeIntervalSince1970 * 1000))
    post(.leftMouseUp, x, y)
case "find":
    var found = false
    walk(AXUIElementCreateApplication(Int32(args[2])!)) { element, role, _ in
        guard !found, role == args[3], attribute(element, kAXTitleAttribute) as? String == args[4],
              let p = attribute(element, kAXPositionAttribute), let s = attribute(element, kAXSizeAttribute)
        else { return }
        var point = CGPoint.zero, size = CGSize.zero
        AXValueGetValue(p as! AXValue, .cgPoint, &point); AXValueGetValue(s as! AXValue, .cgSize, &size)
        print(Int(point.x + size.width / 2), Int(point.y + size.height / 2))
        found = true
    }
    if !found { exit(1) }
case "key":
    let code = CGKeyCode(args[2])!
    for down in [true, false] {
        let event = CGEvent(keyboardEventSource: nil, virtualKey: code, keyDown: down)!
        // Always set: an event with the flags unset takes the modifiers of the
        // key event posted before it, so Escape after Command+O reached nothing
        // (Command+Escape belongs to the system).
        event.flags = args.count > 3 && args[3] == "cmd" ? .maskCommand : []
        event.post(tap: .cghidEventTap)
        usleep(50_000)
    }
case "scroll":
    let (x, y, n) = (Double(args[2])!, Double(args[3])!, Int32(args[4])!)
    post(.mouseMoved, x, y)
    for _ in 0..<abs(n) {
        CGEvent(scrollWheelEvent2Source: nil, units: .line, wheelCount: 1, wheel1: n > 0 ? 3 : -3, wheel2: 0, wheel3: 0)!
            .post(tap: .cghidEventTap)
        usleep(40_000)
    }
case "menu":
    func items(_ element: AXUIElement, _ path: String, _ depth: Int) {
        guard depth < 8 else { return }
        for child in attribute(element, kAXChildrenAttribute) as? [AXUIElement] ?? [] {
            let title = attribute(child, kAXTitleAttribute) as? String ?? ""
            let here = title.isEmpty ? path : (path.isEmpty ? title : "\(path) > \(title)")
            if !title.isEmpty {
                let enabled = attribute(child, kAXEnabledAttribute) as? Bool ?? true
                print(here + (enabled ? "" : " [disabled]"))
            }
            items(child, here, depth + 1)
        }
    }
    let app = AXUIElementCreateApplication(Int32(args[2])!)
    if let bar = attribute(app, kAXMenuBarAttribute) { items(bar as! AXUIElement, "", 0) }
case "text":
    let app = AXUIElementCreateApplication(Int32(args[2])!)
    // Ask for the full tree, as VoiceOver does (web content is built lazily).
    AXUIElementSetAttributeValue(app, "AXEnhancedUserInterface" as CFString, kCFBooleanTrue)
    usleep(500_000)
    walk(app) { element, role, strings in
        if !strings.isEmpty || role == "AXButton" {
            print("\(role)\(position(element)) \(strings.map { String($0.prefix(100)) })")
        }
    }
case "press":
    var pressed = false
    walk(AXUIElementCreateApplication(Int32(args[2])!)) { element, _, strings in
        if !pressed && strings.contains(args[3]) {
            pressed = AXUIElementPerformAction(element, kAXPressAction as CFString) == .success
        }
    }
    print(pressed ? "pressed" : "not found")
default:
    FileHandle.standardError.write("usage: bounds PID | move X Y | click X Y | rclick X Y | scroll X Y N | text PID | press PID TITLE | clickt X Y | find PID ROLE TITLE | key CODE [cmd] | menu PID\n".data(using: .utf8)!)
    exit(2)
}
