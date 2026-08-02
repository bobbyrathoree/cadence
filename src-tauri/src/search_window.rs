use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Show,
    Toggle,
}

pub fn search_window(app: &AppHandle, mode: Mode) -> Result<(), String> {
    let window = app
        .get_webview_window("search")
        .ok_or_else(|| "Search window is unavailable".to_string())?;

    if mode == Mode::Toggle && window.is_visible().map_err(|error| error.to_string())? {
        return window.hide().map_err(|error| error.to_string());
    }

    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    app.emit("search-shown", ())
        .map_err(|error| error.to_string())
}
