package com.ember.android

import android.app.Application
import android.util.Log
import java.io.PrintWriter
import java.io.StringWriter
import java.util.concurrent.Executors

/**
 * Application entry point. Runs connection diagnostics immediately on startup
 * before any Activity is created. Filter Logcat by "EmberDiag" to see results.
 */
class EmberApplication : Application() {

    override fun onCreate() {
        super.onCreate()
        installCrashLogger()
        Log.i("EmberDiag", "EmberApplication.onCreate - starting diagnostics")
        Executors.newSingleThreadExecutor().execute {
            try {
                ConnectionDiagnostics.run(this@EmberApplication)
            } catch (t: Throwable) {
                Log.e("EmberDiag", "Diagnostics crashed", t)
            }
        }
    }

    private fun installCrashLogger() {
        val defaultHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, throwable ->
            val sw = StringWriter()
            throwable.printStackTrace(PrintWriter(sw))
            Log.e("EmberDiag", "UNCAUGHT EXCEPTION on ${thread.name}: ${throwable.message}\n$sw")
            defaultHandler?.uncaughtException(thread, throwable)
        }
    }
}
