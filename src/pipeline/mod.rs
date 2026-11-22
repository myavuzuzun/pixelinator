pub mod color;
pub mod dither;
pub mod downscale;
pub mod postprocess;
pub mod prefilter;

use image::RgbaImage;

use crate::state::{AppSettings, DitherMode, PaletteMode, SourceMode};

#[derive(Clone, PartialEq, Eq)]
struct Stage1Key {
    source_id: u64,
    source_mode: SourceMode,
    brightness_bits: u32,
    contrast_bits: u32,
    saturation_bits: u32,
    alpha_threshold: bool,
}

#[derive(Clone, PartialEq, Eq)]
struct Stage2Key {
    target_width: u32,
    target_height: u32,
}

#[derive(Clone, PartialEq, Eq)]
struct Stage3Key {
    palette_mode: PaletteMode,
    auto_colors: usize,
    custom_palette_id: Option<u64>,
    dither_mode: DitherMode,
}

#[derive(Clone, PartialEq, Eq)]
struct Stage4Key {
    orphan_scrub: u8,
    outline: bool,
}

pub struct PipelineCache {
    stage1_key: Option<Stage1Key>,
    stage1_img: Option<RgbaImage>,
    stage2_key: Option<Stage2Key>,
    stage2_img: Option<RgbaImage>,
    stage3_key: Option<Stage3Key>,
    stage3_img: Option<RgbaImage>,
    stage4_key: Option<Stage4Key>,
    stage4_img: Option<RgbaImage>,
}

impl PipelineCache {
    pub fn new() -> Self {
        Self {
            stage1_key: None,
            stage1_img: None,
            stage2_key: None,
            stage2_img: None,
            stage3_key: None,
            stage3_img: None,
            stage4_key: None,
            stage4_img: None,
        }
    }

    /// Runs the 4-stage pipeline, recomputing only from the earliest dirty stage.
    pub fn process(
        &mut self,
        source_id: u64,
        src: &RgbaImage,
        settings: &AppSettings,
    ) -> RgbaImage {
        let k1 = Stage1Key {
            source_id,
            source_mode: settings.source_mode,
            brightness_bits: settings.brightness.to_bits(),
            contrast_bits: settings.contrast.to_bits(),
            saturation_bits: settings.saturation.to_bits(),
            alpha_threshold: settings.alpha_threshold,
        };
        let k2 = Stage2Key {
            target_width: settings.target_width.max(1),
            target_height: settings.target_height.max(1),
        };
        let k3 = Stage3Key {
            palette_mode: settings.palette_mode,
            auto_colors: settings.auto_colors,
            custom_palette_id: settings.custom_palette.as_ref().map(|p| p.id),
            dither_mode: settings.dither_mode,
        };
        let k4 = Stage4Key {
            orphan_scrub: settings.orphan_scrub,
            outline: settings.outline,
        };

        if self.stage1_key.as_ref() != Some(&k1) || self.stage1_img.is_none() {
            let out1 = prefilter::apply_prefilter(
                src,
                settings.source_mode,
                settings.brightness,
                settings.contrast,
                settings.saturation,
                settings.alpha_threshold,
            );
            self.stage1_key = Some(k1);
            self.stage1_img = Some(out1);
            self.stage2_img = None;
            self.stage3_img = None;
            self.stage4_img = None;
        }

        if self.stage2_key.as_ref() != Some(&k2) || self.stage2_img.is_none() {
            let s1 = self.stage1_img.as_ref().expect("stage 1 cached");
            let out2 = downscale::downscale_edge_aware(s1, k2.target_width, k2.target_height);
            self.stage2_key = Some(k2);
            self.stage2_img = Some(out2);
            self.stage3_img = None;
            self.stage4_img = None;
        }

        if self.stage3_key.as_ref() != Some(&k3) || self.stage3_img.is_none() {
            let s2 = self.stage2_img.as_ref().expect("stage 2 cached");
            let palette = match settings.palette_mode {
                PaletteMode::Auto => color::extract_auto_palette(s2, settings.auto_colors),
                PaletteMode::Custom => match &settings.custom_palette {
                    Some(cp) if !cp.colors.is_empty() => cp.colors.clone(),
                    _ => color::extract_auto_palette(s2, settings.auto_colors),
                },
            };
            let out3 = dither::apply_palette_and_dither(s2, &palette, settings.dither_mode);
            self.stage3_key = Some(k3);
            self.stage3_img = Some(out3);
            self.stage4_img = None;
        }

        if self.stage4_key.as_ref() != Some(&k4) || self.stage4_img.is_none() {
            let s3 = self.stage3_img.as_ref().expect("stage 3 cached");
            let out4 = postprocess::apply_postprocess(s3, settings.orphan_scrub, settings.outline);
            self.stage4_key = Some(k4);
            self.stage4_img = Some(out4);
        }

        self.stage4_img.as_ref().expect("stage 4 cached").clone()
    }
}
