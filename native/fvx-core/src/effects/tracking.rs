use super::{EffectSpec, FxCtx, ParamSpec, check, grp, hidden, no_pad, num, point, popup, slider};
use crate::render::{camsolve, face, stabilize};
use crate::render::image::Img;

pub const CAT: &str = "Face Tracking";

fn pass(_: &FxCtx, _: &mut Img) {}

pub fn specs() -> Vec<EffectSpec> {
    let pts: Vec<ParamSpec> = face::POINTS.iter().map(|(id, name)| point(id, name, 0.5, 0.5)).collect();
    let meas: Vec<ParamSpec> = face::MEASUREMENTS.iter().map(|(id, name)| slider(id, name, 0.0, -100_000.0, 100_000.0, 2)).collect();
    vec![
        EffectSpec { id: "fx.face.trackPoints", name: "Face Track Points", category: CAT, params: pts, render: pass, pad: no_pad },
        EffectSpec { id: "fx.face.measurements", name: "Face Measurements", category: CAT, params: meas, render: pass, pad: no_pad },
        EffectSpec {
            id: "fx.distort.warpStabilizer",
            name: "Warp Stabilizer",
            category: "Distort",
            params: vec![
                popup("result", "Result", 0, &["Smooth Motion", "No Motion"]),
                num("smoothness", "Smoothness", 50.0, 0.0, 1000.0, 0.0, 100.0),
                popup("method", "Method", 3, &["Position", "Position, Scale, Rotation", "Perspective", "Subspace Warp"]),
                check("preserveScale", "Preserve Scale", false),
                popup("framing", "Framing", 2, &["Stabilize Only", "Stabilize, Crop", "Stabilize, Crop, Auto-scale", "Stabilize, Synthesize Edges"]),
                num("maxScale", "Maximum Scale", 150.0, 100.0, 1000.0, 100.0, 200.0),
                num("actionSafe", "Action Safe Margin", 0.0, 0.0, 50.0, 0.0, 50.0),
                num("additionalScale", "Additional Scale", 100.0, 1.0, 1000.0, 50.0, 150.0),
                check("detailed", "Detailed Analysis", false),
                slider("synthRange", "Synthesis Input Range (seconds)", 0.3, 0.0, 30.0, 2),
                num("synthFeather", "Synthesis Edge Feather", 0.0, 0.0, 100.0, 0.0, 100.0),
                hidden("analysis", "Analysis", ""),
            ],
            render: stabilize::render,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.perspective.cameraTracker",
            name: "3D Camera Tracker",
            category: "Perspective",
            params: vec![
                popup("shotType", "Shot Type", 0, &camsolve::SHOT_TYPES),
                num("horizontalAngleOfView", "Horizontal Angle of View", 40.0, 1.0, 170.0, 10.0, 120.0),
                popup("showTrackPoints", "Show Track Points", 1, &["2D Source", "3D Solved"]),
                check("renderTrackPoints", "Render Track Points", false),
                num("trackPointSize", "Track Point Size", 100.0, 0.0, 1000.0, 0.0, 300.0),
                num("targetSize", "Target Size", 100.0, 1.0, 1000.0, 10.0, 300.0),
                grp("advanced", "Advanced"),
                popup("advanced/solveMethod", "Solve Method", 0, &camsolve::SOLVE_METHODS),
                hidden("advanced/methodUsed", "Method Used", ""),
                slider("advanced/averageError", "Average Error", 0.0, 0.0, 1000.0, 3),
                check("advanced/detailedAnalysis", "Detailed Analysis", false),
                hidden("tracks", "Tracks", ""),
                hidden("solve", "Solve", ""),
                hidden("deleted", "Deleted Points", ""),
                hidden("ground", "Ground Plane", ""),
                hidden("camera", "Camera Layer", ""),
            ],
            render: camera_tracker,
            pad: no_pad,
        },
    ]
}

fn camera_tracker(c: &FxCtx, img: &mut Img) {
    if !c.b("renderTrackPoints") {
        return;
    }
    let Some(tr) = camsolve::parsed_tracks(&c.s("tracks")) else { return };
    let solved = c.e("showTrackPoints") == 1;
    let sv = if solved { camsolve::parsed_solve(&c.s("solve")) } else { None };
    if solved && sv.is_none() {
        return;
    }
    let k = tr.frame_of(c.t);
    let deleted: Vec<u32> = c.s("deleted").split(',').filter_map(|v| v.trim().parse().ok()).collect();
    let size = c.f("trackPointSize") / 100.0 * ((tr.frame[2] - tr.frame[0]) * 0.004).max(2.0);
    let mut dots: Vec<([f64; 2], f64, [f32; 3])> = vec![];
    match &sv {
        Some(sv) => {
            let Some(cam) = sv.cams.get(k) else { return };
            let mut depths: Vec<f64> = sv.points.iter().map(|p| cam.depth(p.x)).filter(|d| *d > 0.0).collect();
            depths.sort_by(|a, b| a.total_cmp(b));
            let dmed = depths.get(depths.len() / 2).copied().unwrap_or(1.0);
            let seen: std::collections::HashSet<u32> = tr.tracks.iter().filter(|t| t.at(k).is_some()).map(|t| t.id).collect();
            for p in sv.points.iter().filter(|p| seen.contains(&p.id)) {
                if let Some(q) = sv.project(k, p.x) {
                    dots.push((q, size * (dmed / cam.depth(p.x).max(1e-9)).clamp(0.4, 2.5), camsolve::color_of(p.id)));
                }
            }
        }
        None => {
            for t in tr.tracks.iter().filter(|t| !deleted.contains(&t.id)) {
                if let Some(q) = t.at(k) {
                    dots.push((q, size, camsolve::color_of(t.id)));
                }
            }
        }
    }
    let f = tr.frame;
    for (p, rad, col) in dots {
        let b = [(p[0] - f[0] - c.origin[0]) * c.r, (p[1] - f[1] - c.origin[1]) * c.r];
        let rr = (rad * c.r).max(1.0);
        let (x0, x1) = ((b[0] - rr - 1.0).floor().max(0.0) as usize, ((b[0] + rr + 1.0).ceil().max(0.0) as usize).min(img.w));
        let (y0, y1) = ((b[1] - rr - 1.0).floor().max(0.0) as usize, ((b[1] + rr + 1.0).ceil().max(0.0) as usize).min(img.h));
        for y in y0..y1 {
            for x in x0..x1 {
                let d = (x as f64 + 0.5 - b[0]).hypot(y as f64 + 0.5 - b[1]);
                let a = (rr + 0.5 - d).clamp(0.0, 1.0) as f32;
                if a <= 0.0 {
                    continue;
                }
                let i = (y * img.w + x) * 4;
                for (ch, v) in col.iter().enumerate() {
                    img.px[i + ch] = img.px[i + ch] * (1.0 - a) + v * a;
                }
                img.px[i + 3] = img.px[i + 3] * (1.0 - a) + a;
            }
        }
    }
}
