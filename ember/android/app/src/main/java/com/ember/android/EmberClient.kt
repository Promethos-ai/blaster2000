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
        body { margin: 0; padding: 16px; font-family: Georgia, serif; font-size: 16px; line-height: 1.6; color: #2c2416; background: linear-gradient(160deg, #f5ebe0 0%, #e8dcc8 50%, #f0e6d8 100%); -webkit-user-select: text; user-select: text; }
        .chat { width: 100%; max-width: 100%; }
        .message { margin-bottom: 16px; padding: 14px 18px; border-radius: 4px; word-wrap: break-word; white-space: pre-wrap; box-shadow: 0 2px 8px rgba(0,0,0,0.06); }
        .message.user { margin-left: 48px; margin-right: 0; background: linear-gradient(135deg, #c45c26 0%, #a84818 100%); color: #fff; text-align: right; border-right: 4px solid #8b3a12; }
        .message.ai { margin-left: 0; margin-right: 48px; background: #fff; color: #2c2416; border-left: 4px solid #c45c26; }
    """.trimIndent()
}
