package io.github.r0madeus.foomodern.audio

import android.Manifest
import android.app.Activity
import android.content.ComponentName
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import android.provider.Settings
import androidx.activity.result.ActivityResult
import android.os.Handler
import android.os.Looper
import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import app.tauri.PermissionState
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.Permission
import app.tauri.annotation.PermissionCallback
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.common.util.concurrent.ListenableFuture
import java.io.File

@InvokeArg
class AttachArgs {
    lateinit var events: Channel
}

@InvokeArg
class PathArgs {
    var path: String? = null
    /** ReplayGain volume factor for this file (0..1), computed in Rust. */
    var gain: Double? = null
}

@InvokeArg
class SaveTextArgs {
    lateinit var name: String
    lateinit var text: String
    var mimeType: String = "audio/x-mpegurl"
}

@InvokeArg
class SeekArgs {
    var seconds: Double = 0.0
}

@InvokeArg
class VolumeArgs {
    var percent: Double = 100.0
}

/**
 * Android implementation of FooModern's `AudioEngine` (see
 * `src-tauri/src/audio/android.rs`). Commands come from Rust only.
 *
 * Like the mpv engine, ExoPlayer holds at most two items: the current track
 * and the next one. The queue stays in Rust (AGENTS.md rule 2). Items are
 * identified by their file path (`MediaItem.mediaId`).
 *
 * Events go back to Rust through the channel passed to [attach], as JSON
 * objects with a `type` field.
 */
@TauriPlugin(
    permissions = [
        Permission(strings = [Manifest.permission.READ_MEDIA_AUDIO], alias = "audio"),
        Permission(strings = [Manifest.permission.READ_EXTERNAL_STORAGE], alias = "storage"),
    ]
)
class AudioPlugin(private val activity: Activity) : Plugin(activity) {
    private val player by lazy { PlayerHolder.get(activity) }
    private val handler = Handler(Looper.getMainLooper())
    private var events: Channel? = null
    private var controller: ListenableFuture<MediaController>? = null
    private var webView: WebView? = null

    /** Path of the item that is playing (or loaded), as last reported to Rust. */
    private var currentPath: String? = null
    private var durationSentFor: String? = null

    /** Volume chosen by the user (already cubic), before ReplayGain. */
    private var userVolume = 1f
    /** ReplayGain factor per file path. */
    private val gains = HashMap<String, Float>()
    private var pendingSaveText: String? = null

    private val positionTick = object : Runnable {
        override fun run() {
            if (player.isPlaying) {
                emitPosition()
                handler.postDelayed(this, POSITION_INTERVAL_MS)
            }
        }
    }

    private val listener = object : Player.Listener {
        override fun onMediaItemTransition(item: MediaItem?, reason: Int) {
            val path = item?.mediaId ?: return
            if (reason != Player.MEDIA_ITEM_TRANSITION_REASON_AUTO &&
                reason != Player.MEDIA_ITEM_TRANSITION_REASON_SEEK
            ) {
                return // `loadTrack` reports its own start
            }
            currentPath?.let { emitEnded(it, "eof") }
            currentPath = path
            applyVolume()
            emit(JSObject().put("type", "started").put("path", path).put("auto", true))
            // Keep [current, next]: drop the finished item in front.
            handler.post {
                while (player.currentMediaItemIndex > 0) player.removeMediaItem(0)
            }
        }

        override fun onPlaybackStateChanged(state: Int) {
            when (state) {
                Player.STATE_READY -> maybeEmitDuration()
                Player.STATE_ENDED -> {
                    currentPath?.let { emitEnded(it, "eof") }
                    currentPath = null
                    emit(JSObject().put("type", "idle"))
                }
                else -> {}
            }
        }

        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            emit(JSObject().put("type", "paused").put("paused", !playWhenReady))
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            handler.removeCallbacks(positionTick)
            if (isPlaying) handler.post(positionTick) else emitPosition()
        }

