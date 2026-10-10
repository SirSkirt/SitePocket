package __PACKAGE__

import android.net.Uri
import android.os.Bundle
import android.os.Message
import android.os.SystemClock
import android.view.View
import android.webkit.ConsoleMessage
import android.webkit.JsPromptResult
import android.webkit.JsResult
import android.webkit.PermissionRequest
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebView
import android.webkit.WebViewRenderProcess
import android.webkit.WebViewRenderProcessClient
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** Native diagnostics work before injected scripts or the WebView even exist. */
class WebCaskActivity : TauriActivity() {
    @Volatile private var alive = true
    @Volatile private var foreground = true
    @Volatile private var lastMainPulse = SystemClock.elapsedRealtime()
    @Volatile private var mainStallReported = false
    private val watchdog = Executors.newSingleThreadScheduledExecutor { task ->
        Thread(task, "WebCaskNativeWatchdog").apply { isDaemon = true }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        WebCaskNativeLog.write(this, "info", "native-activity-create-requested",
            "Android started WebCaskActivity", "Entering TauriActivity.onCreate")
        // This worker still records a freeze if Android's main/UI thread blocks
        // before TauriActivity.onCreate returns.
        watchdog.scheduleAtFixedRate({
            val elapsed = SystemClock.elapsedRealtime() - lastMainPulse
            if (alive && foreground && elapsed > 12000 && !mainStallReported) {
                mainStallReported = true
                WebCaskNativeLog.write(this, "error", "native-ui-thread-stalled",
                    "Android WebCask Activity UI thread stopped responding",
                    "No main-thread heartbeat for " + elapsed + " ms. Activity startup or rendering may be blocked.")
            }
        }, 6, 4, TimeUnit.SECONDS)
        super.onCreate(savedInstanceState)
        WebCaskNativeLog.write(this, "info", "native-activity-created",
            "TauriActivity.onCreate completed")
        val ping = object : Runnable {
            override fun run() {
                if (!alive) return
                if (mainStallReported) {
                    WebCaskNativeLog.write(this@WebCaskActivity, "info",
                        "native-ui-thread-recovered", "Android Activity UI thread became responsive")
                }
                mainStallReported = false
                lastMainPulse = SystemClock.elapsedRealtime()
                window?.decorView?.postDelayed(this, 1500)
            }
        }
        window?.decorView?.post(ping)
    }

    override fun onWebViewCreate(webView: WebView) {
        super.onWebViewCreate(webView)
        WebCaskNativeLog.write(this, "info", "native-webview-created",
            "Android WebView object created", "No game JavaScript required.")
        val priorRenderer = webView.webViewRenderProcessClient
        webView.webViewRenderProcessClient = object : WebViewRenderProcessClient() {
            override fun onRenderProcessUnresponsive(view: WebView, renderer: WebViewRenderProcess?) {
                WebCaskNativeLog.write(this@WebCaskActivity, "error", "native-renderer-unresponsive",
                    "Android WebView renderer is unresponsive", "Reported by Android's native WebView renderer watchdog.")
                priorRenderer?.onRenderProcessUnresponsive(view, renderer)
            }
            override fun onRenderProcessResponsive(view: WebView, renderer: WebViewRenderProcess?) {
                WebCaskNativeLog.write(this@WebCaskActivity, "info", "native-renderer-responsive",
                    "Android WebView renderer recovered")
                priorRenderer?.onRenderProcessResponsive(view, renderer)
            }
        }
        var lastProgressBucket = -1
        val observer = object : Runnable {
            override fun run() {
                if (!alive) return
                val current = webView.webChromeClient
                if (current != null && current !is WebCaskConsoleClient) {
                    webView.webChromeClient = WebCaskConsoleClient(current, this@WebCaskActivity)
                    WebCaskNativeLog.write(this@WebCaskActivity, "info",
                        "native-console-hook-installed", "Native console logger attached")
                }
                val bucket = webView.progress / 25
                if (bucket != lastProgressBucket) {
                    lastProgressBucket = bucket
                    WebCaskNativeLog.write(this@WebCaskActivity, "info",
                        "native-webview-progress", "WebView loading progress",
                        "Progress: " + webView.progress + "%")
                }
                webView.postDelayed(this, 1750)
            }
        }
        webView.postDelayed(observer, 100)
    }

