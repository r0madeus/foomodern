//! Registers the Kotlin `AudioPlugin` (Media3/ExoPlayer) with Tauri and hands
//! its [`PluginHandle`] to the app. The app's `audio::android` engine is the
//! only user; it implements `AudioEngine` on top of this handle.

use tauri::plugin::{Builder, PluginHandle, TauriPlugin};
use tauri::{Manager, Runtime};

/// Handle to the Kotlin plugin, managed as app state.
pub struct AndroidAudio<R: Runtime>(pub PluginHandle<R>);

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("android-audio")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            {
                let handle = api
                    .register_android_plugin("io.github.r0madeus.neomodern.audio", "AudioPlugin")?;
                app.manage(AndroidAudio(handle));
            }
            #[cfg(not(target_os = "android"))]
            let _ = (app, api);
            Ok(())
        })
        .build()
}
