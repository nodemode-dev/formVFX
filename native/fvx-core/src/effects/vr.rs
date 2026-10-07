use std::f64::consts::{PI, TAU};

use rayon::prelude::*;

use super::util::{gaussian, lum, perlin3, rand01};
use super::{EffectSpec, FxCtx, angle, check, color, layer_param, no_pad, num, pct, point, popup, slider};
use crate::render::image::Img;

const CAT: &str = "Immersive Video";

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec {
            id: "fx.vr.converter",
            name: "VR Converter",
            category: CAT,
            params: vec![popup("source", "Source Layout", 0, &LAYOUTS), popup("output", "Output Layout", 1, &LAYOUTS), slider("fov", "Fisheye FOV", 180.0, 90.0, 360.0, 0), angle("pan", "Pan", 0.0), angle("tilt", "Tilt", 0.0), angle("roll", "Roll", 0.0)],
            render: converter,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.planeToSphere",
            name: "VR Plane to Sphere",
            category: CAT,
            params: vec![layer_param("plane", "Plane Layer"), angle("longitude", "Longitude", 0.0), angle("latitude", "Latitude", 0.0), angle("rotation", "Rotation", 0.0), slider("fov", "Field of View", 60.0, 1.0, 179.0, 1), check("composite", "Composite On Original", true)],
            render: plane_to_sphere,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.sphereToPlane",
            name: "VR Sphere to Plane",
            category: CAT,
            params: vec![slider("fov", "Field of View", 90.0, 1.0, 179.0, 1), angle("pan", "Pan", 0.0), angle("tilt", "Tilt", 0.0), angle("roll", "Roll", 0.0)],
            render: sphere_to_plane,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.rotateSphere",
            name: "VR Rotate Sphere",
            category: CAT,
            params: vec![angle("tilt", "Tilt (X Axis)", 0.0), angle("pan", "Pan (Y Axis)", 0.0), angle("roll", "Roll (Z Axis)", 0.0)],
            render: rotate_sphere,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.blur",
            name: "VR Blur",
            category: CAT,
            params: vec![num("blurriness", "Blurriness", 0.0, 0.0, 500.0, 0.0, 100.0)],
            render: |c, img| sphere_blur(img, c.px(c.f("blurriness")) * 0.5),
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.sharpen",
            name: "VR Sharpen",
            category: CAT,
            params: vec![num("amount", "Sharpen Amount", 0.0, 0.0, 500.0, 0.0, 200.0)],
            render: |c, img| {
                let k = c.f("amount") as f32 / 100.0;
                if k <= 0.0 {
                    return;
                }
                let mut b = img.clone();
                sphere_blur(&mut b, 1.5 * c.r);
                super::util::unsharp_into(img, &b, k);
            },
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.glow",
            name: "VR Glow",
            category: CAT,
            params: vec![slider("threshold", "Luminance Threshold", 0.6, 0.0, 1.0, 2), num("radius", "Glow Radius", 30.0, 0.0, 500.0, 0.0, 200.0), slider("brightness", "Glow Brightness", 1.0, 0.0, 10.0, 2), check("tint", "Tint", false), color("tintColor", "Tint Color", [1.0, 1.0, 1.0])],
            render: vr_glow,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.chromatic",
            name: "VR Chromatic Aberrations",
            category: CAT,
            params: vec![point("center", "Center Point", 0.5, 0.5), num("red", "Aberration (Red)", 0.0, -100.0, 100.0, -20.0, 20.0), num("green", "Aberration (Green)", 0.0, -100.0, 100.0, -20.0, 20.0), num("blue", "Aberration (Blue)", 0.0, -100.0, 100.0, -20.0, 20.0), slider("falloff", "Falloff Distance", 50.0, 0.0, 100.0, 1)],
            render: chromatic,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.colorGradients",
            name: "VR Color Gradients",
            category: CAT,
            params: vec![point("p1", "Point 1", 0.25, 0.25), color("c1", "Color 1", [1.0, 0.2, 0.2]), point("p2", "Point 2", 0.75, 0.25), color("c2", "Color 2", [0.2, 1.0, 0.2]), point("p3", "Point 3", 0.25, 0.75), color("c3", "Color 3", [0.2, 0.2, 1.0]), point("p4", "Point 4", 0.75, 0.75), color("c4", "Color 4", [1.0, 1.0, 0.2]), slider("blend", "Gradient Power", 2.0, 0.1, 10.0, 2), pct("opacity", "Opacity", 100.0), popup("mode", "Blending Mode", 0, &["Normal", "Add", "Multiply", "Screen", "Overlay"])],
            render: color_gradients,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.denoise",
            name: "VR De-Noise",
            category: CAT,
            params: vec![pct("amount", "Noise Reduction", 50.0), slider("radius", "Radius", 2.0, 1.0, 10.0, 1)],
            render: denoise,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.digitalGlitch",
            name: "VR Digital Glitch",
            category: CAT,
            params: vec![pct("master", "Master Amplitude", 50.0), slider("blocks", "Block Count", 20.0, 1.0, 200.0, 0), num("displacement", "Displacement", 40.0, 0.0, 500.0, 0.0, 200.0), num("rgbSplit", "RGB Split", 10.0, 0.0, 200.0, 0.0, 100.0), slider("speed", "Speed", 10.0, 0.0, 60.0, 1), slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0)],
            render: glitch,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.vr.fractalNoise",
            name: "VR Fractal Noise",
            category: CAT,
            params: vec![popup("type", "Fractal Type", 0, &["Basic", "Turbulent"]), slider("contrast", "Contrast", 100.0, 0.0, 1000.0, 1), slider("brightness", "Brightness", 0.0, -200.0, 200.0, 1), slider("scale", "Scale", 4.0, 0.1, 100.0, 2), slider("complexity", "Complexity", 6.0, 1.0, 20.0, 1), angle("evolution", "Evolution", 0.0), pct("opacity", "Opacity", 100.0)],
            render: vr_fractal,
            pad: no_pad,
        },
    ]
}

