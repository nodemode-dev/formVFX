use super::comp::{RenderOpts, Renderer, fx_frame};
use super::image::Img;
use super::media::Media;
use super::vision::Gray;
use crate::eval::Eval;
use crate::model::Project;

pub struct Source<'a> {
    pub project: &'a Project,
    pub media: &'a Media,
    pub comp: u64,
    pub layer: u64,
    pub scale: f64,
    pub frame: [f64; 4],
}

impl<'a> Source<'a> {
    pub fn new(project: &'a Project, media: &'a Media, comp: u64, layer: u64, max_side: f64) -> Option<Source<'a>> {
        let c = project.comp(comp)?;
        let l = c.layer(layer)?;
        let (sw, sh) = project.source_size(l, c);
        let frame = fx_frame(l, sw, sh);
        let scale = (max_side / sw.max(sh).max(1.0)).min(1.0);
        Some(Source { project, media, comp, layer, scale, frame })
    }

    pub fn size(&self) -> (usize, usize) {
        (((self.frame[2] - self.frame[0]) * self.scale).ceil().max(1.0) as usize, ((self.frame[3] - self.frame[1]) * self.scale).ceil().max(1.0) as usize)
    }

    pub fn image(&self, t: f64) -> Option<Img> {
        let c = self.project.comp(self.comp)?;
        let l = c.layer(self.layer)?;
        let ev = Eval::new(self.project, self.comp, c, t);
        let r = Renderer::new(self.project, self.media, RenderOpts { scale: 1.0, guides: false, draft: false, motion_blur: false, roi: None, view: None, backend: super::gpu::Backend::Cpu });
        let (bw, bh) = self.size();
        r.layer_buffer(&ev, l, self.scale, 0, self.frame, self.scale, bw, bh, None)
    }

    pub fn masked(&self, t: f64) -> Option<Img> {
        let c = self.project.comp(self.comp)?;
        let l = c.layer(self.layer)?;
        let ev = Eval::new(self.project, self.comp, c, t);
        let r = Renderer::new(self.project, self.media, RenderOpts { scale: 1.0, guides: false, draft: false, motion_blur: false, roi: None, view: None, backend: super::gpu::Backend::Cpu });
        let (bw, bh) = self.size();
        r.layer_buffer(&ev, l, self.scale, 0, self.frame, self.scale, bw, bh, Some(0))
    }

    pub fn gray(&self, t: f64, channel: u32, blur: f32) -> Option<Gray> {
        let img = self.image(t)?;
        let mut g = Gray::from_img(&img, channel);
        g.blur(blur * self.scale as f32);
        Some(g)
    }

    pub fn to_layer(&self, p: [f32; 2]) -> [f64; 2] {
        [p[0] as f64 / self.scale + self.frame[0], p[1] as f64 / self.scale + self.frame[1]]
    }

    pub fn to_buf(&self, p: [f64; 2]) -> [f32; 2] {
        [((p[0] - self.frame[0]) * self.scale) as f32, ((p[1] - self.frame[1]) * self.scale) as f32]
    }
}

pub fn frame_times(project: &Project, comp: u64, layer: u64, from: f64, to: f64) -> Vec<f64> {
    let Some(c) = project.comp(comp) else { return vec![] };
    let Some(l) = c.layer(layer) else { return vec![] };
    let fd = c.frame_duration();
    let (a, b) = (from.max(l.in_point), to.min(l.out_point - fd * 0.5));
    if b < a {
        return vec![];
    }
    let n0 = (a / fd + 1e-6).round() as i64;
    let n1 = (b / fd + 1e-6).floor() as i64;
    (n0..=n1).map(|n| n as f64 * fd).collect()
}
