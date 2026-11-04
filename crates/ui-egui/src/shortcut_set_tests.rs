//! Edit › Keyboard Shortcuts › Set in the dialog: a new set saves the dialog's bindings, the
//! dropdown switches sets, Save and Delete wait for OK, and Export / Summarize write the bindings
//! as the dialog shows them.

use std::cell::RefCell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::{Value, json};

use crate::PhotocraftApp;
use crate::file_dialog::FileDialogAnswer;

type Written = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

/// The dialog, with the save picker answering `answers` and writes recorded.
fn dialog(answers: Vec<&str>) -> (Harness<'static, PhotocraftApp>, u64, Written) {
    let written: Written = Rc::default();
    let log = written.clone();
    let mut h = Harness::builder().with_size(egui::vec2(1100.0, 760.0)).with_max_steps(64).build_eframe(move |cc| {
        PhotocraftApp::setup_context(&cc.egui_ctx, Default::default());
        let mut services = crate::Services::default();
        let (picker, _) = crate::file_dialog::fake(answers.iter().map(|a| Some(FileDialogAnswer::SaveTo((*a).into()))).collect());
        services.file_dialog = Some(picker);
        services.write = Some(Box::new(move |path: &str, bytes: &[u8]| {
            log.borrow_mut().push((path.to_string(), bytes.to_vec()));
            Ok(())
        }));
        PhotocraftApp::new(photocraft_engine::Session::new(), services)
    });
    let ctx = h.ctx.clone();
    let id = crate::menus::invoke(h.state_mut(), &ctx, "edit.keyboardShortcuts", json!({})).unwrap()["dialog"].as_u64().unwrap();
    h.run_steps(4);
    (h, id, written)
}

fn field(h: &Harness<'_, PhotocraftApp>, id: u64, name: &str) -> Value {
    h.state().ui.dialogs.iter().find(|d| d.id == id).unwrap().fields.get(name).cloned().unwrap_or(Value::Null)
}

fn set_field(h: &mut Harness<'_, PhotocraftApp>, id: u64, name: &str, v: Value) {
    h.state_mut().ui.dialog_mut(id).unwrap().fields.insert(name.into(), v);
    h.run_steps(2);
}

fn click(h: &mut Harness<'_, PhotocraftApp>, label: &str) {
    // The first match: the set row's buttons come before the dialog's own (its Cancel).
    h.get_all_by_role_and_label(egui::accesskit::Role::Button, label).next().expect("the button").click();
    h.run_steps(3);
}

/// The Set dropdown's shown label: the set in use, with "(modified)" after an edit.
fn shown_set(h: &Harness<'_, PhotocraftApp>) -> String {
    let at = h.query_all_by_label("Set:").next().expect("the Set label").rect().center();
    let combo = h
        .query_all_by_role(egui::accesskit::Role::ComboBox)
        .min_by(|a, b| a.rect().center().distance(at).total_cmp(&b.rect().center().distance(at)))
        .expect("the Set dropdown");
    let node = combo.accesskit_node();
    node.value().or(node.label()).map(|s| s.to_string()).unwrap_or_default()
}

/// Pick `label` in the Set dropdown, as the user does: open it, click the option.
fn pick_set(h: &mut Harness<'_, PhotocraftApp>, label: &str) {
    // The dialog settles where it opens first.
    h.run_steps(6);
    let at = h.query_all_by_label("Set:").next().expect("the Set label").rect().center();
    let at = h
        .query_all_by_role(egui::accesskit::Role::ComboBox)
        .min_by(|a, b| a.rect().center().distance(at).total_cmp(&b.rect().center().distance(at)))
        .expect("the Set dropdown")
        .rect()
        .center();
    h.hover_at(at);
    h.run_steps(1);
    h.drag_at(at);
    h.run_steps(1);
    h.drop_at(at);
    h.run_steps(3);
    let option = h.query_all_by_label(label).rfind(|n| n.accesskit_node().role() != egui::accesskit::Role::ComboBox);
    option.expect("the set in the open list").click();
    h.run_steps(3);
}
