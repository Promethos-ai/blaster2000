// Bridging header for Rust Ember client
// Call from Swift: EmberClient.connect(serverAddr: String): String

#ifndef Ember_Bridging_Header_h
#define Ember_Bridging_Header_h

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Ask the AI a question via the ember server.
/// Returns a pointer to a null-terminated string, or null on error.
/// Caller must free with ember_free_string.
char* ember_ask(const char* server_addr, const char* prompt);

/// Free a string returned by ember_ask.
void ember_free_string(char* ptr);

#ifdef __cplusplus
}
#endif

#endif /* Ember_Bridging_Header_h */
