package com.ember.android

/**
 * Callback for progressive token delivery and AI-driven display control.
 * The AI can dynamically control formatting, layout, and audio via the server.
 */
interface TokenCallback {
    fun onToken(token: String)
    fun onRichContent(content: String) {}
    fun onStyle(css: String) {}
    fun onLayout(json: String) {}
    fun onAudio(text: String) {}
    fun onControlPayload(payload: String) {}
}
