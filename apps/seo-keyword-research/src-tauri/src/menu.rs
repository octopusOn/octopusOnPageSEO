//! Route the macOS Quit accelerator through the running-jobs guard.

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};

pub(super) fn install<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<()> {
    let menu = Menu::default(app)?;
    let quit_text = PredefinedMenuItem::quit(app, None)?.text()?;
    for item in menu.items()? {
        let Some(submenu) = item.as_submenu() else {
            continue;
        };
        for (position, child) in submenu.items()?.into_iter().enumerate() {
            let Some(predefined) = child.as_predefined_menuitem() else {
                continue;
            };
            if predefined.text()? == quit_text {
                // The native predefined item sends NSApplication terminate: directly,
                // bypassing Tauri's cancellable ExitRequested event.
                let quit =
                    MenuItem::with_id(app, "keywords-quit", &quit_text, true, Some("CmdOrCtrl+Q"))?;
                submenu.remove(predefined)?;
                submenu.insert(&quit, position)?;
            }
        }
    }
    app.set_menu(menu)?;
    Ok(())
}
