use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

use crate::models::settings::{default_shortcuts_map, KeyboardShortcut, SHORTCUT_ACTIONS};

const SHORTCUTS_KEY: &str = "keyboard_shortcuts";

pub fn get_setting(conn: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = ?2",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_keyboard_shortcuts(conn: &Connection) -> rusqlite::Result<Vec<KeyboardShortcut>> {
    let saved: HashMap<String, String> = match get_setting(conn, SHORTCUTS_KEY)? {
        Some(json) => serde_json::from_str(&json).unwrap_or_default(),
        None => HashMap::new(),
    };

    let shortcuts = SHORTCUT_ACTIONS
        .iter()
        .map(|(action, default, label, is_global)| {
            let binding = saved
                .get(*action)
                .cloned()
                .unwrap_or_else(|| default.to_string());
            KeyboardShortcut {
                action: action.to_string(),
                binding,
                label: label.to_string(),
                default_binding: default.to_string(),
                is_global: *is_global,
            }
        })
        .collect();

    Ok(shortcuts)
}

pub fn update_shortcut(
    conn: &Connection,
    action: &str,
    binding: &str,
) -> rusqlite::Result<Vec<KeyboardShortcut>> {
    // Load current, update the one action, save back
    let mut map = match get_setting(conn, SHORTCUTS_KEY)? {
        Some(json) => {
            serde_json::from_str::<HashMap<String, String>>(&json).unwrap_or_default()
        }
        None => default_shortcuts_map(),
    };
    map.insert(action.to_string(), binding.to_string());
    let json = serde_json::to_string(&map).unwrap();
    set_setting(conn, SHORTCUTS_KEY, &json)?;
    get_keyboard_shortcuts(conn)
}

pub fn reset_shortcuts(conn: &Connection) -> rusqlite::Result<Vec<KeyboardShortcut>> {
    conn.execute(
        "DELETE FROM settings WHERE key = ?1",
        params![SHORTCUTS_KEY],
    )?;
    get_keyboard_shortcuts(conn)
}
