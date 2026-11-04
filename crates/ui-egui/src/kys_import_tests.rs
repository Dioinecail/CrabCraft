//! Photoshop `.kys` sets import into Edit › Keyboard Shortcuts (#1163): rows match our
//! commands by label, one shortcut per command, with Photoshop's menu order breaking ties.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use super::{KysCommand, parse, plan};
use crate::PhotocraftApp;
use crate::file_dialog::FileDialogAnswer;

/// A cut-down set in Photoshop's own layout: BOM, a dynamic (filter) row, static rows with
/// their ids, a row with two shortcuts, an entity, scripts, and the tool keys.
const SAMPLE: &str = "\u{feff}<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<photoshop-keyboard-shortcuts version=\"4\" filename=\"$$$/FileName/Presets/KeyboardCustomization/texcuts=texcuts\" modified=\"0\" multi-undo=\"2\">
\t<command kind=\"dynamic\" name=\"Script_6F17BFA7-EFC8-40EA-B850-7B95ED8EA713\">
\t\t<shortcut>Control+Opt+Cmd+4</shortcut>
\t</command>
\t<command kind=\"dynamic\" name=\"Gaussian Blur...\">
\t\t<shortcut>Control+Opt+Cmd+G</shortcut>
\t</command>
\t<command kind=\"static\" name=\"Undo\" id=\"101\">
\t\t<shortcut>Cmd+Z</shortcut>
\t\t<shortcut>F1</shortcut>
\t</command>
\t<command kind=\"static\" name=\"Zoom In\" id=\"1004\">
\t\t<shortcut>Cmd++</shortcut>
\t\t<shortcut>Cmd+=</shortcut>
\t</command>
\t<command kind=\"static\" name=\"Levels...\" id=\"1801\">
\t\t<shortcut>Cmd+L</shortcut>
\t</command>
\t<command kind=\"static\" name=\"All\" id=\"1017\">
\t\t<shortcut>Cmd+A</shortcut>
\t</command>
\t<command kind=\"static\" name=\"Layer Via Copy\" id=\"2970\">
\t\t<shortcut>Cmd+J</shortcut>
\t</command>
\t<command kind=\"static\" name=\"Black &amp; White...\" id=\"1824\">
\t\t<shortcut>Opt+Shift+Cmd+B</shortcut>
\t</command>
\t<command kind=\"static\" name=\"General...\" id=\"2311\">
\t\t<shortcut>Cmd+K</shortcut>
\t</command>
\t<command kind=\"static\" name=\"Last Filter\" id=\"1019\">
\t\t<shortcut>Cmd+F</shortcut>
\t</command>
\t<command kind=\"static\" name=\"Hide Photoshop\" id=\"7001\">
\t\t<shortcut>Cmd+H</shortcut>
\t</command>
\t<tool name=\"Move Tool\" type=\"1\" key=\"1819113074\">V</tool>
\t<tool name=\"Default Foreground/Background Colors\" type=\"2\"></tool>
</photoshop-keyboard-shortcuts>
";

fn app() -> PhotocraftApp {
    PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
}

type Store = std::sync::Arc<std::sync::Mutex<Option<String>>>;

/// An app beside a Photoshop whose live set is `set`; `saved_prefs` is this app's own saved
/// preferences file, and the store receives what it writes back.
fn with_photoshop(set: &'static str, saved_prefs: Option<&'static str>) -> (PhotocraftApp, Store) {
    let store: Store = Default::default();
    let written = store.clone();
    let services = crate::Services {
        photoshop_shortcuts: Some(Box::new(move || {
            Some(("/Users/me/Library/Preferences/Adobe Photoshop 2026 Settings/Keyboard Shortcuts.psp".into(), set.into()))
        })),
        load_prefs: Some(Box::new(move || saved_prefs.map(str::to_string))),
        save_prefs: Some(Box::new(move |text: &str| {
            *written.lock().unwrap() = Some(text.to_string());
            Ok(())
        })),
        ..Default::default()
    };
    (PhotocraftApp::new(photocraft_engine::Session::new(), services), store)
}
