package com.atha.reader

import android.content.Context
import android.graphics.Rect
import android.os.BatteryManager
import android.os.Bundle
import android.view.ActionMode
import android.view.Menu
import android.view.MenuItem
import android.view.View
import android.view.ViewGroup
import android.webkit.WebView
import android.widget.FrameLayout
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.webkit.JavaScriptReplyProxy
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature

private const val APP_ORIGIN = "https://tauri.localhost"

private class SystemBarsBridge(private val activity: MainActivity, private val host: ReaderWebViewHost) {
  private var safe = Insets.NONE
  private var imeVisible = false
  private var reply: JavaScriptReplyProxy? = null

  fun onMessage(message: String, proxy: JavaScriptReplyProxy) {
    when (message) {
      "subscribe" -> {
        reply = proxy
        host.setBodySelection(false)
        postInsets()
      }
      "selection:body" -> host.setBodySelection(true)
      "selection:none" -> host.setBodySelection(false)
      "battery:read" -> {
        val level = activity.getSystemService(BatteryManager::class.java)
          ?.getIntProperty(BatteryManager.BATTERY_PROPERTY_CAPACITY)
          ?.takeIf { it in 0..100 }
        proxy.postMessage("""{"batteryPercent":$level}""")
      }
      "bars:visible:light", "bars:visible:dark",
      "bars:immersive:light", "bars:immersive:dark" -> {
        val immersive = message.startsWith("bars:immersive:")
        val dark = message.endsWith(":dark")
        WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
          systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
          isAppearanceLightStatusBars = !dark
          isAppearanceLightNavigationBars = !dark
          if (immersive) hide(WindowInsetsCompat.Type.statusBars())
          else show(WindowInsetsCompat.Type.statusBars())
          show(WindowInsetsCompat.Type.navigationBars())
        }
      }
    }
  }

  fun updateInsets(nextSafe: Insets, nextImeVisible: Boolean) {
    if (safe == nextSafe && imeVisible == nextImeVisible) return
    safe = nextSafe
    imeVisible = nextImeVisible
    postInsets()
  }

  private fun postInsets() {
    reply?.postMessage(
      """{"top":${safe.top},"right":${safe.right},"bottom":${safe.bottom},"left":${safe.left},"imeVisible":$imeVisible}""",
    )
  }
}

// Intercept only this WebView's floating modes, before the decor creates the mode.
// Returning null from Activity.onWindowStartingActionMode would leave the original callback in use.
private class ReaderWebViewHost(context: Context) : FrameLayout(context) {
  private var bodySelection = false
  private var floatingMode: ActionMode? = null

  fun setBodySelection(selected: Boolean) {
    if (bodySelection == selected) return
    bodySelection = selected
    floatingMode?.invalidate()
  }

  override fun startActionModeForChild(child: View, callback: ActionMode.Callback, type: Int): ActionMode? {
    if (type != ActionMode.TYPE_FLOATING) return super.startActionModeForChild(child, callback, type)
    val wrapped = object : ActionMode.Callback2() {
      private val hidden = mutableListOf<MenuItem>()

      private fun hideItems(menu: Menu): Boolean {
        if (!bodySelection) return false
        for (index in 0 until menu.size()) {
          val item = menu.getItem(index)
          if (item.isVisible) {
            hidden.add(item)
            item.isVisible = false
          }
        }
        return hidden.isNotEmpty()
      }

      override fun onCreateActionMode(mode: ActionMode, menu: Menu): Boolean {
        if (!callback.onCreateActionMode(mode, menu)) return false
        floatingMode = mode
        hideItems(menu)
        return true
      }

      override fun onPrepareActionMode(mode: ActionMode, menu: Menu): Boolean {
        val hadHiddenItems = hidden.isNotEmpty()
        hidden.forEach { it.isVisible = true }
        hidden.clear()
        val prepared = callback.onPrepareActionMode(mode, menu)
        return hideItems(menu) || hadHiddenItems || prepared
      }

      override fun onActionItemClicked(mode: ActionMode, item: MenuItem): Boolean =
        callback.onActionItemClicked(mode, item)

      override fun onDestroyActionMode(mode: ActionMode) {
        if (floatingMode === mode) floatingMode = null
        callback.onDestroyActionMode(mode)
      }

      override fun onGetContentRect(mode: ActionMode, view: View, outRect: Rect) {
        if (callback is ActionMode.Callback2) callback.onGetContentRect(mode, view, outRect)
        else super.onGetContentRect(mode, view, outRect)
      }
    }
    return super.startActionModeForChild(child, wrapped, type)
  }
}

