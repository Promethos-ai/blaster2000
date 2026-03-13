package com.ember.android

import android.webkit.WebView
import android.webkit.WebSettings

/**
 * Renders chat messages in a WebView with server-provided CSS.
 * Full width, copy/paste enabled.
 */
object ChatWebView {

    val DEFAULT_CSS = """
        * { box-sizing: border-box; }
        body { margin: 0; padding: 12px; font-family: system-ui; font-size: 15px; color: #e6edf3; background: #0d1117; -webkit-user-select: text; user-select: text; }
        .chat { width: 100%; max-width: 100%; }
        .message { margin-bottom: 12px; padding: 12px 16px; border-radius: 12px; word-wrap: break-word; white-space: pre-wrap; }
        .message.user { margin-left: 24px; margin-right: 0; background: #238636; color: #fff; text-align: right; }
        .message.ai { margin-left: 0; margin-right: 24px; background: #21262d; color: #e6edf3; }
    """.trimIndent()

    fun configure(webView: WebView) {
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
        }
        webView.setBackgroundColor(0xFFf5ebe0.toInt())
    }

    fun render(webView: WebView, messages: List<ChatMessage>, css: String) {
        val html = buildHtml(messages, css)
        webView.loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
    }

    fun updateLastAiMessage(webView: WebView, text: String) {
        val content = contentToHtml(text)
        val escaped = escapeJs(content)
        webView.evaluateJavascript(
            "var el = document.getElementById('streaming-msg'); if (el) el.innerHTML = $escaped;",
            null
        )
    }

    private fun buildHtml(messages: List<ChatMessage>, css: String): String {
        val sb = StringBuilder()
        sb.append("""<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>""")
        sb.append(css)
        sb.append("</style></head><body><div class=\"chat\">")
        for (i in messages.indices) {
            val msg = messages[i]
            val cls = if (msg.isUser) "user" else "ai"
            val idAttr = if (i == messages.lastIndex && !msg.isUser) " id=\"streaming-msg\"" else ""
            sb.append("<div class=\"message $cls\"$idAttr>")
            sb.append(if (msg.isUser) escapeHtml(msg.text) else contentToHtml(msg.text))
            sb.append("</div>")
        }
        sb.append("</div></body></html>")
        return sb.toString()
    }

    /** Renders AI content: HTML dashboard if it looks like one, else escaped text. */
    private fun contentToHtml(text: String): String {
        val trimmed = text.trimStart()
        if (trimmed.startsWith("<div") || trimmed.startsWith("<section") || trimmed.startsWith("<article")) {
            return sanitizeHtml(text)
        }
        return escapeHtml(text)
    }

    /** Allow safe HTML tags; strip script, event handlers, etc. */
    private fun sanitizeHtml(s: String): String = s
        .replace(Regex("<script[^>]*>", RegexOption.IGNORE_CASE), "<!--")
        .replace(Regex("</script>", RegexOption.IGNORE_CASE), "-->")
        .replace(Regex("\\son\\w+\\s*=", RegexOption.IGNORE_CASE)) { " data-removed=" }
        .replace("javascript:", "")

    private fun escapeHtml(s: String): String = s
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")

    private fun escapeJs(s: String): String {
        val escaped = s
            .replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
        return "\"$escaped\""
    }
}
