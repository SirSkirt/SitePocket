package dev.sitepocket.zipimport

import android.app.Activity
import android.net.Uri
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.util.UUID

@InvokeArg
internal class StageArgs {
    lateinit var uri: String
}

/**
 * Copy a ZIP chosen in Android Files (Storage Access Framework) into this
 * app's private cache. No broad storage permission is needed.
 *
 * The JavaScript frontend can only call the Rust import command; the native
 * bridge is intentionally registered without JS-facing command permissions.
 */
@TauriPlugin
class ZipImportPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun stageZip(invoke: Invoke) {
        val args = try { invoke.parseArgs(StageArgs::class.java) }
        catch (err: Exception) {
            invoke.reject("Invalid Android Files selection: ${err.message}")
            return
        }
        val uri = Uri.parse(args.uri)
        if (uri.scheme != "content") {
            invoke.reject("Select the ZIP using the Android Files picker.")
            return
        }
        Thread {
            val destination = File(activity.cacheDir, "sitepocket-import-${UUID.randomUUID()}.zip")
            try {
                val stream = activity.contentResolver.openInputStream(uri)
                    ?: throw IllegalStateException("Android Files returned no file data")
                var total = 0L
                stream.use { input ->
                    destination.outputStream().use { output ->
                        val buffer = ByteArray(128 * 1024)
                        while (true) {
                            val read = input.read(buffer)
                            if (read < 0) break
                            total += read
                            if (total > 1024L * 1024 * 1024)
                                throw IllegalArgumentException("ZIP exceeds the 1 GiB import limit")
                            output.write(buffer, 0, read)
                        }
                    }
                }
                if (total == 0L) throw IllegalArgumentException("The selected ZIP is empty")
                invoke.resolve(JSObject().apply {
                    put("path", destination.canonicalPath)
                })
            } catch (err: Exception) {
                destination.delete()
                invoke.reject("Cannot read the ZIP from Android Files: ${err.message}")
            }
        }.start()
    }
}
