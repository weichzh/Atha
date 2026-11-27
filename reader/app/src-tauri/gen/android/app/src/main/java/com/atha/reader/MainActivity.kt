package com.atha.reader

import android.os.Bundle
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.graphics.Insets
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.core.view.ViewCompat

private class SystemBarsBridge(private val activity: MainActivity) {
  @Volatile
  private var safeAreaInsets = "{\"top\":0,\"right\":0,\"bottom\":0,\"left\":0}"

  fun updateSafeAreaInsets(insets: WindowInsetsCompat) {
    val safe = Insets.max(
      insets.getInsetsIgnoringVisibility(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
      ),
      insets.getInsets(WindowInsetsCompat.Type.systemGestures()),
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
      // Edge-to-edge leaves IME avoidance to the host, not the WebView page.
      val imeBottom = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
      val layout = webView.layoutParams as? ViewGroup.MarginLayoutParams
        ?: return@setOnApplyWindowInsetsListener insets
      if (layout.bottomMargin != imeBottom) {
        layout.bottomMargin = imeBottom
        webView.layoutParams = layout
      }
      systemBars.updateSafeAreaInsets(insets)
      webView.evaluateJavascript(
        "globalThis.dispatchEvent(new Event('atha-safe-area-change'))",
        null,
      )
      WindowInsetsCompat.Builder(insets)
        .setInsets(WindowInsetsCompat.Type.ime(), Insets.NONE)
        .build()
    }
    webView.addJavascriptInterface(systemBars, "AthaSystemBars")
    ViewCompat.requestApplyInsets(webView)
  }
}