class MainActivity : TauriActivity() {
  private var attachedWebView: WebView? = null
  private var webViewHost: ReaderWebViewHost? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    val backCallback = object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val webView = attachedWebView
        if (webView == null) {
          isEnabled = false
          onBackPressedDispatcher.onBackPressed()
          isEnabled = true
          return
        }
        webView.evaluateJavascript(
          "(function(){ try { return Boolean(globalThis.__athaHandleBack && globalThis.__athaHandleBack()); } catch (e) { return false; } })()",
        ) { raw ->
          if (raw != "true") {
            isEnabled = false
            onBackPressedDispatcher.onBackPressed()
            isEnabled = true
          }
        }
      }
    }
    onBackPressedDispatcher.addCallback(this, backCallback)
  }

  override fun onWebViewCreate(webView: WebView) {
    attachedWebView = webView
    val host = ReaderWebViewHost(this)
    webViewHost = host
    val bridge = SystemBarsBridge(this, host)
    val hasMessageBridge = WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)
    if (hasMessageBridge) {
      WebViewCompat.addWebMessageListener(webView, "AthaSystemBars", setOf(APP_ORIGIN)) {
          view, message, _, isMainFrame, reply ->
        if (view === attachedWebView && isMainFrame && message.type == WebMessageCompat.TYPE_STRING) {
          message.data?.let { bridge.onMessage(it, reply) }
        }
      }
    }

    ViewCompat.setOnApplyWindowInsetsListener(webView) { _, insets ->
      val ime = WindowInsetsCompat.Type.ime()
      val imeBottom = insets.getInsets(ime).bottom
      val safe = Insets.max(
        insets.getInsetsIgnoringVisibility(
          WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout(),
        ),
        insets.getInsets(WindowInsetsCompat.Type.systemGestures()),
      )
      // Wry uses the host's full viewport. Fit the WebView to the keyboard here, once.
      // Old WebViews without an origin-scoped bridge instead get native safe margins.
      val margins = if (hasMessageBridge) Insets.of(0, 0, 0, imeBottom)
        else Insets.of(safe.left, safe.top, safe.right, maxOf(safe.bottom, imeBottom))
      val layout = webView.layoutParams as FrameLayout.LayoutParams
      if (layout.leftMargin != margins.left || layout.topMargin != margins.top ||
          layout.rightMargin != margins.right || layout.bottomMargin != margins.bottom) {
        layout.setMargins(margins.left, margins.top, margins.right, margins.bottom)
        webView.layoutParams = layout
      }
      bridge.updateInsets(
        Insets.of(safe.left, safe.top, safe.right, maxOf(0, safe.bottom - imeBottom)),
        insets.isVisible(ime),
      )
      WindowInsetsCompat.Builder(insets)
        .setInsets(ime, Insets.NONE)
        .setVisible(ime, false)
        .build()
    }
  }

  override fun setContentView(view: View?) {
    // Wry calls onWebViewCreate, then hands this WebView to setContentView.
    val host = webViewHost
    if (view == null || view !== attachedWebView || host == null) {
      super.setContentView(view)
      return
    }
    host.addView(view, ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT)
    super.setContentView(host)
    ViewCompat.requestApplyInsets(view)
  }
}
