use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mat3(pub [[f64; 3]; 3]);

impl Default for Mat3 {
    fn default() -> Self {
        Mat3::IDENTITY
    }
}

impl Mat3 {
    pub const IDENTITY: Mat3 = Mat3([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    pub fn translate(x: f64, y: f64) -> Mat3 {
        Mat3([[1.0, 0.0, x], [0.0, 1.0, y], [0.0, 0.0, 1.0]])
    }

    pub fn scale(sx: f64, sy: f64) -> Mat3 {
        Mat3([[sx, 0.0, 0.0], [0.0, sy, 0.0], [0.0, 0.0, 1.0]])
    }

    pub fn rotate_deg(deg: f64) -> Mat3 {
        let (s, c) = deg.to_radians().sin_cos();
        Mat3([[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]])
    }

    pub fn skew(skew_deg: f64, axis_deg: f64) -> Mat3 {
        if skew_deg == 0.0 {
            return Mat3::IDENTITY;
        }
        let k = -(skew_deg.clamp(-85.0, 85.0)).to_radians().tan();
        Mat3::rotate_deg(axis_deg) * Mat3([[1.0, k, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]) * Mat3::rotate_deg(-axis_deg)
    }

    pub fn apply(&self, x: f64, y: f64) -> [f64; 2] {
        let m = &self.0;
        let px = m[0][0] * x + m[0][1] * y + m[0][2];
        let py = m[1][0] * x + m[1][1] * y + m[1][2];
        let w = m[2][0] * x + m[2][1] * y + m[2][2];
        if w != 1.0 && w.abs() > 1e-12 { [px / w, py / w] } else { [px, py] }
    }

    pub fn apply_vec(&self, x: f64, y: f64) -> [f64; 2] {
        let m = &self.0;
        [m[0][0] * x + m[0][1] * y, m[1][0] * x + m[1][1] * y]
    }

    pub fn is_affine(&self) -> bool {
        let m = &self.0;
        m[2][0].abs() < 1e-12 && m[2][1].abs() < 1e-12 && (m[2][2] - 1.0).abs() < 1e-12
    }

    pub fn det(&self) -> f64 {
        let m = &self.0;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }

    pub fn inverse(&self) -> Option<Mat3> {
        let m = &self.0;
        let d = self.det();
        if d.abs() < 1e-14 {
            return None;
        }
        let k = 1.0 / d;
        Some(Mat3([
            [(m[1][1] * m[2][2] - m[1][2] * m[2][1]) * k, (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * k, (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * k],
            [(m[1][2] * m[2][0] - m[1][0] * m[2][2]) * k, (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * k, (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * k],
            [(m[1][0] * m[2][1] - m[1][1] * m[2][0]) * k, (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * k, (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * k],
        ]))
    }

    pub fn max_scale(&self) -> f64 {
        let a = self.apply_vec(1.0, 0.0);
        let b = self.apply_vec(0.0, 1.0);
        a[0].hypot(a[1]).max(b[0].hypot(b[1]))
    }

    pub fn to_skia(&self) -> tiny_skia::Transform {
        let m = &self.0;
        tiny_skia::Transform::from_row(m[0][0] as f32, m[1][0] as f32, m[0][1] as f32, m[1][1] as f32, m[0][2] as f32, m[1][2] as f32)
    }

    pub fn rows(&self) -> [[f64; 3]; 3] {
        self.0
    }
}

impl std::ops::Mul for Mat3 {
    type Output = Mat3;
    fn mul(self, o: Mat3) -> Mat3 {
        let a = &self.0;
        let b = &o.0;
        let mut r = [[0.0; 3]; 3];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j];
            }
        }
        Mat3(r)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [[f64; 4]; 4]);

impl Default for Mat4 {
    fn default() -> Self {
        Mat4::IDENTITY
    }
}

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);

    pub fn translate(x: f64, y: f64, z: f64) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.0[0][3] = x;
        m.0[1][3] = y;
        m.0[2][3] = z;
        m
    }

    pub fn scale(x: f64, y: f64, z: f64) -> Mat4 {
        let mut m = Mat4::IDENTITY;
        m.0[0][0] = x;
        m.0[1][1] = y;
        m.0[2][2] = z;
        m
    }

    pub fn rot_x(deg: f64) -> Mat4 {
        let (s, c) = deg.to_radians().sin_cos();
        Mat4([[1.0, 0.0, 0.0, 0.0], [0.0, c, -s, 0.0], [0.0, s, c, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }

    pub fn rot_y(deg: f64) -> Mat4 {
        let (s, c) = deg.to_radians().sin_cos();
        Mat4([[c, 0.0, s, 0.0], [0.0, 1.0, 0.0, 0.0], [-s, 0.0, c, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }

    pub fn rot_z(deg: f64) -> Mat4 {
        let (s, c) = deg.to_radians().sin_cos();
        Mat4([[c, -s, 0.0, 0.0], [s, c, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }

    pub fn from_mat3_affine(m: &Mat3) -> Mat4 {
        let a = &m.0;
        Mat4([[a[0][0], a[0][1], 0.0, a[0][2]], [a[1][0], a[1][1], 0.0, a[1][2]], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    }

    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        let m = &self.0;
        let x = m[0][0] * p[0] + m[0][1] * p[1] + m[0][2] * p[2] + m[0][3];
        let y = m[1][0] * p[0] + m[1][1] * p[1] + m[1][2] * p[2] + m[1][3];
        let z = m[2][0] * p[0] + m[2][1] * p[1] + m[2][2] * p[2] + m[2][3];
        let w = m[3][0] * p[0] + m[3][1] * p[1] + m[3][2] * p[2] + m[3][3];
        if (w - 1.0).abs() > 1e-12 && w.abs() > 1e-12 { [x / w, y / w, z / w] } else { [x, y, z] }
    }

    pub fn apply_w(&self, p: [f64; 3]) -> [f64; 4] {
        let m = &self.0;
        let mut out = [0.0; 4];
        for (i, o) in out.iter_mut().enumerate() {
            *o = m[i][0] * p[0] + m[i][1] * p[1] + m[i][2] * p[2] + m[i][3];
        }
        out
    }

    pub fn apply_dir(&self, d: [f64; 3]) -> [f64; 3] {
        let m = &self.0;
        [m[0][0] * d[0] + m[0][1] * d[1] + m[0][2] * d[2], m[1][0] * d[0] + m[1][1] * d[1] + m[1][2] * d[2], m[2][0] * d[0] + m[2][1] * d[1] + m[2][2] * d[2]]
    }

    pub fn transpose(&self) -> Mat4 {
        let m = &self.0;
        Mat4([[m[0][0], m[1][0], m[2][0], m[3][0]], [m[0][1], m[1][1], m[2][1], m[3][1]], [m[0][2], m[1][2], m[2][2], m[3][2]], [m[0][3], m[1][3], m[2][3], m[3][3]]])
    }

    pub fn plane_to_mat3(&self) -> Mat3 {
        let m = &self.0;
        Mat3([[m[0][0], m[0][1], m[0][3]], [m[1][0], m[1][1], m[1][3]], [m[3][0], m[3][1], m[3][3]]])
    }

    pub fn inverse(&self) -> Option<Mat4> {
        let m = &self.0;
        let mut a = [[0.0f64; 8]; 4];
        for i in 0..4 {
            for j in 0..4 {
                a[i][j] = m[i][j];
            }
            a[i][4 + i] = 1.0;
        }
        for c in 0..4 {
            let mut piv = c;
            for r in c + 1..4 {
                if a[r][c].abs() > a[piv][c].abs() {
                    piv = r;
                }
            }
            if a[piv][c].abs() < 1e-14 {
                return None;
            }
            a.swap(c, piv);
            let d = a[c][c];
            for v in a[c].iter_mut() {
                *v /= d;
            }
            for r in 0..4 {
                if r != c {
                    let f = a[r][c];
                    if f != 0.0 {
                        for k in 0..8 {
                            a[r][k] -= f * a[c][k];
                        }
                    }
                }
            }
        }
        let mut out = [[0.0; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                out[i][j] = a[i][4 + j];
            }
        }
        Some(Mat4(out))
    }
}

impl std::ops::Mul for Mat4 {
    type Output = Mat4;
    fn mul(self, o: Mat4) -> Mat4 {
        let mut r = [[0.0; 4]; 4];
        for (i, row) in r.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..4).map(|k| self.0[i][k] * o.0[k][j]).sum();
            }
        }
        Mat4(r)
    }
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn homography(src: [[f64; 2]; 4], dst: [[f64; 2]; 4]) -> Option<Mat3> {
    let mut a = [[0.0f64; 9]; 8];
    for i in 0..4 {
        let [x, y] = src[i];
        let [u, v] = dst[i];
        a[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        a[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    for c in 0..8 {
        let mut piv = c;
        for r in c + 1..8 {
            if a[r][c].abs() > a[piv][c].abs() {
                piv = r;
            }
        }
        if a[piv][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, piv);
        let d = a[c][c];
        for v in a[c].iter_mut() {
            *v /= d;
        }
        for r in 0..8 {
            if r != c {
                let f = a[r][c];
                if f != 0.0 {
                    for k in 0..9 {
                        a[r][k] -= f * a[c][k];
                    }
                }
            }
        }
    }
    let h: Vec<f64> = (0..8).map(|i| a[i][8]).collect();
    Some(Mat3([[h[0], h[1], h[2]], [h[3], h[4], h[5]], [h[6], h[7], 1.0]]))
}
