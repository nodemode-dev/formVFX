use serde_json::{Value as Json, json};

use crate::engine::{Engine, Res, pb, pf, ps};
use crate::eval::Eval;
use crate::model::key::{self, Key};
use crate::model::{Layer, LayerKind, Value};

const STEP_RATE: f64 = 240.0;
const MAX_FRAMES: usize = 20000;

struct Body {
    id: u64,
    circle: bool,
    r: f64,
    hw: f64,
    hh: f64,
    inv_m: f64,
    inv_i: f64,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    rot: f64,
    w: f64,
    off: [f64; 2],
    z: f64,
    out: Vec<([f64; 2], f64)>,
}

struct World {
    g: f64,
    bounce: f64,
    mu: f64,
    floor: f64,
    width: f64,
    walls: bool,
    rest: f64,
}

fn extent(e: &Engine, cid: u64, l: &Layer, t: f64) -> [f64; 4] {
    let Some(comp) = e.project.comp(cid) else { return [0.0, 0.0, 100.0, 100.0] };
    let ev = Eval::new(&e.project, cid, comp, t);
    let found = match l.kind {
        LayerKind::Text => l.text_prop().map(|p| ev.value(l, p)).and_then(|v| v.as_text().cloned()).and_then(|d| crate::render::vector::ops_bounds(&crate::render::text::text_ops(&d))),
        LayerKind::Shape => crate::render::vector::ops_bounds(&crate::render::shapes::shape_ops(&ev, l)),
        _ => {
            let (w, h) = e.project.source_size(l, comp);
            Some([0.0, 0.0, w, h])
        }
    };
    found.unwrap_or([0.0, 0.0, 100.0, 100.0])
}

fn perp(r: [f64; 2]) -> [f64; 2] {
    [-r[1], r[0]]
}

