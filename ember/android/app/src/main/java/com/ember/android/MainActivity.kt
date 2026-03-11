package com.ember.android

import android.content.Intent
import android.os.Bundle
import android.speech.RecognizerIntent
import android.widget.Button
import android.widget.EditText
import android.widget.ImageButton
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import androidx.recyclerview.widget.LinearLayoutManager
import androidx.recyclerview.widget.RecyclerView
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : AppCompatActivity() {

    companion object {
        private const val KEY_CHAT_TEXTS = "chat_texts"
        private const val KEY_CHAT_IS_USER = "chat_is_user"
    }

    private lateinit var serverInput: EditText
    private lateinit var promptInput: EditText
    private lateinit var askBtn: Button
    private lateinit var micBtn: ImageButton
    private lateinit var chatRecycler: RecyclerView
    private lateinit var chatAdapter: ChatAdapter

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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_main)

        serverInput = findViewById(R.id.server_address)
        promptInput = findViewById(R.id.prompt_input)
        askBtn = findViewById(R.id.ask_btn)
        micBtn = findViewById(R.id.mic_btn)
        chatRecycler = findViewById(R.id.chat_recycler)

        serverInput.setText(getString(R.string.default_server_address), android.widget.TextView.BufferType.EDITABLE)
        promptInput.hint = getString(R.string.prompt_hint)

        chatAdapter = ChatAdapter()
        savedInstanceState?.getStringArray(KEY_CHAT_TEXTS)?.let { texts ->
            savedInstanceState.getBooleanArray(KEY_CHAT_IS_USER)?.let { isUser ->
                if (texts.size == isUser.size) {
                    val restored = texts.indices.map { i ->
                        ChatMessage(texts[i], isUser[i])
                    }
                    chatAdapter.restoreMessages(restored)
                }
            }
        }
        chatRecycler.layoutManager = LinearLayoutManager(this).apply {
            reverseLayout = true
            stackFromEnd = true
        }
        chatRecycler.adapter = chatAdapter

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
                else -> askAi(addr, prompt)
            }
        }
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        val msgs = chatAdapter.getMessages()
        outState.putStringArray(KEY_CHAT_TEXTS, msgs.map { it.text }.toTypedArray())
        outState.putBooleanArray(KEY_CHAT_IS_USER, msgs.map { it.isUser }.toBooleanArray())
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

    private fun scrollToBottom() {
        chatRecycler.post {
            if (chatAdapter.itemCount > 0) {
                chatRecycler.smoothScrollToPosition(chatAdapter.itemCount - 1)
            }
        }
    }

    private fun askAi(addr: String, prompt: String) {
        promptInput.setText("")
        askBtn.isEnabled = false

        chatAdapter.addUserMessage(prompt)
        chatAdapter.addAiMessage(getString(R.string.asking))
        scrollToBottom()

        lifecycleScope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    EmberClient.askStreaming(addr, prompt, TokenCallback { token ->
                        runOnUiThread {
                            chatAdapter.appendTokenToLastAi(token, getString(R.string.asking))
                            scrollToBottom()
                        }
                    })
                } catch (e: Exception) {
                    "Error: ${e.message}"
                }
            }
            runOnUiThread {
                chatAdapter.updateLastAiMessage(result)
                askBtn.isEnabled = true
                scrollToBottom()
            }
        }
    }
}
