package com.atha.reader

import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.core.view.ViewCompat

private class SystemBarsBridge(private val activity: MainActivity) {
  @Volatile
  private var safeAreaInsets = "{\"top\":0,\"right\":0,\"bottom\":0,\"left\":0}"

  fun updateSafeAreaInsets(insets: WindowInsetsCompat) {
    val safe = insets.getInsetsIgnoringVisibility(
      WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
    )
    safeAreaInsets =
      "{\"top\":${safe.top},\"right\":${safe.right},\"bottom\":${safe.bottom},\"left\":${safe.left}}"
  }

  @JavascriptInterface
  fun setReadingMode(reading: Boolean, dark: Boolean) {
    activity.runOnUiThread {
      WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
        systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        isAppearanceLightStatusBars = !dark
        isAppearanceLightNavigationBars = !dark
        if (reading) hide(WindowInsetsCompat.Type.statusBars())
        else show(WindowInsetsCompat.Type.statusBars())
        show(WindowInsetsCompat.Type.navigationBars())
      }
    }
  }

  @JavascriptInterface
  fun getSafeAreaInsets(): String = safeAreaInsets
}

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  override fun onWebViewCreate(webView: WebView) {
    val systemBars = SystemBarsBridge(this)
    ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
      systemBars.updateSafeAreaInsets(insets)
      webView.evaluateJavascript(
        "globalThis.dispatchEvent(new Event('atha-safe-area-change'))",
        null,
      )
      insets
    }
    webView.addJavascriptInterface(systemBars, "AthaSystemBars")
    ViewCompat.requestApplyInsets(webView)
  }
}
