//! #292: Edit › Keyboard Shortcuts records the key pressed with the modifiers, not the
//! modifier key itself (egui reports Ctrl, Shift, Alt and ⌘ as key presses too), so Ctrl+F and
//! any other non-system combination can be bound; a shortcut already in use is reported.

use egui::{Event, Key, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};

use crate::PhotocraftApp;

fn key(h: &mut Harness<'_, PhotocraftApp>, key: Key, pressed: bool, modifiers: Modifiers) {
    h.event(Event::Key { key, physical_key: None, pressed, repeat: false, modifiers });
}

fn input_state(events: Vec<Event>, modifiers: Modifiers) -> egui::InputState {
    let ctx = egui::Context::default();
    ctx.begin_pass(egui::RawInput { events, ..Default::default() });
    let state = ctx.input(|i| i.clone());
    ctx.end_pass().textures_delta.clear();
    let mut state = state;
    state.modifiers = modifiers;
    state
}

/// The Keyboard Shortcuts dialog with `command` selected, filtered to it.
fn dialog(ppp: f32, command: &str, filter: &str) -> (Harness<'static, PhotocraftApp>, u64) {
    dialog_with_renderer(ppp, command, filter, false)
}

fn dialog_with_renderer(ppp: f32, command: &str, filter: &str, gpu: bool) -> (Harness<'static, PhotocraftApp>, u64) {
    let builder = Harness::builder().with_size(egui::vec2(1100.0, 760.0)).with_pixels_per_point(ppp).with_max_steps(64);
    let builder = if gpu { builder.wgpu() } else { builder };
    let mut h = builder.build_eframe(|cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        PhotocraftApp::new(photocraft_engine::Session::new(), crate::Services::default())
    });
    let ctx = h.ctx.clone();
    let id = crate::menus::invoke(h.state_mut(), &ctx, "edit.keyboardShortcuts", json!({})).unwrap()["dialog"].as_u64().unwrap();
    let f = &mut h.state_mut().ui.dialog_mut(id).unwrap().fields;
    f.insert("filter".into(), json!(filter));
    f.insert("selected".into(), json!(command));
    h.run_steps(4);
    (h, id)
}

fn field(h: &Harness<'_, PhotocraftApp>, id: u64, name: &str) -> Value {
    h.state().ui.dialogs.iter().find(|d| d.id == id).unwrap().fields.get(name).cloned().unwrap_or(Value::Null)
}

/// Click the selected command's shortcut button, then press `modifier` and `k` like a keyboard:
/// the modifier key goes down first (its own key event), then the key with the modifier held.
fn capture(h: &mut Harness<'_, PhotocraftApp>, button: &str, modifier: Key, held: Modifiers, k: Key) {
    h.get_by_label(button).click();
    h.run_steps(2);
    key(h, modifier, true, held);
    h.run_steps(1);
    key(h, k, true, held);
    h.run_steps(1);
    key(h, k, false, held);
    key(h, modifier, false, Modifiers::NONE);
    h.run_steps(2);
}

fn captured_ctrl_k(ppp: f32, gpu: bool) -> Harness<'static, PhotocraftApp> {
    let (mut h, id) = dialog_with_renderer(ppp, "edit.search", "search…", gpu);
    // Search's default is ⌘F (Photoshop); ⌘K is Preferences, so this also reports a conflict.
    let current = crate::shortcuts::pretty("Cmd+F");
    // On Windows and Linux the Ctrl key is the command modifier.
    capture(&mut h, &current, Key::ControlLeft, Modifiers::COMMAND, Key::K);
    assert_eq!(field(&h, id, "overrides")["edit.search"], json!("Cmd+K"), "@{ppp}x");
    assert_eq!(field(&h, id, "capture"), json!(false));
    let message = field(&h, id, "message");
    assert!(!message.as_str().unwrap().contains("ontrol"), "@{ppp}x: {message}");
    // The row shows the new shortcut.
    h.get_by_label(&crate::shortcuts::pretty("Cmd+K"));
    h
}

/// Capture `k` with `m` for the selected command.
fn capture_key(h: &mut Harness<'_, PhotocraftApp>, id: u64, k: Key, m: Modifiers) {
    h.state_mut().ui.dialog_mut(id).unwrap().fields.insert("capture".into(), json!(true));
    h.run_steps(2);
    key(h, k, true, m);
    h.run_steps(2);
    key(h, k, false, m);
    h.run_steps(2);
}
