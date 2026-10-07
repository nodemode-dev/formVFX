use super::image::Img;

pub const POINTS: [(&str, &str); 23] = [
    ("leftEyebrowInner", "Left Eyebrow Inner"),
    ("leftEyebrowMiddle", "Left Eyebrow Middle"),
    ("leftEyebrowOuter", "Left Eyebrow Outer"),
    ("rightEyebrowInner", "Right Eyebrow Inner"),
    ("rightEyebrowMiddle", "Right Eyebrow Middle"),
    ("rightEyebrowOuter", "Right Eyebrow Outer"),
    ("leftEyeInner", "Left Eye Inner"),
    ("leftEyeOuter", "Left Eye Outer"),
    ("leftEyeTop", "Left Eye Top"),
    ("leftEyeBottom", "Left Eye Bottom"),
    ("leftPupil", "Left Pupil"),
    ("rightEyeInner", "Right Eye Inner"),
    ("rightEyeOuter", "Right Eye Outer"),
    ("rightEyeTop", "Right Eye Top"),
    ("rightEyeBottom", "Right Eye Bottom"),
    ("rightPupil", "Right Pupil"),
    ("noseTip", "Nose Tip"),
    ("leftNostril", "Left Nostril"),
    ("rightNostril", "Right Nostril"),
    ("mouthLeft", "Mouth Left"),
    ("mouthRight", "Mouth Right"),
    ("mouthTop", "Mouth Top"),
    ("mouthBottom", "Mouth Bottom"),
];

pub const MEASUREMENTS: [(&str, &str); 10] = [
    ("headPositionX", "Head Position X"),
    ("headPositionY", "Head Position Y"),
    ("headScale", "Head Scale"),
    ("headTilt", "Head Tilt"),
    ("leftEyeOpenness", "Left Eye Openness"),
    ("rightEyeOpenness", "Right Eye Openness"),
    ("leftEyebrowRaise", "Left Eyebrow Raise"),
    ("rightEyebrowRaise", "Right Eyebrow Raise"),
    ("mouthOpenness", "Mouth Openness"),
    ("mouthWidth", "Mouth Width"),
];

#[derive(Clone, Copy, Debug)]
pub struct Ellipse {
    pub c: [f64; 2],
    pub a: f64,
    pub b: f64,
    pub angle: f64,
}

impl Ellipse {
    pub fn to_world(&self, u: f64, v: f64) -> [f64; 2] {
        let (s, co) = self.angle.sin_cos();
        let (x, y) = (u * self.a, v * self.b);
        [self.c[0] + x * co - y * s, self.c[1] + x * s + y * co]
    }

    pub fn to_local(&self, p: [f64; 2]) -> [f64; 2] {
        let (s, co) = self.angle.sin_cos();
        let (dx, dy) = (p[0] - self.c[0], p[1] - self.c[1]);
        [(dx * co + dy * s) / self.a.max(1e-9), (-dx * s + dy * co) / self.b.max(1e-9)]
    }

    pub fn lerp(&self, o: &Ellipse, t: f64) -> Ellipse {
        let mut da = o.angle - self.angle;
        while da > std::f64::consts::PI {
            da -= std::f64::consts::TAU;
        }
        while da < -std::f64::consts::PI {
            da += std::f64::consts::TAU;
        }
        Ellipse { c: [self.c[0] + (o.c[0] - self.c[0]) * t, self.c[1] + (o.c[1] - self.c[1]) * t], a: self.a + (o.a - self.a) * t, b: self.b + (o.b - self.b) * t, angle: self.angle + da * t }
    }
}

fn skin(p: &[f32]) -> f32 {
    let a = p[3].max(1e-6);
    let (r, g, b) = (p[0] / a * 255.0, p[1] / a * 255.0, p[2] / a * 255.0);
    let cb = 128.0 - 0.168_736 * r - 0.331_264 * g + 0.5 * b;
    let cr = 128.0 + 0.5 * r - 0.418_688 * g - 0.081_312 * b;
    let y = 0.299 * r + 0.587 * g + 0.114 * b;
    if y < 35.0 {
        return 0.0;
    }
    let fa = |v: f32, lo: f32, hi: f32| {
        let m = 8.0;
        ((v - (lo - m)) / m).clamp(0.0, 1.0) * (((hi + m) - v) / m).clamp(0.0, 1.0)
    };
    fa(cb, 77.0, 127.0) * fa(cr, 133.0, 173.0)
}

