use eframe::egui;

pub struct CanvasState {
    pub zoom: f32,
    pub pan: egui::Vec2,
    pub compare_toggle: bool,
    pub needs_fit: bool,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            compare_toggle: false,
            needs_fit: true,
        }
    }
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut CanvasState,
    original_tex: Option<&egui::TextureHandle>,
    processed_tex: Option<&egui::TextureHandle>,
    src_dims: Option<(u32, u32)>,
    out_dims: Option<(u32, u32)>,
    is_processing: bool,
) {
    let space_held = ui.ctx().input(|i| i.key_down(egui::Key::Space))
        && !ui.ctx().wants_keyboard_input();
    let show_original = state.compare_toggle || space_held;

    // Top info bar.
    ui.horizontal(|ui| {
        if let Some((sw, sh)) = src_dims {
            ui.label(format!("src: {sw}x{sh}"));
        } else {
            ui.label("src: -");
        }
        ui.separator();
        if let Some((ow, oh)) = out_dims {
            ui.label(format!("out: {ow}x{oh}"));
        } else {
            ui.label("out: -");
        }
        ui.separator();
        ui.label(format!("zoom: {}%", (state.zoom * 100.0).round() as i32));

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("fit").clicked() {
                state.needs_fit = true;
            }
            ui.toggle_value(&mut state.compare_toggle, "compare");
            if is_processing {
                ui.label("processing...");
            } else if original_tex.is_some() {
                ui.weak("hold space to compare");
            }
        });
    });

    ui.separator();

    let available = ui.available_rect_before_wrap();
    let (rect, response) = ui.allocate_exact_size(available.size(), egui::Sense::click_and_drag());

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, egui::Color32::from_rgb(24, 24, 28));

    let active_tex = if show_original {
        original_tex.or(processed_tex)
    } else {
        processed_tex.or(original_tex)
    };

    let Some(tex) = active_tex else {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "drop an image here or click load image",
            egui::FontId::proportional(15.0),
            egui::Color32::from_gray(150),
        );
        return;
    };

    // Fit using consistent reference dimensions so toggling compare never shifts zoom.
    let ref_size = out_dims
        .or(src_dims)
        .map(|(w, h)| egui::vec2(w as f32, h as f32))
        .unwrap_or_else(|| tex.size_vec2());

    if state.needs_fit && rect.width() > 8.0 && rect.height() > 8.0 {
        let sx = (rect.width() * 0.9) / ref_size.x.max(1.0);
        let sy = (rect.height() * 0.9) / ref_size.y.max(1.0);
        state.zoom = sx.min(sy).clamp(0.25, 64.0);
        state.pan = egui::Vec2::ZERO;
        state.needs_fit = false;
    }

    if response.double_clicked() {
        state.needs_fit = true;
    }

    if response.dragged() {
        state.pan += response.drag_delta();
    }

    if response.hovered() {
        let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
        if scroll.abs() > 0.1 {
            let factor = (1.0 + scroll * 0.0025).clamp(0.8, 1.25);
            let old_zoom = state.zoom;
            let new_zoom = (old_zoom * factor).clamp(0.25, 64.0);
            if let Some(pointer_pos) = ui.ctx().input(|i| i.pointer.hover_pos()) {
                let rel = pointer_pos - (rect.center() + state.pan);
                state.pan -= rel * (new_zoom / old_zoom - 1.0);
            }
            state.zoom = new_zoom;
        }
    }

    let draw_size = ref_size * state.zoom;
    let center = rect.center() + state.pan;
    let img_rect = egui::Rect::from_center_size(center, draw_size);

    draw_checkerboard(&painter, img_rect.intersect(rect));

    painter.image(
        tex.id(),
        img_rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        egui::Color32::WHITE,
    );

    if show_original {
        painter.text(
            rect.left_top() + egui::vec2(10.0, 10.0),
            egui::Align2::LEFT_TOP,
            "original",
            egui::FontId::proportional(13.0),
            egui::Color32::from_rgb(240, 220, 120),
        );
    }
}

fn draw_checkerboard(painter: &egui::Painter, rect: egui::Rect) {
    if !rect.is_positive() {
        return;
    }
    let tile = 12.0f32;
    let dark = egui::Color32::from_rgb(38, 38, 44);
    let light = egui::Color32::from_rgb(50, 50, 58);

    let cols = (rect.width() / tile).ceil() as i32;
    let rows = (rect.height() / tile).ceil() as i32;

    // Cap tile count when zoomed out or very large to keep frame time tiny.
    if cols * rows > 4000 {
        painter.rect_filled(rect, 0.0, dark);
        return;
    }

    for r in 0..rows {
        for c in 0..cols {
            let min_x = rect.min.x + c as f32 * tile;
            let min_y = rect.min.y + r as f32 * tile;
            let max_x = (min_x + tile).min(rect.max.x);
            let max_y = (min_y + tile).min(rect.max.y);
            let color = if (r + c) % 2 == 0 { dark } else { light };
            painter.rect_filled(
                egui::Rect::from_min_max(egui::pos2(min_x, min_y), egui::pos2(max_x, max_y)),
                0.0,
                color,
            );
        }
    }
}
