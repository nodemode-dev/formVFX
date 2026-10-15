use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pf, ps};

fn edges(guides: &[f64], size: f64) -> Vec<f64> {
    let mut out: Vec<f64> = vec![0.0, size];
    out.extend(guides.iter().copied().filter(|g| *g > 0.5 && *g < size - 0.5));
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() < 0.5);
    out
}

fn file_stem(name: &str) -> String {
    let s: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    if s.is_empty() { "comp".into() } else { s }
}

pub fn export(e: &mut Engine, p: &Json) -> Res {
    let (cid, comp) = e.comp_of(p)?;
    let folder = ps(p, "folder").ok_or("pass `folder`")?.to_string();
    let format = ps(p, "format").unwrap_or("png");
    if !format.eq_ignore_ascii_case("png") {
        return Err("format is png".into());
    }
    let t = pf(p, "time").unwrap_or_else(|| e.time_of(cid));
    let (w, h) = (comp.width as f64, comp.height as f64);
    let xs = edges(&comp.guides.iter().filter(|g| g.vertical).map(|g| g.position).collect::<Vec<_>>(), w);
    let ys = edges(&comp.guides.iter().filter(|g| !g.vertical).map(|g| g.position).collect::<Vec<_>>(), h);
    let name = e.project.item(cid).map(|i| file_stem(&i.name)).unwrap_or_else(|| "comp".into());
    std::fs::create_dir_all(&folder).map_err(|err| format!("{folder}: {err}"))?;
    let job = crate::engine::state::FrameJob::new(e, cid, t, 1.0, false, false);
    let (fw, fh, px) = job.run();
    let frame = image::RgbaImage::from_raw(fw, fh, px).ok_or("render failed")?;
    let (kx, ky) = (fw as f64 / w, fh as f64 / h);
    let mut files = vec![];
    for (r, row) in ys.windows(2).enumerate() {
        for (c, col) in xs.windows(2).enumerate() {
            let x0 = (col[0] * kx).round() as u32;
            let y0 = (row[0] * ky).round() as u32;
            let x1 = ((col[1] * kx).round() as u32).min(fw);
            let y1 = ((row[1] * ky).round() as u32).min(fh);
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let tile = image::imageops::crop_imm(&frame, x0, y0, x1 - x0, y1 - y0).to_image();
            let path = std::path::Path::new(&folder).join(format!("{name}_r{}_c{}.png", r + 1, c + 1));
            tile.save(&path).map_err(|err| format!("{}: {err}", path.display()))?;
            files.push(json!({"path": path.display().to_string(), "row": r + 1, "column": c + 1, "x": col[0], "y": row[0], "width": col[1] - col[0], "height": row[1] - row[0]}));
        }
    }
    Ok(json!({"files": files, "rows": ys.len() - 1, "columns": xs.len() - 1, "time": t}))
}
