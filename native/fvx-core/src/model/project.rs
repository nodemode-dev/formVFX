use serde::{Deserialize, Serialize};

use super::prop::{self, Group, GroupKind, Ids, Node};
use super::value::{TextDoc, Value};

pub const FORMAT: &str = "formvfx.project/1";
pub const EXTENSION: &str = "fvxproj";

pub const LABELS: [(&str, [u8; 3]); 17] = [
    ("None", [128, 128, 128]),
    ("Red", [191, 64, 64]),
    ("Yellow", [219, 198, 69]),
    ("Aqua", [101, 186, 189]),
    ("Pink", [224, 158, 196]),
    ("Lavender", [172, 158, 218]),
    ("Peach", [232, 182, 138]),
    ("Sea Foam", [178, 210, 172]),
    ("Blue", [80, 112, 218]),
    ("Green", [84, 168, 78]),
    ("Purple", [144, 72, 170]),
    ("Orange", [232, 146, 46]),
    ("Brown", [130, 90, 62]),
    ("Fuchsia", [228, 88, 222]),
    ("Cyan", [74, 168, 212]),
    ("Sandstone", [168, 150, 120]),
    ("Dark Green", [52, 104, 64]),
];

pub fn label_name(i: u8) -> &'static str {
    LABELS.get(i as usize).map(|l| l.0).unwrap_or("None")
}

pub fn label_rgb(i: u8) -> [u8; 3] {
    LABELS.get(i as usize).map(|l| l.1).unwrap_or([128, 128, 128])
}

