//! The Liquify Brush tool: the Liquify dialog's displacement field as a canvas tool. A drag
//! dabs the field like the dialog's brush strokes (the exact recorded data) and shows the active
//! layer warped live; releasing runs `filter.liquify` with the same strokes, so the drag is one
//! "Liquify" history step.
//!
//! The preview is exact: every dab re-renders only its dirty rectangle from the layer's ORIGINAL
//! pixels with the CURRENT field ([`photocraft_engine::distort_cmds::liquify_preview_region`]) —
//! the same per-tile computation the commit's fresh field replays (see
//! `photocraft_algo::liquify::resample_tiles`) — so with no selection the pixels a cache already
//! shows are the commit's bit for bit, and releasing counts the preview as the document
//! (`canvas::shown_as_document`): only the dabs a cache had not shown yet recomposite. With a
//! selection each touched area is also remixed by the selection's coverage in the same call (the
//! selection is captured at the press, what the commit mixes with), so the same statement holds
//! where the selection shows.
//!
//! A smart object commits a Smart Filter (the Layers panel's normal place to edit it), whose
//! result differs from the preview pixels — the canvas then recomposites it as usual.

use std::sync::Arc;

use egui::Modifiers;
use photocraft_algo::liquify::{LiquifyField, LiquifyStroke, LiquifyTool, auto_cell};
use photocraft_doc::{DocId, Document, LayerContent, LayerId};
use photocraft_geom::Rect;
use photocraft_raster::Surface;
use serde_json::json;

use crate::PhotocraftApp;
use crate::canvas::{Drag, ToolEvent, shown_as_document};
use crate::state::Tool;

/// Preview keys of Liquify Brush drags: `BASE + n`, one per dab shown (see `canvas::display_doc`).
const BASE: u64 = 1 << 43;

/// One drag's runtime state: the layer captured at the press, the field its dabs have edited,
/// and the preview pixels the canvas composite shows.
pub(crate) struct LiquifyBrushPreview {
    doc: DocId,
    revision: u64,
    layer: LayerId,
    smart: bool,
    /// The layer's pixels at the press (a smart object keeps its rendering): every tile
    /// re-render resamples from it with the field, so it writes what the commit writes.
    source: Surface,
    /// The selection at the press: what the commit mixes its result by as well.
    selection: Option<Surface>,
    /// Field resolution (px per node), also passed to the commit's `meshSize`.
    cell: f64,
    field: LiquifyField,
    /// The stroke being drawn and its last point.
    cur: Option<(LiquifyStroke, [f64; 3])>,
    /// The layer's warped pixels so far (the source wherever no dab reached).
    preview: Surface,
    /// The document rect each dab changed; preview key `BASE + n` shows the first `n`.
    areas: Vec<Rect>,
    /// The document the canvas shows (the preview in the active layer), one per key.
    shown: Option<Arc<Document>>,
}

/// The layer a stroke would land on and everything a preview needs from it, or why it can't.
struct Target {
    doc: DocId,
    layer: LayerId,
    source: Surface,
    smart: bool,
    canvas: Rect,
    selection: Option<Surface>,
    revision: u64,
}

/// The pointer events of the Liquify Brush: a press on the active layer starts a stroke (the
/// default Forward Warp tool), its moves keep dabbing, releasing commits. Returns true for the
/// events it takes.
pub fn pointer(app: &mut PhotocraftApp, ev: ToolEvent, mods: Modifiers) -> bool {
    match ev {
        ToolEvent::Down { x, y, pressure } => {
            // A drag under way continues over its moves, release and further presses, even when
            // the pointer's tool became another one midway (`hold_keys::cmd_moves`).
            if app.drag.as_ref().is_some_and(|d| d.tool == Tool::LiquifyBrush) {
                return true;
            }
            if app.drag.is_some() || app.tool_override.is_some() || app.ui.tool != Tool::LiquifyBrush {
                return false;
            }
            start(app, [x, y, f64::from(pressure)], mods)
        }
        ToolEvent::Move { x, y, pressure } => {
            if !app.drag.as_ref().is_some_and(|d| d.tool == Tool::LiquifyBrush) {
                return false;
            }
            dab(app, [x, y, f64::from(pressure)]);
            true
        }
        ToolEvent::Up { x, y } => {
            if !app.drag.as_ref().is_some_and(|d| d.tool == Tool::LiquifyBrush) {
                return false;
            }
            release(app, [x, y])
        }
    }
}

/// The layer the stroke would land on and everything a preview needs from it, or why it can't.
fn target(app: &PhotocraftApp) -> Result<Target, String> {
    let (layer, source, smart) = crate::distort_ui::active_pixels(app)?;
    let st = app.session.active().ok_or_else(|| "no document open".to_string())?;
    let locks = st.doc.effective_locks(layer);
    if locks.all || locks.pixels {
        let name = st.doc.layer(layer).map(|l| l.name.clone()).unwrap_or_default();
        return Err(format!("Could not complete your request because the layer \"{name}\" is locked"));
    }
    Ok(Target {
        doc: st.doc.id,
        layer,
        source,
        smart,
        canvas: st.doc.bounds(),
        selection: st.doc.selection.clone(),
        revision: st.revision,
    })
}