pub fn detect(img: &Img, roi: [f64; 4], prior: Option<&Ellipse>) -> Option<Ellipse> {
    let (w, h) = (img.w, img.h);
    let x0 = roi[0].floor().max(0.0) as usize;
    let y0 = roi[1].floor().max(0.0) as usize;
    let x1 = (roi[2].ceil() as usize).min(w);
    let y1 = (roi[3].ceil() as usize).min(h);
    if x1 <= x0 + 4 || y1 <= y0 + 4 {
        return None;
    }
    let (rw, rh) = (x1 - x0, y1 - y0);
    let mut m: Vec<f32> = (0..rw * rh)
        .map(|i| {
            let (x, y) = (x0 + i % rw, y0 + i / rw);
            let k = (y * w + x) * 4;
            skin(&img.px[k..k + 4])
        })
        .collect();
    crate::effects::util::blur_scalar(&mut m, rw, rh, 2.0, 2.0);
    let mask: Vec<bool> = m.iter().map(|v| *v > 0.35).collect();
    let mut label = vec![0u32; rw * rh];
    let mut best: Option<(usize, u32)> = None;
    let mut next = 1u32;
    let center = prior.map(|e| [e.c[0] - x0 as f64, e.c[1] - y0 as f64]);
    for start in 0..rw * rh {
        if !mask[start] || label[start] != 0 {
            continue;
        }
        let mut stack = vec![start];
        label[start] = next;
        let mut size = 0usize;
        let mut hits_center = false;
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = (i % rw, i / rw);
            if let Some(c) = center
                && (x as f64 - c[0]).abs() < 3.0
                && (y as f64 - c[1]).abs() < 3.0
            {
                hits_center = true;
            }
            for (dx, dy) in [(-1i64, 0i64), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= rw as i64 || ny >= rh as i64 {
                    continue;
                }
                let j = ny as usize * rw + nx as usize;
                if mask[j] && label[j] == 0 {
                    label[j] = next;
                    stack.push(j);
                }
            }
        }
        let score = size + if hits_center { rw * rh } else { 0 };
        if best.is_none_or(|b| score > b.0) {
            best = Some((score, next));
        }
        next += 1;
    }
    let (_, id) = best?;
    let (mut n, mut sx, mut sy) = (0.0f64, 0.0f64, 0.0f64);
    for (i, l) in label.iter().enumerate() {
        if *l == id {
            n += 1.0;
            sx += (i % rw) as f64;
            sy += (i / rw) as f64;
        }
    }
    if n < 50.0 {
        return None;
    }
    let (mx, my) = (sx / n, sy / n);
    let (mut cxx, mut cyy, mut cxy) = (0.0, 0.0, 0.0);
    for (i, l) in label.iter().enumerate() {
        if *l == id {
            let (dx, dy) = ((i % rw) as f64 - mx, (i / rw) as f64 - my);
            cxx += dx * dx;
            cyy += dy * dy;
            cxy += dx * dy;
        }
    }
    let (cxx, cyy, cxy) = (cxx / n, cyy / n, cxy / n);
    let tr = cxx + cyy;
    let det = cxx * cyy - cxy * cxy;
    let disc = (tr * tr / 4.0 - det).max(0.0).sqrt();
    let (l1, l2) = (tr / 2.0 + disc, (tr / 2.0 - disc).max(1e-6));
    let major = if cxy.abs() > 1e-9 { (l1 - cxx).atan2(cxy) } else if cxx >= cyy { 0.0 } else { std::f64::consts::FRAC_PI_2 };
    let (ra, rb) = (2.0 * l1.sqrt(), 2.0 * l2.sqrt());
    let mut e = if (major.sin()).abs() > (major.cos()).abs() {
        Ellipse { c: [mx + x0 as f64, my + y0 as f64], a: rb, b: ra, angle: major - std::f64::consts::FRAC_PI_2 }
    } else {
        Ellipse { c: [mx + x0 as f64, my + y0 as f64], a: ra, b: rb, angle: major }
    };
    while e.angle > std::f64::consts::FRAC_PI_2 {
        e.angle -= std::f64::consts::PI;
    }
    while e.angle < -std::f64::consts::FRAC_PI_2 {
        e.angle += std::f64::consts::PI;
    }
    Some(e)
}

fn lum_at(img: &Img, p: [f64; 2]) -> f32 {
    let s = img.sample(p[0], p[1], true);
    let a = s[3].max(1e-6);
    (0.299 * s[0] + 0.587 * s[1] + 0.114 * s[2]) / a
}

fn darkest(img: &Img, e: &Ellipse, u0: f64, u1: f64, v0: f64, v1: f64) -> [f64; 2] {
    let mut best = (f32::MAX, [(u0 + u1) * 0.5, (v0 + v1) * 0.5]);
    let steps = 24;
    for j in 0..=steps {
        for i in 0..=steps {
            let u = u0 + (u1 - u0) * i as f64 / steps as f64;
            let v = v0 + (v1 - v0) * j as f64 / steps as f64;
            let mut acc = 0.0f32;
            for (du, dv) in [(0.0, 0.0), (0.02, 0.0), (-0.02, 0.0), (0.0, 0.015), (0.0, -0.015)] {
                acc += lum_at(img, e.to_world(u + du, v + dv));
            }
            if acc < best.0 {
                best = (acc, [u, v]);
            }
        }
    }
    best.1
}

