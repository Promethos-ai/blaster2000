package com.ember.android

/**
 * Callback for progressive token delivery during streaming inference.
 */
fun interface TokenCallback {
    fun onToken(token: String)
}
