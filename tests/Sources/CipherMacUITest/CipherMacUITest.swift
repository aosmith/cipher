import AppKit
import ApplicationServices
import Foundation
import UniformTypeIdentifiers

struct UIElementQuery {
    static func matches(element: AXUIElement, role: String? = nil, titleContains needle: String? = nil) -> Bool {
        var result = true

        if let role = role {
            if let value = copyAttribute(element, attribute: kAXRoleAttribute as String) as? String {
                result = result && value == role
            } else {
                return false
            }
        }

        if let needle = needle?.lowercased() {
            let attributes: [String] = [
                kAXTitleAttribute as String,
                kAXLabelValueAttribute as String,
                kAXDescriptionAttribute as String,
                kAXIdentifierAttribute as String,
                kAXPlaceholderValueAttribute as String
            ]

            let combined = attributes
                .compactMap { copyAttribute(element, attribute: $0) as? String }
                .joined(separator: " ")
                .lowercased()
            result = result && combined.contains(needle)
        }

        return result
    }

    static func copyAttribute(_ element: AXUIElement, attribute: String) -> AnyObject? {
        var ref: CFTypeRef?
        let error = AXUIElementCopyAttributeValue(element, attribute as CFString, &ref)
        guard error == .success, let value = ref else { return nil }
        return value
    }

    static func firstMatch(root: AXUIElement, timeout: TimeInterval, predicate: (AXUIElement) -> Bool) -> AXUIElement? {
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            if let match = depthFirstSearch(element: root, predicate: predicate) {
                return match
            }
            RunLoop.current.run(mode: .default, before: Date().addingTimeInterval(0.1))
        }
        return nil
    }

    private static func depthFirstSearch(element: AXUIElement, predicate: (AXUIElement) -> Bool) -> AXUIElement? {
        if predicate(element) {
            return element
        }

        guard let children = copyAttribute(element, attribute: kAXChildrenAttribute as String) as? [AXUIElement] else {
            return nil
        }

        for child in children {
            if let match = depthFirstSearch(element: child, predicate: predicate) {
                return match
            }
        }
        return nil
    }

    static func collectText(from element: AXUIElement, into set: inout Set<String>) {
        let attributes: [String] = [
            kAXTitleAttribute as String,
            kAXLabelValueAttribute as String,
            kAXDescriptionAttribute as String,
            kAXIdentifierAttribute as String,
            kAXPlaceholderValueAttribute as String,
            kAXValueAttribute as String
        ]

        for attr in attributes {
            if let value = copyAttribute(element, attribute: attr) as? String {
                let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
                if !trimmed.isEmpty {
                    set.insert(trimmed)
                }
            }
        }

        if let children = copyAttribute(element, attribute: kAXChildrenAttribute as String) as? [AXUIElement] {
            for child in children {
                collectText(from: child, into: &set)
            }
        }
    }
}

