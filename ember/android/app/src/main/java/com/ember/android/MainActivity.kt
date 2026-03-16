package com.ember.android

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.speech.tts.TextToSpeech
import android.speech.RecognizerIntent
import android.util.Log
import android.webkit.WebView
import android.widget.Button
import android.widget.EditText
import android.widget.ImageButton
import android.widget.Switch
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
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
        private const val STREAM_UPDATE_DELAY_MS = 80L  // Batch tokens for smoother display
    }

    private lateinit var serverInput: EditText
    private lateinit var promptInput: EditText
    private lateinit var askBtn: Button
    private lateinit var checkInBtn: Button
    private lateinit var micBtn: ImageButton
    private lateinit var speakSwitch: Switch
    private lateinit var chatWebView: WebView
    private lateinit var locationBtn: Button
    private lateinit var cameraBtn: Button
    private lateinit var saveBtn: Button

    private val chatMessages = mutableListOf<ChatMessage>()
    private var chatCss = ChatWebView.DEFAULT_CSS
    private var hasFetchedStyle = false

    private var tts: TextToSpeech? = null

    private val streamUpdateHandler = Handler(Looper.getMainLooper())
    private var pendingStreamUpdate: Runnable? = null

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

        Log.i(DIAG, "MainActivity - findViewById")
        serverInput = findViewById(R.id.server_address)
        promptInput = findViewById(R.id.prompt_input)
        askBtn = findViewById(R.id.ask_btn)
        checkInBtn = findViewById(R.id.check_in_btn)
        micBtn = findViewById(R.id.mic_btn)
        speakSwitch = findViewById(R.id.speak_switch)
        chatWebView = findViewById(R.id.chat_webview)
        locationBtn = findViewById(R.id.location_btn)
        cameraBtn = findViewById(R.id.camera_btn)
        saveBtn = findViewById(R.id.save_btn)
        Log.i(DIAG, "MainActivity - all findViewById done")

        serverInput.setText(getString(R.string.default_server_address), android.widget.TextView.BufferType.EDITABLE)
        promptInput.hint = getString(R.string.prompt_hint)

        speakSwitch.isChecked = getSharedPreferences("ember", MODE_PRIVATE).getBoolean(PREF_SPEAK_RESPONSES, true)
        speakSwitch.setOnCheckedChangeListener { _, checked ->
            getSharedPreferences("ember", MODE_PRIVATE).edit().putBoolean(PREF_SPEAK_RESPONSES, checked).apply()
        }
        Log.i(DIAG, "MainActivity - prefs/speakSwitch done")

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
        savedInstanceState?.getStringArray(KEY_CHAT_TEXTS)?.let { texts ->
            savedInstanceState.getBooleanArray(KEY_CHAT_IS_USER)?.let { isUser ->
                if (texts.size == isUser.size) {
                    chatMessages.clear()
                    chatMessages.addAll(texts.indices.map { i -> ChatMessage(texts[i], isUser[i]) })
                }
            }
        }
        renderChat()

        micBtn.setOnClickListener {
            when {
                ContextCompat.checkSelfPermission(this, android.Manifest.permission.RECORD_AUDIO) ==
                    android.content.pm.PackageManager.PERMISSION_GRANTED -> startVoiceInput()
                else -> permissionLauncher.launch(android.Manifest.permission.RECORD_AUDIO)
            }
        }

        askBtn.setOnClickListener {
            val addr = serverInput.text.toString().trim()
            val prompt = promptInput.text.toString().trim()
            when {
                addr.isEmpty() -> Toast.makeText(this, getString(R.string.error_server), Toast.LENGTH_SHORT).show()
                prompt.isEmpty() -> Toast.makeText(this, getString(R.string.error_prompt), Toast.LENGTH_SHORT).show()
                else -> askAi(addr, prompt, prompt)
            }
        }

        checkInBtn.setOnClickListener {
            val addr = serverInput.text.toString().trim()
            when {
                addr.isEmpty() -> Toast.makeText(this, getString(R.string.error_server), Toast.LENGTH_SHORT).show()
                else -> askAi(addr, "__check_in__", getString(R.string.check_in))
            }
        }

        locationBtn.setOnClickListener {
            when {
                ContextCompat.checkSelfPermission(this, Manifest.permission.ACCESS_FINE_LOCATION) == PackageManager.PERMISSION_GRANTED -> onLocationRequested()
                else -> multiPermissionLauncher.launch(arrayOf(Manifest.permission.ACCESS_FINE_LOCATION, Manifest.permission.ACCESS_COARSE_LOCATION))
            }
        }

        cameraBtn.setOnClickListener {
            when {
                ContextCompat.checkSelfPermission(this, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED -> launchCamera()
                else -> cameraPermissionLauncher.launch(Manifest.permission.CAMERA)
            }
        }

        saveBtn.setOnClickListener { saveChatToDisk() }
        Log.i(DIAG, "MainActivity.onCreate DONE")
    }

    private fun onLocationRequested() {
        val loc = DeviceCapabilities.getLastLocation(this)
        if (loc != null) {
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
        checkInBtn.isEnabled = false

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
                        EmberClient.askStreaming(addr, prompt, TokenCallback { token ->
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
                                            scrollToBottom()
                                        }
                                    }
                                }
                                pendingStreamUpdate = null
                            }
                            streamUpdateHandler.postDelayed(pendingStreamUpdate!!, STREAM_UPDATE_DELAY_MS)
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
                        val displayResult = if (result.startsWith("Error: ")) result.removePrefix("Error: ") else result
                        chatMessages[last] = ChatMessage(displayResult, isUser = false)
                    }
                    renderChat()
                    askBtn.isEnabled = true
                    checkInBtn.isEnabled = true
                    scrollToBottom()
                    if (result.startsWith("Error:") && (result.contains("warming up") || result.contains("still loading"))) {
                        Toast.makeText(this@MainActivity, getString(R.string.error_model_loading), Toast.LENGTH_LONG).show()
                    }
                    if (!result.startsWith("Error:")) speakIfEnabled(result)
                }
            }
        }
    }

    private fun speakIfEnabled(text: String) {
        if (speakSwitch.isChecked) speakText(text)
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
