#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SourceMode {
    Drawing,
    Photo,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DitherMode {
    None,
    Bayer2x2,
    Bayer4x4,
    FloydSteinberg,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaletteMode {
    Auto,
    Custom,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportScale {
    X1,
    X2,
    X4,
    X8,
    Custom(u32),
}

impl ExportScale {
    pub fn factor(self) -> u32 {
        match self {
            ExportScale::X1 => 1,
            ExportScale::X2 => 2,
            ExportScale::X4 => 4,
            ExportScale::X8 => 8,
            ExportScale::Custom(n) => n.max(1),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CustomPalette {
    pub name: String,
    pub colors: Vec<[u8; 3]>,
    pub id: u64,
}

#[derive(Clone, PartialEq, Debug)]
pub struct AppSettings {
    pub target_width: u32,
    pub target_height: u32,
    pub lock_aspect: bool,
    pub source_mode: SourceMode,
    pub auto_update: bool,
    pub palette_mode: PaletteMode,
    pub auto_colors: usize,
    pub custom_palette: Option<CustomPalette>,
    pub dither_mode: DitherMode,
    pub orphan_scrub: u8,
    pub outline: bool,
    pub alpha_threshold: bool,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub export_scale: ExportScale,
    pub custom_scale_value: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            target_width: 128,
            target_height: 128,
            lock_aspect: true,
            source_mode: SourceMode::Drawing,
            auto_update: true,
            palette_mode: PaletteMode::Auto,
            auto_colors: 16,
            custom_palette: None,
            dither_mode: DitherMode::None,
            orphan_scrub: 0,
            outline: false,
            alpha_threshold: true,
            brightness: 0.0,
            contrast: 0.0,
            saturation: 0.0,
            export_scale: ExportScale::X4,
            custom_scale_value: 6,
        }
    }
}
