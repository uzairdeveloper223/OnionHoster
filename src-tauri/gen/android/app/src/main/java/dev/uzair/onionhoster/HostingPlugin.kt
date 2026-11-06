package dev.uzair.onionhoster

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.ByteArrayOutputStream
import java.util.UUID
import java.net.HttpURLConnection
import java.net.URL

@InvokeArg
class HostingArgs { var active: Boolean = false }
@InvokeArg
class SourceArgs { lateinit var uri: String; var directory: Boolean = false }
@InvokeArg
class PickerArgs { var directory: Boolean = false }
@InvokeArg
class DocumentArgs { lateinit var uri: String; var write: Boolean = false }
@InvokeArg
class ReleaseArgs { lateinit var url: String }

@TauriPlugin
class HostingPlugin(private val activity: Activity) : Plugin(activity) {
    private val runtimeDir by lazy {
        val root = File(activity.filesDir, "runtime")
        val stamp = File(root, "bundle-version")
        if (!stamp.isFile || stamp.readText() != BuildConfig.VERSION_NAME) {
            copyAssets("runtime", root)
            stamp.writeText(BuildConfig.VERSION_NAME)
        }
        root
    }

    private fun copyAssets(source: String, target: File) {
        val entries = activity.assets.list(source) ?: emptyArray()
        if (entries.isEmpty()) {
            target.parentFile?.mkdirs()
            activity.assets.open(source).use { input ->
                target.outputStream().use { input.copyTo(it) }
            }
        } else {
            target.mkdirs()
            entries.forEach { copyAssets("$source/$it", File(target, it)) }
        }
    }

    @Command
    fun runtimePaths(invoke: Invoke) {
        Thread {
            try {
                invoke.resolve(JSObject().apply {
                    put("nativeDir", activity.applicationInfo.nativeLibraryDir)
                    put("dataDir", runtimeDir.absolutePath)
                    put("sitesDir", File(activity.filesDir, "sites").absolutePath)
                })
            } catch (error: Exception) { invoke.reject(error.message ?: "Could not prepare the runtime.") }
        }.start()
    }

