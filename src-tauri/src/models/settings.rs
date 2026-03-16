use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const SHORTCUT_ACTIONS: &[(&str, &str, &str, bool)] = &[
    // (action_id, default_binding, label, is_global)
    ("global_toggle_search", "CommandOrControl+Shift+P", "Toggle Search Window", true),
    ("focus_search", "CommandOrControl+F", "Focus Search", false),
    ("new_prompt", "CommandOrControl+N", "New Prompt", false),
    ("toggle_favorite", "CommandOrControl+D", "Toggle Favorite", false),
    ("toggle_edit", "CommandOrControl+E", "Toggle Edit Mode", false),
    ("open_import", "CommandOrControl+I", "Open Import", false),
    ("copy_selected", "Enter", "Copy Selected Prompt", false),
    ("deselect", "Escape", "Deselect", false),
    ("navigate_up", "ArrowUp", "Navigate Up", false),
    ("navigate_down", "ArrowDown", "Navigate Down", false),
    ("open_settings", "CommandOrControl+Comma", "Open Settings", false),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyboardShortcut {
    pub action: String,
    pub binding: String,
    pub label: String,
    pub default_binding: String,
    pub is_global: bool,
}

pub fn default_shortcuts_map() -> HashMap<String, String> {
    SHORTCUT_ACTIONS
        .iter()
        .map(|(action, binding, _, _)| (action.to_string(), binding.to_string()))
        .collect()
}