    override fun onResume() {
        super.onResume()
        foreground = true
        lastMainPulse = SystemClock.elapsedRealtime()
        WebCaskNativeLog.write(this, "info", "native-activity-resumed",
            "WebCask Activity resumed")
    }

    override fun onPause() {
        foreground = false
        WebCaskNativeLog.write(this, "info", "native-activity-paused",
            "WebCask Activity paused")
        super.onPause()
    }

    override fun onDestroy() {
        alive = false
        watchdog.shutdownNow()
        WebCaskNativeLog.write(this, "info", "native-activity-destroyed",
            "WebCask Activity closing")
        super.onDestroy()
    }
}

/**
 * Preserve the existing Tauri ChromeClient for permissions, file selection,
 * popup windows, fullscreen media and JavaScript dialogs.
 */
private class WebCaskConsoleClient(
    private val previous: WebChromeClient,
    private val activity: WebCaskActivity
) : WebChromeClient() {
    override fun onConsoleMessage(message: ConsoleMessage): Boolean {
        val level = message.messageLevel()
        if (level == ConsoleMessage.MessageLevel.ERROR ||
            level == ConsoleMessage.MessageLevel.WARNING) {
            val severity = if (level == ConsoleMessage.MessageLevel.ERROR) "error" else "warning"
            val location = try {
                Uri.parse(message.sourceId()).buildUpon().clearQuery()
                    .fragment(null).build().toString().take(200)
            } catch (_: Exception) { "unknown source" }
            WebCaskNativeLog.write(activity, severity, "native-console-" + severity,
                "WebView console " + severity,
                "Line " + message.lineNumber() + " (" + location + "): " + message.message().take(600))
        }
        return previous.onConsoleMessage(message)
    }
    override fun onProgressChanged(view: WebView, newProgress: Int) =
        previous.onProgressChanged(view, newProgress)
    override fun onReceivedTitle(view: WebView, title: String) =
        previous.onReceivedTitle(view, title)
    override fun onPermissionRequest(request: PermissionRequest) =
        previous.onPermissionRequest(request)
    override fun onPermissionRequestCanceled(request: PermissionRequest) =
        previous.onPermissionRequestCanceled(request)
    override fun onGeolocationPermissionsShowPrompt(origin: String, callback: android.webkit.GeolocationPermissions.Callback) =
        previous.onGeolocationPermissionsShowPrompt(origin, callback)
    override fun onGeolocationPermissionsHidePrompt() =
        previous.onGeolocationPermissionsHidePrompt()
    override fun onJsAlert(view: WebView, url: String, message: String, result: JsResult): Boolean =
        previous.onJsAlert(view, url, message, result)
    override fun onJsConfirm(view: WebView, url: String, message: String, result: JsResult): Boolean =
        previous.onJsConfirm(view, url, message, result)
    override fun onJsPrompt(view: WebView, url: String, message: String,
                            defaultValue: String, result: JsPromptResult): Boolean =
        previous.onJsPrompt(view, url, message, defaultValue, result)
    override fun onJsBeforeUnload(view: WebView, url: String, message: String, result: JsResult): Boolean =
        previous.onJsBeforeUnload(view, url, message, result)
    override fun onShowFileChooser(webView: WebView,
                                   filePathCallback: ValueCallback<Array<Uri>>,
                                   fileChooserParams: FileChooserParams): Boolean =
        previous.onShowFileChooser(webView, filePathCallback, fileChooserParams)
    override fun onCreateWindow(view: WebView, isDialog: Boolean,
                                isUserGesture: Boolean, resultMsg: Message): Boolean =
        previous.onCreateWindow(view, isDialog, isUserGesture, resultMsg)
    override fun onCloseWindow(window: WebView) =
        previous.onCloseWindow(window)
    override fun onShowCustomView(view: View, callback: CustomViewCallback) =
        previous.onShowCustomView(view, callback)
    override fun onHideCustomView() =
        previous.onHideCustomView()
}
