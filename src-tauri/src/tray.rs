use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "search", "Search...  \u{2318}\u{21E7}P", true, None::<&str>)?,
            &MenuItem::with_id(app, "open", "Open Cadence", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", "Quit Cadence", true, None::<&str>)?,
        ],
    )?;

    let icon = Image::from_bytes(include_bytes!("../icons/tray_44.png"))
        .expect("Failed to load embedded tray icon");

    TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("Cadence")
        .icon(icon)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "search" => {
                if let Some(window) = app.get_webview_window("search") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}
