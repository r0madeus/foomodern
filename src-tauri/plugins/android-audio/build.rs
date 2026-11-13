// The Kotlin side is only ever called from Rust (never from JavaScript), so
// no commands are exposed to the webview.
const COMMANDS: &[&str] = &[];

fn main() {
    tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .build();
}
