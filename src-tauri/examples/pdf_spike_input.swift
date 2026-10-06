// For examples/pdf_spike_mac.sh: real mouse input and window bounds on
// macOS, as e2e/scripts/x11-input.py gives the Linux runs. The terminal (or
// sshd) running it needs Accessibility access to post events.
//
//   pdf_spike_input bounds PID      the PID's largest window: x y width height
//   pdf_spike_input move X Y        move the pointer (screen points)
//   pdf_spike_input click X Y       left click
//   pdf_spike_input rclick X Y      right click
//   pdf_spike_input text PID        every text the PID's accessibility tree
//                                   exposes (what VoiceOver could read), with
//                                   each element's role and position
//   pdf_spike_input press PID TITLE press the PID's element titled TITLE
//                                   (a context menu item, a button)
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
    FileHandle.standardError.write("usage: bounds PID | move X Y | click X Y | rclick X Y | text PID | press PID TITLE\n".data(using: .utf8)!)
    exit(2)
}
