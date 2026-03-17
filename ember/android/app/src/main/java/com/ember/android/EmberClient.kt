package com.ember.android

object EmberClient {
    init {
        System.loadLibrary("ember_native")
    }

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
        body { margin: 0; padding: 12px; font-family: system-ui; font-size: 13px; line-height: 1.5; color: #fff; background: #000; -webkit-user-select: text; user-select: text; }
        .chat { width: 100%; max-width: 100%; }
        .message { display: block; width: fit-content; max-width: 85%; margin-bottom: 10px; padding: 10px 14px; border-radius: 9999px; word-wrap: break-word; white-space: pre-wrap; font-size: 13px; overflow: hidden; }
        .message.user { margin-left: auto; margin-right: 0; background: #238636; color: #fff; text-align: right; }
        .message.ai { margin-left: 0; margin-right: auto; background: #000; color: #fff; border: 1px solid #333; }
    """.trimIndent()
}
