//! System tray icon and "keep playing in the tray" window behaviour.
//!
//! Whether closing the window hides it or quits is decided by the pure
//! [`close_action`]. The user's setting lives in
//! [`SessionStore`](crate::session::SessionStore); whether a tray icon really
//! exists on this run lives in [`TrayStatus`]. Without a tray the window is
//! always allowed to close, otherwise the user would be left with a hidden
//! window and no way to bring it back.

use crate::player::Player;
use crate::session::SessionStore;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Window, WindowEvent};

pub const MAIN_WINDOW: &str = "main";

const ID_SHOW: &str = "tray-show";
const ID_PLAY_PAUSE: &str = "tray-play-pause";
const ID_NEXT: &str = "tray-next";
const ID_PREVIOUS: &str = "tray-previous";
const ID_QUIT: &str = "tray-quit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    /// Hide the window; playback and the app keep running.
    HideToTray,
    /// Let the window close, which quits the app.
    Exit,
}

/// What a window close request should do.
pub fn close_action(close_to_tray: bool, tray_available: bool) -> CloseAction {
    if close_to_tray && tray_available {
        CloseAction::HideToTray
    } else {
        CloseAction::Exit
    }
}

/// Whether a tray icon exists in this run. Starts out false.
#[derive(Default)]
pub struct TrayStatus(AtomicBool);

impl TrayStatus {
    pub fn available(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    fn set_available(&self, yes: bool) {
        self.0.store(yes, Ordering::SeqCst);
    }
}

pub fn show_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    if let Err(e) = window.show() {
        log::warn!("failed to show the window: {e}");
    }
    if window.is_minimized().unwrap_or(false)
        && let Err(e) = window.unminimize()
    {
        log::warn!("failed to restore the window: {e}");
    }
    if let Err(e) = window.set_focus() {
        log::warn!("failed to focus the window: {e}");
    }
}

fn run_player(app: &AppHandle, what: &str, f: impl FnOnce(&Player) -> Result<(), String>) {
    let Some(player) = app.try_state::<Arc<Player>>() else {
        return;
    };
    if let Err(e) = f(&player) {
        log::warn!("tray {what} failed: {e}");
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    match id {
        ID_SHOW => show_main_window(app),
        ID_PLAY_PAUSE => run_player(app, "play/pause", |p| {
            p.toggle_pause().map(|_| ()).map_err(|e| e.to_string())
        }),
        ID_NEXT => run_player(app, "next", |p| {
            p.next().map(|_| ()).map_err(|e| e.to_string())
        }),
        ID_PREVIOUS => run_player(app, "previous", |p| {
            p.previous().map(|_| ()).map_err(|e| e.to_string())
        }),
        // `exit` runs the normal exit path, so the session is saved.
        ID_QUIT => app.exit(0),
        _ => {}
    }
}

fn build(app: &AppHandle) -> tauri::Result<tauri::tray::TrayIcon> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, ID_SHOW, "Show FooModern", true, None::<&str>)?,
            &MenuItem::with_id(app, ID_PLAY_PAUSE, "Play/Pause", true, None::<&str>)?,
            &MenuItem::with_id(app, ID_NEXT, "Next", true, None::<&str>)?,
            &MenuItem::with_id(app, ID_PREVIOUS, "Previous", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, ID_QUIT, "Quit", true, None::<&str>)?,
        ],
    )?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("FooModern")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| on_menu(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| {
            // Click events are not delivered on Linux; the menu has "Show".
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)
}

/// Creates the tray icon. On failure the app still works: the tray is marked
/// unavailable, so closing the window quits as it did before.
pub fn install(app: &AppHandle) {
    let status = app.state::<TrayStatus>();
    match build(app) {
        Ok(tray) => {
            app.manage(tray);
            status.set_available(true);
        }
        Err(e) => {
            log::warn!("could not create the tray icon, closing the window will quit: {e}");
            status.set_available(false);
        }
    }
}

/// Window close requests: hide to the tray or let the app quit.
pub fn on_window_event(window: &Window, event: &WindowEvent) {
    let WindowEvent::CloseRequested { api, .. } = event else {
        return;
    };
    if window.label() != MAIN_WINDOW {
        return;
    }
    let app = window.app_handle();
    let (Some(store), Some(status)) = (
        app.try_state::<Arc<SessionStore>>(),
        app.try_state::<TrayStatus>(),
    ) else {
        return;
    };
    if close_action(store.close_to_tray(), status.available()) == CloseAction::HideToTray {
        api.prevent_close();
        if let Err(e) = window.hide() {
            log::warn!("failed to hide the window: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_hides_only_with_setting_and_tray() {
        assert_eq!(close_action(true, true), CloseAction::HideToTray);
        assert_eq!(close_action(false, true), CloseAction::Exit);
        // No tray: never hide, whatever the setting says.
        assert_eq!(close_action(true, false), CloseAction::Exit);
        assert_eq!(close_action(false, false), CloseAction::Exit);
    }

    #[test]
    fn tray_is_unavailable_until_installed() {
        let s = TrayStatus::default();
        assert!(!s.available());
        s.set_available(true);
        assert!(s.available());
    }
}