fn extent(img: &Img, e: &Ellipse, c: [f64; 2], dir: [f64; 2], max: f64, thresh: f32) -> f64 {
    let base = lum_at(img, e.to_world(c[0], c[1]));
    let mut t = 0.0;
    while t < max {
        let p = [c[0] + dir[0] * t, c[1] + dir[1] * t];
        if lum_at(img, e.to_world(p[0], p[1])) > base + thresh {
            break;
        }
        t += 0.01;
    }
    t
}

pub fn features(img: &Img, e: &Ellipse) -> Vec<[f64; 2]> {
    let le = darkest(img, e, -0.62, -0.12, -0.45, -0.05);
    let re = darkest(img, e, 0.12, 0.62, -0.45, -0.05);
    let mouth = darkest(img, e, -0.3, 0.3, 0.35, 0.75);
    let eye_w = 0.17;
    let open = |c: [f64; 2]| {
        let up = extent(img, e, c, [0.0, -1.0], 0.12, 0.08);
        let dn = extent(img, e, c, [0.0, 1.0], 0.12, 0.08);
        (up.max(0.02), dn.max(0.02))
    };
    let (lu, ld) = open(le);
    let (ru, rd) = open(re);
    let mw_l = extent(img, e, mouth, [-1.0, 0.0], 0.45, 0.06).max(0.12);
    let mw_r = extent(img, e, mouth, [1.0, 0.0], 0.45, 0.06).max(0.12);
    let mu = extent(img, e, mouth, [0.0, -1.0], 0.2, 0.06).max(0.02);
    let md = extent(img, e, mouth, [0.0, 1.0], 0.2, 0.06).max(0.02);
    let brow = |c: [f64; 2], outer: f64| -> [[f64; 2]; 3] {
        let v = c[1] - 0.17;
        [[c[0] - outer * 0.12, v + 0.02], [c[0], v - 0.01], [c[0] + outer * 0.14, v + 0.03]]
    };
    let lb = brow(le, -1.0);
    let rb = brow(re, 1.0);
    let nose = [(le[0] + re[0]) * 0.5, (le[1] + mouth[1]) * 0.5 + 0.06];
    let local = [
        lb[0],
        lb[1],
        lb[2],
        rb[0],
        rb[1],
        rb[2],
        [le[0] + eye_w * 0.5, le[1]],
        [le[0] - eye_w * 0.5, le[1]],
        [le[0], le[1] - lu],
        [le[0], le[1] + ld],
        le,
        [re[0] - eye_w * 0.5, re[1]],
        [re[0] + eye_w * 0.5, re[1]],
        [re[0], re[1] - ru],
        [re[0], re[1] + rd],
        re,
        nose,
        [nose[0] - 0.1, nose[1] + 0.05],
        [nose[0] + 0.1, nose[1] + 0.05],
        [mouth[0] - mw_l, mouth[1]],
        [mouth[0] + mw_r, mouth[1]],
        [mouth[0], mouth[1] - mu],
        [mouth[0], mouth[1] + md],
    ];
    local.iter().map(|p| e.to_world(p[0], p[1])).collect()
}

fn d(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

pub fn measure(pts: &[[f64; 2]]) -> Vec<f64> {
    if pts.len() < POINTS.len() {
        return vec![0.0; MEASUREMENTS.len()];
    }
    let p = |name: &str| POINTS.iter().position(|q| q.0 == name).map(|i| pts[i]).unwrap_or([0.0; 2]);
    let (lp, rp) = (p("leftPupil"), p("rightPupil"));
    let iod = d(lp, rp).max(1e-6);
    let center = [(lp[0] + rp[0]) * 0.5, (lp[1] + rp[1]) * 0.5];
    let tilt = (rp[1] - lp[1]).atan2(rp[0] - lp[0]).to_degrees();
    vec![
        center[0],
        center[1],
        iod,
        tilt,
        d(p("leftEyeTop"), p("leftEyeBottom")) / iod * 100.0,
        d(p("rightEyeTop"), p("rightEyeBottom")) / iod * 100.0,
        d(p("leftEyebrowMiddle"), lp) / iod * 100.0,
        d(p("rightEyebrowMiddle"), rp) / iod * 100.0,
        d(p("mouthTop"), p("mouthBottom")) / iod * 100.0,
        d(p("mouthLeft"), p("mouthRight")) / iod * 100.0,
    ]
}
