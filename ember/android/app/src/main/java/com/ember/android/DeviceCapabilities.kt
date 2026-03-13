package com.ember.android

import android.content.Context
import android.content.Intent
import android.location.Location
import android.location.LocationManager
import android.net.Uri
import android.provider.MediaStore
import androidx.core.content.FileProvider
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Access to device hardware: storage, camera, microphone, GPS.
 * Used to provide context to the AI (e.g. location, photos) and save chat to disk.
 */
object DeviceCapabilities {

    /** Save chat transcript to app's Documents directory. Returns path or error. */
    fun saveChatToDisk(context: Context, messages: List<ChatMessage>): Result<String> = runCatching {
        val sb = StringBuilder()
        for (m in messages) {
            val role = if (m.isUser) "User" else "AI"
            sb.append("[$role] ${m.text}\n\n")
        }
        val content = sb.toString().trim()
        val date = SimpleDateFormat("yyyy-MM-dd_HH-mm", Locale.US).format(Date())
        val filename = "ember_chat_$date.txt"

        val base = context.getExternalFilesDir(null) ?: context.filesDir
        val dir = File(base, "chats").apply { mkdirs() }
        val file = File(dir, filename)
        file.writeText(content)
        file.absolutePath
    }

    /** Get last known location. Returns null if unavailable or permission denied. */
    fun getLastLocation(context: Context): Location? {
        val lm = context.getSystemService(Context.LOCATION_SERVICE) as? LocationManager ?: return null
        val providers = listOf(LocationManager.GPS_PROVIDER, LocationManager.NETWORK_PROVIDER)
        for (p in providers) {
            try {
                lm.getLastKnownLocation(p)?.let { return it }
            } catch (_: SecurityException) {}
        }
        return null
    }

    /** Format location for inclusion in prompt. */
    fun formatLocationForPrompt(loc: Location): String =
        "My location: lat ${loc.latitude}, lon ${loc.longitude} (accuracy ~${loc.accuracy.toInt()}m)"

    /** Create intent to take a photo. Returns Pair(intent, uri for the output file). */
    fun createTakePhotoIntent(context: Context): Pair<Intent, Uri>? {
        val photoFile = File(context.cacheDir, "ember_photo_${System.currentTimeMillis()}.jpg")
        val uri = FileProvider.getUriForFile(
            context,
            "${context.packageName}.fileprovider",
            photoFile
        )
        val intent = Intent(MediaStore.ACTION_IMAGE_CAPTURE).apply {
            putExtra(MediaStore.EXTRA_OUTPUT, uri)
            addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        }
        return intent.resolveActivity(context.packageManager)?.let { Pair(intent, uri) }
    }

    /** Check if camera is available. */
    fun hasCamera(context: Context): Boolean =
        Intent(MediaStore.ACTION_IMAGE_CAPTURE).resolveActivity(context.packageManager) != null

    /** Check if location is available. */
    fun hasLocation(context: Context): Boolean =
        (context.getSystemService(Context.LOCATION_SERVICE) as? LocationManager)?.let { lm ->
            lm.isProviderEnabled(LocationManager.GPS_PROVIDER) ||
                lm.isProviderEnabled(LocationManager.NETWORK_PROVIDER)
        } ?: false
}
