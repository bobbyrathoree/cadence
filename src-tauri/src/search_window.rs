use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

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

pub fn register_search_shortcut(app: &AppHandle, binding: &str) -> Result<(), String> {
    let handle = app.clone();
    app.global_shortcut()
        .on_shortcut(binding, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Err(error) = search_window(&handle, Mode::Toggle) {
                    eprintln!("Failed to toggle search window: {error}");
                }
            }
        })
        .map_err(|error| error.to_string())
}

pub fn reregister_shortcut<T, Register, Unregister, Persist>(
    current: &str,
    replacement: &str,
    mut register: Register,
    mut unregister: Unregister,
    persist: Persist,
) -> Result<T, String>
where
    Register: FnMut(&str) -> Result<(), String>,
    Unregister: FnMut(&str) -> Result<(), String>,
    Persist: FnOnce() -> Result<T, String>,
{
    if current == replacement {
        return persist();
    }

    replace_registration(current, replacement, &mut register, &mut unregister)?;
    match persist() {
        Ok(value) => Ok(value),
        Err(persist_error) => {
            match replace_registration(replacement, current, &mut register, &mut unregister) {
                Ok(()) => Err(format!(
                    "{persist_error}. Restored the previous shortcut registration."
                )),
                Err(rollback_error) => Err(format!(
                    "{persist_error}. Failed to restore the previous shortcut: {rollback_error}"
                )),
            }
        }
    }
}

pub fn register_shortcut_and_persist<T, Register, Unregister, Persist>(
    binding: &str,
    mut register: Register,
    mut unregister: Unregister,
    persist: Persist,
) -> Result<T, String>
where
    Register: FnMut(&str) -> Result<(), String>,
    Unregister: FnMut(&str) -> Result<(), String>,
    Persist: FnOnce() -> Result<T, String>,
{
    register(binding)
        .map_err(|error| format!("Failed to register shortcut '{binding}': {error}"))?;
    match persist() {
        Ok(value) => Ok(value),
        Err(persist_error) => match unregister(binding) {
            Ok(()) => Err(format!(
                "{persist_error}. Removed the unpersisted shortcut registration."
            )),
            Err(rollback_error) => Err(format!(
                "{persist_error}. Failed to remove the unpersisted shortcut: {rollback_error}"
            )),
        },
    }
}

fn replace_registration<Register, Unregister>(
    current: &str,
    replacement: &str,
    register: &mut Register,
    unregister: &mut Unregister,
) -> Result<(), String>
where
    Register: FnMut(&str) -> Result<(), String>,
    Unregister: FnMut(&str) -> Result<(), String>,
{
    register(replacement)
        .map_err(|error| format!("Failed to register shortcut '{replacement}': {error}"))?;

    if let Err(error) = unregister(current) {
        return match unregister(replacement) {
            Ok(()) => Err(format!(
                "Failed to unregister shortcut '{current}': {error}. Removed the replacement."
            )),
            Err(rollback_error) => Err(format!(
                "Failed to unregister shortcut '{current}': {error}. \
                 Failed to remove replacement '{replacement}': {rollback_error}"
            )),
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::{register_shortcut_and_persist, reregister_shortcut};

    #[test]
    fn registers_new_before_unregistering_old_and_persisting() {
        let events = RefCell::new(Vec::new());
        let result = reregister_shortcut(
            "old",
            "new",
            |binding| {
                events.borrow_mut().push(format!("register:{binding}"));
                Ok(())
            },
            |binding| {
                events.borrow_mut().push(format!("unregister:{binding}"));
                Ok(())
            },
            || {
                events.borrow_mut().push("persist".to_string());
                Ok("saved")
            },
        );

        assert_eq!(result.unwrap(), "saved");
        assert_eq!(
            events.into_inner(),
            ["register:new", "unregister:old", "persist"]
        );
    }

    #[test]
    fn persistence_failure_restores_the_previous_registration() {
        let events = RefCell::new(Vec::new());
        let error = reregister_shortcut::<(), _, _, _>(
            "old",
            "new",
            |binding| {
                events.borrow_mut().push(format!("register:{binding}"));
                Ok(())
            },
            |binding| {
                events.borrow_mut().push(format!("unregister:{binding}"));
                Ok(())
            },
            || {
                events.borrow_mut().push("persist".to_string());
                Err("database failed".to_string())
            },
        )
        .unwrap_err();

        assert!(error.contains("Restored the previous shortcut"));
        assert_eq!(
            events.into_inner(),
            [
                "register:new",
                "unregister:old",
                "persist",
                "register:old",
                "unregister:new",
            ]
        );
    }

    #[test]
    fn rollback_failure_is_propagated() {
        let error = reregister_shortcut::<(), _, _, _>(
            "old",
            "new",
            |binding| {
                if binding == "old" {
                    Err("old binding unavailable".to_string())
                } else {
                    Ok(())
                }
            },
            |_binding| Ok(()),
            || Err("database failed".to_string()),
        )
        .unwrap_err();

        assert!(error.contains("database failed"));
        assert!(error.contains("old binding unavailable"));
    }

    #[test]
    fn missing_equal_binding_is_registered_and_rolled_back_on_persist_failure() {
        let events = RefCell::new(Vec::new());
        let error = register_shortcut_and_persist::<(), _, _, _>(
            "default",
            |binding| {
                events.borrow_mut().push(format!("register:{binding}"));
                Ok(())
            },
            |binding| {
                events.borrow_mut().push(format!("unregister:{binding}"));
                Ok(())
            },
            || {
                events.borrow_mut().push("persist".to_string());
                Err("database failed".to_string())
            },
        )
        .unwrap_err();

        assert!(error.contains("Removed the unpersisted shortcut"));
        assert_eq!(
            events.into_inner(),
            ["register:default", "persist", "unregister:default"]
        );
    }
}
