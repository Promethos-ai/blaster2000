package com.ember.android

object EmberClient {
    init {
        System.loadLibrary("ember_native")
    }

    external fun ask(serverAddr: String, prompt: String): String
}