const LAYOUTS: [&str; 4] = ["Equirectangular", "Cube Map 3x2", "Fisheye", "Rectilinear"];

fn rot(v: [f64; 3], pan: f64, tilt: f64, roll: f64) -> [f64; 3] {
    let (sr, cr) = roll.sin_cos();
    let v = [v[0] * cr - v[1] * sr, v[0] * sr + v[1] * cr, v[2]];
    let (st, ct) = tilt.sin_cos();
    let v = [v[0], v[1] * ct - v[2] * st, v[1] * st + v[2] * ct];
    let (sp, cp) = pan.sin_cos();
    [v[0] * cp + v[2] * sp, v[1], -v[0] * sp + v[2] * cp]
}

fn dir_from_equi(u: f64, v: f64) -> [f64; 3] {
    let lon = (u - 0.5) * TAU;
    let lat = (0.5 - v) * PI;
    [lat.cos() * lon.sin(), -lat.sin(), lat.cos() * lon.cos()]
}

fn equi_from_dir(d: [f64; 3]) -> (f64, f64) {
    let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-12);
    let lon = d[0].atan2(d[2]);
    let lat = (-d[1] / l).clamp(-1.0, 1.0).asin();
    (lon / TAU + 0.5, 0.5 - lat / PI)
}

fn cube_dir(u: f64, v: f64) -> Option<[f64; 3]> {
    let col = (u * 3.0).floor().clamp(0.0, 2.0) as usize;
    let row = (v * 2.0).floor().clamp(0.0, 1.0) as usize;
    let fu = (u * 3.0 - col as f64) * 2.0 - 1.0;
    let fv = (v * 2.0 - row as f64) * 2.0 - 1.0;
    Some(match (row, col) {
        (0, 0) => [-1.0, fv, fu],
        (0, 1) => [fu, fv, 1.0],
        (0, 2) => [1.0, fv, -fu],
        (1, 0) => [-fu, -1.0, -fv],
        (1, 1) => [-fu, fv, -1.0],
        _ => [fu, 1.0, fv],
    })
}

