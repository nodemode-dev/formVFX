use crate::geom::{Mat3, Mat4};
use crate::model::{Comp, Layer, LayerKind, Project, Prop, Value};

pub struct Eval<'a> {
    pub project: &'a Project,
    pub comp_id: u64,
    pub comp: &'a Comp,
    pub t: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub view: Mat4,
    pub zoom: f64,
    pub eye: [f64; 3],
    pub forward: [f64; 3],
    pub ortho: Option<f64>,
    pub dof: Option<[f64; 3]>,
}

impl Camera {
    pub fn default_for(w: f64, h: f64) -> Camera {
        let zoom = w * 50.0 / 36.0;
        let eye = [w / 2.0, h / 2.0, -zoom];
        let view = Mat4::translate(-eye[0], -eye[1], -eye[2]);
        Camera { view, zoom, eye, forward: [0.0, 0.0, 1.0], ortho: None, dof: None }
    }

    pub fn looking(eye: [f64; 3], poi: [f64; 3], zoom: f64, ortho: Option<f64>) -> Camera {
        let world = Mat4::translate(eye[0], eye[1], eye[2]) * aim(eye, poi);
        let view = world.inverse().unwrap_or(Mat4::IDENTITY);
        Camera { view, zoom, eye, forward: norm(sub(poi, eye)), ortho, dof: None }
    }

    pub fn projection(&self, w: f64, h: f64) -> Mat4 {
        if let Some(k) = self.ortho {
            return Mat4([[k, 0.0, 0.0, w / 2.0], [0.0, k, 0.0, h / 2.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]]);
        }
        let z = self.zoom;
        Mat4([[z, 0.0, w / 2.0, 0.0], [0.0, z, h / 2.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 1.0, 0.0]])
    }

    pub fn project(&self, w: f64, h: f64, p: [f64; 3]) -> Option<[f64; 2]> {
        let c = self.view.apply(p);
        if let Some(k) = self.ortho {
            return Some([w / 2.0 + k * c[0], h / 2.0 + k * c[1]]);
        }
        if c[2] <= 1e-6 {
            return None;
        }
        Some([w / 2.0 + self.zoom * c[0] / c[2], h / 2.0 + self.zoom * c[1] / c[2]])
    }

    pub fn ray(&self, w: f64, h: f64, x: f64, y: f64) -> ([f64; 3], [f64; 3]) {
        let inv = self.view.inverse().unwrap_or(Mat4::IDENTITY);
        match self.ortho {
            Some(k) => {
                let o = inv.apply([(x - w / 2.0) / k, (y - h / 2.0) / k, 0.0]);
                (o, self.forward)
            }
            None => {
                let d = inv.apply([(x - w / 2.0) / self.zoom, (y - h / 2.0) / self.zoom, 1.0]);
                (self.eye, norm(sub(d, self.eye)))
            }
        }
    }

    pub fn depth(&self, p: [f64; 3]) -> f64 {
        self.view.apply(p)[2]
    }
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn norm(a: [f64; 3]) -> [f64; 3] {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
    if l < 1e-12 { [0.0, 0.0, 1.0] } else { [a[0] / l, a[1] / l, a[2] / l] }
}

pub fn aim(eye: [f64; 3], target: [f64; 3]) -> Mat4 {
    let f = norm(sub(target, eye));
    let down = if f[1].abs() > 0.999 { [0.0, 0.0, -f[1].signum()] } else { [0.0, 1.0, 0.0] };
    let r = norm(cross(down, f));
    let d = cross(f, r);
    Mat4([[r[0], d[0], f[0], 0.0], [r[1], d[1], f[1], 0.0], [r[2], d[2], f[2], 0.0], [0.0, 0.0, 0.0, 1.0]])
}

impl<'a> Eval<'a> {
    pub fn new(project: &'a Project, comp_id: u64, comp: &'a Comp, t: f64) -> Eval<'a> {
        Eval { project, comp_id, comp, t }
    }

