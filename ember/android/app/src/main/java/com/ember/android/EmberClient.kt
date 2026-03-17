package com.ember.android

object EmberClient {
    init {
        System.loadLibrary("ember_native")
    }

    /** Live tuning: set inference stream timeout (seconds). Default 120. Applied on next request. */
    external fun setInferenceTimeoutSec(seconds: Int)

    /** Request full response (blocks until complete). */
    external fun ask(serverAddr: String, prompt: String): String

    /** Request with streaming; callback receives each token as it arrives. Returns final string. */
    external fun askStreaming(serverAddr: String, prompt: String, callback: TokenCallback): String

    /** Fetch CSS style template from server (for dynamic styling). Returns default if fetch fails. */
    fun fetchStyle(serverAddr: String): String = try {
        val result = askStreaming(serverAddr, "__get_style__", object : TokenCallback {
            override fun onToken(token: String) {}
        })
        if (result.startsWith("Error:")) DEFAULT_CHAT_CSS else result
    } catch (_: Throwable) {
        DEFAULT_CHAT_CSS
    }

    internal val DEFAULT_CHAT_CSS = """
        * { box-sizing: border-box; }
        body { margin: 0; padding: 12px; padding-bottom: 80px; font-family: system-ui; color: #fff; background: #000; -webkit-user-select: text; user-select: text; }
        .message, .message.user, .message.ai { margin: 0 0 8px 0; padding: 0; border: none; border-radius: 0; background: none; white-space: pre-wrap; }
    """.trimIndent()
}
