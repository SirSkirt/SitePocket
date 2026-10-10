package __PACKAGE__

import android.content.Context
import android.util.Log
import org.json.JSONObject
import java.io.File
import java.util.UUID

/**
 * Small native-only, bounded event journal for WebCask's Android player.
 * Unlike injected scripts, this logger works before JavaScript or a WebView exists.
 * Stored in the app sandbox; no network calls or game-to-native JavaScript bridge.
 */
object WebCaskNativeLog {
    const val FILE_NAME = "webcask-native-webview.jsonl"
    const val PREFS = "webcask-native-debug"
    private val mutex = Any()
    private const val MAX_BYTES = 160 * 1024
    private const val KEEP_BYTES = 110 * 1024

    fun write(
        context: Context, severity: String, code: String,
        message: String, detail: String = ""
    ) {
        val line = JSONObject().apply {
            put("id", UUID.randomUUID().toString())
            put("time", System.currentTimeMillis())
            put("appId", context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
                .getString("activeGameId", "") ?: "")
            put("severity", severity)
            put("source", "native-webview")
            put("code", code.take(72))
            put("message", message.take(450))
            put("detail", detail.take(1100))
        }.toString()
        // The private report contains no cookies, request bodies, or full URLs.
        val level = when (severity) {
            "error" -> Log.ERROR
            "warning" -> Log.WARN
            else -> Log.INFO
        }
        Log.println(level, "WebCaskNative", (code + ": " + message).take(700))
        try {
            synchronized(mutex) {
                val file = File(context.filesDir, FILE_NAME)
                if (file.length() > MAX_BYTES) {
                    val tail = file.readText(Charsets.UTF_8).takeLast(KEEP_BYTES)
                    val safe = tail.substringAfter('\n', "")
                    file.writeText(safe, Charsets.UTF_8)
                }
                file.appendText(line + "\n", Charsets.UTF_8)
            }
        } catch (err: Exception) {
            Log.e("WebCaskNative", "Could not persist native diagnostics", err)
        }
    }
}
