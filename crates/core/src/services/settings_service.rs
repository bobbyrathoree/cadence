use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::models::settings::{default_shortcuts_map, KeyboardShortcut, SHORTCUT_ACTIONS};
use crate::services::transaction;

const SHORTCUTS_KEY: &str = "keyboard_shortcuts";
pub const API_ENABLED_KEY: &str = "api_enabled";

pub fn get_setting(conn: &Connection, key: &str) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(AppError::from)
}

pub fn set_setting(conn: &mut Db, key: &str, value: &str) -> AppResult<()> {
    transaction::immediate(conn, |tx| set_setting_tx(tx, key, value))
}

pub(crate) fn set_setting_tx(conn: &Connection, key: &str, value: &str) -> AppResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(AppError::from)?;
    Ok(())
}

pub fn get_keyboard_shortcuts(conn: &Connection) -> AppResult<Vec<KeyboardShortcut>> {
    let saved = match get_setting(conn, SHORTCUTS_KEY)? {
        Some(json) => serde_json::from_str::<HashMap<String, String>>(&json).unwrap_or_default(),
        None => HashMap::new(),
    };

    Ok(SHORTCUT_ACTIONS
        .iter()
        .map(|(action, default, label, is_global)| KeyboardShortcut {
            action: action.to_string(),
            binding: saved
                .get(*action)
                .cloned()
                .unwrap_or_else(|| default.to_string()),
            label: label.to_string(),
            default_binding: default.to_string(),
            is_global: *is_global,
        })
        .collect())
}

pub fn update_shortcut(
    conn: &mut Db,
    action: &str,
    binding: &str,
) -> AppResult<Vec<KeyboardShortcut>> {
    transaction::immediate(conn, |tx| update_shortcut_tx(tx, action, binding))
}

pub(crate) fn update_shortcut_tx(
    conn: &Connection,
    action: &str,
    binding: &str,
) -> AppResult<Vec<KeyboardShortcut>> {
    if !SHORTCUT_ACTIONS
        .iter()
        .any(|(known_action, _, _, _)| *known_action == action)
    {
        return Err(AppError::invalid("Unknown shortcut action"));
    }

    let mut shortcuts = match get_setting(conn, SHORTCUTS_KEY)? {
        Some(json) => serde_json::from_str::<HashMap<String, String>>(&json).unwrap_or_default(),
        None => default_shortcuts_map(),
    };
    shortcuts.insert(action.to_string(), binding.to_string());
    let json = serde_json::to_string(&shortcuts).map_err(AppError::from)?;
    set_setting_tx(conn, SHORTCUTS_KEY, &json)?;
    get_keyboard_shortcuts(conn)
}

pub fn reset_shortcuts(conn: &mut Db) -> AppResult<Vec<KeyboardShortcut>> {
    transaction::immediate(conn, reset_shortcuts_tx)
}

fn reset_shortcuts_tx(conn: &Connection) -> AppResult<Vec<KeyboardShortcut>> {
    // An absent row is the fresh-install state and is intentionally a success.
    conn.execute(
        "DELETE FROM settings WHERE key = ?1",
        params![SHORTCUTS_KEY],
    )
    .map_err(AppError::from)?;
    get_keyboard_shortcuts(conn)
}

pub fn get_api_enabled(conn: &Connection) -> AppResult<bool> {
    Ok(matches!(
        get_setting(conn, API_ENABLED_KEY)?.as_deref(),
        Some("true")
    ))
}

pub fn set_api_enabled(conn: &mut Db, enabled: bool) -> AppResult<()> {
    transaction::immediate(conn, |tx| set_api_enabled_tx(tx, enabled))
}

pub(crate) fn set_api_enabled_tx(conn: &Connection, enabled: bool) -> AppResult<()> {
    if get_api_enabled(conn)? == enabled {
        return Ok(());
    }
    set_setting_tx(
        conn,
        API_ENABLED_KEY,
        if enabled { "true" } else { "false" },
    )
}
