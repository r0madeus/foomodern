package io.github.r0madeus.foomodern.audio

import android.content.Intent
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.Player
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * Keeps playback alive in the background: Media3 promotes this service to a
 * foreground service with a media notification while the player is playing.
 */
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        session = MediaSession.Builder(this, RemotePlayer(PlayerHolder.get(this))).build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        // The player itself belongs to PlayerHolder and is not released here.
        session?.release()
        session = null
        super.onDestroy()
    }
}

/**
 * ExoPlayer only ever holds [current, next] (the queue lives in Rust), so
 * "next" / "previous" from the system UI are forwarded to Rust instead of
 * moving inside ExoPlayer's two-item playlist.
 */
private class RemotePlayer(player: Player) : ForwardingPlayer(player) {
    private val extra = listOf(
        COMMAND_SEEK_TO_NEXT,
        COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
        COMMAND_SEEK_TO_PREVIOUS,
        COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
    )

    override fun getAvailableCommands(): Player.Commands =
        super.getAvailableCommands().buildUpon().addAll(*extra.toIntArray()).build()

    override fun isCommandAvailable(command: Int): Boolean =
        command in extra || super.isCommandAvailable(command)

    override fun seekToNext() = remote("next")
    override fun seekToNextMediaItem() = remote("next")
    override fun seekToPrevious() = remote("previous")
    override fun seekToPreviousMediaItem() = remote("previous")

    private fun remote(action: String) {
        PlayerHolder.onRemoteCommand?.invoke(action)
    }
}
