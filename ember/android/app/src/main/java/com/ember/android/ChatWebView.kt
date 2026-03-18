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
        html { scroll-behavior: smooth; }
        body { margin: 0; padding: 12px; padding-bottom: 80px; font-family: system-ui; color: #fff; background: #000; -webkit-user-select: text; user-select: text; }
        .message, .message.user, .message.ai { margin: 0 0 8px 0; padding: 0; border: none; border-radius: 0; background: none; white-space: pre-wrap; }
    """.trimIndent()

    fun configure(webView: WebView) {
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
        }
        webView.setBackgroundColor(0xFF000000.toInt())
    }

    fun render(webView: WebView, messages: List<ChatMessage>, css: String) {
        val html = buildHtml(messages, css)
        webView.loadDataWithBaseURL(null, html, "text/html", "UTF-8", null)
    }

    fun updateLastAiMessage(webView: WebView, text: String) {
        try {
            val content = contentToHtml(text)
            val escaped = escapeJs(content)
            webView.evaluateJavascript(
                "try { var el = document.getElementById('streaming-msg'); if (el) { el.innerHTML = $escaped; requestAnimationFrame(function(){ requestAnimationFrame(function(){ window.scrollTo(0, document.body.scrollHeight); }); }); } } catch(e) {}",
                null
            )
        } catch (_: Throwable) {
            val escaped = escapeJs(escapeHtml(text))
            webView.evaluateJavascript(
                "try { var el = document.getElementById('streaming-msg'); if (el) { el.textContent = $escaped; requestAnimationFrame(function(){ requestAnimationFrame(function(){ window.scrollTo(0, document.body.scrollHeight); }); }); } } catch(e) {}",
                null
            )
        }
    }

    private fun buildHtml(messages: List<ChatMessage>, css: String): String {
        val sb = StringBuilder()
        sb.append("""<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>""")
        sb.append(css.replace("</style>", ""))  // Prevent injection from breaking style tag
        sb.append("</style></head><body><div class=\"chat\">")
        for (i in messages.indices) {
            val msg = messages[i]
            val cls = if (msg.isUser) "user" else "ai"
            val idAttr = if (i == messages.lastIndex && !msg.isUser) " id=\"streaming-msg\"" else ""
            sb.append("<div class=\"message $cls\"$idAttr>")
            sb.append(if (msg.isUser) escapeHtml(msg.text) else contentToHtml(msg.text))
            sb.append("</div>")
        }
        sb.append("</div><script>(function(){var f=function(){window.scrollTo(0,document.body.scrollHeight);};if(document.readyState==='complete')setTimeout(f,50);else window.addEventListener('load',function(){setTimeout(f,50);});})();</script></body></html>")
        return sb.toString()
    }

    /** Renders AI content: HTML dashboard if it looks like one, else escaped text. */
    private fun contentToHtml(text: String): String {
        return try {
            val trimmed = text.trimStart()
            if (trimmed.startsWith("<div") || trimmed.startsWith("<section") || trimmed.startsWith("<article")) {
                sanitizeHtml(text)
            } else {
                escapeHtml(text)
            }
        } catch (_: Throwable) {
            escapeHtml(text)
        }
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
            .replace("\u2028", "\\u2028")
            .replace("\u2029", "\\u2029")
        return "\"$escaped\""
    }
}
