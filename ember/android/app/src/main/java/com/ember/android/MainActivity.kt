package com.ember.android

import android.content.Intent
import android.os.Bundle
import android.speech.RecognizerIntent
import android.widget.Button
import android.widget.EditText
import android.widget.ImageButton
import android.widget.TextView
import android.widget.Toast
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.core.content.ContextCompat
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : AppCompatActivity() {

    private lateinit var serverInput: EditText
    private lateinit var promptInput: EditText
    private lateinit var askBtn: Button
    private lateinit var micBtn: ImageButton
    private lateinit var resultText: TextView

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
        resultText = findViewById(R.id.result_text)

        serverInput.setText(getString(R.string.default_server_address), TextView.BufferType.EDITABLE)
        promptInput.hint = getString(R.string.prompt_hint)

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

    private fun askAi(addr: String, prompt: String) {
        askBtn.isEnabled = false
        resultText.text = getString(R.string.asking)

        lifecycleScope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    EmberClient.askStreaming(addr, prompt, TokenCallback { token ->
                        runOnUiThread {
                            if (resultText.text == getString(R.string.asking)) {
                                resultText.text = ""
                            }
                            resultText.append(token)
                        }
                    })
                } catch (e: Exception) {
                    "Error: ${e.message}"
                }
            }
            runOnUiThread {
                resultText.text = result
                askBtn.isEnabled = true
            }
        }
    }
}