pub fn label_parse(s: &str) -> Option<u8> {
    if let Ok(n) = s.parse::<u8>() {
        return ((n as usize) < LABELS.len()).then_some(n);
    }
    LABELS.iter().position(|l| l.0.eq_ignore_ascii_case(s)).map(|i| i as u8)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectSettings {
    pub bit_depth: u8,
    pub time_display: String,
    pub frame_start: u32,
    pub audio_rate: u32,
    pub working_space: String,
    pub linearize: bool,
    pub color_engine: String,
    pub ocio_config: String,
    pub blend_linear: bool,
    pub display: String,
    pub view: String,
    pub hdr: String,
    pub output_space: String,
    pub renderer: String,
    pub comment: String,
}

impl Default for ProjectSettings {
    fn default() -> Self {
        ProjectSettings {
            bit_depth: 8,
            time_display: "timecode".into(),
            frame_start: 0,
            audio_rate: 48000,
            working_space: "none".into(),
            linearize: false,
            color_engine: "adobe".into(),
            ocio_config: String::new(),
            blend_linear: false,
            display: String::new(),
            view: String::new(),
            hdr: "clip".into(),
            output_space: String::new(),
            renderer: "gpu".into(),
            comment: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Marker {
    pub time: f64,
    pub duration: f64,
    pub comment: String,
    pub label: u8,
    pub chapter: String,
    pub protected: bool,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub url: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub frame_target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cue_point: Option<CuePoint>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct CuePoint {
    pub name: String,
    pub navigation: bool,
    pub params: Vec<(String, String)>,
}

impl Default for Marker {
    fn default() -> Self {
        Marker { time: 0.0, duration: 0.0, comment: String::new(), label: 0, chapter: String::new(), protected: false, url: String::new(), frame_target: String::new(), cue_point: None }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Guide {
    pub vertical: bool,
    pub position: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Controller {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub layer: u64,
    pub prop: u64,
    pub group: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Comp {
    pub width: u32,
    pub height: u32,
    pub pixel_aspect: f64,
    pub frame_rate: f64,
    pub duration: f64,
    pub display_start: f64,
    pub background: [f32; 3],
    pub work_area: [f64; 2],
    pub layers: Vec<Layer>,
    pub markers: Vec<Marker>,
    pub guides: Vec<Guide>,
    pub hide_shy: bool,
    pub motion_blur: bool,
    pub frame_blending: bool,
    pub draft_3d: bool,
    pub shutter_angle: f64,
    pub shutter_phase: f64,
    pub motion_blur_samples: u32,
    pub preserve_frame_rate: bool,
    pub preserve_resolution: bool,
    pub global_light: [f64; 2],
    pub poster_time: f64,
    pub renderer: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub essential: Vec<Controller>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub template_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vr: Option<Vr>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stereo: Option<Stereo>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Vr {
    pub source: u64,
    pub face: u32,
    pub yaw: f64,
    pub pitch: f64,
    pub roll: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Stereo {
    pub source: u64,
    pub eye: String,
    pub controls: Option<[u64; 2]>,
}

impl Default for Comp {
    fn default() -> Self {
        Comp {
            width: 1920,
            height: 1080,
            pixel_aspect: 1.0,
            frame_rate: 30.0,
            duration: 10.0,
            display_start: 0.0,
            background: [0.0, 0.0, 0.0],
            work_area: [0.0, 10.0],
            layers: vec![],
            markers: vec![],
            guides: vec![],
            hide_shy: false,
            motion_blur: false,
            frame_blending: false,
            draft_3d: false,
            shutter_angle: 180.0,
            shutter_phase: -90.0,
            motion_blur_samples: 16,
            preserve_frame_rate: false,
            preserve_resolution: false,
            global_light: [120.0, 30.0],
            poster_time: 0.0,
            renderer: "classic3d".into(),
            essential: vec![],
            template_name: String::new(),
            vr: None,
            stereo: None,
        }
    }
}

impl Comp {
    pub fn frame_duration(&self) -> f64 {
        1.0 / self.frame_rate.max(0.001)
    }

    pub fn snap(&self, t: f64) -> f64 {
        let fr = self.frame_rate.max(0.001);
        ((t * fr) + 1e-6).floor() / fr
    }

    pub fn layer(&self, id: u64) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    pub fn layer_mut(&mut self, id: u64) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn index_of(&self, id: u64) -> Option<usize> {
        self.layers.iter().position(|l| l.id == id)
    }

    pub fn has_3d(&self) -> bool {
        self.layers.iter().any(|l| l.is_3d())
    }

    pub fn advanced_3d(&self) -> bool {
        self.renderer == "advanced3d"
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FootageKind {
    #[default]
    Image,
    Video,
    Audio,
    Sequence,
    Vector,
    Model,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AlphaMode {
    #[default]
    Straight,
    Premultiplied,
    Ignore,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Footage {
    pub path: String,
    pub kind: FootageKind,
    pub width: u32,
    pub height: u32,
    pub duration: f64,
    pub frame_rate: f64,
    pub has_video: bool,
    pub has_audio: bool,
    pub codec: String,
    pub pixel_aspect: f64,
    pub alpha: AlphaMode,
    pub missing: bool,
    pub loops: u32,
    pub sequence: Vec<String>,
    pub audio_rate: u32,
    pub audio_channels: u32,
    pub model_depth: u32,
    pub color_profile: String,
    pub preserve_rgb: bool,
    pub transfer: String,
    pub layer: Option<u32>,
    pub layer_name: String,
    pub layer_size: bool,
    pub page: u32,
}

impl Default for Footage {
    fn default() -> Self {
        Footage {
            path: String::new(),
            kind: FootageKind::Image,
            width: 0,
            height: 0,
            duration: 0.0,
            frame_rate: 0.0,
            has_video: true,
            has_audio: false,
            codec: String::new(),
            pixel_aspect: 1.0,
            alpha: AlphaMode::Straight,
            missing: false,
            loops: 1,
            sequence: vec![],
            audio_rate: 0,
            audio_channels: 0,
            model_depth: 0,
            color_profile: String::new(),
            preserve_rgb: false,
            transfer: String::new(),
            layer: None,
            layer_name: String::new(),
            layer_size: false,
            page: 0,
        }
    }
}

impl Footage {
    pub fn is_still(&self) -> bool {
        matches!(self.kind, FootageKind::Image | FootageKind::Vector | FootageKind::Model)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Solid {
    pub color: [f64; 3],
    pub width: u32,
    pub height: u32,
    pub pixel_aspect: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ItemKind {
    Folder,
    Comp(Comp),
    Footage(Footage),
    Solid(Solid),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub parent: Option<u64>,
    #[serde(default)]
    pub label: u8,
    #[serde(default)]
    pub comment: String,
    pub kind: ItemKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy: Option<Proxy>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub footage: Footage,
    #[serde(default)]
    pub enabled: bool,
}

impl Item {
    pub fn type_name(&self) -> &'static str {
        match self.kind {
            ItemKind::Folder => "Folder",
            ItemKind::Comp(_) => "Composition",
            ItemKind::Footage(_) => "Footage",
            ItemKind::Solid(_) => "Solid",
        }
    }

    pub fn dimensions(&self) -> Option<(u32, u32)> {
        match &self.kind {
            ItemKind::Comp(c) => Some((c.width, c.height)),
            ItemKind::Footage(f) if f.has_video => Some((f.width, f.height)),
            ItemKind::Solid(s) => Some((s.width, s.height)),
            _ => None,
        }
    }

    pub fn duration(&self) -> Option<f64> {
        match &self.kind {
            ItemKind::Comp(c) => Some(c.duration),
            ItemKind::Footage(f) if !f.is_still() => Some(f.duration),
            _ => None,
        }
    }

    pub fn frame_rate(&self) -> Option<f64> {
        match &self.kind {
            ItemKind::Comp(c) => Some(c.frame_rate),
            ItemKind::Footage(f) if f.frame_rate > 0.0 => Some(f.frame_rate),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Quality {
    #[default]
    Best,
    Draft,
    Wireframe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum FrameBlend {
    #[default]
    Off,
    FrameMix,
    PixelMotion,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Switches {
    pub video: bool,
    pub audio: bool,
    pub solo: bool,
    pub locked: bool,
    pub shy: bool,
    pub collapse: bool,
    pub quality: Quality,
    pub sampling: String,
    pub effects: bool,
    pub frame_blend: FrameBlend,
    pub motion_blur: bool,
    pub adjustment: bool,
    pub three_d: bool,
    pub guide: bool,
    pub markers_locked: bool,
}

impl Default for Switches {
    fn default() -> Self {
        Switches {
            video: true,
            audio: true,
            solo: false,
            locked: false,
            shy: false,
            collapse: false,
            quality: Quality::Best,
            sampling: "Bilinear".into(),
            effects: true,
            frame_blend: FrameBlend::Off,
            motion_blur: false,
            adjustment: false,
            three_d: false,
            guide: false,
            markers_locked: false,
        }
    }
}

macro_rules! blend_modes {
    ($($v:ident => $l:expr),* $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
        pub enum BlendMode { #[default] $($v),* }
        impl BlendMode {
            pub const ALL: &'static [BlendMode] = &[$(BlendMode::$v),*];
            pub fn label(&self) -> &'static str { match self { $(BlendMode::$v => $l),* } }
        }
    };
}

blend_modes! {
    Normal => "Normal",
    Dissolve => "Dissolve",
    DancingDissolve => "Dancing Dissolve",
    Darken => "Darken",
    Multiply => "Multiply",
    ColorBurn => "Color Burn",
    ClassicColorBurn => "Classic Color Burn",
    LinearBurn => "Linear Burn",
    DarkerColor => "Darker Color",
    Add => "Add",
    Lighten => "Lighten",
    Screen => "Screen",
    ColorDodge => "Color Dodge",
    ClassicColorDodge => "Classic Color Dodge",
    LinearDodge => "Linear Dodge",
    LighterColor => "Lighter Color",
    Overlay => "Overlay",
    SoftLight => "Soft Light",
    HardLight => "Hard Light",
    LinearLight => "Linear Light",
    VividLight => "Vivid Light",
    PinLight => "Pin Light",
    HardMix => "Hard Mix",
    Difference => "Difference",
    ClassicDifference => "Classic Difference",
    Exclusion => "Exclusion",
    Subtract => "Subtract",
    Divide => "Divide",
    Hue => "Hue",
    Saturation => "Saturation",
    Color => "Color",
    Luminosity => "Luminosity",
    StencilAlpha => "Stencil Alpha",
    StencilLuma => "Stencil Luma",
    SilhouetteAlpha => "Silhouette Alpha",
    SilhouetteLuma => "Silhouette Luma",
    AlphaAdd => "Alpha Add",
    LuminescentPremul => "Luminescent Premul",
}

impl BlendMode {
    pub fn parse(s: &str) -> Option<BlendMode> {
        let flat = |x: &str| x.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase();
        let w = flat(s);
        BlendMode::ALL.iter().copied().find(|m| flat(m.label()) == w || flat(&format!("{m:?}")) == w)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MatteKind {
    #[default]
    Alpha,
    AlphaInverted,
    Luma,
    LumaInverted,
}

impl MatteKind {
    pub fn label(&self) -> &'static str {
        match self {
            MatteKind::Alpha => "Alpha Matte",
            MatteKind::AlphaInverted => "Alpha Inverted Matte",
            MatteKind::Luma => "Luma Matte",
            MatteKind::LumaInverted => "Luma Inverted Matte",
        }
    }

    pub fn parse(s: &str) -> Option<MatteKind> {
        let w: String = s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_ascii_lowercase().replace("matte", "");
        Some(match w.as_str() {
            "alpha" => MatteKind::Alpha,
            "alphainverted" | "alphainvert" => MatteKind::AlphaInverted,
            "luma" => MatteKind::Luma,
            "lumainverted" | "lumainvert" => MatteKind::LumaInverted,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Matte {
    pub layer: u64,
    pub kind: MatteKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AutoOrient {
    #[default]
    Off,
    AlongPath,
    TowardsCamera,
    TowardsPointOfInterest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Footage,
    Solid,
    Precomp,
    Null,
    Shape,
    Text,
    Camera,
    Light,
    Model,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub kind: LayerKind,
    #[serde(default)]
    pub source: Option<u64>,
    pub in_point: f64,
    pub out_point: f64,
    pub start_time: f64,
    #[serde(default = "hundred")]
    pub stretch: f64,
    #[serde(default)]
    pub parent: Option<u64>,
    #[serde(default)]
    pub switches: Switches,
    #[serde(default)]
    pub blend: BlendMode,
    #[serde(default)]
    pub preserve_transparency: bool,
    #[serde(default)]
    pub track_matte: Option<Matte>,
    #[serde(default)]
    pub label: u8,
    #[serde(default)]
    pub comment: String,
    pub props: Group,
    #[serde(default)]
    pub markers: Vec<Marker>,
    #[serde(default)]
    pub auto_orient: AutoOrient,
    #[serde(default)]
    pub environment: bool,
    #[serde(default)]
    pub env_background: Option<u64>,
}

fn hundred() -> f64 {
    100.0
}

impl Layer {
    pub fn layer_time(&self, comp_t: f64) -> f64 {
        let s = if self.stretch.abs() < 1e-9 { 100.0 } else { self.stretch };
        (comp_t - self.start_time) * 100.0 / s
    }

    pub fn comp_time(&self, layer_t: f64) -> f64 {
        self.start_time + layer_t * self.stretch / 100.0
    }

    pub fn is_active_at(&self, t: f64) -> bool {
        t + 1e-9 >= self.in_point && t < self.out_point - 1e-9
    }

    pub fn is_3d(&self) -> bool {
        self.switches.three_d || matches!(self.kind, LayerKind::Camera | LayerKind::Light | LayerKind::Model)
    }

    pub fn is_model(&self) -> bool {
        self.kind == LayerKind::Model
    }

    pub fn is_camera(&self) -> bool {
        self.kind == LayerKind::Camera
    }

    pub fn is_light(&self) -> bool {
        self.kind == LayerKind::Light
    }

    pub fn transform(&self) -> Option<&Group> {
        self.props.sub("transform")
    }

    pub fn masks(&self) -> Option<&Group> {
        self.props.sub("masks")
    }

    pub fn effects(&self) -> Option<&Group> {
        self.props.sub("effects")
    }

    pub fn contents(&self) -> Option<&Group> {
        self.props.sub("contents")
    }

    pub fn text_prop(&self) -> Option<&prop::Prop> {
        self.props.sub("text")?.get("sourceText")
    }

    pub fn type_name(&self) -> &'static str {
        match self.kind {
            LayerKind::Footage => "Footage",
            LayerKind::Solid => {
                if self.switches.adjustment {
                    "Adjustment"
                } else {
                    "Solid"
                }
            }
            LayerKind::Precomp => "Composition",
            LayerKind::Null => "Null",
            LayerKind::Shape => "Shape",
            LayerKind::Text => "Text",
            LayerKind::Camera => "Camera",
            LayerKind::Light => "Light",
            LayerKind::Model => "Model",
        }
    }

    pub fn has_av(&self) -> bool {
        matches!(self.kind, LayerKind::Footage | LayerKind::Precomp)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RenderItem {
    pub id: u64,
    pub comp: u64,
    pub render: bool,
    pub start: f64,
    pub end: f64,
    pub status: String,
    pub progress: f64,
    pub error: Option<String>,
    pub settings: RenderSettings,
    pub modules: Vec<OutputModule>,
    pub log: String,
    #[serde(skip_serializing)]
    pub format: String,
    #[serde(skip_serializing)]
    pub output: String,
    #[serde(skip_serializing)]
    pub quality: u32,
    #[serde(skip_serializing)]
    pub scale: f64,
    #[serde(skip_serializing)]
    pub audio: String,
    #[serde(skip_serializing)]
    pub output_space: String,
    #[serde(skip_serializing)]
    pub depth: u8,
}

impl Default for RenderItem {
    fn default() -> Self {
        RenderItem {
            id: 0,
            comp: 0,
            render: true,
            start: 0.0,
            end: 0.0,
            status: "Queued".into(),
            progress: 0.0,
            error: None,
            settings: RenderSettings::default(),
            modules: vec![],
            log: "errorsOnly".into(),
            format: "h264".into(),
            output: String::new(),
            quality: 80,
            scale: 1.0,
            audio: "auto".into(),
            output_space: String::new(),
            depth: 8,
        }
    }
}

impl RenderItem {
    pub fn fix(&mut self) {
        if self.modules.is_empty() {
            let channels = if matches!(self.format.as_str(), "png" | "tiff" | "webm") { "rgba" } else { "rgb" };
            self.modules.push(OutputModule { format: self.format.clone(), output: self.output.clone(), quality: self.quality, audio: self.audio.clone(), output_space: self.output_space.clone(), depth: self.depth, channels: channels.into(), ..OutputModule::default() });
            if self.scale > 0.0 && self.scale != 1.0 {
                self.settings.resolution = self.scale;
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RenderSettings {
    pub template: String,
    pub quality: String,
    pub resolution: f64,
    pub proxy_use: String,
    pub effects: String,
    pub solo: String,
    pub guide_layers: String,
    pub color_depth: u8,
    pub frame_blending: String,
    pub field_render: String,
    pub pulldown: String,
    pub motion_blur: String,
    pub time_span: String,
    pub frame_rate: f64,
    pub skip_existing: bool,
    pub storage_overflow: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        RenderSettings {
            template: "Best Settings".into(),
            quality: "best".into(),
            resolution: 1.0,
            proxy_use: "none".into(),
            effects: "current".into(),
            solo: "current".into(),
            guide_layers: "allOff".into(),
            color_depth: 0,
            frame_blending: "onForChecked".into(),
            field_render: "off".into(),
            pulldown: "off".into(),
            motion_blur: "onForChecked".into(),
            time_span: "workArea".into(),
            frame_rate: 0.0,
            skip_existing: false,
            storage_overflow: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct OutputModule {
    pub template: String,
    pub format: String,
    pub output: String,
    pub channels: String,
    pub color: String,
    pub quality: u32,
    pub bitrate: u32,
    pub keyframe_interval: u32,
    pub prores_profile: String,
    pub profile: String,
    pub webm_codec: String,
    pub audio: String,
    pub sample_rate: u32,
    pub audio_channels: String,
    pub audio_format: u32,
    pub audio_bitrate: u32,
    pub resize: Option<[u32; 2]>,
    pub crop: [i32; 4],
    pub post_render: String,
    pub output_space: String,
    pub depth: u8,
    pub loop_gif: bool,
}

impl Default for OutputModule {
    fn default() -> Self {
        OutputModule {
            template: String::new(),
            format: "h264".into(),
            output: String::new(),
            channels: "rgb".into(),
            color: "straight".into(),
            quality: 80,
            bitrate: 0,
            keyframe_interval: 0,
            prores_profile: "hq".into(),
            profile: "main".into(),
            webm_codec: "vp9".into(),
            audio: "auto".into(),
            sample_rate: 48000,
            audio_channels: "stereo".into(),
            audio_format: 16,
            audio_bitrate: 256,
            resize: None,
            crop: [0; 4],
            post_render: "none".into(),
            output_space: String::new(),
            depth: 8,
            loop_gif: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct DataTable {
    pub id: u64,
    pub name: String,
    pub path: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub format: String,
    #[serde(default)]
    pub settings: ProjectSettings,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data: Vec<DataTable>,
    pub items: Vec<Item>,
    pub next_id: u64,
    pub next_uid: u64,
    #[serde(default)]
    pub render_queue: Vec<RenderItem>,
    #[serde(default)]
    pub saved_by: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub uid: String,
}

impl Default for Project {
    fn default() -> Self {
        Project { format: FORMAT.into(), settings: ProjectSettings::default(), data: vec![], items: vec![], next_id: 0, next_uid: 0, render_queue: vec![], saved_by: String::new(), uid: new_project_uid() }
    }
}

pub fn new_project_uid() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNT: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let n = COUNT.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}-{:x}", nanos, std::process::id(), n)
}

impl Project {
    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn ids(&mut self) -> Ids<'_> {
        Ids(&mut self.next_uid)
    }

    pub fn with_layer_ids<R>(&mut self, cid: u64, lid: u64, f: impl FnOnce(&mut Layer, &mut Ids) -> R) -> Option<R> {
        let Project { items, next_uid, .. } = self;
        let it = items.iter_mut().find(|i| i.id == cid)?;
        let ItemKind::Comp(c) = &mut it.kind else { return None };
        let l = c.layers.iter_mut().find(|l| l.id == lid)?;
        Some(f(l, &mut Ids(next_uid)))
    }

    pub fn upgrade_3d(&mut self, only: Option<(u64, u64)>) {
        let Project { items, next_uid, .. } = self;
        for it in items.iter_mut() {
            let ItemKind::Comp(c) = &mut it.kind else { continue };
            for l in c.layers.iter_mut() {
                if only.is_none_or(|(cid, lid)| cid == it.id && lid == l.id) {
                    prop::upgrade_3d_options(l, &mut Ids(next_uid));
                }
            }
        }
    }

    pub fn item(&self, id: u64) -> Option<&Item> {
        self.items.iter().find(|i| i.id == id)
    }

    pub fn item_mut(&mut self, id: u64) -> Option<&mut Item> {
        self.items.iter_mut().find(|i| i.id == id)
    }

    pub fn comp(&self, id: u64) -> Option<&Comp> {
        match &self.item(id)?.kind {
            ItemKind::Comp(c) => Some(c),
            _ => None,
        }
    }

    pub fn comp_mut(&mut self, id: u64) -> Option<&mut Comp> {
        match &mut self.item_mut(id)?.kind {
            ItemKind::Comp(c) => Some(c),
            _ => None,
        }
    }

    pub fn comp_of_layer(&self, layer: u64) -> Option<u64> {
        self.items.iter().find_map(|i| match &i.kind {
            ItemKind::Comp(c) if c.layer(layer).is_some() => Some(i.id),
            _ => None,
        })
    }

    pub fn item_name_exists(&self, name: &str) -> bool {
        self.items.iter().any(|i| i.name == name)
    }

    pub fn unique_item_name(&self, base: &str) -> String {
        if !self.item_name_exists(base) {
            return base.to_string();
        }
        let stem = base.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end();
        let mut n = 2;
        loop {
            let cand = format!("{stem} {n}");
            if !self.item_name_exists(&cand) {
                return cand;
            }
            n += 1;
        }
    }

    pub fn folder_named(&mut self, name: &str) -> u64 {
        if let Some(f) = self.items.iter().find(|i| i.name == name && matches!(i.kind, ItemKind::Folder) && i.parent.is_none()) {
            return f.id;
        }
        let id = self.new_id();
        self.items.push(Item { id, name: name.into(), parent: None, label: 0, comment: String::new(), kind: ItemKind::Folder, proxy: None });
        id
    }

    pub fn source_size(&self, layer: &Layer, comp: &Comp) -> (f64, f64) {
        match layer.kind {
            LayerKind::Null => (100.0, 100.0),
            LayerKind::Shape | LayerKind::Text | LayerKind::Camera | LayerKind::Light => (comp.width as f64, comp.height as f64),
            _ => layer.source.and_then(|s| self.item(s)).and_then(|i| i.dimensions()).map(|(w, h)| (w as f64, h as f64)).unwrap_or((comp.width as f64, comp.height as f64)),
        }
    }

    pub fn uses_item(&self, item: u64) -> usize {
        self.items
            .iter()
            .map(|i| match &i.kind {
                ItemKind::Comp(c) => c.layers.iter().filter(|l| l.source == Some(item)).count(),
                _ => 0,
            })
            .sum()
    }
}

pub struct LayerInit {
    pub kind: LayerKind,
    pub name: String,
    pub source: Option<u64>,
    pub size: (f64, f64),
    pub start: f64,
    pub duration: f64,
}

pub fn build_layer(project: &mut Project, comp_size: (u32, u32), init: LayerInit) -> Layer {
    let id = project.new_id();
    let (cw, ch) = (comp_size.0 as f64, comp_size.1 as f64);
    let (w, h) = init.size;
    let mut ids = project.ids();
    let mut children: Vec<Node> = vec![];
    match init.kind {
        LayerKind::Text => children.push(prop::g(prop::text_group(&mut ids, TextDoc::default()))),
        LayerKind::Shape => children.push(prop::g(ids.group("contents", "Contents", GroupKind::Indexed, vec![]))),
        _ => {}
    }
    if !matches!(init.kind, LayerKind::Camera | LayerKind::Light | LayerKind::Null) {
        children.push(prop::g(ids.group("masks", "Masks", GroupKind::Indexed, vec![])));
    }
    if !matches!(init.kind, LayerKind::Camera | LayerKind::Light) {
        children.push(prop::g(ids.group("effects", "Effects", GroupKind::Indexed, vec![])));
    }
    match init.kind {
        LayerKind::Camera => {
            let zoom = cw * 50.0 / 36.0;
            children.push(prop::g(prop::camera_transform_group(&mut ids, [cw / 2.0, ch / 2.0, 0.0], [cw / 2.0, ch / 2.0, -zoom])));
            children.push(prop::g(prop::camera_options(&mut ids, zoom)));
        }
        LayerKind::Light => {
            children.push(prop::g(prop::camera_transform_group(&mut ids, [cw / 2.0, ch / 2.0, 0.0], [cw / 2.0 - 300.0, ch / 2.0 - 300.0, -400.0])));
            children.push(prop::g(prop::light_options(&mut ids, 2, [1.0, 1.0, 1.0, 1.0])));
        }
        LayerKind::Shape | LayerKind::Text => {
            children.push(prop::g(prop::transform_group(&mut ids, [0.0; 3], [cw / 2.0, ch / 2.0, 0.0])));
        }
        LayerKind::Null => {
            children.push(prop::g(prop::transform_group(&mut ids, [50.0, 50.0, 0.0], [cw / 2.0, ch / 2.0, 0.0])));
        }
        LayerKind::Model => {
            children.push(prop::g(prop::transform_group(&mut ids, [0.0; 3], [cw / 2.0, ch / 2.0, 0.0])));
        }
        _ => {
            children.push(prop::g(prop::transform_group(&mut ids, [w / 2.0, h / 2.0, 0.0], [cw / 2.0, ch / 2.0, 0.0])));
        }
    }
    if !matches!(init.kind, LayerKind::Camera | LayerKind::Light | LayerKind::Null) {
        children.push(prop::g(prop::material_options(&mut ids, init.kind == LayerKind::Model)));
    }
    if matches!(init.kind, LayerKind::Footage | LayerKind::Precomp) {
        children.push(prop::g(prop::audio_group(&mut ids)));
    }
    let root = ids.group("root", "Layer", GroupKind::Plain, children);
    Layer {
        id,
        name: init.name,
        kind: init.kind,
        source: init.source,
        in_point: init.start,
        out_point: init.start + init.duration,
        start_time: init.start,
        stretch: 100.0,
        parent: None,
        switches: Switches { audio: matches!(init.kind, LayerKind::Footage | LayerKind::Precomp), ..Switches::default() },
        blend: BlendMode::Normal,
        preserve_transparency: false,
        track_matte: None,
        label: match init.kind {
            LayerKind::Footage => 3,
            LayerKind::Solid => 1,
            LayerKind::Precomp => 15,
            LayerKind::Null => 2,
            LayerKind::Shape => 8,
            LayerKind::Text => 1,
            LayerKind::Camera => 4,
            LayerKind::Light => 6,
            LayerKind::Model => 10,
        },
        comment: String::new(),
        props: root,
        markers: vec![],
        auto_orient: if matches!(init.kind, LayerKind::Camera | LayerKind::Light) { AutoOrient::TowardsPointOfInterest } else { AutoOrient::Off },
        environment: false,
        env_background: None,
    }
}

pub fn set_text(layer: &mut Layer, doc: TextDoc) {
    if let Some(p) = layer.props.sub_mut("text").and_then(|t| t.get_mut("sourceText")) {
        p.value = Value::Text(Box::new(doc));
    }
}
