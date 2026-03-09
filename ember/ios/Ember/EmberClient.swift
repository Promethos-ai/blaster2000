import Foundation

/// Swift wrapper for the Rust Ember client.
enum EmberClient {
    /// Ask the AI a question via the ember server.
    static func ask(serverAddr: String, prompt: String) -> String {
        guard let addrCString = serverAddr.cString(using: .utf8) else {
            return "Error: invalid server address"
        }
        guard let promptCString = prompt.cString(using: .utf8) else {
            return "Error: invalid prompt"
        }
        guard let resultPtr = ember_ask(addrCString, promptCString) else {
            return "Error: connection failed"
        }
        defer { ember_free_string(resultPtr) }
        return String(cString: resultPtr)
    }
}