fn dot(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn cross(a: [f64; 2], b: [f64; 2]) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

fn surface(b: &mut Body, n: [f64; 2], depth: f64, w: &World) {
    b.x += n[0] * depth;
    b.y += n[1] * depth;
    let reach = if b.circle { b.r } else { n[0].abs() * b.hw + n[1].abs() * b.hh };
    let r = [-n[0] * reach, -n[1] * reach];
    let vrel = [b.vx + b.w * perp(r)[0], b.vy + b.w * perp(r)[1]];
    let vn = dot(vrel, n);
    if vn >= 0.0 {
        return;
    }
    let e = if -vn < w.rest { 0.0 } else { w.bounce };
    let m = 1.0 / b.inv_m;
    let jn = -(1.0 + e) * vn * m;
    let t = perp(n);
    let vt = dot(vrel, t);
    let arm = cross(r, t);
    let jt = (-vt / (b.inv_m + arm * arm * b.inv_i)).clamp(-w.mu * jn, w.mu * jn);
    let j = [jn * n[0] + jt * t[0], jn * n[1] + jt * t[1]];
    b.vx += j[0] * b.inv_m;
    b.vy += j[1] * b.inv_m;
    b.w += cross(r, j) * b.inv_i;
    if n[1] < 0.0 && b.circle {
        let k = (1.0 - w.mu * 0.25 / STEP_RATE).max(0.0);
        b.vx *= k;
        b.w *= k;
    }
}

fn pair(a: &mut Body, b: &mut Body, w: &World) {
    let d = [b.x - a.x, b.y - a.y];
    let (n, depth, ra, rb) = if !a.circle && !b.circle {
        let ox = a.hw + b.hw - d[0].abs();
        let oy = a.hh + b.hh - d[1].abs();
        if ox <= 0.0 || oy <= 0.0 {
            return;
        }
        if ox < oy { ([d[0].signum(), 0.0], ox, 0.0, 0.0) } else { ([0.0, d[1].signum()], oy, 0.0, 0.0) }
    } else {
        let req = |q: &Body| if q.circle { q.r } else { (q.hw * q.hh).sqrt() * 1.128 };
        let (ra, rb) = (req(a), req(b));
        let dist = d[0].hypot(d[1]);
        let depth = ra + rb - dist;
        if depth <= 0.0 {
            return;
        }
        let n = if dist > 1e-9 { [d[0] / dist, d[1] / dist] } else { [0.0, -1.0] };
        (n, depth, if a.circle { ra } else { 0.0 }, if b.circle { rb } else { 0.0 })
    };
    let total = a.inv_m + b.inv_m;
    a.x -= n[0] * depth * a.inv_m / total;
    a.y -= n[1] * depth * a.inv_m / total;
    b.x += n[0] * depth * b.inv_m / total;
    b.y += n[1] * depth * b.inv_m / total;
    let rav = [n[0] * ra, n[1] * ra];
    let rbv = [-n[0] * rb, -n[1] * rb];
    let va = [a.vx + a.w * perp(rav)[0], a.vy + a.w * perp(rav)[1]];
    let vb = [b.vx + b.w * perp(rbv)[0], b.vy + b.w * perp(rbv)[1]];
    let vrel = [vb[0] - va[0], vb[1] - va[1]];
    let vn = dot(vrel, n);
    if vn >= 0.0 {
        return;
    }
    let e = if -vn < w.rest { 0.0 } else { w.bounce };
    let jn = -(1.0 + e) * vn / total;
    let t = perp(n);
    let vt = dot(vrel, t);
    let denom = total + ra * ra * a.inv_i + rb * rb * b.inv_i;
    let jt = (-vt / denom).clamp(-w.mu * jn, w.mu * jn);
    let j = [jn * n[0] + jt * t[0], jn * n[1] + jt * t[1]];
    a.vx -= j[0] * a.inv_m;
    a.vy -= j[1] * a.inv_m;
    b.vx += j[0] * b.inv_m;
    b.vy += j[1] * b.inv_m;
    a.w -= cross(rav, j) * a.inv_i;
    b.w += cross(rbv, j) * b.inv_i;
}

fn substep(bodies: &mut [Body], w: &World, dt: f64) {
    for b in bodies.iter_mut() {
        b.vy += w.g * dt;
        b.x += b.vx * dt;
        b.y += b.vy * dt;
        b.rot += b.w * dt;
    }
    for _ in 0..3 {
        for b in bodies.iter_mut() {
            let (ex, ey) = if b.circle { (b.r, b.r) } else { (b.hw, b.hh) };
            let below = b.y + ey - w.floor;
            if below > 0.0 {
                surface(b, [0.0, -1.0], below, w);
            }
            if w.walls {
                let left = ex - b.x;
                if left > 0.0 {
                    surface(b, [1.0, 0.0], left, w);
                }
                let right = b.x + ex - w.width;
                if right > 0.0 {
                    surface(b, [-1.0, 0.0], right, w);
                }
            }
        }
        for i in 0..bodies.len() {
            let (head, tail) = bodies.split_at_mut(i + 1);
            for other in tail.iter_mut() {
                pair(&mut head[i], other, w);
            }
        }
    }
}

pub fn bake(e: &mut Engine, p: &Json) -> Res {
    let (cid, ids) = e.layers_param(p)?;
    if ids.is_empty() {
        return Err("pass `layers` (or select layers)".into());
    }
    let (fps, work, cw, ch) = {
        let c = e.project.comp(cid).ok_or("no composition")?;
        (c.frame_rate, c.work_area, c.width as f64, c.height as f64)
    };
    let start = pf(p, "start").unwrap_or(work[0]);
    let end = pf(p, "end").unwrap_or(work[1]);
    if end <= start {
        return Err("`end` must be after `start`".into());
    }
    let frames = (((end - start) * fps).floor() as usize + 1).min(MAX_FRAMES);
    let shape = ps(p, "shape").unwrap_or("auto");
    if !["auto", "circle", "box"].contains(&shape) {
        return Err("shape is auto, circle or box".into());
    }
    let world = World {
        g: pf(p, "gravity").unwrap_or(980.0),
        bounce: pf(p, "bounce").unwrap_or(0.6).clamp(0.0, 1.0),
        mu: pf(p, "friction").unwrap_or(0.2).max(0.0),
        floor: pf(p, "floor").unwrap_or(ch),
        width: cw,
        walls: pb(p, "walls").unwrap_or(true),
        rest: 0.0,
    };
    let per_frame = ((STEP_RATE / fps).ceil() as usize).max(1);
    let dt = 1.0 / fps / per_frame as f64;
    let world = World { rest: world.g.abs() * dt * 2.5 + 0.5, ..world };
    let mut bodies: Vec<Body> = vec![];
    for id in &ids {
        let l = e.layer_ref(cid, *id)?;
        if l.parent.is_some() {
            return Err(format!("{} has a parent; physics works on unparented layers", l.name));
        }
        if l.is_3d() {
            return Err(format!("{} is 3D; physics is 2D", l.name));
        }
        let tr = l.props.sub("transform").ok_or("no transform")?;
        let pos = tr.get("position").ok_or_else(|| format!("{} has separate position dimensions; join them first", l.name))?;
        if pos.expr.is_some() {
            return Err(format!("{} has a position expression; remove it before baking", l.name));
        }
        let rotation = tr.get("rotation").ok_or("no rotation")?;
        if rotation.expr.is_some() {
            return Err(format!("{} has a rotation expression; remove it before baking", l.name));
        }
        let lt = l.layer_time(start);
        let b = extent(e, cid, l, start);
        let comp = e.project.comp(cid).ok_or("no composition")?;
        let ev = Eval::new(&e.project, cid, comp, start);
        let sc = ev.tr_v3(l, "scale", [100.0; 3]);
        let (sx, sy) = (sc[0].abs() / 100.0, sc[1].abs() / 100.0);
        let (w, h) = (((b[2] - b[0]) * sx).max(1.0), ((b[3] - b[1]) * sy).max(1.0));
        let anchor = ev.tr_v3(l, "anchor", [0.0; 3]);
        let off = [((b[0] + b[2]) / 2.0 - anchor[0]) * sx, ((b[1] + b[3]) / 2.0 - anchor[1]) * sy];
        let p0 = pos.at(lt).vec3();
        let rot0 = rotation.at(lt).num();
        let circle = match shape {
            "circle" => true,
            "box" => false,
            _ => l.kind == LayerKind::Shape && (w - h).abs() <= 0.1 * w.max(h),
        };
        let a = rot0.to_radians();
        let (c, s) = (a.cos(), a.sin());
        let (x, y) = (p0[0] + c * off[0] - s * off[1], p0[1] + s * off[0] + c * off[1]);
        let hstep = 1.0 / 120.0;
        let (vx, vy) = if pos.keys.len() >= 2 {
            let q = pos.at(lt + hstep).vec3();
            ((q[0] - p0[0]) / hstep, (q[1] - p0[1]) / hstep)
        } else {
            (0.0, 0.0)
        };
        let r = (w + h) / 4.0;
        let mass = w * h / 1.0e4;
        bodies.push(Body {
            id: *id,
            circle,
            r,
            hw: w / 2.0,
            hh: h / 2.0,
            inv_m: 1.0 / mass.max(1e-6),
            inv_i: if circle { 1.0 / (0.5 * mass.max(1e-6) * r * r) } else { 0.0 },
            x,
            y,
            vx,
            vy,
            rot: a,
            w: 0.0,
            off,
            z: p0[2],
            out: Vec::with_capacity(frames),
        });
    }
    for _ in 0..frames {
        for b in bodies.iter_mut() {
            let (c, s) = (b.rot.cos(), b.rot.sin());
            b.out.push(([b.x - (c * b.off[0] - s * b.off[1]), b.y - (s * b.off[0] + c * b.off[1])], b.rot.to_degrees()));
        }
        for _ in 0..per_frame {
            substep(&mut bodies, &world, dt);
        }
    }
    let fd = 1.0 / fps;
    let t_last = start + (frames - 1) as f64 * fd;
    for b in &bodies {
        let l = e.layer_mut(cid, b.id)?;
        let times: Vec<f64> = (0..frames).map(|f| l.layer_time(start + f as f64 * fd)).collect();
        let (lo, hi) = (l.layer_time(start) - 1e-6, l.layer_time(t_last) + 1e-6);
        let tr = l.props.sub_mut("transform").ok_or("no transform")?;
        let pos = tr.get_mut("position").ok_or("no position")?;
        pos.keys.retain(|k| k.time < lo || k.time > hi);
        for (f, t) in times.iter().enumerate() {
            let q = b.out[f].0;
            let mut k = Key::new(*t, Value::Vec3([q[0], q[1], b.z]), true);
            k.spatial_auto = false;
            pos.keys.push(k);
        }
        key::sort(&mut pos.keys);
        if b.circle {
            let rot = tr.get_mut("rotation").ok_or("no rotation")?;
            rot.keys.retain(|k| k.time < lo || k.time > hi);
            for (f, t) in times.iter().enumerate() {
                rot.keys.push(Key::new(*t, Value::Num(b.out[f].1), false));
            }
            key::sort(&mut rot.keys);
        }
    }
    e.set_label("Bake Physics");
    Ok(json!({"frames": frames, "start": start, "end": t_last, "bodies": bodies.iter().map(|b| json!({"layer": b.id, "shape": if b.circle { "circle" } else { "box" }})).collect::<Vec<_>>()}))
}
