use std::sync::Arc;

use hayro::hayro_interpret::InterpreterSettings;
use hayro::hayro_syntax::Pdf;
use hayro::{PixmapSettings, RenderCache, RenderSettings};

use super::image::Img;

pub const EXT: [&str; 2] = ["pdf", "ai"];

pub fn sniff(b: &[u8]) -> bool {
    b.len() > 5 && b[..b.len().min(1024)].windows(5).any(|w| w == b"%PDF-")
}

fn open(path: &str) -> Result<Pdf, String> {
    let b = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    if !sniff(&b) {
        return Err(format!("{path}: not a PDF-compatible file (.ai files need PDF compatibility saved in)"));
    }
    Pdf::new(Arc::new(b)).map_err(|e| format!("{path}: {e:?}"))
}

pub fn pages(path: &str) -> Result<Vec<(f32, f32)>, String> {
    let pdf = open(path)?;
    let out: Vec<(f32, f32)> = pdf.pages().iter().map(|p| p.render_dimensions()).collect();
    if out.is_empty() {
        return Err(format!("{path}: the document has no pages"));
    }
    Ok(out)
}

pub fn render(path: &str, page: usize, w: usize, h: usize) -> Result<Img, String> {
    let pdf = open(path)?;
    let pages = pdf.pages();
    let p = pages.get(page).ok_or_else(|| format!("{path}: no page {}", page + 1))?;
    let (pw, ph) = p.render_dimensions();
    let settings = PixmapSettings { x_scale: w as f32 / pw.max(1.0), y_scale: h as f32 / ph.max(1.0), ..PixmapSettings::default() };
    let cache = RenderCache::new();
    let pm = hayro::render(p, &cache, &InterpreterSettings::default(), &RenderSettings::default(), &settings);
    let (iw, ih) = (pm.width() as usize, pm.height() as usize);
    let img = Img::from_rgba8(iw, ih, pm.data_as_u8_slice(), true);
    if iw == w && ih == h {
        return Ok(img);
    }
    let mut out = Img::new(w.max(1), h.max(1));
    for y in 0..out.h.min(ih) {
        for x in 0..out.w.min(iw) {
            let (s, d) = ((y * iw + x) * 4, (y * out.w + x) * 4);
            out.px[d..d + 4].copy_from_slice(&img.px[s..s + 4]);
        }
    }
    Ok(out)
}
