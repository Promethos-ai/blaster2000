package com.ember.android

import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.appcompat.app.AppCompatActivity

class SplashActivity : AppCompatActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        Log.i("EmberDiag", "SplashActivity.onCreate START")
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_splash)
        Log.i("EmberDiag", "SplashActivity.onCreate - layout inflated, scheduling MainActivity in 1500ms")

        Handler(Looper.getMainLooper()).postDelayed({
            Log.i("EmberDiag", "SplashActivity - starting MainActivity")
            try {
                startActivity(Intent(this, MainActivity::class.java))
                finish()
                Log.i("EmberDiag", "SplashActivity - MainActivity started, finish() called")
            } catch (t: Throwable) {
                Log.e("EmberDiag", "SplashActivity - failed to start MainActivity", t)
                throw t
            }
        }, 1500)
    }
}
