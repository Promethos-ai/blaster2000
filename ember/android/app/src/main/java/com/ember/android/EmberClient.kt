package com.ember.android

object EmberClient {
    init {
        System.loadLibrary("ember_native")
    }

    /** Request full response (blocks until complete). */
    external fun ask(serverAddr: String, prompt: String): String

    /** Request with streaming; callback receives each token as it arrives. Returns final string. */
    external fun askStreaming(serverAddr: String, prompt: String, callback: TokenCallback): String
}
