package com.ember.android

import android.Manifest
import android.content.Intent
import android.location.Location
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.appcompat.app.AlertDialog
import android.os.Handler
import android.os.Looper
import android.speech.tts.TextToSpeech
import android.speech.RecognizerIntent
import android.util.Log
import android.view.View
import android.webkit.WebView
import android.widget.ArrayAdapter
import android.widget.EditText
import android.widget.ImageButton
import android.widget.Spinner
import android.widget.TextView
import android.widget.Toast
import android.widget.PopupMenu
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.util.Locale

class MainActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "MainActivity"
        private const val DIAG = "EmberDiag"
        private const val KEY_CHAT_TEXTS = "chat_texts"
        private const val KEY_CHAT_IS_USER = "chat_is_user"
        private const val PREF_SPEAK_RESPONSES = "speak_responses"
        private const val PREF_CUSTOM_SERVER = "custom_server"
        private const val PREF_SERVER_INDEX = "server_index"
        private const val STREAM_UPDATE_DELAY_MS = 80L  // Batch tokens for smoother display
        private const val CONTROL_POLL_INITIAL_DELAY_MS = 1_000L  // Brief delay before first long-poll
    }

    private lateinit var appTitle: TextView
    private lateinit var serverSpinner: Spinner
    private lateinit var promptInput: EditText
    private lateinit var askBtn: ImageButton
    private lateinit var speakBtn: ImageButton
    private lateinit var micBtn: ImageButton
    private lateinit var menuBtn: ImageButton
    private lateinit var chatWebView: WebView
    private lateinit var richContentWebView: WebView
    private lateinit var richContentContainer: View
    private lateinit var errorText: TextView

    private val chatMessages = mutableListOf<ChatMessage>()
    private var chatCss = ChatWebView.DEFAULT_CSS
    private var hasFetchedStyle = false
    private var lastSharedLocation: Location? = null

    private var tts: TextToSpeech? = null

    private val streamUpdateHandler = Handler(Looper.getMainLooper())
    private var pendingStreamUpdate: Runnable? = null

    private var pushPollJob: Job? = null

    private val voiceLauncher = registerForActivityResult(
        ActivityResultContracts.StartActivityForResult()
    ) { result ->
        if (result.resultCode == RESULT_OK) {
            val data = result.data
            val matches = data?.getStringArrayListExtra(RecognizerIntent.EXTRA_RESULTS)
            if (!matches.isNullOrEmpty()) {
                val spoken = matches[0]
                promptInput.setText(spoken)
                promptInput.setSelection(spoken.length)
            }
        }
    }

    private val permissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) {
            startVoiceInput()
        } else {
            Toast.makeText(this, getString(R.string.error_voice), Toast.LENGTH_SHORT).show()
        }
    }

    private val multiPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestMultiplePermissions()
    ) { granted ->
        if (granted.values.any { it }) {
            onLocationRequested()
        }
    }

    private val cameraPermissionLauncher = registerForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) launchCamera()
        else Toast.makeText(this, getString(R.string.camera_unavailable), Toast.LENGTH_SHORT).show()
    }

    private var pendingCameraUri: android.net.Uri? = null

    private val cameraLauncher = registerForActivityResult(
        ActivityResultContracts.TakePicture()
    ) { success ->
        if (success) {
            Toast.makeText(this, "Photo saved", Toast.LENGTH_SHORT).show()
        }
        pendingCameraUri = null
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        Log.i(DIAG, "MainActivity.onCreate START")
        super.onCreate(savedInstanceState)
        Log.i(DIAG, "MainActivity - setContentView")
        setContentView(R.layout.activity_main)

        // Edge-to-edge: draw behind system bars, apply insets for cutouts/notches
        WindowCompat.setDecorFitsSystemWindows(window, false)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            window.attributes.layoutInDisplayCutoutMode =
                android.view.WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
        }
        val rootLayout = findViewById<View>(R.id.root_layout)
        ViewCompat.setOnApplyWindowInsetsListener(rootLayout) { v, insets ->
            val systemBars = insets.getInsets(WindowInsetsCompat.Type.systemBars())
            v.setPadding(systemBars.left, systemBars.top, systemBars.right, systemBars.bottom)
            insets
        }

        Log.i(DIAG, "MainActivity - findViewById")
        appTitle = findViewById(R.id.app_title)
        serverSpinner = findViewById(R.id.server_spinner)
        promptInput = findViewById(R.id.prompt_input)
        askBtn = findViewById(R.id.ask_btn)
        speakBtn = findViewById(R.id.speak_btn)
        micBtn = findViewById(R.id.mic_btn)
        menuBtn = findViewById(R.id.menu_btn)
        chatWebView = findViewById(R.id.chat_webview)
        richContentWebView = findViewById(R.id.rich_content_webview)
        richContentContainer = findViewById(R.id.rich_content_container)
        errorText = findViewById(R.id.error_text)
        Log.i(DIAG, "MainActivity - all findViewById done")

        appTitle.text = "Ember ${BuildConfig.VERSION_NAME}"
        val presets = resources.getStringArray(R.array.server_presets)
        val adapter = ArrayAdapter(this, R.layout.spinner_item_small, presets).apply {
            setDropDownViewResource(R.layout.spinner_dropdown_small)
        }
        serverSpinner.adapter = adapter
        serverSpinner.setSelection(
            getSharedPreferences("ember", MODE_PRIVATE).getInt(PREF_SERVER_INDEX, 0).coerceIn(0, presets.size - 1)
        )
        serverSpinner.setOnItemSelectedListener(object : android.widget.AdapterView.OnItemSelectedListener {
            override fun onItemSelected(parent: android.widget.AdapterView<*>, view: View?, pos: Int, id: Long) {
                getSharedPreferences("ember", MODE_PRIVATE).edit().putInt(PREF_SERVER_INDEX, pos).apply()
                if (presets[pos] == getString(R.string.server_custom)) showCustomServerDialog()
            }
            override fun onNothingSelected(parent: android.widget.AdapterView<*>) {}
        })
        promptInput.hint = getString(R.string.prompt_hint)

        menuBtn.setOnClickListener { showOverflowMenu() }
        speakBtn.setOnClickListener {
            val next = !isSpeakEnabled()
            getSharedPreferences("ember", MODE_PRIVATE).edit().putBoolean(PREF_SPEAK_RESPONSES, next).apply()
            updateSpeakButtonIcon()
        }
        updateSpeakButtonIcon()
        Log.i(DIAG, "MainActivity - menu setup done")

        Log.i(DIAG, "MainActivity - initializing TTS")
        tts = TextToSpeech(this) { status ->
            if (status == TextToSpeech.SUCCESS) {
                tts?.language = Locale.getDefault()
            } else {
                Log.e(TAG, "TTS init failed: $status")
            }
        }
        Log.i(DIAG, "MainActivity - TTS callback registered")

        Log.i(DIAG, "MainActivity - ChatWebView.configure")
        ChatWebView.configure(chatWebView)
        RichContentWebView.configure(richContentWebView)
        reinitializeDisplay()

        micBtn.setOnClickListener {
            when {
                ContextCompat.checkSelfPermission(this, android.Manifest.permission.RECORD_AUDIO) ==
                    android.content.pm.PackageManager.PERMISSION_GRANTED -> startVoiceInput()
                else -> permissionLauncher.launch(android.Manifest.permission.RECORD_AUDIO)
            }
        }

        askBtn.setOnClickListener {
            val addr = getServerAddress()
            var prompt = promptInput.text.toString().trim()
            when {
                addr.isEmpty() -> Toast.makeText(this, getString(R.string.error_server), Toast.LENGTH_SHORT).show()
                prompt.isEmpty() -> Toast.makeText(this, getString(R.string.error_prompt), Toast.LENGTH_SHORT).show()
                else -> {
                    // Prepend last known location so AI remembers it for follow-up questions
                    lastSharedLocation?.let { loc ->
                        if (!prompt.contains("lat ", ignoreCase = true)) {
                            prompt = "${DeviceCapabilities.formatLocationForPrompt(loc)}\n\n$prompt"
                        }
                    }
                    askAi(addr, prompt, prompt)
                }
            }
        }

        Log.i(DIAG, "MainActivity.onCreate DONE")
    }

    override fun onResume() {
        super.onResume()
        startControlSupervisor()
    }

    override fun onPause() {
        stopControlSupervisor()
        super.onPause()
    }

    /**
     * Supervisory process: long-polls __fetch_push__ for pushed messages.
     * Server holds the connection until a push arrives (or 60s timeout).
     * Pushes are delivered immediately; no polling delay.
     */
    private fun startControlSupervisor() {
        stopControlSupervisor()
        pushPollJob = lifecycleScope.launch(Dispatchers.IO) {
            delay(CONTROL_POLL_INITIAL_DELAY_MS)
            while (isActive && !isDestroyed) {
                val addr = withContext(Dispatchers.Main) { getServerAddress() }
                if (addr.isNotEmpty()) {
                    try {
                        val result = EmberClient.askStreaming(addr, "__fetch_push__", object : TokenCallback {
                            override fun onToken(token: String) {}
                        })
                        if (result.isNotBlank()) {
                            withContext(Dispatchers.Main) {
                                if (!isDestroyed) {
                                    applyPushPayload(result.trim())
                                    commitControlToDom()
                                }
                            }
                        }
                    } catch (_: Exception) {}
                }
                // No delay: immediately start next long-poll. Server blocks until push or 60s.
            }
        }
    }

    private fun stopControlSupervisor() {
        pushPollJob?.cancel()
        pushPollJob = null
    }

    /** Commit control changes to DOM and refresh display. Called after applyPushPayload. */
    private fun commitControlToDom() {
        renderChat()
        RichContentWebView.refresh(richContentWebView)
        scrollToBottom()
    }

    /**
     * Apply a push payload. Supports:
     * - Plain text: append as AI message
     * - JSON: { chat, chatCss, rich, richStyle, layout, input, message }
     *   - chat: [{text, isUser}, ...] - replace entire chat
     *   - chatCss: CSS string - chat area styling
     *   - rich: HTML - replace rich content area
     *   - richStyle: CSS - inject into rich area
     *   - layout: {rich_height, theme} - layout hints
     *   - input: string - prefill prompt input
     *   - message: string - append as AI message (fallback)
     */
    private fun applyPushPayload(payload: String) {
        val trimmed = payload.trim()
        if (trimmed.equals("app clear", ignoreCase = true) ||
            trimmed.equals("__app_clear__", ignoreCase = true)) {
            reinitializeDisplay()
            return
        }
        if (trimmed.equals("refresh", ignoreCase = true)) {
            renderChat()
            RichContentWebView.refresh(richContentWebView)
            return
        }
        if (payload.startsWith("{")) {
            try {
                hideError()
                val obj = org.json.JSONObject(payload)
                if (obj.has("chat")) {
                    chatMessages.clear()
                    val arr = obj.getJSONArray("chat")
                    for (i in 0 until arr.length()) {
                        val m = arr.getJSONObject(i)
                        chatMessages.add(ChatMessage(
                            m.optString("text", ""),
                            m.optBoolean("isUser", false)
                        ))
                    }
                }
                if (obj.has("chatCss")) {
                    chatCss = obj.getString("chatCss")
                }
                if (obj.has("rich")) {
                    val html = obj.getString("rich")
                    if (html.isBlank()) {
                        RichContentWebView.clear(richContentWebView, richContentContainer)
                    } else {
                        RichContentWebView.updateContent(richContentWebView, richContentContainer, html)
                    }
                }
                if (obj.has("richStyle")) {
                    RichContentWebView.applyStyle(richContentWebView, obj.getString("richStyle"))
                }
                if (obj.has("layout")) {
                    applyLayout(obj.getJSONObject("layout").toString())
                }
                if (obj.has("input")) {
                    promptInput.setText(obj.getString("input"))
                }
                if (obj.has("message")) {
                    val msg = obj.getString("message")
                    if (msg.isNotBlank()) {
                        chatMessages.add(ChatMessage(msg, isUser = false))
                        if (isSpeakEnabled()) tts?.speak(msg, TextToSpeech.QUEUE_FLUSH, null, null)
                    }
                }
                if (obj.optBoolean("refresh", false)) {
                    RichContentWebView.refresh(richContentWebView)
                }
                renderChat()
                scrollToBottom()
            } catch (e: Exception) {
                Log.e(TAG, "applyPushPayload parse error", e)
                showError(e.message ?: getString(R.string.error_generic))
            }
        } else {
            chatMessages.add(ChatMessage(payload, isUser = false))
            renderChat()
            scrollToBottom()
            if (isSpeakEnabled()) tts?.speak(payload, TextToSpeech.QUEUE_FLUSH, null, null)
        }
    }

    private fun onLocationRequested() {
        val loc = DeviceCapabilities.getLastLocation(this)
        if (loc != null) {
            lastSharedLocation = loc
            val locText = DeviceCapabilities.formatLocationForPrompt(loc)
            promptInput.setText("$locText\n\n${promptInput.text}")
            promptInput.setSelection(0)
        } else {
            Toast.makeText(this, getString(R.string.location_unavailable), Toast.LENGTH_SHORT).show()
        }
    }

    private fun launchCamera() {
        DeviceCapabilities.createTakePhotoIntent(this)?.let { (_, uri) ->
            pendingCameraUri = uri
            cameraLauncher.launch(uri)
        } ?: Toast.makeText(this, getString(R.string.camera_unavailable), Toast.LENGTH_SHORT).show()
    }

    private fun saveChatToDisk() {
        DeviceCapabilities.saveChatToDisk(this, chatMessages).fold(
            onSuccess = { path -> Toast.makeText(this, getString(R.string.saved_to, path), Toast.LENGTH_LONG).show() },
            onFailure = { Toast.makeText(this, getString(R.string.error_save_failed), Toast.LENGTH_SHORT).show() }
        )
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        outState.putStringArray(KEY_CHAT_TEXTS, chatMessages.map { it.text }.toTypedArray())
        outState.putBooleanArray(KEY_CHAT_IS_USER, chatMessages.map { it.isUser }.toBooleanArray())
    }

    private fun renderChat() {
        ChatWebView.render(chatWebView, chatMessages, chatCss)
    }

    private fun scrollToBottom() {
        chatWebView.evaluateJavascript("window.scrollTo(0, document.body.scrollHeight);", null)
    }

    private fun showError(msg: String) {
        errorText.text = msg
        errorText.visibility = View.VISIBLE
    }

    private fun hideError() {
        errorText.visibility = View.GONE
    }

    private fun isSpeakEnabled(): Boolean =
        getSharedPreferences("ember", MODE_PRIVATE).getBoolean(PREF_SPEAK_RESPONSES, true)

    private fun updateSpeakButtonIcon() {
        speakBtn.setImageResource(if (isSpeakEnabled()) R.drawable.ic_face_speak else R.drawable.ic_face_mute)
    }

    private fun getServerAddress(): String {
        val presets = resources.getStringArray(R.array.server_presets)
        val pos = serverSpinner.selectedItemPosition
        return if (pos in presets.indices && presets[pos] == getString(R.string.server_custom)) {
            getSharedPreferences("ember", MODE_PRIVATE).getString(PREF_CUSTOM_SERVER, "")?.trim() ?: ""
        } else {
            presets.getOrNull(pos)?.trim() ?: ""
        }
    }

    private fun showCustomServerDialog() {
        val input = EditText(this).apply {
            setPadding(48, 32, 48, 32)
            hint = getString(R.string.server_address_hint)
            setText(getSharedPreferences("ember", MODE_PRIVATE).getString(PREF_CUSTOM_SERVER, ""))
        }
        AlertDialog.Builder(this)
            .setTitle(getString(R.string.server_label))
            .setView(input)
            .setPositiveButton(android.R.string.ok) { _, _ ->
                val url = input.text.toString().trim()
                if (url.isNotEmpty()) {
                    getSharedPreferences("ember", MODE_PRIVATE).edit().putString(PREF_CUSTOM_SERVER, url).apply()
                }
            }
            .setNegativeButton(android.R.string.cancel, null)
            .show()
    }

    private fun showOverflowMenu() {
        val popup = PopupMenu(this, menuBtn)
        popup.menuInflater.inflate(R.menu.overflow_menu, popup.menu)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) popup.setForceShowIcon(true)
        popup.setOnMenuItemClickListener { item ->
            when (item.itemId) {
                R.id.menu_check_in -> {
                    val addr = getServerAddress()
                    if (addr.isEmpty()) Toast.makeText(this, getString(R.string.error_server), Toast.LENGTH_SHORT).show()
                    else askAi(addr, "__check_in__", getString(R.string.check_in))
                    true
                }
                R.id.menu_location -> {
                    when {
                        ContextCompat.checkSelfPermission(this, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED -> onLocationRequested()
                        else -> multiPermissionLauncher.launch(arrayOf(Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION))
                    }
                    true
                }
                R.id.menu_camera -> {
                    when {
                        ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED -> launchCamera()
                        else -> cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
                    }
                    true
                }
                R.id.menu_save -> {
                    saveChatToDisk()
                    true
                }
                else -> false
            }
        }
        popup.show()
    }

    /** Reinitialize display: clear chat, rich content, error, prompt input; reset to default state. Called when __app_clear__ or "app clear" is received from ember server. */
    private fun reinitializeDisplay() {
        chatMessages.clear()
        lastSharedLocation = null
        chatCss = ChatWebView.DEFAULT_CSS
        RichContentWebView.clear(richContentWebView, richContentContainer)
        promptInput.setText("")
        hideError()
        renderChat()
    }

    /** If the text contains HTML (weather, email preview, etc.), show it in the rich content area. */
    private fun updateRichContentIfHtml(text: String) {
        val trimmed = text.trimStart()
        if (trimmed.startsWith("<div") || trimmed.startsWith("<section") || trimmed.startsWith("<article") ||
            trimmed.contains("<div") || trimmed.contains("<p ") || trimmed.contains("<strong>")) {
            val html = extractHtmlBlock(text)
            if (html.isNotBlank()) {
                RichContentWebView.updateContent(richContentWebView, richContentContainer, html)
            }
        }
    }

    /** Apply layout hints from AI (rich_height, theme). */
    private fun applyLayout(json: String) {
        try {
            val obj = org.json.JSONObject(json.trim())
            val richHeight = obj.optString("rich_height", "140")
            val lp = richContentContainer.layoutParams ?: return
            val heightPx = when (richHeight) {
                "full" -> (resources.displayMetrics.heightPixels * 0.4).toInt()
                "auto", "tall" -> (140 * resources.displayMetrics.density).toInt() * 2
                else -> (140 * resources.displayMetrics.density).toInt()
            }
            lp.height = heightPx
            richContentContainer.layoutParams = lp
        } catch (_: Exception) {}
    }

    /** Extract HTML block from mixed content (e.g. weather card, email preview). */
    private fun extractHtmlBlock(text: String): String {
        val start = text.indexOf('<')
        if (start < 0) return ""
        // Take from first < to end; WebView tolerates trailing text
        return text.substring(start)
    }

    private fun startVoiceInput() {
        val intent = Intent(RecognizerIntent.ACTION_RECOGNIZE_SPEECH).apply {
            putExtra(RecognizerIntent.EXTRA_LANGUAGE_MODEL, RecognizerIntent.LANGUAGE_MODEL_FREE_FORM)
            putExtra(RecognizerIntent.EXTRA_LANGUAGE, java.util.Locale.getDefault())
            putExtra(RecognizerIntent.EXTRA_PROMPT, getString(R.string.listening))
        }
        try {
            voiceLauncher.launch(intent)
        } catch (e: Exception) {
            Toast.makeText(this, getString(R.string.error_voice), Toast.LENGTH_SHORT).show()
        }
    }

    private fun askAi(addr: String, prompt: String, displayPrompt: String = prompt) {
        if (prompt != "__check_in__") promptInput.setText("")
        askBtn.isEnabled = false
        menuBtn.isEnabled = false

        // Fetch style from server on first use (or use cached)
        if (!hasFetchedStyle) {
            hasFetchedStyle = true
            lifecycleScope.launch {
                chatCss = withContext(Dispatchers.IO) { EmberClient.fetchStyle(addr) }
                runOnUiThread { doAskAi(addr, prompt, displayPrompt) }
            }
        } else {
            doAskAi(addr, prompt, displayPrompt)
        }
    }

    private fun doAskAi(addr: String, prompt: String, displayPrompt: String) {
        hideError()
        RichContentWebView.clear(richContentWebView, richContentContainer)
        chatMessages.add(ChatMessage(displayPrompt, isUser = true))
        chatMessages.add(ChatMessage(getString(R.string.asking), isUser = false))
        renderChat()
        scrollToBottom()

        lifecycleScope.launch {
            val result = try {
                withContext(Dispatchers.IO) {
                    try {
                        var accumulatedText = ""
                        var hasShownFirstToken = false
                        EmberClient.askStreaming(addr, prompt, object : TokenCallback {
                            override fun onToken(token: String) {
                                accumulatedText += token
                                val textToShow = accumulatedText
                                val isFirstToken = !hasShownFirstToken && textToShow.isNotEmpty()
                                if (isFirstToken) {
                                    hasShownFirstToken = true
                                    runOnUiThread {
                                        if (!isDestroyed) {
                                            val last = chatMessages.lastIndex
                                            if (last >= 0 && !chatMessages[last].isUser) {
                                                chatMessages[last] = ChatMessage(textToShow, isUser = false)
                                                ChatWebView.updateLastAiMessage(chatWebView, textToShow)
                                                updateRichContentIfHtml(textToShow)
                                                scrollToBottom()
                                            }
                                        }
                                    }
                                } else {
                                    pendingStreamUpdate?.let { streamUpdateHandler.removeCallbacks(it) }
                                    pendingStreamUpdate = Runnable {
                                        runOnUiThread {
                                            if (!isDestroyed) {
                                                val last = chatMessages.lastIndex
                                                if (last >= 0 && !chatMessages[last].isUser) {
                                                    chatMessages[last] = ChatMessage(textToShow, isUser = false)
                                                    ChatWebView.updateLastAiMessage(chatWebView, textToShow)
                                                    updateRichContentIfHtml(textToShow)
                                                    scrollToBottom()
                                                }
                                            }
                                        }
                                        pendingStreamUpdate = null
                                    }
                                    streamUpdateHandler.postDelayed(pendingStreamUpdate!!, STREAM_UPDATE_DELAY_MS)
                                }
                            }
                            override fun onRichContent(content: String) {
                                runOnUiThread {
                                    if (!isDestroyed) {
                                        RichContentWebView.updateContent(richContentWebView, richContentContainer, content)
                                    }
                                }
                            }
                            override fun onStyle(css: String) {
                                runOnUiThread {
                                    if (!isDestroyed) {
                                        RichContentWebView.applyStyle(richContentWebView, css)
                                    }
                                }
                            }
                            override fun onLayout(json: String) {
                                runOnUiThread {
                                    if (!isDestroyed) {
                                        applyLayout(json)
                                    }
                                }
                            }
                            override fun onAudio(text: String) {
                                runOnUiThread {
                                    if (!isDestroyed && text.isNotBlank()) {
                                        speakText(text)
                                    }
                                }
                            }
                            override fun onControlPayload(payload: String) {
                                runOnUiThread {
                                    if (!isDestroyed && payload.isNotBlank()) {
                                        applyPushPayload(payload)
                                    }
                                }
                            }
                        })
                    } catch (e: Exception) {
                        e.message?.takeIf { it.isNotBlank() } ?: getString(R.string.error_generic)
                    }
                }
            } catch (t: Throwable) {
                if (t is CancellationException) throw t
                Log.e(TAG, "askStreaming failed", t)
                t.message?.takeIf { it.isNotBlank() } ?: getString(R.string.error_generic)
            }
            runOnUiThread {
                if (!isDestroyed) {
                    pendingStreamUpdate?.let { streamUpdateHandler.removeCallbacks(it) }
                    pendingStreamUpdate = null
                    val last = chatMessages.lastIndex
                    if (last >= 0 && !chatMessages[last].isUser) {
                        if (result.startsWith("Error:")) {
                            val displayResult = result.removePrefix("Error: ").trim()
                            showError(displayResult)
                            chatMessages.removeAt(last)
                            if (result.contains("warming up") || result.contains("still loading")) {
                                Toast.makeText(this@MainActivity, getString(R.string.error_model_loading), Toast.LENGTH_LONG).show()
                            }
                        } else {
                            hideError()
                            if (result.contains(Regex("(?i)app\\s+clear"))) {
                                reinitializeDisplay()
                            } else {
                                chatMessages[last] = ChatMessage(result, isUser = false)
                                updateRichContentIfHtml(result)
                                speakIfEnabled(result)
                            }
                        }
                    }
                    renderChat()
                    askBtn.isEnabled = true
                    menuBtn.isEnabled = true
                    scrollToBottom()
                }
            }
        }
    }

    private fun speakIfEnabled(text: String) {
        val cleaned = text.replace(Regex("(?i)app\\s+clear[?!.\\s,]*"), "").trim()
        if (cleaned.isNotEmpty() && isSpeakEnabled()) speakText(cleaned)
    }

    private fun speakText(text: String) {
        val toSpeak = text.trim()
        if (toSpeak.isNotEmpty()) {
            tts?.speak(toSpeak, TextToSpeech.QUEUE_FLUSH, null, "ember_tts")
        }
    }

    override fun onDestroy() {
        tts?.stop()
        tts?.shutdown()
        super.onDestroy()
    }
}