    @Command
    fun setHosting(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(HostingArgs::class.java)
            val intent = Intent(activity, HostingService::class.java)
            if (args.active) {
                if (Build.VERSION.SDK_INT >= 26) activity.startForegroundService(intent)
                else activity.startService(intent)
                if (Build.VERSION.SDK_INT >= 33 &&
                    activity.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
                    activity.requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 401)
                }
            } else {
                activity.stopService(intent)
            }
            invoke.resolve()
        } catch (error: Exception) { invoke.reject(error.message ?: "Could not update background hosting.") }
    }

    @Command
    fun openDocument(invoke: Invoke) {
        Thread { try {
            val args = invoke.parseArgs(DocumentArgs::class.java)
            val uri = Uri.parse(args.uri)
            require(uri.scheme == "content") { "Choose a document with the Android file picker." }
            val descriptor = activity.contentResolver.openFileDescriptor(uri, if (args.write) "wt" else "r")
                ?: error("The document provider did not return a file.")
            invoke.resolve(JSObject().apply { put("fd", descriptor.detachFd()) })
            descriptor.close()
        } catch (error: Exception) { invoke.reject(error.message ?: "Could not open the document.") } }.start()
    }

    @Command
    fun chooseSource(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(PickerArgs::class.java)
            val intent = if (args.directory) Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
                else Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "text/html"
                }
            startActivityForResult(invoke, intent, "sourceResult")
        } catch (error: Exception) { invoke.reject(error.message ?: "Could not open the file picker.") }
    }

    @ActivityCallback
    fun sourceResult(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(JSObject().apply { put("path", org.json.JSONObject.NULL) })
            return
        }
        val args = SourceArgs().apply {
            this.uri = uri.toString()
            directory = invoke.parseArgs(PickerArgs::class.java).directory
        }
        importSite(invoke, args)
    }

    private fun importSite(invoke: Invoke, args: SourceArgs) {
        Thread {
            val destination = File(activity.filesDir, "sites/${UUID.randomUUID()}")
            try {
                val uri = Uri.parse(args.uri)
                require(uri.scheme == "content") { "Choose a site with the Android file picker." }
                destination.mkdirs()
                val budget = ImportBudget()
                val path = if (args.directory) {
                    copyTree(uri, DocumentsContract.getTreeDocumentId(uri), destination, budget, 0)
                    destination
                } else {
                    val name = activity.contentResolver.query(uri,
                        arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use {
                        if (it.moveToFirst()) it.getString(0) else null
                    } ?: "index.html"
                    require(name.endsWith(".html", true) || name.endsWith(".htm", true)) { "Choose an HTML file." }
                    File(destination, safeName(name)).also { copyDocument(uri, it, budget) }
                }
                invoke.resolve(JSObject().apply { put("path", path.absolutePath) })
            } catch (error: Exception) {
                destination.deleteRecursively()
                invoke.reject(error.message ?: "Could not import the site.")
            }
        }.start()
    }

    private class ImportBudget { var bytes = 0L; var files = 0 }

    private fun safeName(name: String): String {
        require(name.isNotEmpty() && name != "." && name != ".." &&
            !name.contains('/') && !name.contains('\\') && !name.contains('\u0000')) { "The site contains an invalid file name." }
        return name
    }

    private fun copyTree(tree: Uri, id: String, target: File, budget: ImportBudget, depth: Int) {
        require(depth <= 32) { "The site contains too many nested folders." }
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(tree, id)
        val resolver = activity.contentResolver
        resolver.query(children, arrayOf(DocumentsContract.Document.COLUMN_DOCUMENT_ID,
            DocumentsContract.Document.COLUMN_DISPLAY_NAME, DocumentsContract.Document.COLUMN_MIME_TYPE), null, null, null)?.use { cursor ->
            while (cursor.moveToNext()) {
                require(++budget.files <= 10000) { "The site contains more than 10,000 files." }
                val childId = cursor.getString(0)
                val file = File(target, safeName(cursor.getString(1)))
                if (cursor.getString(2) == DocumentsContract.Document.MIME_TYPE_DIR) {
                    require(file.mkdirs()) { "Could not create the imported folder." }
                    copyTree(tree, childId, file, budget, depth + 1)
                } else {
                    copyDocument(DocumentsContract.buildDocumentUriUsingTree(tree, childId), file, budget)
                }
            }
        } ?: error("Could not read the selected folder.")
    }

    private fun copyDocument(uri: Uri, target: File, budget: ImportBudget) {
        activity.contentResolver.openInputStream(uri)?.use { input ->
            target.outputStream().use { output ->
                val buffer = ByteArray(65536)
                while (true) {
                    val read = input.read(buffer)
                    if (read < 0) break
                    budget.bytes += read
                    require(budget.bytes <= 512L * 1024 * 1024) { "The site is larger than 512 MB." }
                    output.write(buffer, 0, read)
                }
            }
        } ?: error("Could not read a site file.")
    }

    @Command
    fun checkUpdate(invoke: Invoke) {
        Thread {
            var connection: HttpURLConnection? = null
            try {
                connection = URL("https://github.com/uzairdeveloper223/OnionHoster/releases/latest/download/android-latest.json")
                    .openConnection() as HttpURLConnection
                connection.connectTimeout = 20000
                connection.readTimeout = 20000
                if (connection.responseCode == 404) {
                    invoke.resolve(JSObject())
                } else {
                    require(connection.responseCode == 200) { "GitHub returned HTTP ${connection.responseCode}." }
                    val bytes = connection.inputStream.use { input ->
                        val output = ByteArrayOutputStream()
                        val buffer = ByteArray(8192)
                        while (true) {
                            val count = input.read(buffer)
                            if (count < 0) break
                            require(output.size() + count <= 128 * 1024) { "The update manifest is too large." }
                            output.write(buffer, 0, count)
                        }
                        output.toByteArray()
                    }
                    invoke.resolve(JSObject(String(bytes, Charsets.UTF_8)))
                }
            } catch (error: Exception) { invoke.reject(error.message ?: "Could not check updates.") }
            finally { connection?.disconnect() }
        }.start()
    }

    @Command
    fun openRelease(invoke: Invoke) {
        try {
            val args = invoke.parseArgs(ReleaseArgs::class.java)
            val uri = Uri.parse(args.url)
            require(uri.scheme == "https" && uri.host == "github.com" &&
                uri.path?.startsWith("/uzairdeveloper223/OnionHoster/releases/download/v") == true &&
                uri.path?.endsWith("_android_arm64.apk") == true) { "The Android release URL is invalid." }
            activity.startActivity(Intent(Intent.ACTION_VIEW, uri))
            invoke.resolve()
        } catch (error: Exception) { invoke.reject(error.message ?: "Could not open the release.") }
    }
}
