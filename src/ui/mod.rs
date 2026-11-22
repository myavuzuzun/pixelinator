pub mod canvas;
pub mod controls;

use std::path::Path;
use std::sync::Arc;

use eframe::egui;
use image::RgbaImage;

use crate::export;
use crate::pipeline::color;
use crate::state::{AppSettings, CustomPalette, PaletteMode};
use crate::ui::canvas::CanvasState;
use crate::worker::{PipelineWorker, WorkerRequest};

pub struct PixelinatorApp {
    settings: AppSettings,
    source_image: Option<Arc<RgbaImage>>,
    source_id: u64,
    processed_image: Option<RgbaImage>,
    original_tex: Option<egui::TextureHandle>,
    processed_tex: Option<egui::TextureHandle>,
    canvas: CanvasState,
    worker: PipelineWorker,
    next_job_id: u64,
    pending_job_id: Option<u64>,
    next_palette_id: u64,
    status_msg: Option<String>,
}

impl PixelinatorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            settings: AppSettings::default(),
            source_image: None,
            source_id: 0,
            processed_image: None,
            original_tex: None,
            processed_tex: None,
            canvas: CanvasState::default(),
            worker: PipelineWorker::spawn(cc.egui_ctx.clone()),
            next_job_id: 1,
            pending_job_id: None,
            next_palette_id: 1,
            status_msg: None,
        }
    }

    fn load_source_from_path(&mut self, ctx: &egui::Context, path: &Path) {
        match image::open(path) {
            Ok(dyn_img) => {
                self.set_source_image(ctx, dyn_img.to_rgba8());
                self.status_msg = None;
            }
            Err(e) => {
                self.status_msg = Some(format!("failed to load image: {e}").to_lowercase());
            }
        }
    }

    fn load_source_from_bytes(&mut self, ctx: &egui::Context, bytes: &[u8]) {
        match image::load_from_memory(bytes) {
            Ok(dyn_img) => {
                self.set_source_image(ctx, dyn_img.to_rgba8());
                self.status_msg = None;
            }
            Err(e) => {
                self.status_msg = Some(format!("failed to decode dropped image: {e}").to_lowercase());
            }
        }
    }

    fn set_source_image(&mut self, ctx: &egui::Context, rgba: RgbaImage) {
        let sw = rgba.width().max(1);
        let sh = rgba.height().max(1);

        let max_dim = 128.0f32;
        if sw as f32 > max_dim || sh as f32 > max_dim {
            let scale = max_dim / (sw.max(sh) as f32);
            self.settings.target_width = ((sw as f32 * scale).round() as u32).max(1);
            self.settings.target_height = ((sh as f32 * scale).round() as u32).max(1);
        } else {
            self.settings.target_width = sw;
            self.settings.target_height = sh;
        }

        self.original_tex = Some(upload_nearest_texture(ctx, "original", &rgba));
        self.source_id += 1;
        self.source_image = Some(Arc::new(rgba));
        self.processed_image = None;
        self.processed_tex = None;
        self.canvas.needs_fit = true;

        self.submit_job(true);
    }

    fn submit_job(&mut self, immediate: bool) {
        let Some(src) = &self.source_image else {
            return;
        };
        let job_id = self.next_job_id;
        self.next_job_id += 1;
        self.pending_job_id = Some(job_id);

        self.worker.submit(WorkerRequest {
            job_id,
            source_id: self.source_id,
            source: Arc::clone(src),
            settings: self.settings.clone(),
            immediate,
        });
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped.into_iter().next() {
            if let Some(path) = file.path {
                self.load_source_from_path(ctx, &path);
            } else if let Some(bytes) = file.bytes {
                self.load_source_from_bytes(ctx, &bytes);
            }
        }
    }

    fn open_image_dialog(&mut self, ctx: &egui::Context) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("images", &["png", "jpg", "jpeg", "webp", "bmp", "gif"])
            .pick_file()
        {
            self.load_source_from_path(ctx, &path);
        }
    }

    fn open_palette_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("palettes", &["hex", "pal", "gpl"])
            .pick_file()
        {
            match color::load_palette_file(&path) {
                Ok(colors) => {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("palette")
                        .to_lowercase();
                    let id = self.next_palette_id;
                    self.next_palette_id += 1;
                    self.settings.custom_palette = Some(CustomPalette { name, colors, id });
                    self.settings.palette_mode = PaletteMode::Custom;
                    self.status_msg = None;
                    if self.settings.auto_update {
                        self.submit_job(false);
                    }
                }
                Err(e) => {
                    self.status_msg = Some(e.to_lowercase());
                }
            }
        }
    }

    fn save_png_dialog(&mut self) {
        let Some(img) = &self.processed_image else {
            return;
        };
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("PNG", &["png"])
            .set_file_name("pixelinator.png")
            .save_file()
        {
            let scale = self.settings.export_scale.factor();
            match export::save_png(img, scale, &path) {
                Ok(()) => {
                    self.status_msg = Some("saved PNG".to_string());
                }
                Err(e) => {
                    self.status_msg = Some(e.to_lowercase());
                }
            }
        }
    }
}

impl eframe::App for PixelinatorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_dropped_files(ctx);

        if let Some(resp) = self.worker.poll() {
            if self.pending_job_id == Some(resp.job_id) {
                self.pending_job_id = None;
            }
            self.processed_tex = Some(upload_nearest_texture(ctx, "processed", &resp.image));
            self.processed_image = Some(resp.image);
        }

        let src_dims = self
            .source_image
            .as_ref()
            .map(|img| (img.width(), img.height()));
        let out_dims = self
            .processed_image
            .as_ref()
            .map(|img| (img.width(), img.height()));

        let mut actions = controls::ControlActions::default();

        egui::SidePanel::right("controls_panel")
            .resizable(false)
            .exact_width(270.0)
            .show(ctx, |ui| {
                actions = controls::show(
                    ui,
                    &mut self.settings,
                    src_dims,
                    self.processed_image.is_some(),
                    self.status_msg.as_deref(),
                );
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            canvas::show(
                ui,
                &mut self.canvas,
                self.original_tex.as_ref(),
                self.processed_tex.as_ref(),
                src_dims,
                out_dims,
                self.pending_job_id.is_some(),
            );
        });

        if actions.load_image {
            self.open_image_dialog(ctx);
        }
        if actions.load_palette {
            self.open_palette_dialog();
        }
        if actions.save_png {
            self.save_png_dialog();
        }
        if actions.process_now {
            self.submit_job(true);
        } else if actions.settings_changed && self.settings.auto_update {
            self.submit_job(false);
        }
    }
}

fn upload_nearest_texture(
    ctx: &egui::Context,
    name: &str,
    img: &RgbaImage,
) -> egui::TextureHandle {
    let size = [img.width() as usize, img.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    ctx.load_texture(name, color_image, egui::TextureOptions::NEAREST)
}
