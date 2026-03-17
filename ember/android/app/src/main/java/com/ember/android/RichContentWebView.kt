package com.ember.android

import android.view.View
import android.webkit.WebView
import android.webkit.WebSettings

/**
 * Renders HTML in real time for weather, email previews, etc.
 * Used in the reserved area at the top of the chat.
 */
object RichContentWebView {

    private val BASE_CSS = """
        * { box-sizing: border-box; }
        body { margin: 0; padding: 0; font-family: system-ui; font-size: 14px; color: #e6edf3; background: transparent; -webkit-user-select: text; user-select: text; }
        .rich-card { padding: 8px 12px; border-radius: 8px; background: rgba(255,255,255,0.06); margin-bottom: 8px; }
        .rich-card:last-child { margin-bottom: 0; }
    """.trimIndent()

    fun configure(webView: WebView) {
        webView.settings.apply {
            javaScriptEnabled = true
            domStorageEnabled = true
        }
        webView.setBackgroundColor(0)
    }

    /** Update content with HTML. Safe to call from any thread; run on UI for visibility. */
    fun updateContent(webView: WebView, container: View?, html: String) {
        val wrapped = wrapHtml(html)
        webView.post {
            webView.loadDataWithBaseURL(null, wrapped, "text/html", "UTF-8", null)
            container?.visibility = View.VISIBLE
        }
    }

    /** Append HTML (e.g. for streaming email lines). Merges with existing body content. */
    fun appendContent(webView: WebView, container: View?, htmlFragment: String) {
        webView.post {
            val escaped = htmlFragment
                .replace("\\", "\\\\")
                .replace("'", "\\'")
                .replace("\n", "\\n")
                .replace("\r", "\\r")
            webView.evaluateJavascript(
                "try { var body = document.body; if (body) { var div = document.createElement('div'); div.className='rich-card'; div.innerHTML = '$escaped'; body.appendChild(div); body.scrollTop = body.scrollHeight; } } catch(e) {}",
                null
            )
            container?.visibility = View.VISIBLE
        }
    }

    /** Clear content and hide the container. */
    fun clear(webView: WebView, container: View?) {
        webView.post {
            webView.loadDataWithBaseURL(null, wrapHtml(""), "text/html", "UTF-8", null)
            container?.visibility = View.GONE
        }
    }

    private fun wrapHtml(bodyContent: String): String {
        if (bodyContent.isBlank()) {
            return """<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>$BASE_CSS</style></head><body></body></html>"""
        }
        val sanitized = bodyContent
            .replace(Regex("<script[^>]*>", RegexOption.IGNORE_CASE), "<!--")
            .replace(Regex("</script>", RegexOption.IGNORE_CASE), "-->")
            .replace(Regex("\\son\\w+\\s*=", RegexOption.IGNORE_CASE)) { " data-removed=" }
            .replace("javascript:", "")
        return """<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>$BASE_CSS</style></head><body>$sanitized</body></html>"""
    }
}
