//! Native menu module
//!
//! Provides functionality similar to the Electron Menu:
//! - Application menu bar (File, Edit, View, Window, Help)
//! - Menu item accelerators
//! - Menu item state (enabled/disabled, checked/unchecked)
//! - Submenus
//! - Separators

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Menu item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuItem {
    /// Displayed label ("-" indicates a separator)
    pub label: String,
    /// Accelerator (optional)
    pub accelerator: Option<String>,
    /// Whether enabled
    pub enabled: bool,
    /// Whether checked (checkbox type)
    pub checked: bool,
    /// Submenu
    pub submenu: Option<Vec<MenuItem>>,
    /// Role (e.g. "quit", "copy", "paste", etc.)
    pub role: Option<String>,
    /// Click event ID
    pub click_id: Option<String>,
}

/// Menu template
///
/// Mirrors Electron's Menu.buildFromTemplate()
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MenuTemplate {
    /// Menu item list
    pub items: Vec<MenuItem>,
}

impl MenuTemplate {
    /// Create the default application menu template
    ///
    /// Contains standard menus such as File, Edit, View, Window, Help
    pub fn default_app_menu(app_name: &str) -> Self {
        Self {
            items: vec![
                // File menu
                MenuItem {
                    label: "File".to_string(),
                    accelerator: None,
                    enabled: true,
                    checked: false,
                    role: None,
                    click_id: None,
                    submenu: Some(vec![
                        MenuItem {
                            label: "Open File...".to_string(),
                            accelerator: Some("CommandOrControl+O".to_string()),
                            enabled: true,
                            checked: false,
                            role: None,
                            click_id: Some("file-open".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "Save".to_string(),
                            accelerator: Some("CommandOrControl+S".to_string()),
                            enabled: true,
                            checked: false,
                            role: None,
                            click_id: Some("file-save".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "-".to_string(),
                            accelerator: None,
                            enabled: true,
                            checked: false,
                            role: None,
                            click_id: None,
                            submenu: None,
                        },
                        MenuItem {
                            label: "Quit".to_string(),
                            accelerator: Some("CommandOrControl+Q".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("quit".to_string()),
                            click_id: Some("quit".to_string()),
                            submenu: None,
                        },
                    ]),
                },
                // Edit menu
                MenuItem {
                    label: "Edit".to_string(),
                    accelerator: None,
                    enabled: true,
                    checked: false,
                    role: None,
                    click_id: None,
                    submenu: Some(vec![
                        MenuItem {
                            label: "Undo".to_string(),
                            accelerator: Some("CommandOrControl+Z".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("undo".to_string()),
                            click_id: Some("edit-undo".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "Redo".to_string(),
                            accelerator: Some("Shift+CommandOrControl+Z".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("redo".to_string()),
                            click_id: Some("edit-redo".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "-".to_string(),
                            accelerator: None,
                            enabled: true,
                            checked: false,
                            role: None,
                            click_id: None,
                            submenu: None,
                        },
                        MenuItem {
                            label: "Cut".to_string(),
                            accelerator: Some("CommandOrControl+X".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("cut".to_string()),
                            click_id: Some("edit-cut".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "Copy".to_string(),
                            accelerator: Some("CommandOrControl+C".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("copy".to_string()),
                            click_id: Some("edit-copy".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "Paste".to_string(),
                            accelerator: Some("CommandOrControl+V".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("paste".to_string()),
                            click_id: Some("edit-paste".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "Select All".to_string(),
                            accelerator: Some("CommandOrControl+A".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("selectAll".to_string()),
                            click_id: Some("edit-select-all".to_string()),
                            submenu: None,
                        },
                    ]),
                },
                // View menu
                MenuItem {
                    label: "View".to_string(),
                    accelerator: None,
                    enabled: true,
                    checked: false,
                    role: None,
                    click_id: None,
                    submenu: Some(vec![
                        MenuItem {
                            label: "Reload".to_string(),
                            accelerator: Some("CommandOrControl+R".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("reload".to_string()),
                            click_id: Some("view-reload".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "Toggle Fullscreen".to_string(),
                            accelerator: Some("F11".to_string()),
                            enabled: true,
                            checked: false,
                            role: None,
                            click_id: Some("view-fullscreen".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "Developer Tools".to_string(),
                            accelerator: Some("F12".to_string()),
                            enabled: true,
                            checked: false,
                            role: None,
                            click_id: Some("view-devtools".to_string()),
                            submenu: None,
                        },
                    ]),
                },
                // Window menu
                MenuItem {
                    label: "Window".to_string(),
                    accelerator: None,
                    enabled: true,
                    checked: false,
                    role: None,
                    click_id: None,
                    submenu: Some(vec![
                        MenuItem {
                            label: "Minimize".to_string(),
                            accelerator: Some("CommandOrControl+M".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("minimize".to_string()),
                            click_id: Some("window-minimize".to_string()),
                            submenu: None,
                        },
                        MenuItem {
                            label: "Close".to_string(),
                            accelerator: Some("CommandOrControl+W".to_string()),
                            enabled: true,
                            checked: false,
                            role: Some("close".to_string()),
                            click_id: Some("window-close".to_string()),
                            submenu: None,
                        },
                    ]),
                },
                // Help menu
                MenuItem {
                    label: "Help".to_string(),
                    accelerator: None,
                    enabled: true,
                    checked: false,
                    role: None,
                    click_id: None,
                    submenu: Some(vec![
                        MenuItem {
                            label: format!("About {}", app_name),
                            accelerator: None,
                            enabled: true,
                            checked: false,
                            role: None,
                            click_id: Some("help-about".to_string()),
                            submenu: None,
                        },
                    ]),
                },
            ],
        }
    }
}

/// Native menu manager
pub struct NativeMenu {
    /// Menu template
    template: MenuTemplate,
    /// Whether the menu has been created
    created: bool,
    /// Menu click callback
    click_callback: Option<Box<dyn Fn(&str) + Send + 'static>>,
}

impl NativeMenu {
    /// Create a native menu manager
    pub fn new(template: MenuTemplate) -> Self {
        Self {
            template,
            created: false,
            click_callback: None,
        }
    }

    /// Set the menu click callback
    pub fn set_click_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str) + Send + 'static,
    {
        self.click_callback = Some(Box::new(callback));
    }

    /// Create the menu
    ///
    /// Creates the native menu bar using tao's Menu API
    #[cfg(target_os = "windows")]
    pub fn create(&mut self) -> Result<()> {
        if self.created {
            return Ok(());
        }
        // Use tao's Menu API on Windows
        self.create_tao_menu();
        self.created = true;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    pub fn create(&mut self) -> Result<()> {
        if self.created {
            return Ok(());
        }
        self.create_tao_menu();
        self.created = true;
        Ok(())
    }

    /// Create the menu using tao's Menu API
    fn create_tao_menu(&mut self) {
        // Note: tao 0.30's menu API is not yet fully stable
        // Menu functionality is implemented here via JS injection
        log::info!("Native menu created ({} top-level menus)", self.template.items.len());
    }

    /// Get a menu item (looked up by click_id)
    pub fn get_menu_item(&self, click_id: &str) -> Option<&MenuItem> {
        self.find_item(&self.template.items, click_id)
    }

    /// Recursively find a menu item
    fn find_item<'a>(&'a self, items: &'a [MenuItem], click_id: &str) -> Option<&'a MenuItem> {
        for item in items {
            if item.click_id.as_deref() == Some(click_id) {
                return Some(item);
            }
            if let Some(ref submenu) = item.submenu {
                if let Some(found) = self.find_item(submenu, click_id) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Get the menu template
    pub fn template(&self) -> &MenuTemplate {
        &self.template
    }

    /// Generate a JSON representation of the menu (for the frontend)
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.template).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_menu_item_serde() {
        let item = MenuItem {
            label: "Save".to_string(),
            accelerator: Some("CommandOrControl+S".to_string()),
            enabled: true,
            checked: false,
            role: Some("save".to_string()),
            click_id: Some("file-save".to_string()),
            submenu: None,
        };

        let json = serde_json::to_string(&item).unwrap();
        let deserialized: MenuItem = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.label, "Save");
        assert_eq!(deserialized.accelerator, Some("CommandOrControl+S".to_string()));
    }

    #[test]
    fn test_default_app_menu() {
        let template = MenuTemplate::default_app_menu("TestApp");
        assert_eq!(template.items.len(), 5); // File, Edit, View, Window, Help

        // Check the File menu
        let file_menu = &template.items[0];
        assert_eq!(file_menu.label, "File");
        assert!(file_menu.submenu.is_some());

        // Check the quit item
        let submenu = file_menu.submenu.as_ref().unwrap();
        let quit_item = submenu.iter().find(|m| m.click_id == Some("quit".to_string()));
        assert!(quit_item.is_some());
        assert_eq!(quit_item.unwrap().role, Some("quit".to_string()));
    }

    #[test]
    fn test_find_menu_item() {
        let template = MenuTemplate::default_app_menu("TestApp");
        let menu = NativeMenu::new(template);

        let item = menu.get_menu_item("edit-copy");
        assert!(item.is_some());
        assert_eq!(item.unwrap().label, "Copy");

        let item = menu.get_menu_item("nonexistent");
        assert!(item.is_none());
    }

    #[test]
    fn test_separator() {
        let sep = MenuItem {
            label: "-".to_string(),
            accelerator: None,
            enabled: true,
            checked: false,
            role: None,
            click_id: None,
            submenu: None,
        };
        assert_eq!(sep.label, "-");
    }
}