fn dir_cube(d: [f64; 3]) -> (f64, f64) {
    let (ax, ay, az) = (d[0].abs(), d[1].abs(), d[2].abs());
    let (row, col, fu, fv) = if ax >= ay && ax >= az {
        if d[0] < 0.0 { (0, 0, d[2] / ax, d[1] / ax) } else { (0, 2, -d[2] / ax, d[1] / ax) }
    } else if ay >= az {
        if d[1] < 0.0 { (1, 0, -d[0] / ay, -d[2] / ay) } else { (1, 2, d[0] / ay, d[2] / ay) }
    } else if d[2] > 0.0 {
        (0, 1, d[0] / az, d[1] / az)
    } else {
        (1, 1, -d[0] / az, d[1] / az)
    };
    (((fu + 1.0) * 0.5 + col as f64) / 3.0, ((fv + 1.0) * 0.5 + row as f64) / 2.0)
}

fn layout_dir(layout: u32, u: f64, v: f64, fov: f64) -> Option<[f64; 3]> {
    match layout {
        1 => cube_dir(u, v),
        2 => {
            let (x, y) = (u * 2.0 - 1.0, v * 2.0 - 1.0);
            let r = x.hypot(y);
            if r > 1.0 {
                return None;
            }
            let theta = r * fov.to_radians() * 0.5;
            let phi = y.atan2(x);
            Some([theta.sin() * phi.cos(), theta.sin() * phi.sin(), theta.cos()])
        }
        3 => {
            let t = (fov.to_radians() * 0.5).min(1.5).tan();
            Some([(u * 2.0 - 1.0) * t, (v * 2.0 - 1.0) * t, 1.0])
        }
        _ => Some(dir_from_equi(u, v)),
    }
}

fn dir_layout(layout: u32, d: [f64; 3], fov: f64) -> Option<(f64, f64)> {
    match layout {
        1 => Some(dir_cube(d)),
        2 => {
            let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-12);
            let theta = (d[2] / l).clamp(-1.0, 1.0).acos();
            let r = theta / (fov.to_radians() * 0.5);
            if r > 1.0 {
                return None;
            }
            let phi = d[1].atan2(d[0]);
            Some(((r * phi.cos() + 1.0) * 0.5, (r * phi.sin() + 1.0) * 0.5))
        }
        3 => {
            if d[2] <= 1e-6 {
                return None;
            }
            let t = (fov.to_radians() * 0.5).min(1.5).tan();
            Some(((d[0] / d[2] / t + 1.0) * 0.5, (d[1] / d[2] / t + 1.0) * 0.5))
        }
        _ => Some(equi_from_dir(d)),
    }
}

fn remap_uv(c: &FxCtx, img: &mut Img, f: impl Fn(f64, f64) -> Option<(f64, f64)> + Sync) {
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    super::util::remap(img, true, |x, y| {
        let (u, v) = ((x - rect[0]) / lw, (y - rect[1]) / lh);
        let (su, sv) = f(u, v)?;
        Some([rect[0] + su.rem_euclid(1.0) * lw, rect[1] + sv.clamp(0.0, 1.0) * lh])
    });
}

fn converter(c: &FxCtx, img: &mut Img) {
    let (src, out) = (c.e("source"), c.e("output"));
    let fov = c.f("fov");
    let (p, t, r) = (c.f("pan").to_radians(), c.f("tilt").to_radians(), c.f("roll").to_radians());
    if src == out && p == 0.0 && t == 0.0 && r == 0.0 {
        return;
    }
    remap_uv(c, img, |u, v| {
        let d = layout_dir(out, u, v, fov)?;
        dir_layout(src, rot(d, p, t, r), fov)
    });
}