/// Starts the stroke: captures the layer, dabs once at the press and shows the result. The
/// press is consumed when the active tool is the Liquify Brush, whatever the checks find.
fn start(app: &mut PhotocraftApp, p: [f64; 3], mods: Modifiers) -> bool {
    if let Some(hide) = photocraft_engine::hidden_target::refusal(&app.session, "paint.stroke", &json!({})) {
        app.ui.status = hide.into();
        app.ui.status_error = true;
        return true;
    }
    let target = match target(app) {
        Ok(t) => t,
        Err(e) => {
            app.ui.status = e;
            app.ui.status_error = true;
            return true;
        }
    };
    let o = &app.ui.tool_options;

    // Default behaviour only: the tool stays the Forward Warp the options bar starts with.
    let (tool, size, density, pressure, rate) = (
        LiquifyTool::ForwardWarp,
        // f64::from(o.liquify_size.clamp(1.0, 15000.0)),
        // TODO: figure out how to get current preset here and take values from it
        f64::from(app.session.tools.brush.size),
        f64::from(o.liquify_density.clamp(0.0, 100.0)),
        f64::from(o.liquify_pressure.clamp(0.0, 100.0)),
        f64::from(o.liquify_rate.clamp(0.0, 100.0)),
    );
    let stroke = LiquifyStroke { tool, size, density, pressure, rate, points: vec![p.to_vec()], amount: None };
    let cell = auto_cell(target.canvas);
    let mut field = LiquifyField::new(target.canvas, cell);
    let t0 = crate::gpu_canvas::now_ms();
    let dirty = field.stroke_begin(&stroke, p);
    let mut preview = target.source.clone();
    if !dirty.is_empty() {
        photocraft_engine::distort_cmds::liquify_preview_region(&target.source, &mut preview, &field, target.selection.as_ref(), dirty);
    }
    app.liquify_brush = Some(LiquifyBrushPreview {
        doc: target.doc,
        revision: target.revision,
        layer: target.layer,
        smart: target.smart,
        source: target.source,
        selection: target.selection,
        cell,
        field,
        cur: Some((stroke, p)),
        preview,
        areas: vec![dirty],
        shown: None,
    });
    app.drag = Some(Drag::new(Tool::LiquifyBrush, [p[0], p[1]], vec![p], mods, false));
    if !dirty.is_empty() {
        rebuild_shown(app);
    }
    app.perf.span("liquify brush", crate::gpu_canvas::now_ms() - t0);
    true
}

/// One more point on the open stroke: the field dabs there, its touched rectangle is re-rendered
/// (and remixed by the selection) into the preview, and the composite gets a new preview key.
fn dab(app: &mut PhotocraftApp, point: [f64; 3]) {
    let t0 = crate::gpu_canvas::now_ms();
    let touched = {
        let Some(st) = app.liquify_brush.as_mut() else { return };
        let Some((mut s, last)) = st.cur.take() else { return };
        let dirty = st.field.stroke_segment(&s, last, point);
        s.points.push(point.to_vec());
        let touched = !dirty.is_empty();
        if touched {
            photocraft_engine::distort_cmds::liquify_preview_region(&st.source, &mut st.preview, &st.field, st.selection.as_ref(), dirty);
        }
        st.areas.push(dirty);
        st.cur = Some((s, point));
        touched
    };
    if touched {
        rebuild_shown(app);
    }
    app.perf.span("liquify brush", crate::gpu_canvas::now_ms() - t0);
}

/// The document the canvas composite shows: the preview in the active layer (through the colour
/// mode's display, like every preview), from the session document the drag belongs to.
fn shown_doc(app: &PhotocraftApp, p: &LiquifyBrushPreview) -> Option<Document> {
    let base = app.session.documents().iter().find(|st| st.doc.id == p.doc)?.doc.clone();
    let mut d = (*base).clone();
    let layer = d.layer_mut(p.layer)?;
    layer.content = LayerContent::Raster(p.preview.clone());
    Some(photocraft_engine::mode_cmds::display_document(&d).unwrap_or(d))
}

/// Rebuilds the shown preview document for the current key; when the document is gone the drag
/// goes with it (the preview would have nothing to show anyway).
fn rebuild_shown(app: &mut PhotocraftApp) {
    let shown = match app.liquify_brush.as_ref().map(|p| shown_doc(app, p).map(Arc::new)) {
        Some(shown) => shown,
        None => return,
    };
    match (shown, app.liquify_brush.as_mut()) {
        (Some(shown), Some(p)) => p.shown = Some(shown),
        _ => app.liquify_brush = None,
    }
}

