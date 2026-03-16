package com.ember.android

import android.content.Context
import android.util.Log
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter
import java.net.InetSocketAddress
import java.net.Socket
import java.util.concurrent.atomic.AtomicBoolean

/**
 * Connection and startup diagnostics. Runs immediately on app launch.
 * Filter Logcat by tag "EmberDiag" to see all diagnostic output.
 */
object ConnectionDiagnostics {

    private const val TAG = "EmberDiag"

    private val hasRun = AtomicBoolean(false)

    /** Run all diagnostics. Call from Application.onCreate or earliest possible point. */
    @JvmStatic
    fun run(context: Context) {
        if (!hasRun.compareAndSet(false, true)) return
        log("=== CONNECTION DIAGNOSTICS START ===")
        log("App version: ${getVersionName(context)}")
        log("SDK: ${android.os.Build.VERSION.SDK_INT} (${android.os.Build.VERSION.RELEASE})")
        log("Device: ${android.os.Build.MANUFACTURER} ${android.os.Build.MODEL}")
        log("ABI: ${android.os.Build.SUPPORTED_ABIS.joinToString()}")
        runStep("1_native_lib_check") { testNativeLibExists(context) }
        runStep("2_network_permission") { testNetworkPermission(context) }
        runStep("3_default_server_resolve") { testDefaultServerResolve(context) }
        runStep("4_native_lib_load") { testNativeLibLoad() }
        runStep("5_ember_client_init") { testEmberClientInit() }
        runStep("6_native_jni_call") { testNativeJniCall() }
        log("=== CONNECTION DIAGNOSTICS END ===")
    }

    private fun log(msg: String) {
        Log.i(TAG, msg)
    }

    private fun logError(step: String, t: Throwable) {
        val sw = StringWriter()
        t.printStackTrace(PrintWriter(sw))
        Log.e(TAG, "[$step] ERROR: ${t.message}\n${sw}")
    }

    private fun runStep(step: String, block: () -> Unit) {
        try {
            log("[$step] START")
            block()
            log("[$step] OK")
        } catch (t: Throwable) {
            logError(step, t)
        }
    }

    private fun testNativeLibExists(ctx: Context): Unit {
        log("  java.library.path: ${System.getProperty("java.library.path")}")
        val appInfo = ctx.applicationInfo
        val nativeDir = appInfo.nativeLibraryDir
        log("  nativeLibraryDir: $nativeDir")
        val dir = File(nativeDir)
        if (dir.exists()) {
            val soFiles = dir.listFiles()?.filter { it.name.endsWith(".so") }?.map { it.name } ?: emptyList()
            log("  .so files: ${soFiles.sorted()}")
            val emberSo = soFiles.find { it.contains("ember") }
            if (emberSo != null) {
                log("  ember_native.so: FOUND ($emberSo)")
            } else {
                log("  WARNING: no ember*.so in native dir")
            }
        } else {
            log("  WARNING: native dir does not exist")
        }
    }

    private fun testNativeLibLoad(): Unit {
        try {
            System.loadLibrary("ember_native")
            log("  System.loadLibrary(ember_native) succeeded")
        } catch (e: UnsatisfiedLinkError) {
            log("  UnsatisfiedLinkError: ${e.message}")
            throw e
        }
    }

    private fun testNativeJniCall(): Unit {
        val result = EmberClient.ask("127.0.0.1:4433", "__ping__")
        log("  EmberClient.ask(__ping__) -> ${result.take(80)}${if (result.length > 80) "..." else ""}")
        if (result.startsWith("Error:")) {
            log("  (Expected if server not running - connection test)")
        }
    }

    private fun testEmberClientInit(): Unit {
        val css = EmberClient.DEFAULT_CHAT_CSS
        log("  EmberClient.DEFAULT_CHAT_CSS length: ${css.length}")
    }

    private fun testNetworkPermission(context: Context): Unit {
        val perm = context.checkCallingOrSelfPermission(android.Manifest.permission.INTERNET)
        val granted = perm == android.content.pm.PackageManager.PERMISSION_GRANTED
        log("  INTERNET permission: ${if (granted) "GRANTED" else "DENIED"}")
        if (!granted) {
            log("  WARNING: INTERNET permission not granted - connections will fail")
        }
    }

    private fun testDefaultServerResolve(context: Context): Unit {
        val defaultAddr = context.getString(R.string.default_server_address).trim()
        if (defaultAddr.isEmpty()) {
            log("  No default server address in strings")
            return
        }
        log("  Default server: $defaultAddr")
        val (host, portStr) = parseHostPort(defaultAddr)
        if (host == null || portStr == null) {
            log("  Could not parse host:port")
            return
        }
        val port = portStr.toIntOrNull() ?: return
        try {
            Socket().use { socket ->
                socket.connect(InetSocketAddress(host, port), 3000)
                log("  TCP connect to $host:$port: OK")
            }
        } catch (e: Exception) {
            log("  TCP connect to $host:$port: ${e.javaClass.simpleName} - ${e.message}")
            log("  (Expected if server not reachable - ember uses QUIC/UDP, not TCP)")
        }
    }

    private fun parseHostPort(addr: String): Pair<String?, String?> {
        val trimmed = addr.trim()
        val colonIdx = trimmed.lastIndexOf(':')
        if (colonIdx < 0) return null to null
        val host = trimmed.substring(0, colonIdx).trim().takeIf { it.isNotEmpty() }
        val port = trimmed.substring(colonIdx + 1).trim().takeIf { it.isNotEmpty() }
        return host to port
    }

    private fun getVersionName(context: Context): String {
        return try {
            context.packageManager.getPackageInfo(context.packageName, 0).versionName ?: "?"
        } catch (_: Exception) {
            "?"
        }
    }
}