fn plane_to_sphere(c: &FxCtx, img: &mut Img) {
    let Some((plane, psize)) = c.other("plane", true) else { return };
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let lon = c.f("longitude").to_radians();
    let lat = c.f("latitude").to_radians();
    let roll = c.f("rotation").to_radians();
    let tf = (c.f("fov").to_radians() * 0.5).tan();
    let aspect = psize[1] / psize[0].max(1.0);
    let comp = c.b("composite");
    let r = c.r;
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (u, v) = ((x as f64 + 0.5 - rect[0]) / lw, (y as f64 + 0.5 - rect[1]) / lh);
            let d = rot(dir_from_equi(u, v), -lon, 0.0, 0.0);
            let d = rot(d, 0.0, lat, -roll);
            let p = &mut row[x * 4..x * 4 + 4];
            if d[2] <= 1e-6 {
                if !comp {
                    p.iter_mut().for_each(|v| *v = 0.0);
                }
                continue;
            }
            let (px, py) = (d[0] / d[2] / tf, d[1] / d[2] / tf / aspect);
            if px.abs() > 1.0 || py.abs() > 1.0 {
                if !comp {
                    p.iter_mut().for_each(|v| *v = 0.0);
                }
                continue;
            }
            let s = plane.sample((px + 1.0) * 0.5 * psize[0] * r, (py + 1.0) * 0.5 * psize[1] * r, true);
            if comp {
                let k = 1.0 - s[3];
                for i in 0..4 {
                    p[i] = s[i] + p[i] * k;
                }
            } else {
                p.copy_from_slice(&s);
            }
        }
    });
}

fn sphere_to_plane(c: &FxCtx, img: &mut Img) {
    let fov = c.f("fov");
    let (p, t, r) = (c.f("pan").to_radians(), c.f("tilt").to_radians(), c.f("roll").to_radians());
    let rect = c.layer_rect();
    let aspect = (rect[3] - rect[1]) / (rect[2] - rect[0]).max(1.0);
    remap_uv(c, img, |u, v| {
        let tf = (fov.to_radians() * 0.5).tan();
        let d = [(u * 2.0 - 1.0) * tf, (v * 2.0 - 1.0) * tf * aspect, 1.0];
        Some(equi_from_dir(rot(d, p, t, r)))
    });
}

fn rotate_sphere(c: &FxCtx, img: &mut Img) {
    let (t, p, r) = (c.f("tilt").to_radians(), c.f("pan").to_radians(), c.f("roll").to_radians());
    if t == 0.0 && p == 0.0 && r == 0.0 {
        return;
    }
    remap_uv(c, img, |u, v| Some(equi_from_dir(rot(dir_from_equi(u, v), p, t, r))));
}

fn sphere_blur(img: &mut Img, sigma: f64) {
    if sigma < 0.3 {
        return;
    }
    let (w, h) = (img.w, img.h);
    let src = img.clone();
    let rows: Vec<Vec<f32>> = (0..h)
        .into_par_iter()
        .map(|y| {
            let lat = ((y as f64 + 0.5) / h as f64 - 0.5) * PI;
            let k = (1.0 / lat.cos().max(0.05)).min(20.0);
            let s = (sigma * k).min(w as f64);
            let r = (s * 2.5).ceil() as isize;
            let mut out = vec![0.0f32; w * 4];
            let wts: Vec<f32> = (-r..=r).map(|d| (-(d * d) as f64 / (2.0 * s * s)).exp() as f32).collect();
            let sum: f32 = wts.iter().sum();
            for x in 0..w {
                let mut acc = [0.0f32; 4];
                for (j, d) in (-r..=r).enumerate() {
                    let sx = (x as isize + d).rem_euclid(w as isize) as usize;
                    let i = (y * w + sx) * 4;
                    for c in 0..4 {
                        acc[c] += src.px[i + c] * wts[j];
                    }
                }
                for c in 0..4 {
                    out[x * 4 + c] = acc[c] / sum;
                }
            }
            out
        })
        .collect();
    for (y, r) in rows.into_iter().enumerate() {
        img.px[y * w * 4..(y + 1) * w * 4].copy_from_slice(&r);
    }
    gaussian(img, 0.0, sigma, true);
}