/// Releasing: the last point lands in the recording, the stroke commits as one `filter.liquify`
/// history step, and for a pixel layer the canvas counts the preview as the document (a smart
/// object commits a Smart Filter, whose result differs from the preview pixels).
fn release(app: &mut PhotocraftApp, at: [f64; 2]) -> bool {
    let Some(mut p) = app.liquify_brush.take() else { return false };
    app.drag = None;
    let Some((mut stroke, last)) = p.cur.take() else { return true };
    // The button can come up at a point the last Move didn't land on: it ends the path there.
    let point = [at[0], at[1], last[2]];
    let dirty = p.field.stroke_segment(&stroke, last, point);
    stroke.points.push(point.to_vec());
    p.areas.push(dirty);
    if !dirty.is_empty() {
        photocraft_engine::distort_cmds::liquify_preview_region(&p.source, &mut p.preview, &p.field, p.selection.as_ref(), dirty);
        p.shown = shown_doc(app, &p).map(Arc::new);
    }
    let (doc, smart, cell, layer) = (p.doc, p.smart, p.cell, p.layer);
    if let Err(e) = app.run(photocraft_engine::distort_cmds::LIQUIFY, json!({ "strokes": [stroke], "meshSize": cell, "layer": layer.0 })) {
        app.ui.status = e;
        app.ui.status_error = true;
    } else if !smart {
        shown_as_document(app, doc, is_preview_key);
    }
    true
}

/// The document to render while a Liquify Brush drag is on document `idx`: its preview, keyed
/// `BASE + n` for the `n` dabs shown (see `canvas::display_doc`).
pub(crate) fn display_doc(app: &mut PhotocraftApp, idx: usize) -> Option<(Arc<Document>, u64)> {
    if app.session.active_index() != Some(idx) {
        return None;
    }
    let p = app.liquify_brush.as_ref()?;
    let document = app.session.documents().get(idx)?.doc.id;
    if document != p.doc {
        return None;
    }
    Some((p.shown.clone()?, BASE + p.areas.len() as u64))
}

/// What changed between preview (or document) key `seen` and `now` of the current drag on
/// document `doc` at `revision`: the areas every dab in between changed. `None` when either key
/// isn't one of this drag's (the canvas then recomposites everything).
pub(crate) fn damage(app: &PhotocraftApp, doc: DocId, revision: u64, seen: u64, now: u64) -> Option<Rect> {
    let p = app.liquify_brush.as_ref().filter(|p| p.doc == doc && p.revision == revision)?;
    let ours = |k: u64| k == 0 || is_preview_key(k);
    if seen == now || !ours(seen) || !ours(now) {
        return None;
    }
    // A key maps to the number of dabs its cache had shown (`0` had shown none); several moves
    // can land in one frame, so the whole range joins (not just the endpoints).
    let updates = |k: u64| if k == 0 { Some(0_usize) } else if is_preview_key(k) { Some((k - BASE) as usize) } else { None };
    let e = updates(now)?.min(p.areas.len());
    let s = updates(seen)?.min(e);
    let mut r = Rect::EMPTY;
    for area in &p.areas[s..e] {
        r = r.union(area);
    }
    Some(r)
}

/// Whether a drag's preview is on the canvas.
pub(crate) fn showing(app: &PhotocraftApp) -> bool {
    app.liquify_brush.as_ref().is_some_and(|p| p.shown.is_some())
}

/// Whether `key` is a Liquify Brush drag preview key.
pub(crate) fn is_preview_key(key: u64) -> bool {
    key > BASE && key < BASE << 1
}

/// The drag belongs to its document and its tool: a tool change or the active document changing
/// drops the preview, so the canvas recomposites the committed document on its own.
pub(crate) fn cancel_stale(app: &mut PhotocraftApp) {
    let Some(p) = app.liquify_brush.as_ref() else { return };
    let dragging = app.drag.as_ref().is_some_and(|d| d.tool == Tool::LiquifyBrush);
    if !dragging || app.session.active().map(|st| st.doc.id) != Some(p.doc) {
        app.liquify_brush = None;
        if app.drag.as_ref().is_some_and(|d| d.tool == Tool::LiquifyBrush) {
            app.drag = None;
        }
    }
}

/// Options bar for the Liquify Brush (the brush Size and the Liquify dialog's Density, Pressure
/// and Rate, %; the tool is the default Forward Warp). Returns false for other tools.
pub fn options_bar(app: &mut PhotocraftApp, ui: &mut egui::Ui, tool: Tool) -> bool {
    if tool != Tool::LiquifyBrush {
        return false;
    }
    let o = &mut app.ui.tool_options;
    opt(ui, "Size:");
    crate::widgets::value_field(ui, &mut o.liquify_size, 1.0..=15000.0, "px", 62.0);
    opt(ui, "Density:");
    crate::widgets::value_field(ui, &mut o.liquify_density, 0.0..=100.0, "%", 54.0);
    opt(ui, "Pressure:");
    crate::widgets::value_field(ui, &mut o.liquify_pressure, 0.0..=100.0, "%", 54.0);
    opt(ui, "Rate:");
    crate::widgets::value_field(ui, &mut o.liquify_rate, 0.0..=100.0, "%", 54.0);
    true
}

/// A dim options-bar label, as the other options bars show their own.
fn opt(ui: &mut egui::Ui, text: &str) {
    let t = crate::theme::Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(tl!(text)).color(t.text_dim).size(12.0));
}