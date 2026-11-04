//! Toolbar tools in Edit › Keyboard Shortcuts (#4086). Explicit assignments select an
//! individual tool; unchanged tools keep Photoshop's shared keys and Shift cycling.

use crate::{PhotocraftApp, state::Tool};

pub(crate) fn id(tool: Tool) -> String {
    let mut name = format!("{tool:?}");
    if let Some(first) = name.get_mut(..1) {
        first.make_ascii_lowercase();
    }
    format!("tools.select.{name}")
}

pub(crate) fn tool(id: &str) -> Option<Tool> {
    let name = id.strip_prefix("tools.select.")?;
    let tool = Tool::from_name(name)?;
    (id == self::id(tool)).then_some(tool)
}

pub(crate) fn default(tool: Tool) -> Option<String> {
    (tool.key() != '\0').then(|| tool.key().to_string())
}

pub(crate) fn shortcut(app: &PhotocraftApp, tool: Tool) -> Option<String> {
    crate::shortcuts::effective_shortcut(app, &id(tool), default(tool).as_deref())
}

pub(crate) fn uses_default(app: &PhotocraftApp, tool: Tool) -> bool {
    !app.session.prefs().shortcuts.contains_key(&id(tool))
}