package com.ember.loader

import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.view.View
import android.widget.Button
import android.widget.ProgressBar
import android.widget.TextView
import android.widget.Toast
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.FileProvider
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import org.json.JSONArray
import java.io.File
import java.io.FileOutputStream
import java.net.HttpURLConnection
import java.net.URL
import javax.net.ssl.HttpsURLConnection

class MainActivity : AppCompatActivity() {

    companion object {
        private const val GITHUB_RELEASES = "https://api.github.com/repos/Promethos-ai/blaster2000/releases"
        private const val EMBER_PACKAGE = "com.ember.android"
        private const val APK_NAME_PREFIX = "ember-"
        private const val APK_NAME_SUFFIX = ".apk"
        private const val INSTALL_REQUEST = 1001
    }

    private lateinit var statusText: TextView
    private lateinit var currentVersion: TextView
    private lateinit var latestVersion: TextView
    private lateinit var installBtn: Button
    private lateinit var progress: ProgressBar

    private var latestRelease: Release? = null

    data class Release(val tagName: String, val name: String, val body: String, val apkUrl: String)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        statusText = findViewById(R.id.status_text)
        currentVersion = findViewById(R.id.current_version)
        latestVersion = findViewById(R.id.latest_version)
        installBtn = findViewById(R.id.install_btn)
        progress = findViewById(R.id.progress)

        installBtn.setOnClickListener { latestRelease?.let { downloadAndInstall(it) } }

        fetchReleases()
    }

    private fun getInstalledVersion(): String? {
        return try {
            val info = packageManager.getPackageInfo(EMBER_PACKAGE, 0)
            @Suppress("DEPRECATION")
            info.versionName ?: info.longVersionCode.toString()
        } catch (e: PackageManager.NameNotFoundException) {
            null
        }
    }

    private fun fetchReleases() {
        progress.visibility = View.VISIBLE
        statusText.text = getString(R.string.checking)

        lifecycleScope.launch {
            try {
                val releases = withContext(Dispatchers.IO) {
                    val conn = URL(GITHUB_RELEASES).openConnection() as HttpsURLConnection
                    conn.requestMethod = "GET"
                    conn.setRequestProperty("Accept", "application/vnd.github.v3+json")
                    conn.connectTimeout = 15000
                    conn.readTimeout = 15000
                    conn.inputStream.bufferedReader().use { it.readText() }
                }
                val arr = JSONArray(releases)
                for (i in 0 until arr.length()) {
                    val obj = arr.getJSONObject(i)
                    val tagName = obj.optString("tag_name", "")
                    val name = obj.optString("name", "")
                    val body = obj.optString("body", "")
                    val assets = obj.optJSONArray("assets") ?: continue
                    for (j in 0 until assets.length()) {
                        val asset = assets.getJSONObject(j)
                        val assetName = asset.optString("name", "")
                        if (assetName.startsWith(APK_NAME_PREFIX) && assetName.endsWith(APK_NAME_SUFFIX)) {
                            val apkUrl = asset.optString("browser_download_url", "")
                            if (apkUrl.isNotEmpty()) {
                                latestRelease = Release(tagName, name, body, apkUrl)
                                break
                            }
                        }
                    }
                    if (latestRelease != null) break
                }
                withContext(Dispatchers.Main) { updateUI() }
            } catch (e: Exception) {
                withContext(Dispatchers.Main) {
                    progress.visibility = View.GONE
                    statusText.text = getString(R.string.error_fetch)
                    Toast.makeText(this@MainActivity, e.message, Toast.LENGTH_LONG).show()
                }
            }
        }
    }

    private fun updateUI() {
        progress.visibility = View.GONE
        val installed = getInstalledVersion()
        val release = latestRelease

        if (release == null) {
            statusText.text = getString(R.string.error_fetch)
            return
        }

        val versionFromTag = release.tagName.removePrefix("v")
        currentVersion.visibility = View.VISIBLE
        latestVersion.visibility = View.VISIBLE
        currentVersion.text = getString(R.string.current, installed ?: "—")
        latestVersion.text = getString(R.string.latest, versionFromTag)

        val hasUpdate = installed == null || isNewer(versionFromTag, installed)
        if (hasUpdate) {
            statusText.text = getString(R.string.latest, versionFromTag)
            installBtn.visibility = View.VISIBLE
        } else {
            statusText.text = getString(R.string.up_to_date)
            installBtn.visibility = View.GONE
        }
    }

    private fun isNewer(available: String, installed: String): Boolean {
        val a = available.split(".").map { it.toIntOrNull() ?: 0 }
        val b = installed.split(".").map { it.toIntOrNull() ?: 0 }
        for (i in 0 until maxOf(a.size, b.size)) {
            val av = a.getOrElse(i) { 0 }
            val bv = b.getOrElse(i) { 0 }
            if (av > bv) return true
            if (av < bv) return false
        }
        return false
    }

    private fun downloadAndInstall(release: Release) {
        installBtn.isEnabled = false
        progress.visibility = View.VISIBLE
        statusText.text = getString(R.string.downloading, 0)

        lifecycleScope.launch {
            try {
                val file = withContext(Dispatchers.IO) {
                    val apkFile = File(cacheDir, "ember-update.apk")
                    val conn = URL(release.apkUrl).openConnection() as HttpURLConnection
                    conn.instanceFollowRedirects = true
                    conn.connectTimeout = 30000
                    conn.readTimeout = 60000
                    val total = conn.contentLengthLong
                    var downloaded = 0L
                    conn.inputStream.use { input ->
                        FileOutputStream(apkFile).use { output ->
                            val buf = ByteArray(8192)
                            var n: Int
                            while (input.read(buf).also { n = it } != -1) {
                                output.write(buf, 0, n)
                                downloaded += n
                                if (total > 0) {
                                    val pct = (100 * downloaded / total).toInt()
                                    withContext(Dispatchers.Main) {
                                        statusText.text = getString(R.string.downloading, pct)
                                    }
                                }
                            }
                        }
                    }
                    apkFile
                }
                withContext(Dispatchers.Main) {
                    progress.visibility = View.GONE
                    statusText.text = getString(R.string.installing)
                    installApk(file)
                }
            } catch (e: Exception) {
                withContext(Dispatchers.Main) {
                    progress.visibility = View.GONE
                    installBtn.isEnabled = true
                    statusText.text = getString(R.string.error_download)
                    Toast.makeText(this@MainActivity, e.message, Toast.LENGTH_LONG).show()
                }
            }
        }
    }

    private fun installApk(file: File) {
        try {
            val uri = FileProvider.getUriForFile(this, "$packageName.fileprovider", file)
            val intent = Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "application/vnd.android.package-archive")
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
            startActivityForResult(intent, INSTALL_REQUEST)
        } catch (e: Exception) {
            Toast.makeText(this, getString(R.string.error_install), Toast.LENGTH_LONG).show()
            installBtn.isEnabled = true
        }
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == INSTALL_REQUEST) {
            installBtn.isEnabled = true
            statusText.text = if (resultCode == RESULT_OK) "Installed" else "Install cancelled"
        }
    }
}
