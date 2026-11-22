use eframe::egui;

use crate::state::{AppSettings, DitherMode, ExportScale, PaletteMode, SourceMode};

#[derive(Default)]
pub struct ControlActions {
    pub load_image: bool,
    pub process_now: bool,
    pub load_palette: bool,
    pub save_png: bool,
    pub settings_changed: bool,
}

pub fn show(
    ui: &mut egui::Ui,
    settings: &mut AppSettings,
    src_dims: Option<(u32, u32)>,
    has_output: bool,
    status_msg: Option<&str>,
) -> ControlActions {
    let mut actions = ControlActions::default();

    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_min_width(ui.available_width());

        // Input & size
        ui.label("input & size");
        if ui.button("load image").clicked() {
            actions.load_image = true;
        }
        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label("width");
            let mut w = settings.target_width;
            if ui
                .add(egui::DragValue::new(&mut w).range(1..=2048).speed(1))
                .changed()
            {
                settings.target_width = w.max(1);
                if settings.lock_aspect {
                    if let Some((sw, sh)) = src_dims {
                        if sw > 0 {
                            settings.target_height =
                                ((settings.target_width as f32 * sh as f32) / sw as f32)
                                    .round()
                                    .max(1.0) as u32;
                        }
                    }
                }
                actions.settings_changed = true;
            }

            ui.label("height");
            let mut h = settings.target_height;
            if ui
                .add(egui::DragValue::new(&mut h).range(1..=2048).speed(1))
                .changed()
            {
                settings.target_height = h.max(1);
                if settings.lock_aspect {
                    if let Some((sw, sh)) = src_dims {
                        if sh > 0 {
                            settings.target_width =
                                ((settings.target_height as f32 * sw as f32) / sh as f32)
                                    .round()
                                    .max(1.0) as u32;
                        }
                    }
                }
                actions.settings_changed = true;
            }
        });

        if ui
            .checkbox(&mut settings.lock_aspect, "lock aspect")
            .changed()
        {
            if settings.lock_aspect {
                if let Some((sw, sh)) = src_dims {
                    if sw > 0 {
                        settings.target_height =
                            ((settings.target_width as f32 * sh as f32) / sw as f32)
                                .round()
                                .max(1.0) as u32;
                        actions.settings_changed = true;
                    }
                }
            }
        }

        ui.horizontal(|ui| {
            ui.label("source mode");
            if ui
                .selectable_value(&mut settings.source_mode, SourceMode::Drawing, "drawing")
                .changed()
            {
                settings.dither_mode = DitherMode::None;
                actions.settings_changed = true;
            }
            if ui
                .selectable_value(&mut settings.source_mode, SourceMode::Photo, "photo")
                .changed()
            {
                actions.settings_changed = true;
            }
        });

        ui.separator();

        // Update mode
        ui.horizontal(|ui| {
            ui.checkbox(&mut settings.auto_update, "auto-update");
            if !settings.auto_update && ui.button("process").clicked() {
                actions.process_now = true;
            }
        });

        ui.separator();

        // Palette
        ui.label("palette");
        ui.horizontal(|ui| {
            if ui
                .selectable_value(&mut settings.palette_mode, PaletteMode::Auto, "auto")
                .changed()
            {
                actions.settings_changed = true;
            }
            if ui
                .selectable_value(&mut settings.palette_mode, PaletteMode::Custom, "custom")
                .changed()
            {
                actions.settings_changed = true;
            }
        });

        match settings.palette_mode {
            PaletteMode::Auto => {
                let mut count = settings.auto_colors;
                if ui
                    .add(egui::Slider::new(&mut count, 2..=64).text("colors"))
                    .changed()
                {
                    settings.auto_colors = count;
                    actions.settings_changed = true;
                }
            }
            PaletteMode::Custom => {
                if ui.button("load palette").clicked() {
                    actions.load_palette = true;
                }
                if let Some(cp) = &settings.custom_palette {
                    ui.small(format!("{} ({} colors)", cp.name, cp.colors.len()));
                } else {
                    ui.small("no palette loaded (using auto)");
                }
            }
        }

        ui.separator();

        // Style & cleanup
        ui.label("style & cleanup");
        egui::ComboBox::from_label("dithering")
            .selected_text(dither_label(settings.dither_mode))
            .show_ui(ui, |ui| {
                for mode in [
                    DitherMode::None,
                    DitherMode::Bayer2x2,
                    DitherMode::Bayer4x4,
                    DitherMode::FloydSteinberg,
                ] {
                    if ui
                        .selectable_value(&mut settings.dither_mode, mode, dither_label(mode))
                        .changed()
                    {
                        actions.settings_changed = true;
                    }
                }
            });

        let mut scrub = settings.orphan_scrub;
        if ui
            .add(egui::Slider::new(&mut scrub, 0..=3).text("orphan scrub"))
            .changed()
        {
            settings.orphan_scrub = scrub;
            actions.settings_changed = true;
        }

        if ui.checkbox(&mut settings.outline, "1px outline").changed() {
            actions.settings_changed = true;
        }
        if ui
            .checkbox(&mut settings.alpha_threshold, "alpha threshold")
            .changed()
        {
            actions.settings_changed = true;
        }

        ui.separator();

        // Adjustments (collapsed by default)
        egui::CollapsingHeader::new("adjustments")
            .default_open(false)
            .show(ui, |ui| {
                if ui
                    .add(egui::Slider::new(&mut settings.brightness, -1.0..=1.0).text("brightness"))
                    .changed()
                {
                    actions.settings_changed = true;
                }
                if ui
                    .add(egui::Slider::new(&mut settings.contrast, -1.0..=1.0).text("contrast"))
                    .changed()
                {
                    actions.settings_changed = true;
                }
                if ui
                    .add(egui::Slider::new(&mut settings.saturation, -1.0..=1.0).text("saturation"))
                    .changed()
                {
                    actions.settings_changed = true;
                }
                if ui.small_button("reset").clicked() {
                    settings.brightness = 0.0;
                    settings.contrast = 0.0;
                    settings.saturation = 0.0;
                    actions.settings_changed = true;
                }
            });

        ui.separator();

        // Export
        ui.label("export");
        egui::ComboBox::from_label("scale")
            .selected_text(scale_label(settings.export_scale))
            .show_ui(ui, |ui| {
                for opt in [
                    ExportScale::X1,
                    ExportScale::X2,
                    ExportScale::X4,
                    ExportScale::X8,
                    ExportScale::Custom(settings.custom_scale_value),
                ] {
                    ui.selectable_value(&mut settings.export_scale, opt, scale_label(opt));
                }
            });

        if let ExportScale::Custom(_) = settings.export_scale {
            ui.horizontal(|ui| {
                ui.label("custom factor");
                if ui
                    .add(
                        egui::DragValue::new(&mut settings.custom_scale_value)
                            .range(1..=32)
                            .speed(0.2),
                    )
                    .changed()
                {
                    settings.custom_scale_value = settings.custom_scale_value.max(1);
                    settings.export_scale = ExportScale::Custom(settings.custom_scale_value);
                }
            });
        }

        ui.add_space(4.0);
        if ui
            .add_enabled(has_output, egui::Button::new("save PNG"))
            .clicked()
        {
            actions.save_png = true;
        }

        if let Some(msg) = status_msg {
            ui.add_space(6.0);
            ui.small(msg);
        }
    });

    actions
}

fn dither_label(mode: DitherMode) -> &'static str {
    match mode {
        DitherMode::None => "none",
        DitherMode::Bayer2x2 => "bayer 2x2",
        DitherMode::Bayer4x4 => "bayer 4x4",
        DitherMode::FloydSteinberg => "floyd-steinberg",
    }
}

fn scale_label(scale: ExportScale) -> String {
    match scale {
        ExportScale::X1 => "1x".to_string(),
        ExportScale::X2 => "2x".to_string(),
        ExportScale::X4 => "4x".to_string(),
        ExportScale::X8 => "8x".to_string(),
        ExportScale::Custom(n) => format!("custom ({n}x)"),
    }
}
