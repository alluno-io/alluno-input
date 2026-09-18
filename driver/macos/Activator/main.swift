import Cocoa
import SystemExtensions

let extensionIdentifier = "io.alluno.AllunoVHID"

final class Activator: NSObject, NSApplicationDelegate, OSSystemExtensionRequestDelegate {
    func applicationDidFinishLaunching(_ notification: Notification) {
        let deactivate = CommandLine.arguments.contains("deactivate")
        let request = deactivate
            ? OSSystemExtensionRequest.deactivationRequest(forExtensionWithIdentifier: extensionIdentifier, queue: .main)
            : OSSystemExtensionRequest.activationRequest(forExtensionWithIdentifier: extensionIdentifier, queue: .main)
        request.delegate = self
        OSSystemExtensionManager.shared.submitRequest(request)
        print("\(deactivate ? "deactivation" : "activation") requested for \(extensionIdentifier)")
    }

    func request(
        _ request: OSSystemExtensionRequest,
        actionForReplacingExtension existing: OSSystemExtensionProperties,
        withExtension ext: OSSystemExtensionProperties
    ) -> OSSystemExtensionRequest.ReplacementAction {
        print("replacing \(existing.bundleVersion) with \(ext.bundleVersion)")
        return .replace
    }

    func requestNeedsUserApproval(_ request: OSSystemExtensionRequest) {
        print("approve it: System Settings, General, Login Items & Extensions, Driver Extensions (older macOS: Privacy & Security, Allow)")
    }

    func request(_ request: OSSystemExtensionRequest, didFinishWithResult result: OSSystemExtensionRequest.Result) {
        print(result == .completed ? "done" : "done, takes effect after a reboot")
        NSApp.terminate(nil)
    }

    func request(_ request: OSSystemExtensionRequest, didFailWithError error: Error) {
        print("failed: \(error)")
        NSApp.terminate(nil)
    }
}

let app = NSApplication.shared
let activator = Activator()
app.delegate = activator
app.run()