        override fun onPlayerError(error: PlaybackException) {
            val failed = currentPath
            emit(JSObject().put("type", "error").put("message", "${error.errorCodeName}: ${error.message}"))
            failed?.let { emitEnded(it, "error") }
            currentPath = null
            // Same as mpv: skip to the next entry if there is one.
            if (player.hasNextMediaItem()) {
                player.seekToNextMediaItem()
                player.prepare()
            } else {
                emit(JSObject().put("type", "idle"))
            }
        }
    }

    override fun load(webView: WebView) {
        this.webView = webView
    }

    /**
     * How far the status bar (top) and navigation bar (bottom) overlap the
     * web view, in CSS pixels. Zero when the app is not drawn edge-to-edge.
     */
    @Command
    fun windowInsets(invoke: Invoke) {
        val view = webView
        val root = activity.window.decorView
        val insets = ViewCompat.getRootWindowInsets(root)
        if (view == null || insets == null) {
            invoke.resolve(JSObject().put("top", 0.0).put("bottom", 0.0))
            return
        }
        val bars = insets.getInsets(
            WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
        )
        val location = IntArray(2)
        view.getLocationInWindow(location)
        val top = (bars.top - location[1]).coerceAtLeast(0)
        val bottom = (location[1] + view.height - (root.height - bars.bottom)).coerceAtLeast(0)
        val density = view.resources.displayMetrics.density
        invoke.resolve(JSObject().put("top", top / density.toDouble()).put("bottom", bottom / density.toDouble()))
    }

    /** First call from Rust: wires up events and starts the media session service. */
    @Command
    fun attach(invoke: Invoke) {
        val args = invoke.parseArgs(AttachArgs::class.java)
        events = args.events
        player.removeListener(listener)
        player.addListener(listener)
        PlayerHolder.onRemoteCommand = { action ->
            emit(JSObject().put("type", "remote").put("action", action))
        }
        if (controller == null) {
            // Binding a controller creates the service; Media3 then moves it to
            // the foreground (with a notification) whenever playback runs.
            val token = SessionToken(activity, ComponentName(activity, PlaybackService::class.java))
            controller = MediaController.Builder(activity, token).buildAsync()
        }
        invoke.resolve()
    }

    @Command
    fun loadTrack(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        val path = args.path
        if (path == null) {
            invoke.reject("path is required")
            return
        }
        currentPath?.let { emitEnded(it, "stopped") }
        currentPath = path
        gains.clear()
        args.gain?.let { gains[path] = it.toFloat() }
        applyVolume()
        durationSentFor = null
        // `load` keeps a paused engine paused (same as mpv); Rust calls `play`.
        player.setMediaItem(item(path))
        player.prepare()
        emit(JSObject().put("type", "started").put("path", path).put("auto", false))
        invoke.resolve()
    }

    @Command
    fun setNextTrack(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        val path = args.path
        if (path != null) args.gain?.let { gains[path] = it.toFloat() }
        val keep = player.currentMediaItemIndex + 1
        if (player.mediaItemCount > keep) player.removeMediaItems(keep, player.mediaItemCount)
        if (path != null && player.mediaItemCount > 0) player.addMediaItem(item(path))
        invoke.resolve()
    }

    @Command
    fun play(invoke: Invoke) {
        if (player.playbackState == Player.STATE_IDLE && player.mediaItemCount > 0) player.prepare()
        player.play()
        invoke.resolve()
    }

    @Command
    fun pause(invoke: Invoke) {
        player.pause()
        invoke.resolve()
    }

    @Command
    fun stop(invoke: Invoke) {
        val was = currentPath
        currentPath = null
        player.stop()
        player.clearMediaItems()
        if (was != null) {
            emitEnded(was, "stopped")
            emit(JSObject().put("type", "idle"))
        }
        invoke.resolve()
    }

    @Command
    fun seek(invoke: Invoke) {
        val seconds = invoke.parseArgs(SeekArgs::class.java).seconds
        player.seekTo((seconds * 1000).toLong().coerceAtLeast(0))
        emitPosition()
        invoke.resolve()
    }

    @Command
    fun setVolume(invoke: Invoke) {
        val percent = invoke.parseArgs(VolumeArgs::class.java).percent.coerceIn(0.0, 100.0)
        // mpv maps its 0..100 volume cubically; do the same so a saved volume
        // sounds alike on both platforms.
        val linear = percent / 100.0
        userVolume = (linear * linear * linear).toFloat()
        applyVolume()
        invoke.resolve()
    }

    /** New ReplayGain factor for a file (the mode changed). */
    @Command
    fun setGain(invoke: Invoke) {
        val args = invoke.parseArgs(PathArgs::class.java)
        val path = args.path
        if (path != null && args.gain != null) {
            gains[path] = args.gain!!.toFloat()
            if (path == currentPath) applyVolume()
        }
        invoke.resolve()
    }

    private fun applyVolume() {
        val gain = currentPath?.let { gains[it] } ?: 1f
        player.volume = (userVolume * gain).coerceIn(0f, 1f)
    }

    /**
     * Lets the user pick a folder (system picker) and resolves `{ path }`
     * with its file-system path, or `{ path: null }` when cancelled. Only
     * folders on device storage or an SD card have such a path.
     */
    @Command
    fun pickFolder(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)
        startActivityForResult(invoke, intent, "folderPicked")
    }

    @ActivityCallback
    fun folderPicked(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(JSObject().put("path", null as String?))
            return
        }
        val path = storagePath(uri, DocumentsContract.getTreeDocumentId(uri))
        if (path == null) {
            invoke.reject("Choose a folder on the device storage or SD card.")
        } else {
            invoke.resolve(JSObject().put("path", path))
        }
    }

    /** Picks one file and resolves `{ path }` (file-system path) or `{ path: null }`. */
    @Command
    fun pickFile(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
        }
        startActivityForResult(invoke, intent, "filePicked")
    }

    @ActivityCallback
    fun filePicked(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(JSObject().put("path", null as String?))
            return
        }
        val path = if (DocumentsContract.isDocumentUri(activity, uri)) {
            storagePath(uri, DocumentsContract.getDocumentId(uri))
        } else {
            null
        }
        if (path == null) {
            invoke.reject("Open the file from the device storage (\"Browse\" in the picker).")
        } else {
            invoke.resolve(JSObject().put("path", path))
        }
    }

    /** Picks a text file (any location) and resolves `{ name, text }` or `{ text: null }`. */
    @Command
    fun openTextFile(invoke: Invoke) {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = "*/*"
        }
        startActivityForResult(invoke, intent, "textFileOpened")
    }

    @ActivityCallback
    fun textFileOpened(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || uri == null) {
            invoke.resolve(JSObject().put("text", null as String?))
            return
        }
        try {
            val text = activity.contentResolver.openInputStream(uri)?.use { it.readBytes().toString(Charsets.UTF_8) }
            invoke.resolve(JSObject().put("name", displayName(uri)).put("text", text))
        } catch (e: Exception) {
            invoke.reject("Could not read the file: ${e.message}")
        }
    }

    /** Asks where to save `text` (system picker) and writes it; resolves `{ saved }`. */
    @Command
    fun saveTextFile(invoke: Invoke) {
        val args = invoke.parseArgs(SaveTextArgs::class.java)
        pendingSaveText = args.text
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
            addCategory(Intent.CATEGORY_OPENABLE)
            type = args.mimeType
            putExtra(Intent.EXTRA_TITLE, args.name)
        }
        startActivityForResult(invoke, intent, "textFileSaved")
    }

    @ActivityCallback
    fun textFileSaved(invoke: Invoke, result: ActivityResult) {
        val uri = result.data?.data
        val text = pendingSaveText
        pendingSaveText = null
        if (result.resultCode != Activity.RESULT_OK || uri == null || text == null) {
            invoke.resolve(JSObject().put("saved", false))
            return
        }
        try {
            activity.contentResolver.openOutputStream(uri, "wt")?.use { it.write(text.toByteArray(Charsets.UTF_8)) }
            invoke.resolve(JSObject().put("saved", true))
        } catch (e: Exception) {
            invoke.reject("Could not save the file: ${e.message}")
        }
    }

    /** `{ granted }`: whether the app may read and write all files (tag editing, .lrc). */
    @Command
    fun fileAccessStatus(invoke: Invoke) {
        invoke.resolve(JSObject().put("granted", hasFullFileAccess()))
    }

    /** Opens the system screen where the user allows access to all files. */
    @Command
    fun requestFileAccess(invoke: Invoke) {
        if (Build.VERSION.SDK_INT >= 30 && !hasFullFileAccess()) {
            val intent = Intent(
                Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION,
                Uri.parse("package:${activity.packageName}"),
            )
            try {
                activity.startActivity(intent)
            } catch (e: Exception) {
                activity.startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION))
            }
        }
        invoke.resolve()
    }

    private fun hasFullFileAccess(): Boolean =
        if (Build.VERSION.SDK_INT >= 30) Environment.isExternalStorageManager() else true

    /** "primary:Music/x" style document ids of the external storage provider → a path. */
    private fun storagePath(uri: Uri, docId: String): String? {
        if (uri.authority != "com.android.externalstorage.documents") return null
        val parts = docId.split(":", limit = 2)
        if (parts.size != 2) return null
        val base = if (parts[0].equals("primary", ignoreCase = true)) {
            Environment.getExternalStorageDirectory().path
        } else {
            "/storage/${parts[0]}"
        }
        return if (parts[1].isEmpty()) base else "$base/${parts[1]}"
    }

    private fun displayName(uri: Uri): String? =
        activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        }

    /** Asks for read access to the music library; resolves `{ granted }`. */
    @Command
    fun requestAudioAccess(invoke: Invoke) {
        val alias = accessAlias()
        if (getPermissionState(alias) == PermissionState.GRANTED) {
            invoke.resolve(JSObject().put("granted", true))
        } else {
            requestPermissionForAlias(alias, invoke, "audioAccessResult")
        }
    }

    @PermissionCallback
    fun audioAccessResult(invoke: Invoke) {
        invoke.resolve(JSObject().put("granted", getPermissionState(accessAlias()) == PermissionState.GRANTED))
    }

    private fun accessAlias() = if (Build.VERSION.SDK_INT >= 33) "audio" else "storage"

    private fun item(path: String): MediaItem =
        MediaItem.Builder().setMediaId(path).setUri(Uri.fromFile(File(path))).build()

    private fun maybeEmitDuration() {
        val path = currentPath ?: return
        val duration = player.duration
        if (duration == C.TIME_UNSET || durationSentFor == path) return
        durationSentFor = path
        emit(JSObject().put("type", "duration").put("seconds", duration / 1000.0))
    }

    private fun emitPosition() {
        if (currentPath == null) return
        maybeEmitDuration()
        emit(JSObject().put("type", "position").put("seconds", player.currentPosition / 1000.0))
    }

    private fun emitEnded(path: String, reason: String) {
        emit(JSObject().put("type", "ended").put("path", path).put("reason", reason))
    }

    private fun emit(event: JSObject) {
        events?.send(event)
    }

    private companion object {
        /** At most ~4 position updates per second (AGENTS.md rule 5). */
        const val POSITION_INTERVAL_MS = 250L
    }
}