@main
struct CipherMacUITest {
    static func main() async {
        guard AXIsProcessTrusted() else {
            fputs("⚠️  Accessibility permission required. Enable it in System Settings > Privacy & Security > Accessibility.\n", stderr)
            exit(1)
        }

        let arguments = CommandLine.arguments.dropFirst()
        let appPathArg = arguments.first(where: { $0.hasPrefix("--app=") })?.split(separator: "=").last.map(String.init)
        let homeArg = arguments.first(where: { $0.hasPrefix("--home=") })?.split(separator: "=").last.map(String.init)
        let expectArgs = arguments
            .filter { $0.hasPrefix("--expect=") }
            .compactMap { $0.split(separator: "=").last.map { String($0).lowercased() } }

        let defaultAppPath = URL(fileURLWithPath: "../target/release/bundle/macos/Cipher.app")
        let appURL = appPathArg.map(URL.init(fileURLWithPath:)) ?? defaultAppPath

        guard FileManager.default.fileExists(atPath: appURL.path) else {
            fputs("❌ Cipher.app not found at \(appURL.path). Build the app with 'cargo tauri build'.\n", stderr)
            exit(1)
        }

        let tempHome: URL
        let shouldRemoveHome: Bool
        if let providedHome = homeArg.map(URL.init(fileURLWithPath:)) {
            tempHome = providedHome
            shouldRemoveHome = false
            try? FileManager.default.createDirectory(at: tempHome, withIntermediateDirectories: true)
        } else {
            let generated = URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("cipher_ui_home_\(UUID().uuidString)", isDirectory: true)
            try? FileManager.default.createDirectory(at: generated, withIntermediateDirectories: true)
            tempHome = generated
            shouldRemoveHome = true
        }

        let openConfig = NSWorkspace.OpenConfiguration()
        openConfig.activates = false
        let xdgDir = tempHome.appendingPathComponent("xdg", isDirectory: true)
        try? FileManager.default.createDirectory(at: xdgDir, withIntermediateDirectories: true)

        var environment = ProcessInfo.processInfo.environment
        environment["HOME"] = tempHome.path
        environment["XDG_DATA_HOME"] = xdgDir.path
        openConfig.environment = environment

        let app: NSRunningApplication
        do {
            app = try await NSWorkspace.shared.openApplication(at: appURL, configuration: openConfig)
        } catch {
            fputs("❌ Failed to launch app: \(error.localizedDescription)\n", stderr)
            exit(1)
        }

        defer {
            app.terminate()
            if shouldRemoveHome {
                try? FileManager.default.removeItem(at: tempHome)
            }
        }

        let pid = app.processIdentifier
        let appElement = AXUIElementCreateApplication(pid)

        guard let window = UIElementQuery.firstMatch(root: appElement, timeout: 10, predicate: { UIElementQuery.matches(element: $0, role: kAXWindowRole as String) }) else {
            fputs("❌ Could not find main window.\n", stderr)
            exit(1)
        }

        let loginLabel = UIElementQuery.firstMatch(root: window, timeout: 5, predicate: { UIElementQuery.matches(element: $0, titleContains: "username") })
        let usernameField = UIElementQuery.firstMatch(root: window, timeout: 5, predicate: { UIElementQuery.matches(element: $0, role: kAXTextFieldRole as String, titleContains: "username") })
        let signInButton = UIElementQuery.firstMatch(root: window, timeout: 5, predicate: { UIElementQuery.matches(element: $0, role: kAXButtonRole as String, titleContains: "sign in") })

        let signOutButton = UIElementQuery.firstMatch(root: window, timeout: 2, predicate: { UIElementQuery.matches(element: $0, role: kAXButtonRole as String, titleContains: "sign out") })
        let postsTab = UIElementQuery.firstMatch(root: window, timeout: 2, predicate: { UIElementQuery.matches(element: $0, titleContains: "posts") })

        let loginVisible = loginLabel != nil && usernameField != nil && signInButton != nil
        let dashboardVisible = signOutButton != nil && postsTab != nil

        var snapshotStrings = Set<String>()
        UIElementQuery.collectText(from: window, into: &snapshotStrings)

        let knownDashboardStrings: [String] = ["encrypted social network", "toggle theme", "online", "delete"]
        let looksLikeDashboard = knownDashboardStrings.contains { candidate in
            snapshotStrings.contains { $0.lowercased().contains(candidate) }
        }

        guard loginVisible || dashboardVisible || looksLikeDashboard else {
            fputs("❌ Could not identify login or dashboard UI. Visible strings: \(snapshotStrings.joined(separator: ", "))\n", stderr)
            exit(1)
        }

        if loginVisible {
            print("ℹ️ Login screen detected")
        } else {
            print("ℹ️ Dashboard detected; skipping login assertions")
        }

        for expected in expectArgs {
            let found = snapshotStrings.contains { $0.lowercased().contains(expected) }
            if !found {
                fputs("❌ Expected text '\(expected)' not present. Visible strings: \(snapshotStrings.joined(separator: ", "))\n", stderr)
                exit(1)
            }
        }

        do {
            try captureScreenshot(of: pid)
        } catch {
            fputs("⚠️  Failed to capture screenshot: \(error)\n", stderr)
        }

        print("✅ macOS UI smoke test passed.")
    }

    private static func captureScreenshot(of pid: pid_t) throws {
        guard let windowList = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else {
            throw NSError(domain: "CipherMacUITest", code: 1, userInfo: [NSLocalizedDescriptionKey: "Unable to list windows"])
        }

        guard let windowInfo = windowList.first(where: { ($0[kCGWindowOwnerPID as String] as? pid_t) == pid }),
              let windowID = windowInfo[kCGWindowNumber as String] as? CGWindowID
        else {
            throw NSError(domain: "CipherMacUITest", code: 2, userInfo: [NSLocalizedDescriptionKey: "Window ID not found"])
        }

        guard let image = CGWindowListCreateImage(.null, .optionIncludingWindow, windowID, [.bestResolution, .boundsIgnoreFraming]) else {
            throw NSError(domain: "CipherMacUITest", code: 3, userInfo: [NSLocalizedDescriptionKey: "Unable to capture image"])
        }

        let destinationDir = URL(fileURLWithPath: "artifacts", relativeTo: URL(fileURLWithPath: FileManager.default.currentDirectoryPath))
        try FileManager.default.createDirectory(at: destinationDir, withIntermediateDirectories: true)
        let fileURL = destinationDir.appendingPathComponent("mac-login-\(Int(Date().timeIntervalSince1970)).png")

        guard let destination = CGImageDestinationCreateWithURL(fileURL as CFURL, UTType.png.identifier as CFString, 1, nil) else {
            throw NSError(domain: "CipherMacUITest", code: 4, userInfo: [NSLocalizedDescriptionKey: "Failed to create image destination"])
        }

        CGImageDestinationAddImage(destination, image, nil)
        if !CGImageDestinationFinalize(destination) {
            throw NSError(domain: "CipherMacUITest", code: 5, userInfo: [NSLocalizedDescriptionKey: "Failed to write screenshot"])
        }

        print("📸 Saved screenshot to \(fileURL.path)")
    }
}
