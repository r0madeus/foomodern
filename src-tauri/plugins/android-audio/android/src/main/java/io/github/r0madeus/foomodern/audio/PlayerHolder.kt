package io.github.r0madeus.foomodern.audio

import android.content.Context
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.exoplayer.ExoPlayer

/**
 * The single ExoPlayer instance, shared by [AudioPlugin] (which drives it on
 * behalf of Rust) and [PlaybackService] (which exposes it to the system as a
 * media session for the notification, lock screen and headset buttons).
 * Only touched on the main thread.
 */
object PlayerHolder {
    private var player: ExoPlayer? = null

    /** Called for "next" / "previous" from the notification or a headset. */
    var onRemoteCommand: ((String) -> Unit)? = null

    fun get(context: Context): ExoPlayer {
        player?.let { return it }
        val attributes = AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA)
            .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
            .build()
        val created = ExoPlayer.Builder(context.applicationContext)
            .setAudioAttributes(attributes, /* handleAudioFocus = */ true)
            .setHandleAudioBecomingNoisy(true)
            .setWakeMode(C.WAKE_MODE_LOCAL)
            .build()
        player = created
        return created
    }
}
