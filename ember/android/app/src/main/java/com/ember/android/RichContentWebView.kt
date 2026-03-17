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
            mixedContentMode = WebSettings.MIXED_CONTENT_ALWAYS_ALLOW
        }
        webView.setBackgroundColor(0)
    }

    /** Update content with HTML. Safe to call from any thread; run on UI for visibility. */
    fun updateContent(webView: WebView, container: View?, html: String) {
        webView.post {
            val wrapped = wrapHtml(html, webView.context)
            // Use HTTPS base URL so external images (e.g. GitHub release assets) load
            webView.loadDataWithBaseURL("https://github.com/", wrapped, "text/html", "UTF-8", null)
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

    /** Force WebView to re-render the current DOM. Fixes layout glitches, orientation changes. */
    fun refresh(webView: WebView) {
        webView.post { webView.reload() }
    }

    /** Clear content and show Promethos logo placeholder. */
    fun clear(webView: WebView, container: View?) {
        webView.post {
            webView.loadDataWithBaseURL("https://github.com/", wrapHtml("", webView.context), "text/html", "UTF-8", null)
            container?.visibility = View.VISIBLE
        }
    }

    /** Apply dynamic CSS from AI. Injects into rich area. */
    fun applyStyle(webView: WebView, css: String) {
        if (css.isBlank()) return
        val sanitized = css
            .replace("javascript:", "")
            .replace(Regex("<script[^>]*>", RegexOption.IGNORE_CASE), "<!--")
        webView.post {
            val escaped = sanitized
                .replace("\\", "\\\\")
                .replace("'", "\\'")
                .replace("\n", "\\n")
                .replace("\r", "\\r")
            webView.evaluateJavascript(
                "try { var s = document.createElement('style'); s.textContent = '$escaped'; document.head.appendChild(s); } catch(e) {}",
                null
            )
        }
    }

    private fun logoPlaceholder(context: android.content.Context): String {
        val base = "android.resource://${context.packageName}/drawable/splash_promethous"
        return """<div class="rich-card" style="text-align:center;padding:16px;"><img src="$base" style="max-width:24px;max-height:24px;opacity:0.7;" /></div>"""
    }

    private fun wrapHtml(bodyContent: String, context: android.content.Context? = null): String {
        if (bodyContent.isBlank()) {
            val placeholder = context?.let { logoPlaceholder(it) }
                ?: """<div class="rich-card" style="text-align:center;padding:16px;opacity:0.6;"></div>"""
            return """<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>$BASE_CSS</style></head><body>$placeholder</body></html>"""
        }
        val sanitized = bodyContent
            .replace(Regex("<script[^>]*>", RegexOption.IGNORE_CASE), "<!--")
            .replace(Regex("</script>", RegexOption.IGNORE_CASE), "-->")
            .replace(Regex("\\son\\w+\\s*=", RegexOption.IGNORE_CASE)) { " data-removed=" }
            .replace("javascript:", "")
        return """<!DOCTYPE html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><style>$BASE_CSS</style></head><body>$sanitized</body></html>"""
    }
}