    pub fn at(&self, t: f64) -> Eval<'a> {
        Eval { project: self.project, comp_id: self.comp_id, comp: self.comp, t }
    }

    pub fn value(&self, layer: &Layer, prop: &Prop) -> Value {
        crate::expr::evaluate(self, layer, prop)
    }

    pub fn raw_value(&self, layer: &Layer, prop: &Prop) -> Value {
        prop.at(layer.layer_time(self.t))
    }

    pub fn tr(&self, layer: &Layer, id: &str) -> Option<Value> {
        let p = layer.transform()?.get(id)?;
        Some(self.value(layer, p))
    }

    pub fn tr_f(&self, layer: &Layer, id: &str, d: f64) -> f64 {
        self.tr(layer, id).map(|v| v.num()).unwrap_or(d)
    }

    pub fn tr_v3(&self, layer: &Layer, id: &str, d: [f64; 3]) -> [f64; 3] {
        self.tr(layer, id).map(|v| v.vec3()).unwrap_or(d)
    }

    pub fn position(&self, layer: &Layer) -> [f64; 3] {
        let Some(t) = layer.transform() else { return [0.0; 3] };
        if t.get("xPosition").is_none() {
            return self.tr_v3(layer, "position", [0.0; 3]);
        }
        [self.tr_f(layer, "xPosition", 0.0), self.tr_f(layer, "yPosition", 0.0), self.tr_f(layer, "zPosition", 0.0)]
    }

    pub fn opacity(&self, layer: &Layer) -> f64 {
        (self.tr_f(layer, "opacity", 100.0) / 100.0).clamp(0.0, 1.0)
    }

    pub fn local_matrix(&self, layer: &Layer) -> Mat4 {
        match layer.kind {
            LayerKind::Camera | LayerKind::Light => self.camera_like_local(layer),
            _ => {
                let a = self.tr_v3(layer, "anchor", [0.0; 3]);
                let p = self.position(layer);
                let s = self.tr_v3(layer, "scale", [100.0; 3]);
                let mut rz = self.tr_f(layer, "rotation", 0.0);
                let orient = if layer.auto_orient == crate::model::AutoOrient::AlongPath { self.path_direction(layer) } else { None };
                if let Some(d) = orient
                    && !layer.is_3d()
                {
                    rz += d[1].atan2(d[0]).to_degrees();
                }
                if layer.is_3d() {
                    let aimed = match orient {
                        Some(d) => aim([0.0; 3], d) * Mat4::rot_y(-90.0),
                        None => Mat4::IDENTITY,
                    };
                    let o = self.tr_v3(layer, "orientation", [0.0; 3]);
                    let rx = self.tr_f(layer, "rotationX", 0.0);
                    let ry = self.tr_f(layer, "rotationY", 0.0);
                    Mat4::translate(p[0], p[1], p[2])
                        * aimed
                        * Mat4::rot_x(o[0])
                        * Mat4::rot_y(o[1])
                        * Mat4::rot_z(o[2])
                        * Mat4::rot_x(rx)
                        * Mat4::rot_y(ry)
                        * Mat4::rot_z(rz)
                        * Mat4::scale(s[0] / 100.0, s[1] / 100.0, s[2] / 100.0)
                        * Mat4::translate(-a[0], -a[1], -a[2])
                } else {
                    Mat4::translate(p[0], p[1], 0.0) * Mat4::rot_z(rz) * Mat4::scale(s[0] / 100.0, s[1] / 100.0, 1.0) * Mat4::translate(-a[0], -a[1], 0.0)
                }
            }
        }
    }

    pub fn path_direction(&self, layer: &Layer) -> Option<[f64; 3]> {
        let tg = layer.transform()?;
        let moving = ["position", "xPosition", "yPosition", "zPosition"].iter().filter_map(|id| tg.get(id)).any(|pr| pr.keys.len() >= 2 || crate::expr::active(pr));
        if !moving {
            return None;
        }
        let h = self.comp.frame_duration() * 0.25;
        for k in [1.0, 4.0, 16.0] {
            let a = self.at(self.t - h * k).position(layer);
            let b = self.at(self.t + h * k).position(layer);
            let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            if d[0].hypot(d[1]).hypot(d[2]) > 1e-6 {
                return Some(d);
            }
        }
        None
    }

    fn camera_like_local(&self, layer: &Layer) -> Mat4 {
        let p = self.tr_v3(layer, "position", [0.0; 3]);
        let poi = self.tr_v3(layer, "poi", [self.comp.width as f64 / 2.0, self.comp.height as f64 / 2.0, 0.0]);
        let o = self.tr_v3(layer, "orientation", [0.0; 3]);
        let rx = self.tr_f(layer, "rotationX", 0.0);
        let ry = self.tr_f(layer, "rotationY", 0.0);
        let rz = self.tr_f(layer, "rotation", 0.0);
        let aimed = if layer.auto_orient == crate::model::AutoOrient::TowardsPointOfInterest { aim(p, poi) } else { Mat4::IDENTITY };
        Mat4::translate(p[0], p[1], p[2]) * aimed * Mat4::rot_x(o[0]) * Mat4::rot_y(o[1]) * Mat4::rot_z(o[2]) * Mat4::rot_x(rx) * Mat4::rot_y(ry) * Mat4::rot_z(rz)
    }

    pub fn world_matrix(&self, layer: &Layer) -> Mat4 {
        let mut m = self.local_matrix(layer);
        let mut cur = layer.parent;
        let mut guard = 0;
        while let Some(pid) = cur {
            guard += 1;
            if guard > 64 {
                break;
            }
            let Some(pl) = self.comp.layer(pid) else { break };
            m = self.local_matrix(pl) * m;
            cur = pl.parent;
        }
        m
    }

    pub fn parent_matrix(&self, layer: &Layer) -> Mat4 {
        match layer.parent.and_then(|p| self.comp.layer(p)) {
            Some(pl) => self.world_matrix(pl),
            None => Mat4::IDENTITY,
        }
    }

    pub fn layer_to_comp(&self, layer: &Layer) -> Mat3 {
        let w = self.world_matrix(layer);
        let m = &w.0;
        Mat3([[m[0][0], m[0][1], m[0][3]], [m[1][0], m[1][1], m[1][3]], [0.0, 0.0, 1.0]])
    }

    pub fn parent_to_comp(&self, layer: &Layer) -> Mat3 {
        let w = self.parent_matrix(layer);
        let m = &w.0;
        Mat3([[m[0][0], m[0][1], m[0][3]], [m[1][0], m[1][1], m[1][3]], [0.0, 0.0, 1.0]])
    }

    pub fn camera_layer(&self) -> Option<&'a Layer> {
        self.comp.layers.iter().find(|l| l.kind == LayerKind::Camera && l.switches.video && l.is_active_at(self.t))
    }

    pub fn camera(&self) -> Camera {
        let (w, h) = (self.comp.width as f64, self.comp.height as f64);
        if let Some(l) = self.camera_layer()
            && let Some(view) = self.world_matrix(l).inverse()
        {
            let world = self.world_matrix(l);
            let opts = l.props.sub("cameraOptions");
            let num = |id: &str, d: f64| opts.and_then(|g| g.get(id)).map(|p| self.value(l, p).num()).unwrap_or(d);
            let zoom = num("zoom", w * 50.0 / 36.0);
            let eye = world.apply([0.0; 3]);
            let fwd = {
                let f = world.apply([0.0, 0.0, 1.0]);
                norm(sub(f, eye))
            };
            let dof_on = opts.and_then(|g| g.get("dof")).map(|p| self.value(l, p).boolean()).unwrap_or(false);
            let dof = dof_on.then(|| [num("focusDistance", zoom), num("aperture", 25.0), num("blurLevel", 100.0)]);
            return Camera { view, zoom, eye, forward: fwd, ortho: None, dof };
        }
        Camera::default_for(w, h)
    }

    pub fn layer_to_screen(&self, layer: &Layer, cam: &Camera) -> Mat3 {
        let (w, h) = (self.comp.width as f64, self.comp.height as f64);
        if !layer.is_3d() {
            return self.layer_to_comp(layer);
        }
        let full = cam.projection(w, h) * cam.view * self.world_matrix(layer);
        let m = &full.0;
        Mat3([[m[0][0], m[0][1], m[0][3]], [m[1][0], m[1][1], m[1][3]], [m[3][0], m[3][1], m[3][3]]])
    }

    pub fn layer_time(&self, layer: &Layer) -> f64 {
        layer.layer_time(self.t)
    }

    pub fn source_time(&self, layer: &Layer) -> f64 {
        match layer.props.get("timeRemap") {
            Some(p) => self.value(layer, p).num(),
            None => layer.layer_time(self.t),
        }
    }
}