fn vr_glow(c: &FxCtx, img: &mut Img) {
    let th = c.f("threshold") as f32;
    let rad = c.px(c.f("radius"));
    let br = c.f("brightness") as f32;
    let tint = c.b("tint");
    let tc = c.c("tintColor");
    let mut hi = img.clone();
    hi.px.par_chunks_mut(4).for_each(|p| {
        let l = if p[3] > 1e-6 { lum(p[0] / p[3], p[1] / p[3], p[2] / p[3]) } else { 0.0 };
        let k = ((l - th) / (1.0 - th).max(1e-3)).clamp(0.0, 1.0);
        for v in p.iter_mut() {
            *v *= k;
        }
        if tint {
            let g = lum(p[0], p[1], p[2]);
            p[0] = g * tc[0];
            p[1] = g * tc[1];
            p[2] = g * tc[2];
        }
    });
    sphere_blur(&mut hi, rad * 0.5);
    img.px.par_chunks_mut(4).zip(hi.px.par_chunks(4)).for_each(|(p, g)| {
        for k in 0..3 {
            p[k] += g[k] * br;
        }
        p[3] = (p[3] + g[3] * br).min(1.0);
    });
}

fn chromatic(c: &FxCtx, img: &mut Img) {
    let ctr = c.pt("center");
    let amt = [c.px(c.f("red")), c.px(c.f("green")), c.px(c.f("blue"))];
    if amt.iter().all(|a| a.abs() < 1e-3) {
        return;
    }
    let fall = (c.f("falloff") / 100.0).max(0.01);
    let rect = c.layer_rect();
    let diag = ((rect[2] - rect[0]).hypot(rect[3] - rect[1]) * 0.5).max(1.0);
    let src = img.clone();
    let w = img.w as f64;
    let ww = img.w;
    img.px.par_chunks_mut(ww * 4).enumerate().for_each(|(y, row)| {
        for x in 0..ww {
            let (dx, dy) = (x as f64 + 0.5 - ctr[0], y as f64 + 0.5 - ctr[1]);
            let d = dx.hypot(dy).max(1e-6);
            let k = (d / diag / fall).min(1.0);
            for ch in 0..3 {
                let off = amt[ch] * k;
                let sx = (x as f64 + 0.5 + dx / d * off).rem_euclid(w);
                let s = src.sample(sx, y as f64 + 0.5 + dy / d * off, true);
                row[x * 4 + ch] = s[ch];
            }
        }
    });
}

fn color_gradients(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let pts: Vec<([f64; 3], [f32; 4])> = [("p1", "c1"), ("p2", "c2"), ("p3", "c3"), ("p4", "c4")]
        .iter()
        .map(|(p, k)| {
            let q = c.pt(p);
            (dir_from_equi((q[0] - rect[0]) / lw, (q[1] - rect[1]) / lh), c.c(k))
        })
        .collect();
    let pw = c.f("blend");
    let op = c.f("opacity") as f32 / 100.0;
    let mode = c.e("mode");
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let d = dir_from_equi((x as f64 + 0.5 - rect[0]) / lw, (y as f64 + 0.5 - rect[1]) / lh);
            let mut acc = [0.0f32; 3];
            let mut ws = 0.0f32;
            for (pd, col) in &pts {
                let ang = (d[0] * pd[0] + d[1] * pd[1] + d[2] * pd[2]).clamp(-1.0, 1.0).acos();
                let wgt = 1.0 / (ang + 1e-3).powf(pw) as f32;
                for k in 0..3 {
                    acc[k] += col[k] * wgt;
                }
                ws += wgt;
            }
            let g = acc.map(|v| v / ws);
            let p = &mut row[x * 4..x * 4 + 4];
            let a = p[3];
            for k in 0..3 {
                let u = if a > 1e-6 { p[k] / a } else { 0.0 };
                let b = match mode {
                    1 => u + g[k],
                    2 => u * g[k],
                    3 => 1.0 - (1.0 - u) * (1.0 - g[k]),
                    4 => if u < 0.5 { 2.0 * u * g[k] } else { 1.0 - 2.0 * (1.0 - u) * (1.0 - g[k]) },
                    _ => g[k],
                };
                let na = a + (1.0 - a) * op;
                p[k] = u * a + (b * na - u * a) * op;
            }
            p[3] += (1.0 - p[3]) * op;
        }
    });
}

fn denoise(c: &FxCtx, img: &mut Img) {
    let amt = c.f("amount") as f32 / 100.0;
    if amt <= 0.0 {
        return;
    }
    let mut b = img.clone();
    sphere_blur(&mut b, c.f("radius") * c.r * 0.6);
    img.px.par_chunks_mut(4).zip(b.px.par_chunks(4)).for_each(|(p, q)| {
        let d = (lum(p[0], p[1], p[2]) - lum(q[0], q[1], q[2])).abs();
        let k = amt * (1.0 - (d * 8.0).min(1.0));
        for i in 0..4 {
            p[i] += (q[i] - p[i]) * k;
        }
    });
}

fn glitch(c: &FxCtx, img: &mut Img) {
    let m = c.f("master") / 100.0;
    if m <= 0.0 {
        return;
    }
    let blocks = c.f("blocks").max(1.0);
    let disp = c.px(c.f("displacement")) * m;
    let split = c.px(c.f("rgbSplit")) * m;
    let frame = (c.t * c.f("speed")).floor() as i32;
    let seed = c.f("seed") as u32;
    let src = img.clone();
    let (w, h) = (img.w, img.h);
    let bh = (h as f64 / blocks).max(1.0);
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        let band = (y as f64 / bh) as i32;
        let r = rand01(band, frame, 0, seed);
        let active = r < m as f32 * 0.6;
        let off = if active { (rand01(band, frame, 1, seed) as f64 - 0.5) * 2.0 * disp } else { 0.0 };
        for x in 0..w {
            let sx = (x as f64 + 0.5 + off).rem_euclid(w as f64);
            let mut o = src.sample(sx, y as f64 + 0.5, true);
            if active && split > 0.0 {
                o[0] = src.sample((sx + split).rem_euclid(w as f64), y as f64 + 0.5, true)[0];
                o[2] = src.sample((sx - split).rem_euclid(w as f64), y as f64 + 0.5, true)[2];
            }
            row[x * 4..x * 4 + 4].copy_from_slice(&o);
        }
    });
}

fn vr_fractal(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let turb = c.e("type") == 1;
    let con = c.f("contrast") as f32 / 100.0;
    let br = c.f("brightness") as f32 / 100.0;
    let sc = c.f("scale") as f32;
    let oct = c.f("complexity").round().clamp(1.0, 20.0) as usize;
    let evo = (c.f("evolution") / 360.0) as f32;
    let op = c.f("opacity") as f32 / 100.0;
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let d = dir_from_equi((x as f64 + 0.5 - rect[0]) / lw, (y as f64 + 0.5 - rect[1]) / lh);
            let mut acc = 0.0f32;
            let mut amp = 1.0f32;
            let mut f = sc;
            let mut norm = 0.0;
            for o in 0..oct {
                let n = perlin3(d[0] as f32 * f + evo * 2.0, d[1] as f32 * f, d[2] as f32 * f + o as f32 * 3.1, 21 + o as u32);
                acc += if turb { n.abs() * 2.0 - 1.0 } else { n } * amp;
                norm += amp;
                amp *= 0.5;
                f *= 2.0;
            }
            let g = (((acc / norm) * 0.5 + 0.5 - 0.5) * con + 0.5 + br).clamp(0.0, 1.0);
            let p = &mut row[x * 4..x * 4 + 4];
            for k in 0..3 {
                p[k] += (g - p[k]) * op;
            }
            p[3] += (1.0 - p[3]) * op;
        }
    });
}
