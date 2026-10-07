use std::f64::consts::{PI, TAU};

use rayon::prelude::*;

use super::util::{lum, rand01, smooth};
use super::{EffectSpec, FxCtx, angle, check, color, layer_param, no_pad, num, pct, point, popup, slider};
use crate::geom::{Mat3, homography};
use crate::render::image::Img;

const CAT: &str = "Simulation";

const PTYPES: [&str; 8] = ["Line", "Star", "Faded Sphere", "Shaded Sphere", "Bubble", "Square", "Motion Square", "Textured Square"];
const ANIMS: [&str; 7] = ["Explosive", "Direction", "Cone", "Vortex", "Fire", "Jet Sideways", "Twirly"];

fn particle_params(world: bool) -> Vec<super::ParamSpec> {
    let mut v = vec![
        slider("birthRate", "Birth Rate", 2.0, 0.0, 50.0, 2),
        slider("longevity", "Longevity (sec)", 1.0, 0.0, 30.0, 2),
        point("producer", "Producer Position", 0.5, 0.5),
        num("radiusX", "Producer Radius X", 10.0, 0.0, 4000.0, 0.0, 500.0),
        num("radiusY", "Producer Radius Y", 10.0, 0.0, 4000.0, 0.0, 500.0),
        popup("animation", "Animation", 0, &ANIMS),
        num("velocity", "Velocity", 1.5, -100.0, 100.0, 0.0, 10.0),
        num("gravity", "Gravity", 1.0, -100.0, 100.0, -5.0, 5.0),
        slider("resistance", "Resistance", 0.0, 0.0, 10.0, 2),
        angle("direction", "Direction", 0.0),
        slider("extra", "Extra", 1.0, 0.0, 5.0, 2),
        popup("particleType", "Particle Type", 2, &PTYPES),
        num("birthSize", "Birth Size", 4.0, 0.0, 400.0, 0.0, 50.0),
        num("deathSize", "Death Size", 1.0, 0.0, 400.0, 0.0, 50.0),
        pct("sizeVariation", "Size Variation", 0.0),
        pct("maxOpacity", "Max Opacity", 75.0),
        color("birthColor", "Birth Color", [1.0, 1.0, 0.0]),
        color("deathColor", "Death Color", [0.6, 0.0, 0.0]),
        popup("transfer", "Transfer Mode", 1, &["Composite", "Add", "Screen", "Lighten"]),
        slider("seed", "Random Seed", 0.0, 0.0, 10000.0, 0),
    ];
    if world {
        v.push(num("producerZ", "Producer Position Z", 0.0, -10000.0, 10000.0, -1000.0, 1000.0));
        v.push(angle("cameraRotationY", "Camera Rotation Y", 0.0));
        v.push(angle("cameraRotationX", "Camera Rotation X", 0.0));
        v.push(slider("cameraDistance", "Camera Distance", 2.0, 0.1, 10.0, 2));
        v.push(slider("floor", "Floor Position", 0.5, -10.0, 10.0, 2));
        v.push(check("floorBounce", "Floor Bounce", false));
    }
    v
}

pub fn specs() -> Vec<EffectSpec> {
    vec![
        EffectSpec { id: "fx.sim.ccParticleSystemsII", name: "CC Particle Systems II", category: CAT, params: particle_params(false), render: |c, img| particles(c, img, false), pad: no_pad },
        EffectSpec { id: "fx.sim.ccParticleWorld", name: "CC Particle World", category: CAT, params: particle_params(true), render: |c, img| particles(c, img, true), pad: no_pad },
        EffectSpec {
            id: "fx.sim.particlePlayground",
            name: "Particle Playground",
            category: CAT,
            params: vec![
                point("cannon", "Cannon Position", 0.5, 0.9),
                num("barrelRadius", "Barrel Radius", 0.0, 0.0, 1000.0, 0.0, 100.0),
                slider("perSecond", "Particles Per Second", 20.0, 0.0, 1000.0, 1),
                angle("direction", "Direction", 0.0),
                slider("spread", "Direction Random Spread", 20.0, 0.0, 360.0, 1),
                num("velocity", "Velocity", 130.0, 0.0, 2000.0, 0.0, 500.0),
                slider("velocitySpread", "Velocity Random Spread", 20.0, 0.0, 500.0, 1),
                color("color", "Color", [1.0, 0.0, 0.0]),
                num("radius", "Particle Radius", 2.0, 0.0, 100.0, 0.0, 20.0),
                num("gravity", "Gravity Force", 150.0, -2000.0, 2000.0, -500.0, 500.0),
                angle("gravityDirection", "Gravity Direction", 180.0),
                slider("lifespan", "Lifespan (sec)", 3.0, 0.0, 30.0, 2),
            ],
            render: playground,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.ccRainfall",
            name: "CC Rainfall",
            category: CAT,
            params: vec![slider("drops", "Drops", 5000.0, 0.0, 30000.0, 0), slider("size", "Size", 3.0, 0.0, 20.0, 2), num("depth", "Scene Depth", 5000.0, 0.0, 20000.0, 0.0, 10000.0), num("speed", "Speed", 4000.0, 0.0, 20000.0, 0.0, 10000.0), num("wind", "Wind", 800.0, -5000.0, 5000.0, -2000.0, 2000.0), pct("variation", "Variation % (Wind)", 30.0), color("color", "Color", [0.7, 0.8, 1.0]), pct("opacity", "Opacity", 25.0), slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0), check("composite", "Composite With Original", true)],
            render: rainfall,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.ccSnowfall",
            name: "CC Snowfall",
            category: CAT,
            params: vec![slider("flakes", "Flakes", 5000.0, 0.0, 30000.0, 0), slider("size", "Size", 3.0, 0.0, 20.0, 2), pct("variation", "Variation % (Size)", 50.0), num("depth", "Scene Depth", 5000.0, 0.0, 20000.0, 0.0, 10000.0), num("speed", "Speed", 250.0, 0.0, 5000.0, 0.0, 2000.0), num("wind", "Wind", 50.0, -2000.0, 2000.0, -500.0, 500.0), num("wiggle", "Wiggle Amount", 3.0, 0.0, 50.0, 0.0, 20.0), color("color", "Color", [1.0, 1.0, 1.0]), pct("opacity", "Opacity", 100.0), slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0), check("composite", "Composite With Original", true)],
            render: snowfall,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.ccDrizzle",
            name: "CC Drizzle",
            category: CAT,
            params: vec![slider("dripRate", "Drip Rate", 4.0, 0.0, 100.0, 2), slider("longevity", "Longevity (sec)", 1.5, 0.0, 20.0, 2), slider("rippling", "Rippling", 2.0, 0.0, 20.0, 1), num("displacement", "Displacement", 50.0, 0.0, 500.0, 0.0, 200.0), num("rippleHeight", "Ripple Height", 30.0, 0.0, 200.0, 0.0, 100.0), num("spreading", "Spreading", 100.0, 0.0, 1000.0, 0.0, 400.0), slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0), angle("lightDirection", "Light Direction", -45.0)],
            render: drizzle,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.ccBubbles",
            name: "CC Bubbles",
            category: CAT,
            params: vec![slider("amount", "Bubble Amount", 50.0, 0.0, 1000.0, 0), num("speed", "Bubble Speed", 0.5, -10.0, 10.0, -5.0, 5.0), num("wobble", "Wobble Amplitude", 5.0, 0.0, 100.0, 0.0, 50.0), slider("wobbleFrequency", "Wobble Frequency", 1.0, 0.0, 10.0, 2), num("size", "Bubble Size", 6.0, 0.0, 100.0, 0.0, 30.0), popup("reflection", "Reflection Type", 0, &["Liquid", "Metal"]), popup("shading", "Shading Type", 0, &["Fade Inwards", "Fade Outwards", "None"])],
            render: bubbles,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.ccStarBurst",
            name: "CC Star Burst",
            category: CAT,
            params: vec![num("scatter", "Scatter", 100.0, 0.0, 1000.0, 0.0, 500.0), num("speed", "Speed", 1.0, -10.0, 10.0, -5.0, 5.0), angle("phase", "Phase", 0.0), num("grid", "Grid Spacing", 4.0, 1.0, 50.0, 1.0, 20.0), num("size", "Size", 100.0, 0.0, 1000.0, 0.0, 300.0), pct("blend", "Blend w. Original", 0.0)],
            render: star_burst,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.ccBallAction",
            name: "CC Ball Action",
            category: CAT,
            params: vec![num("scatter", "Scatter", 0.0, 0.0, 1000.0, 0.0, 200.0), popup("axis", "Rotation Axis", 0, &["X Axis", "Y Axis", "Z Axis", "XY Axis", "XZ Axis", "YZ Axis", "XYZ Axis"]), angle("rotation", "Rotation", 0.0), angle("twist", "Twist Angle", 0.0), num("grid", "Grid Spacing", 4.0, 1.0, 100.0, 1.0, 40.0), num("ballSize", "Ball Size", 100.0, 0.0, 200.0, 0.0, 200.0), slider("instability", "Instability State", 0.0, 0.0, 360.0, 1)],
            render: ball_action,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.ccPixelPolly",
            name: "CC Pixel Polly",
            category: CAT,
            params: vec![num("force", "Force", 100.0, 0.0, 1000.0, 0.0, 400.0), num("gravity", "Gravity", 1.0, -10.0, 10.0, -5.0, 5.0), angle("spinning", "Spinning", 0.0), point("center", "Force Center", 0.5, 0.5), pct("directionRandom", "Direction Randomness", 10.0), pct("speedRandom", "Speed Randomness", 10.0), num("grid", "Grid Spacing", 20.0, 1.0, 200.0, 2.0, 100.0), popup("object", "Object", 0, &["Polygon", "Textured Polygon", "Square", "Textured Square"]), slider("startTime", "Start Time (sec)", 0.0, 0.0, 100.0, 2)],
            render: pixel_polly,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.ccScatterize",
            name: "CC Scatterize",
            category: CAT,
            params: vec![num("scatter", "Scatter", 0.0, 0.0, 1000.0, 0.0, 300.0), angle("rightTwist", "Right Twist", 0.0), angle("leftTwist", "Left Twist", 0.0), popup("transfer", "Transfer Mode", 0, &["Composite", "Add", "Screen", "Lighten"])],
            render: scatterize,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.ccHair",
            name: "CC Hair",
            category: CAT,
            params: vec![num("length", "Length", 20.0, 0.0, 500.0, 0.0, 200.0), num("thickness", "Thickness", 1.0, 0.0, 20.0, 0.0, 10.0), slider("weight", "Weight", 0.5, -10.0, 10.0, 2), slider("density", "Density", 100.0, 0.0, 1000.0, 0), angle("direction", "Direction", 180.0), slider("curl", "Curl", 0.2, 0.0, 5.0, 2), pct("opacity", "Opacity", 100.0), pct("shading", "Shading", 50.0), slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0)],
            render: hair,
            pad: |c| c.f("length"),
        },
        EffectSpec {
            id: "fx.sim.ccMrMercury",
            name: "CC Mr. Mercury",
            category: CAT,
            params: vec![num("radiusX", "Radius X", 50.0, 0.0, 2000.0, 0.0, 400.0), num("radiusY", "Radius Y", 50.0, 0.0, 2000.0, 0.0, 400.0), point("producer", "Producer", 0.5, 0.3), angle("direction", "Direction", 0.0), num("velocity", "Velocity", 1.0, 0.0, 10.0, 0.0, 10.0), slider("birthRate", "Birth Rate", 1.0, 0.0, 20.0, 2), slider("longevity", "Longevity (sec)", 2.0, 0.0, 20.0, 2), num("gravity", "Gravity", 0.5, -10.0, 10.0, -2.0, 2.0), num("blobSize", "Blob Birth Size", 0.3, 0.0, 2.0, 0.0, 1.0), angle("lightDirection", "Light Direction", -45.0), slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0)],
            render: mercury,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.cardDance",
            name: "Card Dance",
            category: CAT,
            params: vec![
                slider("rows", "Rows", 10.0, 1.0, 100.0, 0),
                slider("columns", "Columns", 10.0, 1.0, 100.0, 0),
                layer_param("backLayer", "Back Layer"),
                layer_param("gradient1", "Gradient Layer 1"),
                num("zSource", "Z Position Multiplier", 0.0, -1000.0, 1000.0, -200.0, 200.0),
                num("xRotation", "X Rotation Multiplier", 0.0, -360.0, 360.0, -180.0, 180.0),
                num("yRotation", "Y Rotation Multiplier", 0.0, -360.0, 360.0, -180.0, 180.0),
                num("zRotation", "Z Rotation Multiplier", 0.0, -360.0, 360.0, -180.0, 180.0),
                num("scale", "Scale Multiplier", 0.0, -2.0, 2.0, -1.0, 1.0),
                slider("offset", "Offset", 0.0, -1.0, 1.0, 2),
                angle("lightDirection", "Light Direction", -45.0),
                pct("ambient", "Ambient Light", 25.0),
            ],
            render: card_dance,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.caustics",
            name: "Caustics",
            category: CAT,
            params: vec![layer_param("bottom", "Bottom"), layer_param("waterSurface", "Water Surface"), slider("waveHeight", "Wave Height", 0.2, 0.0, 2.0, 2), slider("smoothing", "Smoothing", 5.0, 0.0, 30.0, 1), slider("depth", "Water Depth", 0.1, 0.0, 1.0, 3), slider("ior", "Refractive Index", 1.2, 1.0, 3.0, 2), color("surfaceColor", "Surface Color", [1.0, 1.0, 1.0]), pct("surfaceOpacity", "Surface Opacity", 30.0), pct("strength", "Caustics Strength", 50.0), angle("lightDirection", "Light Direction", -45.0)],
            render: caustics,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.foam",
            name: "Foam",
            category: CAT,
            params: vec![point("producer", "Producer Point", 0.5, 0.9), num("producerX", "Producer X Size", 50.0, 0.0, 2000.0, 0.0, 500.0), slider("rate", "Production Rate", 10.0, 0.0, 200.0, 1), num("size", "Bubble Size", 8.0, 0.0, 200.0, 0.0, 60.0), pct("sizeVariance", "Size Variance", 50.0), slider("lifespan", "Lifespan", 3.0, 0.0, 30.0, 2), slider("growth", "Bubble Growth Speed", 0.1, 0.0, 5.0, 2), num("speed", "Initial Speed", 80.0, 0.0, 1000.0, 0.0, 300.0), num("wind", "Wind Speed", 0.0, -500.0, 500.0, -200.0, 200.0), num("wobble", "Wobble Amount", 5.0, 0.0, 100.0, 0.0, 30.0), color("color", "Bubble Color", [0.85, 0.9, 1.0]), pct("opacity", "Opacity", 80.0), slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0)],
            render: foam,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.shatter",
            name: "Shatter",
            category: CAT,
            params: vec![
                popup("view", "View", 0, &["Rendered", "Wireframe"]),
                popup("pattern", "Pattern", 0, &["Squares", "Bricks", "Triangles", "Hexagons", "Glass"]),
                slider("repetitions", "Repetitions", 10.0, 1.0, 200.0, 0),
                point("forcePosition", "Force 1 Position", 0.5, 0.5),
                num("forceRadius", "Force 1 Radius", 0.4, 0.0, 10.0, 0.0, 2.0),
                num("forceStrength", "Force 1 Strength", 5.0, -50.0, 50.0, -20.0, 20.0),
                slider("rotationSpeed", "Rotation Speed", 0.2, 0.0, 10.0, 2),
                slider("randomness", "Randomness", 0.1, 0.0, 1.0, 2),
                slider("viscosity", "Viscosity", 0.1, 0.0, 1.0, 2),
                num("gravity", "Gravity", 3.0, -50.0, 50.0, -10.0, 10.0),
                angle("gravityDirection", "Gravity Direction", 180.0),
                slider("startTime", "Start Time (sec)", 0.0, 0.0, 100.0, 2),
                slider("seed", "Random Seed", 1.0, 0.0, 10000.0, 0),
            ],
            render: shatter,
            pad: no_pad,
        },
        EffectSpec {
            id: "fx.sim.waveWorld",
            name: "Wave World",
            category: CAT,
            params: vec![
                popup("view", "View", 0, &["Height Map", "Wireframe Preview"]),
                num("waveSpeed", "Wave Speed", 0.3, 0.0, 5.0, 0.0, 2.0),
                slider("damping", "Damping", 0.1, 0.0, 1.0, 3),
                popup("type", "Producer 1 Type", 0, &["Ring", "Line"]),
                point("position", "Producer 1 Position", 0.5, 0.5),
                num("length", "Producer 1 Length", 0.1, 0.0, 2.0, 0.0, 1.0),
                num("amplitude", "Producer 1 Amplitude", 0.2, 0.0, 2.0, 0.0, 1.0),
                slider("frequency", "Producer 1 Frequency", 1.0, 0.0, 20.0, 2),
                angle("phase", "Producer 1 Phase", 0.0),
                slider("brightness", "Brightness", 0.5, 0.0, 1.0, 2),
                slider("contrast", "Contrast", 0.5, 0.0, 5.0, 2),
            ],
            render: wave_world,
            pad: no_pad,
        },
    ]
}

#[inline]
fn rnd(i: i64, k: i32, seed: u32) -> f64 {
    rand01((i & 0x7fff_ffff) as i32, (i >> 31) as i32, k, seed) as f64
}

struct Particle {
    pos: [f64; 3],
    prev: [f64; 3],
    age: f64,
    life: f64,
    rnd: f64,
}

fn spawn(c: &FxCtx, world: bool) -> Vec<Particle> {
    let rate = c.f("birthRate") * 50.0;
    let life = c.f("longevity").max(0.01);
    let t = c.t;
    if rate <= 0.0 || t < 0.0 {
        return vec![];
    }
    let seed = c.f("seed") as u32 + 1;
    let p0 = c.pt("producer");
    let pz = if world { c.px(c.f("producerZ")) } else { 0.0 };
    let (rx, ry) = (c.px(c.f("radiusX")), c.px(c.f("radiusY")));
    let anim = c.e("animation");
    let unit = c.layer_rect()[2].max(c.layer_rect()[3]) * 0.1;
    let vel = c.f("velocity") * unit;
    let grav = c.f("gravity") * unit;
    let res = c.f("resistance");
    let dir = c.f("direction").to_radians();
    let extra = c.f("extra");
    let floor = if world { Some(p0[1] + c.f("floor") * unit * 5.0) } else { None };
    let bounce = world && c.b("floorBounce");
    let first = (((t - life) * rate).floor()).max(0.0) as i64;
    let last = (t * rate).floor() as i64;
    let fd = 1.0 / c.fps.max(1.0);
    let mut out = Vec::with_capacity((last - first + 1).max(0) as usize);
    for i in first..=last {
        let birth = i as f64 / rate;
        let age = t - birth;
        if age < 0.0 || age > life {
            continue;
        }
        let (r1, r2, r3, r4) = (rnd(i, 1, seed), rnd(i, 2, seed), rnd(i, 3, seed), rnd(i, 4, seed));
        let start = [p0[0] + (r1 - 0.5) * 2.0 * rx, p0[1] + (r2 - 0.5) * 2.0 * ry, pz + if world { (r3 - 0.5) * 2.0 * rx } else { 0.0 }];
        let spread = (1.0 + (r4 - 0.5) * extra).max(0.0);
        let th = r3 * TAU;
        let phi = (r4 * 2.0 - 1.0).acos();
        let v: [f64; 3] = match anim {
            1 => [dir.sin() * vel * spread, -dir.cos() * vel * spread, 0.0],
            2 => {
                let a = dir + (r3 - 0.5) * 0.6;
                [a.sin() * vel * spread, -a.cos() * vel * spread, if world { (r4 - 0.5) * vel * 0.3 } else { 0.0 }]
            }
            3 => [-(start[1] - p0[1]) * 0.02 * vel, (start[0] - p0[0]) * 0.02 * vel, 0.0],
            4 => [(r3 - 0.5) * vel * 0.3, -vel * spread, 0.0],
            5 => [vel * spread * if r4 > 0.5 { 1.0 } else { -1.0 }, (r3 - 0.5) * vel * 0.2, 0.0],
            6 => [th.cos() * vel * spread * 0.5, th.sin() * vel * spread * 0.5, 0.0],
            _ => {
                if world {
                    [phi.sin() * th.cos() * vel * spread, phi.sin() * th.sin() * vel * spread, phi.cos() * vel * spread]
                } else {
                    [th.cos() * vel * spread, th.sin() * vel * spread, 0.0]
                }
            }
        };
        let at = |a: f64| -> [f64; 3] {
            let damp = if res > 1e-6 { (1.0 - (-res * a).exp()) / res } else { a };
            let mut p = [start[0] + v[0] * damp, start[1] + v[1] * damp + 0.5 * grav * a * a, start[2] + v[2] * damp];
            if anim == 6 {
                let ang = a * 4.0 * spread;
                let (s, co) = ang.sin_cos();
                let (dx, dy) = (p[0] - start[0], p[1] - start[1]);
                p[0] = start[0] + dx * co - dy * s;
                p[1] = start[1] + dx * s + dy * co;
            }
            if let Some(f) = floor
                && p[1] > f
            {
                p[1] = if bounce { f - (p[1] - f) * 0.5 } else { f };
            }
            p
        };
        out.push(Particle { pos: at(age), prev: at((age - fd).max(0.0)), age, life, rnd: r1 });
    }
    out
}

type Dot = (f64, f64, f64, [f32; 4], u32);

fn draw_particles(img: &mut Img, parts: &[Dot], transfer: u32) {
    let (w, h) = (img.w, img.h);
    let mut layer = vec![[0.0f32; 4]; w * h];
    let rows: Vec<(usize, Vec<[f32; 4]>)> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut row = vec![[0.0f32; 4]; w];
            let py = y as f64 + 0.5;
            for &(sx0, sy0, size, col, ty) in parts {
                let r = size * 0.5 + 1.0;
                if (py - sy0).abs() > r {
                    continue;
                }
                let xa = ((sx0 - r).floor().max(0.0)) as usize;
                let xb = ((sx0 + r).ceil() as isize).min(w as isize - 1);
                if xb < 0 {
                    continue;
                }
                for (x, px) in row.iter_mut().enumerate().take(xb as usize + 1).skip(xa) {
                    let dx = x as f64 + 0.5 - sx0;
                    let dy = py - sy0;
                    let d = dx.hypot(dy) / (size * 0.5).max(0.3);
                    let a = match ty {
                        0 | 5 | 6 | 7 => if dx.abs().max(dy.abs()) <= size * 0.5 { 1.0 } else { 0.0 },
                        1 => {
                            let ang = dy.atan2(dx);
                            let k = 0.4 + 0.6 * ((ang * 2.5).cos().abs());
                            if d <= k { 1.0 - d / k * 0.3 } else { 0.0 }
                        }
                        3 => if d <= 1.0 { (1.0 - d * d).sqrt() } else { 0.0 },
                        4 => if d <= 1.0 { 0.15 + 0.85 * d.powi(6) } else { 0.0 },
                        _ => (1.0 - d).max(0.0).powf(1.5),
                    } as f32;
                    if a <= 0.0 {
                        continue;
                    }
                    let aa = a * col[3];
                    let k = 1.0 - aa;
                    for i in 0..3 {
                        px[i] = col[i] * aa + px[i] * if transfer == 0 { k } else { 1.0 };
                    }
                    px[3] = aa + px[3] * k;
                }
            }
            (y, row)
        })
        .collect();
    for (y, r) in rows {
        layer[y * w..(y + 1) * w].copy_from_slice(&r);
    }
    img.px.par_chunks_mut(4).zip(layer.par_iter()).for_each(|(d, s)| match transfer {
        1 => {
            for i in 0..3 {
                d[i] += s[i];
            }
            d[3] = (d[3] + s[3]).min(1.0);
        }
        2 => {
            for i in 0..3 {
                d[i] = d[i] + s[i] - d[i] * s[i];
            }
            d[3] = (d[3] + s[3] - d[3] * s[3]).min(1.0);
        }
        3 => {
            for i in 0..3 {
                d[i] = d[i].max(s[i]);
            }
            d[3] = d[3].max(s[3]);
        }
        _ => {
            let k = 1.0 - s[3];
            for i in 0..4 {
                d[i] = s[i] + d[i] * k;
            }
        }
    });
}

fn particles(c: &FxCtx, img: &mut Img, world: bool) {
    let ps = spawn(c, world);
    let rect = c.layer_rect();
    let (cx, cy) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
    let ry = c.f("cameraRotationY").to_radians();
    let rx = c.f("cameraRotationX").to_radians();
    let dist = c.f("cameraDistance").max(0.1) * (rect[2] - rect[0]).max(1.0);
    let ty = c.e("particleType");
    let (bs, ds) = (c.px(c.f("birthSize")), c.px(c.f("deathSize")));
    let var = c.f("sizeVariation") / 100.0;
    let maxo = c.f("maxOpacity") as f32 / 100.0;
    let (bc, dc) = (c.c("birthColor"), c.c("deathColor"));
    let project = |p: [f64; 3]| -> Option<(f64, f64, f64)> {
        if !world {
            return Some((p[0], p[1], 1.0));
        }
        let (x, y, z) = (p[0] - cx, p[1] - cy, p[2]);
        let (sy, cy2) = ry.sin_cos();
        let (x, z) = (x * cy2 + z * sy, -x * sy + z * cy2);
        let (sx, cx2) = rx.sin_cos();
        let (y, z) = (y * cx2 - z * sx, y * sx + z * cx2);
        let zz = z + dist;
        if zz <= 1.0 {
            return None;
        }
        let k = dist / zz;
        Some((cx + x * k, cy + y * k, k))
    };
    let mut items: Vec<(f64, Dot)> = vec![];
    for p in &ps {
        let u = p.age / p.life;
        let Some((x, y, k)) = project(p.pos) else { continue };
        let size = (bs + (ds - bs) * u) * (1.0 + (p.rnd - 0.5) * 2.0 * var) * k;
        let col = [bc[0] + (dc[0] - bc[0]) * u as f32, bc[1] + (dc[1] - bc[1]) * u as f32, bc[2] + (dc[2] - bc[2]) * u as f32, maxo * (1.0 - (u as f32 - 0.8).max(0.0) * 5.0)];
        if ty == 0 || ty == 6 {
            if let Some((px, py, _)) = project(p.prev) {
                let n = ((x - px).hypot(y - py) / (size * 0.5).max(0.5)).ceil().clamp(1.0, 24.0) as usize;
                for j in 0..=n {
                    let t = j as f64 / n as f64;
                    items.push((-k, (px + (x - px) * t, py + (y - py) * t, size.max(0.5) * if ty == 0 { 0.5 } else { 1.0 }, col, ty)));
                }
                continue;
            }
        }
        items.push((-k, (x, y, size.max(0.3), col, ty)));
    }
    items.sort_by(|a, b| a.0.total_cmp(&b.0));
    let parts: Vec<Dot> = items.into_iter().map(|i| i.1).collect();
    draw_particles(img, &parts, c.e("transfer"));
}

fn playground(c: &FxCtx, img: &mut Img) {
    let rate = c.f("perSecond");
    let life = c.f("lifespan").max(0.01);
    let t = c.t;
    if rate <= 0.0 || t < 0.0 {
        return;
    }
    let p0 = c.pt("cannon");
    let br = c.px(c.f("barrelRadius"));
    let dir = c.f("direction").to_radians();
    let spread = c.f("spread").to_radians();
    let vel = c.px(c.f("velocity"));
    let vs = c.px(c.f("velocitySpread"));
    let g = c.px(c.f("gravity"));
    let gd = c.f("gravityDirection").to_radians();
    let (gx, gy) = (gd.sin() * g, -gd.cos() * g);
    let col = c.c("color");
    let rad = c.px(c.f("radius")) * 2.0;
    let first = ((t - life) * rate).floor().max(0.0) as i64;
    let last = (t * rate).floor() as i64;
    let mut parts = vec![];
    for i in first..=last {
        let age = t - i as f64 / rate;
        if age < 0.0 || age > life {
            continue;
        }
        let a = dir + (rnd(i, 1, 9) - 0.5) * spread;
        let v = vel + (rnd(i, 2, 9) - 0.5) * vs;
        let off = (rnd(i, 3, 9) - 0.5) * 2.0 * br;
        let start = [p0[0] + a.cos() * off, p0[1] + a.sin() * off];
        let pos = [start[0] + a.sin() * v * age + 0.5 * gx * age * age, start[1] - a.cos() * v * age + 0.5 * gy * age * age];
        parts.push((pos[0], pos[1], rad.max(0.5), [col[0], col[1], col[2], 1.0], 3u32));
    }
    draw_particles(img, &parts, 0);
}

fn rain_snow(c: &FxCtx, img: &mut Img, n: f64, snow: bool) {
    let (w, h) = (img.w, img.h);
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let depth = c.f("depth").max(1.0);
    let speed = c.px(c.f("speed"));
    let wind = c.px(c.f("wind"));
    let size = c.px(c.f("size")).max(0.1);
    let var = c.f("variation") / 100.0;
    let col = c.c("color");
    let op = c.f("opacity") as f32 / 100.0;
    let seed = c.f("seed") as u32;
    let wig = if snow { c.px(c.f("wiggle")) } else { 0.0 };
    let t = c.t;
    let count = (n * (lw * lh) / (1920.0 * 1080.0)).round().clamp(0.0, 60000.0) as i64;
    let mut segs: Vec<(f64, f64, f64, f64, f64, f32)> = Vec::with_capacity(count as usize);
    for i in 0..count {
        let z = 0.2 + rnd(i, 1, seed) * 0.8;
        let persp = 1.0 / (z * depth / 2000.0 + 0.5);
        let sp = speed * persp * (0.8 + 0.4 * rnd(i, 2, seed));
        let wv = wind * persp * (1.0 + (rnd(i, 3, seed) - 0.5) * var * 2.0);
        let x0 = rnd(i, 4, seed) * (lw + lh * 0.5) - lh * 0.25;
        let y0 = rnd(i, 5, seed) * lh;
        let travel = lh + 40.0;
        let y = rect[1] + (y0 + sp * t).rem_euclid(travel) - 20.0;
        let mut x = rect[0] + (x0 + wv * t).rem_euclid(lw + lh * 0.5) - lh * 0.25;
        if snow {
            x += (t * 2.0 + rnd(i, 6, seed) * TAU).sin() * wig * persp;
        }
        let s = size * persp * if snow { 1.0 - var * rnd(i, 7, seed) * 0.9 } else { 1.0 };
        let (dx, dy) = if snow { (0.0, 0.0) } else { (wv * 0.02, sp * 0.02 * (0.5 + s * 0.2)) };
        segs.push((x, y, x - dx, y - dy, s, op * (0.4 + 0.6 * z as f32)));
    }
    let mut layer = Img::new(w, h);
    let rows: Vec<(usize, Vec<f32>)> = (0..h)
        .into_par_iter()
        .map(|yy| {
            let mut row = vec![0.0f32; w];
            let py = yy as f64 + 0.5;
            for (x0, y0, x1, y1, s, o) in &segs {
                let r = s * 0.5 + 1.0;
                if py < y0.min(*y1) - r || py > y0.max(*y1) + r {
                    continue;
                }
                let xa = (x0.min(*x1) - r).floor().max(0.0) as usize;
                let xb = ((x0.max(*x1) + r).ceil() as isize).min(w as isize - 1);
                if xb < 0 {
                    continue;
                }
                for (x, v) in row.iter_mut().enumerate().take(xb as usize + 1).skip(xa) {
                    let d = super::draw::seg_dist([x as f64 + 0.5, py], [*x0, *y0], [*x1, *y1]);
                    let a = (1.0 - d / (s * 0.5).max(0.4)).clamp(0.0, 1.0) as f32 * o;
                    *v = v.max(a);
                }
            }
            (yy, row)
        })
        .collect();
    for (y, r) in rows {
        for x in 0..w {
            let a = r[x];
            let i = (y * w + x) * 4;
            layer.px[i] = col[0] * a;
            layer.px[i + 1] = col[1] * a;
            layer.px[i + 2] = col[2] * a;
            layer.px[i + 3] = a;
        }
    }
    if c.b("composite") {
        img.px.par_chunks_mut(4).zip(layer.px.par_chunks(4)).for_each(|(d, s)| {
            let k = 1.0 - s[3];
            for i in 0..4 {
                d[i] = s[i] + d[i] * k;
            }
        });
    } else {
        *img = layer;
    }
}

fn rainfall(c: &FxCtx, img: &mut Img) {
    rain_snow(c, img, c.f("drops"), false);
}

fn snowfall(c: &FxCtx, img: &mut Img) {
    rain_snow(c, img, c.f("flakes"), true);
}

fn drizzle(c: &FxCtx, img: &mut Img) {
    let rate = c.f("dripRate");
    let life = c.f("longevity").max(0.01);
    let t = c.t;
    if rate <= 0.0 || t < 0.0 {
        return;
    }
    let rect = c.layer_rect();
    let seed = c.f("seed") as u32;
    let spread = c.px(c.f("spreading"));
    let ripples = c.f("rippling").max(0.1);
    let disp = c.px(c.f("displacement")) * 0.05;
    let rh = c.f("rippleHeight") as f32 / 100.0;
    let la = c.f("lightDirection").to_radians();
    let first = ((t - life) * rate).floor().max(0.0) as i64;
    let last = (t * rate).floor() as i64;
    let drops: Vec<([f64; 2], f64)> = (first..=last)
        .filter_map(|i| {
            let age = t - i as f64 / rate;
            (0.0..=life).contains(&age).then(|| ([rect[0] + rnd(i, 1, seed) * (rect[2] - rect[0]), rect[1] + rnd(i, 2, seed) * (rect[3] - rect[1])], age))
        })
        .collect();
    if drops.is_empty() {
        return;
    }
    let (w, h) = (img.w, img.h);
    let hf: Vec<f32> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
            let mut v = 0.0f64;
            for (p, age) in &drops {
                let r = (x - p[0]).hypot(y - p[1]);
                let front = age / life * spread;
                let d = front - r;
                if d < 0.0 || d > spread / ripples * 2.0 {
                    continue;
                }
                v += (d / (spread / ripples) * TAU).sin() * (1.0 - age / life) * (1.0 - d / (spread / ripples * 2.0));
            }
            v as f32
        })
        .collect();
    let src = img.clone();
    let (lx, ly) = (la.cos() as f32, la.sin() as f32);
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let g = |xx: usize, yy: usize| hf[yy.min(h - 1) * w + xx.min(w - 1)];
            let gx = g(x + 1, y) - g(x.saturating_sub(1), y);
            let gy = g(x, y + 1) - g(x, y.saturating_sub(1));
            let s = src.sample(x as f64 + 0.5 + gx as f64 * disp * 20.0, y as f64 + 0.5 + gy as f64 * disp * 20.0, true);
            let shade = -(gx * lx + gy * ly) * rh * 2.0;
            for k in 0..3 {
                row[x * 4 + k] = (s[k] + shade * s[3]).max(0.0);
            }
            row[x * 4 + 3] = s[3];
        }
    });
}

fn bubbles(c: &FxCtx, img: &mut Img) {
    let n = c.f("amount").round().max(0.0) as i64;
    if n == 0 {
        return;
    }
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let speed = c.f("speed") * lh * 0.2;
    let wob = c.px(c.f("wobble"));
    let wf = c.f("wobbleFrequency");
    let size = c.px(c.f("size")).max(0.5);
    let metal = c.e("reflection") == 1;
    let shading = c.e("shading");
    let t = c.t;
    let src = img.clone();
    let bubbles: Vec<([f64; 2], f64)> = (0..n)
        .map(|i| {
            let r = size * (0.5 + rnd(i, 1, 3));
            let x = rect[0] + rnd(i, 2, 3) * lw + (t * wf * TAU + rnd(i, 3, 3) * TAU).sin() * wob;
            let y = rect[1] + (rnd(i, 4, 3) * (lh + 4.0 * size) - speed * t * (0.6 + 0.8 * rnd(i, 5, 3))).rem_euclid(lh + 4.0 * size) - 2.0 * size;
            ([x, y], r)
        })
        .collect();
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(yy, row)| {
        let py = yy as f64 + 0.5;
        for x in 0..w {
            let px = x as f64 + 0.5;
            for (p, r) in &bubbles {
                let (dx, dy) = (px - p[0], py - p[1]);
                let d = dx.hypot(dy) / r;
                if d > 1.0 {
                    continue;
                }
                let z = (1.0 - d * d).sqrt();
                let s = src.sample(p[0] + dx * (1.0 - z) * 2.0 * if metal { -1.0 } else { 1.0 }, p[1] + dy * (1.0 - z) * 2.0, true);
                let k = match shading {
                    0 => (1.0 - z) as f32 * 0.8 + 0.2,
                    1 => z as f32,
                    _ => 1.0,
                };
                let spec = ((-dx - dy) / (r * 1.414) + z * 0.6).max(0.0).powi(8) as f32;
                let q = &mut row[x * 4..x * 4 + 4];
                for i in 0..3 {
                    q[i] = s[i] * k + spec;
                }
                q[3] = (s[3].max(0.3) * k + spec).min(1.0);
            }
        }
    });
}

fn star_burst(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let (cx, cy) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
    let grid = c.px(c.f("grid")).max(1.0);
    let scatter = c.f("scatter") / 100.0;
    let sp = c.f("speed");
    let ph = c.f("phase") / 360.0;
    let sz = c.f("size") / 100.0;
    let blend = c.f("blend") as f32 / 100.0;
    let src = img.clone();
    let mut parts = vec![];
    let (nx, ny) = (((rect[2] - rect[0]) / grid).ceil() as i64, ((rect[3] - rect[1]) / grid).ceil() as i64);
    let (w, h) = (img.w, img.h);
    for j in 0..ny {
        for i in 0..nx {
            let id = j * 100_000 + i;
            let z0 = rnd(id, 1, 7);
            let z = (z0 + c.t * sp * 0.25 + ph).rem_euclid(1.0);
            let k = 0.3 / (1.05 - z);
            let x = rect[0] + (i as f64 + 0.5) * grid;
            let y = rect[1] + (j as f64 + 0.5) * grid;
            let jx = (rnd(id, 2, 7) - 0.5) * grid * scatter * 10.0;
            let jy = (rnd(id, 3, 7) - 0.5) * grid * scatter * 10.0;
            let (px, py) = (cx + (x + jx - cx) * k, cy + (y + jy - cy) * k);
            if px < -grid || py < -grid || px > w as f64 + grid || py > h as f64 + grid {
                continue;
            }
            let col = src.sample(x, y, true);
            if col[3] <= 0.01 {
                continue;
            }
            parts.push((px, py, (grid * 0.5 * k * sz).max(0.5),[col[0] / col[3].max(1e-6), col[1] / col[3].max(1e-6), col[2] / col[3].max(1e-6), col[3] * (z as f32).min(1.0)], 1u32));
        }
    }
    let orig = img.clone();
    *img = Img::new(w, h);
    draw_particles(img, &parts, 0);
    if blend > 0.0 {
        img.px.par_iter_mut().zip(orig.px.par_iter()).for_each(|(v, o)| *v += (o - *v) * blend);
    }
}

fn ball_action(c: &FxCtx, img: &mut Img) {
    let grid = c.px(c.f("grid")).max(1.0);
    let scatter = c.px(c.f("scatter"));
    let rot = c.f("rotation").to_radians();
    let twist = c.f("twist").to_radians();
    let axis = c.e("axis");
    let bsize = c.f("ballSize") / 100.0;
    let inst = c.f("instability").to_radians();
    let rect = c.layer_rect();
    let (cx, cy) = ((rect[0] + rect[2]) * 0.5, (rect[1] + rect[3]) * 0.5);
    let src = img.clone();
    let (w, h) = (img.w, img.h);
    let mut balls = vec![];
    let (nx, ny) = (((rect[2] - rect[0]) / grid).ceil() as i64, ((rect[3] - rect[1]) / grid).ceil() as i64);
    for j in 0..ny {
        for i in 0..nx {
            let id = j * 100_000 + i;
            let (x, y) = (rect[0] + (i as f64 + 0.5) * grid, rect[1] + (j as f64 + 0.5) * grid);
            let col = src.sample(x, y, true);
            if col[3] <= 0.01 {
                continue;
            }
            let (dx, dy) = (x - cx, y - cy);
            let a = rot + twist * (dx.hypot(dy) / (rect[2] - rect[0]).max(1.0));
            let mut p = [dx, dy, 0.0];
            let (s, co) = a.sin_cos();
            if matches!(axis, 0 | 3 | 4 | 6) {
                p = [p[0], p[1] * co - p[2] * s, p[1] * s + p[2] * co];
            }
            if matches!(axis, 1 | 3 | 5 | 6) {
                p = [p[0] * co + p[2] * s, p[1], -p[0] * s + p[2] * co];
            }
            if matches!(axis, 2 | 4 | 5 | 6) {
                p = [p[0] * co - p[1] * s, p[0] * s + p[1] * co, p[2]];
            }
            let jit = (rnd(id, 1, 5) - 0.5 + (inst + rnd(id, 2, 5) * TAU).sin() * 0.3) * scatter;
            let k = 1000.0 / (1000.0 + p[2]);
            balls.push((p[2], (cx + (p[0] + jit) * k, cy + (p[1] + jit) * k, grid * bsize * k,[col[0] / col[3], col[1] / col[3], col[2] / col[3], col[3]], 3u32)));
        }
    }
    balls.sort_by(|a, b| b.0.total_cmp(&a.0));
    let parts: Vec<Dot> = balls.into_iter().map(|b| b.1).collect();
    *img = Img::new(w, h);
    draw_particles(img, &parts, 0);
}

fn piece_render(img: &mut Img, src: &Img, pieces: &[(Vec<[f64; 2]>, Mat3, f32)], wire: bool, flat: Option<&[[f32; 4]]>) {
    let (w, h) = (img.w, img.h);
    let mut out = Img::new(w, h);
    for (pi, (poly, m, shade)) in pieces.iter().enumerate() {
        let pts: Vec<[f64; 2]> = poly.iter().map(|p| m.apply(p[0], p[1])).collect();
        let Some(inv) = m.inverse() else { continue };
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in &pts {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        let (xa, xb) = (x0.floor().max(0.0) as usize, (x1.ceil() as isize).min(w as isize - 1));
        let (ya, yb) = (y0.floor().max(0.0) as usize, (y1.ceil() as isize).min(h as isize - 1));
        if xb < 0 || yb < 0 {
            continue;
        }
        for y in ya..=yb as usize {
            for x in xa..=xb as usize {
                let p = [x as f64 + 0.5, y as f64 + 0.5];
                let mut inside = false;
                let n = pts.len();
                let mut edge = f64::MAX;
                for i in 0..n {
                    let (a, b) = (pts[i], pts[(i + 1) % n]);
                    if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                        inside = !inside;
                    }
                    edge = edge.min(super::draw::seg_dist(p, a, b));
                }
                if !inside {
                    continue;
                }
                let i = (y * w + x) * 4;
                if wire {
                    if edge < 1.0 {
                        out.px[i..i + 4].copy_from_slice(&[1.0, 1.0, 1.0, 1.0]);
                    }
                    continue;
                }
                let c = match flat {
                    Some(f) => f[pi],
                    None => {
                        let s = inv.apply(p[0], p[1]);
                        src.sample(s[0], s[1], false)
                    }
                };
                if c[3] <= 0.0 {
                    continue;
                }
                out.px[i] = c[0] * shade;
                out.px[i + 1] = c[1] * shade;
                out.px[i + 2] = c[2] * shade;
                out.px[i + 3] = c[3];
            }
        }
    }
    *img = out;
}

fn cells(rect: [f64; 4], pattern: u32, n: f64, seed: u32) -> Vec<Vec<[f64; 2]>> {
    let (lw, lh) = (rect[2] - rect[0], rect[3] - rect[1]);
    let s = (lw.max(lh) / n.max(1.0)).max(2.0);
    let (cols, rows) = ((lw / s).ceil() as i64, (lh / s).ceil() as i64);
    let mut out = vec![];
    for j in 0..rows {
        for i in 0..cols {
            let off = if pattern == 1 && j % 2 == 1 { s * 0.5 } else { 0.0 };
            let (x, y) = (rect[0] + i as f64 * s - off, rect[1] + j as f64 * s);
            match pattern {
                2 => {
                    out.push(vec![[x, y], [x + s, y], [x, y + s]]);
                    out.push(vec![[x + s, y], [x + s, y + s], [x, y + s]]);
                }
                3 => {
                    let r = s * 0.55;
                    let (hx, hy) = (x + s * 0.5 + if j % 2 == 1 { s * 0.5 } else { 0.0 }, y + s * 0.5);
                    out.push((0..6).map(|k| {
                        let a = k as f64 / 6.0 * TAU + PI / 6.0;
                        [hx + a.cos() * r, hy + a.sin() * r]
                    }).collect());
                }
                4 => {
                    let jit = |a: i64, b: i64, k: i32| (rand01(a as i32, b as i32, k, seed) as f64 - 0.5) * s * 0.6;
                    let p = |a: i64, b: i64| [rect[0] + a as f64 * s + jit(a, b, 1), rect[1] + b as f64 * s + jit(a, b, 2)];
                    out.push(vec![p(i, j), p(i + 1, j), p(i + 1, j + 1), p(i, j + 1)]);
                }
                1 => out.push(vec![[x, y], [x + s, y], [x + s, y + s * 0.5], [x, y + s * 0.5]]),
                _ => out.push(vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s]]),
            }
            if pattern == 1 {
                let y2 = y + s * 0.5;
                let off2 = if j % 2 == 1 { 0.0 } else { s * 0.5 };
                let x2 = rect[0] + i as f64 * s - off2;
                out.push(vec![[x2, y2], [x2 + s, y2], [x2 + s, y2 + s * 0.5], [x2, y2 + s * 0.5]]);
            }
        }
    }
    out
}

fn shatter(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let seed = c.f("seed") as u32;
    let polys = cells(rect, c.e("pattern"), c.f("repetitions"), seed);
    let t = (c.t - c.f("startTime")).max(0.0);
    let fp = c.pt("forcePosition");
    let fr = c.f("forceRadius") * (rect[2] - rect[0]).max(rect[3] - rect[1]) * 0.5;
    let fs = c.f("forceStrength") * (rect[2] - rect[0]) * 0.02;
    let rs = c.f("rotationSpeed");
    let rnd_amt = c.f("randomness");
    let visc = c.f("viscosity");
    let g = c.f("gravity") * (rect[3] - rect[1]) * 0.02;
    let gd = c.f("gravityDirection").to_radians();
    let src = img.clone();
    let mut pieces = vec![];
    for (k, poly) in polys.iter().enumerate() {
        let n = poly.len() as f64;
        let ctr = [poly.iter().map(|p| p[0]).sum::<f64>() / n, poly.iter().map(|p| p[1]).sum::<f64>() / n];
        let d =(ctr[0] - fp[0]).hypot(ctr[1] - fp[1]);
        let hit = d < fr;
        if !hit || t <= 0.0 {
            pieces.push((poly.clone(), Mat3::IDENTITY, 1.0f32));
            continue;
        }
        let kk = k as i64;
        let push = fs * (1.0 - d / fr.max(1e-6)) * (1.0 + (rnd(kk, 1, seed) - 0.5) * rnd_amt * 2.0);
        let (ux, uy) = if d > 1e-6 { ((ctr[0] - fp[0]) / d, (ctr[1] - fp[1]) / d) } else { (rnd(kk, 2, seed) - 0.5, rnd(kk, 3, seed) - 0.5) };
        let damp = if visc > 1e-6 { (1.0 - (-visc * 3.0 * t).exp()) / (visc * 3.0) } else { t };
        let off = [ux * push * damp * 10.0 + gd.sin() * g * t * t * 0.5, uy * push * damp * 10.0 - gd.cos() * g * t * t * 0.5];
        let ang = rs * t * TAU * (rnd(kk, 4, seed) - 0.5) * 2.0;
        let z = push * damp * 0.01;
        let sc = 1.0 / (1.0 - z.min(0.9)).max(0.1);
        let m = Mat3::translate(ctr[0] + off[0], ctr[1] + off[1]) * Mat3::rotate_deg(ang.to_degrees()) * Mat3::scale(sc, sc) * Mat3::translate(-ctr[0], -ctr[1]);
        let shade = (0.6 + 0.4 * ang.cos().abs()) as f32;
        pieces.push((poly.clone(), m, shade));
    }
    piece_render(img, &src, &pieces, c.e("view") == 1, None);
}

fn pixel_polly(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let t = (c.t - c.f("startTime")).max(0.0);
    if t <= 0.0 {
        return;
    }
    let grid = c.px(c.f("grid")).max(1.0);
    let n = ((rect[2] - rect[0]).max(rect[3] - rect[1]) / grid).max(1.0);
    let obj = c.e("object");
    let polys = cells(rect, if obj >= 2 { 0 } else { 2 }, n, 3);
    let ctr = c.pt("center");
    let force = c.px(c.f("force"));
    let g = c.f("gravity") * (rect[3] - rect[1]) * 0.1;
    let spin = c.f("spinning").to_radians();
    let dr = c.f("directionRandom") / 100.0;
    let sr = c.f("speedRandom") / 100.0;
    let src = img.clone();
    let textured = obj == 1 || obj == 3;
    let mut pieces = vec![];
    for (k, poly) in polys.iter().enumerate() {
        let m = poly.len() as f64;
        let pc = [poly.iter().map(|p| p[0]).sum::<f64>() / m, poly.iter().map(|p| p[1]).sum::<f64>() / m];
        let kk = k as i64;
        let (dx, dy) = (pc[0] - ctr[0], pc[1] - ctr[1]);
        let d = dx.hypot(dy).max(1e-3);
        let a = dy.atan2(dx) + (rnd(kk, 1, 3) - 0.5) * dr * PI;
        let v = force * (1.0 + (rnd(kk, 2, 3) - 0.5) * sr * 2.0) * (100.0 / (d + 100.0));
        let off = [a.cos() * v * t, a.sin() * v * t + 0.5 * g * t * t];
        let ang = (spin + (rnd(kk, 3, 3) - 0.5) * PI) * t;
        let mm = Mat3::translate(pc[0] + off[0], pc[1] + off[1]) * Mat3::rotate_deg(ang.to_degrees()) * Mat3::translate(-pc[0], -pc[1]);
        pieces.push((poly.clone(), mm, (0.7 + 0.3 * ang.cos().abs()) as f32));
    }
    if textured {
        piece_render(img, &src, &pieces, false, None);
    } else {
        let flat: Vec<[f32; 4]> = pieces
            .iter()
            .map(|p| {
                let n = p.0.len() as f64;
                let pc = [p.0.iter().map(|q| q[0]).sum::<f64>() / n, p.0.iter().map(|q| q[1]).sum::<f64>() / n];
                src.sample(pc[0], pc[1], false)
            })
            .collect();
        piece_render(img, &src, &pieces, false, Some(&flat));
    }
}

fn scatterize(c: &FxCtx, img: &mut Img) {
    let sc = c.px(c.f("scatter"));
    if sc < 0.5 {
        return;
    }
    let (rt, lt) = (c.f("rightTwist").to_radians(), c.f("leftTwist").to_radians());
    let rect = c.layer_rect();
    let cx = (rect[0] + rect[2]) * 0.5;
    let lw = (rect[2] - rect[0]).max(1.0);
    let src = img.clone();
    let (w, h) = (img.w, img.h);
    let mut parts = vec![];
    let step = 2usize;
    for y in (0..h).step_by(step) {
        for x in (0..w).step_by(step) {
            let s = src.get(x as isize, y as isize);
            if s[3] <= 0.02 {
                continue;
            }
            let id = (y * w + x) as i64;
            let u = (x as f64 - cx) / lw;
            let tw = if u > 0.0 { rt * u * 2.0 } else { lt * -u * 2.0 };
            let ang = rnd(id, 1, 11) * TAU + tw;
            let r = rnd(id, 2, 11) * sc;
            let (px, py) = (x as f64 + ang.cos() * r, y as f64 + ang.sin() * r);
            parts.push((px, py, step as f64 * 1.2,[s[0] / s[3], s[1] / s[3], s[2] / s[3], s[3]], 5u32));
        }
    }
    *img = Img::new(w, h);
    draw_particles(img, &parts, c.e("transfer"));
}

fn hair(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let len = c.px(c.f("length"));
    if len < 0.5 {
        return;
    }
    let th = c.px(c.f("thickness")).max(0.3);
    let weight = c.f("weight");
    let dens = c.f("density") / 100.0;
    let dir = c.f("direction").to_radians();
    let curl = c.f("curl");
    let op = c.f("opacity") as f32 / 100.0;
    let shading = c.f("shading") as f32 / 100.0;
    let seed = c.f("seed") as u32;
    let src = img.clone();
    let spacing = (th * 2.0 / dens.max(0.01)).max(1.0);
    let mut segs: Vec<(super::draw::Seg, [f32; 4])> = vec![];
    let (nx, ny) = ((w as f64 / spacing) as i64, (h as f64 / spacing) as i64);
    for j in 0..ny {
        for i in 0..nx {
            let id = j * 100_000 + i;
            let x0 = (i as f64 + rnd(id, 1, seed)) * spacing;
            let y0 = (j as f64 + rnd(id, 2, seed)) * spacing;
            let s = src.sample(x0, y0, false);
            if s[3] <= 0.05 {
                continue;
            }
            let col = [s[0] / s[3], s[1] / s[3], s[2] / s[3], s[3] * op];
            let mut p = [x0, y0];
            let mut a = dir - std::f64::consts::FRAC_PI_2 + (rnd(id, 3, seed) - 0.5) * 0.5;
            let n = 6;
            for k in 0..n {
                a += (rnd(id, 4 + k, seed) - 0.5) * curl + weight * 0.05;
                let q = [p[0] + a.cos() * len / n as f64, p[1] + a.sin() * len / n as f64];
                let shade = 1.0 - shading * (1.0 - k as f32 / n as f32) * 0.5;
                segs.push(((p, q, th), [col[0] * shade, col[1] * shade, col[2] * shade, col[3]]));
                p = q;
            }
        }
    }
    let rows: Vec<(usize, Vec<[f32; 4]>)> = (0..h)
        .into_par_iter()
        .map(|y| {
            let mut row: Vec<[f32; 4]> = (0..w).map(|x| { let i = (y * w + x) * 4; [src.px[i], src.px[i + 1], src.px[i + 2], src.px[i + 3]] }).collect();
            let py = y as f64 + 0.5;
            for ((a, b, t), col) in &segs {
                let r = t * 0.5 + 1.0;
                if py < a[1].min(b[1]) - r || py > a[1].max(b[1]) + r {
                    continue;
                }
                let xa = (a[0].min(b[0]) - r).floor().max(0.0) as usize;
                let xb = ((a[0].max(b[0]) + r).ceil() as isize).min(w as isize - 1);
                if xb < 0 {
                    continue;
                }
                for (x, px) in row.iter_mut().enumerate().take(xb as usize + 1).skip(xa) {
                    let d = super::draw::seg_dist([x as f64 + 0.5, py], *a, *b);
                    let k = ((1.0 - d / (t * 0.5).max(0.3)).clamp(0.0, 1.0) as f32) * col[3];
                    if k <= 0.0 {
                        continue;
                    }
                    for i in 0..3 {
                        px[i] = col[i] * k + px[i] * (1.0 - k);
                    }
                    px[3] = k + px[3] * (1.0 - k);
                }
            }
            (y, row)
        })
        .collect();
    for (y, r) in rows {
        for (x, p) in r.iter().enumerate() {
            img.px[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(p);
        }
    }
}

fn mercury(c: &FxCtx, img: &mut Img) {
    let rate = c.f("birthRate") * 10.0;
    let life = c.f("longevity").max(0.01);
    let t = c.t;
    if rate <= 0.0 || t < 0.0 {
        return;
    }
    let p0 = c.pt("producer");
    let (rx, ry) = (c.px(c.f("radiusX")), c.px(c.f("radiusY")));
    let dir = c.f("direction").to_radians();
    let rect = c.layer_rect();
    let unit = (rect[2] - rect[0]).max(rect[3] - rect[1]) * 0.1;
    let vel = c.f("velocity") * unit;
    let g = c.f("gravity") * unit;
    let bsz = c.f("blobSize") * unit * 0.6;
    let seed = c.f("seed") as u32;
    let la = c.f("lightDirection").to_radians();
    let first = ((t - life) * rate).floor().max(0.0) as i64;
    let last = (t * rate).floor() as i64;
    let blobs: Vec<([f64; 2], f64)> = (first..=last)
        .filter_map(|i| {
            let age = t - i as f64 / rate;
            if !(0.0..=life).contains(&age) {
                return None;
            }
            let a = dir + (rnd(i, 1, seed) - 0.5) * 1.2;
            let start = [p0[0] + (rnd(i, 2, seed) - 0.5) * 2.0 * rx * 0.2, p0[1] + (rnd(i, 3, seed) - 0.5) * 2.0 * ry * 0.2];
            let v = vel * (0.5 + rnd(i, 4, seed));
            Some(([start[0] + a.sin() * v * age, start[1] - a.cos() * v * age + 0.5 * g * age * age], bsz * (1.0 - age / life * 0.5)))
        })
        .collect();
    if blobs.is_empty() {
        return;
    }
    let (w, h) = (img.w, img.h);
    let field: Vec<f32> = (0..w * h)
        .into_par_iter()
        .map(|i| {
            let (x, y) = ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5);
            blobs.iter().map(|(p, r)| r * r / ((x - p[0]).powi(2) + (y - p[1]).powi(2) + 1.0)).sum::<f64>() as f32
        })
        .collect();
    let src = img.clone();
    let (lx, ly) = (la.cos() as f32, la.sin() as f32);
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let f = field[y * w + x];
            if f < 0.6 {
                continue;
            }
            let g = |xx: usize, yy: usize| field[yy.min(h - 1) * w + xx.min(w - 1)];
            let gx = g(x + 1, y) - g(x.saturating_sub(1), y);
            let gy = g(x, y + 1) - g(x, y.saturating_sub(1));
            let a = smooth(0.6, 0.8, f);
            let s = src.sample(x as f64 + 0.5 - gx as f64 * 20.0, y as f64 + 0.5 - gy as f64 * 20.0, true);
            let spec = (-(gx * lx + gy * ly) * 2.0).max(0.0).powi(2).min(1.0);
            let p = &mut row[x * 4..x * 4 + 4];
            for k in 0..3 {
                let v = s[k] * 0.5 + 0.4 - 0.2 * (gx * lx + gy * ly).clamp(-1.0, 1.0) + spec;
                p[k] = p[k] * (1.0 - a) + v * a;
            }
            p[3] = p[3] + (1.0 - p[3]) * a;
        }
    });
}

fn card_dance(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let (w, h) = (img.w, img.h);
    let rows = c.f("rows").round().max(1.0);
    let cols = c.f("columns").round().max(1.0);
    let grad = c.other_fit("gradient1", true, w, h, 1).unwrap_or_else(|| img.clone());
    let back = c.other_fit("backLayer", true, w, h, 1);
    let (zm, xm, ym, rzm, sm) = (c.px(c.f("zSource")), c.f("xRotation").to_radians(), c.f("yRotation").to_radians(), c.f("zRotation").to_radians(), c.f("scale"));
    if zm == 0.0 && xm == 0.0 && ym == 0.0 && rzm == 0.0 && sm == 0.0 {
        return;
    }
    let off = c.f("offset");
    let la = c.f("lightDirection").to_radians();
    let amb = c.f("ambient") as f32 / 100.0;
    let (cw, chh) = ((rect[2] - rect[0]) / cols, (rect[3] - rect[1]) / rows);
    let f = (rect[2] - rect[0]).max(rect[3] - rect[1]) * 2.0;
    let src = img.clone();
    let mut cards = vec![];
    for j in 0..rows as i64 {
        for i in 0..cols as i64 {
            let (cx, cy) = (rect[0] + (i as f64 + 0.5) * cw, rect[1] + (j as f64 + 0.5) * chh);
            let gv = grad.sample(cx, cy, true);
            let v = lum(gv[0], gv[1], gv[2]) as f64 + off;
            let (z, rx, ry, rz, sc) = (v * zm, v * xm, v * ym, v * rzm, 1.0 + v * sm);
            let corners = [[-cw * 0.5, -chh * 0.5], [cw * 0.5, -chh * 0.5], [-cw * 0.5, chh * 0.5], [cw * 0.5, chh * 0.5]];
            let proj: Vec<[f64; 2]> = corners
                .iter()
                .map(|p| {
                    let (x, y, zz) = (p[0] * sc, p[1] * sc, 0.0);
                    let (s, co) = rz.sin_cos();
                    let (x, y) = (x * co - y * s, x * s + y * co);
                    let (s, co) = rx.sin_cos();
                    let (y, zz) = (y * co - zz * s, y * s + zz * co);
                    let (s, co) = ry.sin_cos();
                    let (x, zz) = (x * co + zz * s, -x * s + zz * co);
                    let k = f / (f + zz - z);
                    [cx + x * k, cy + y * k]
                })
                .collect();
            let front = (rx.cos() * ry.cos()) >= 0.0;
            let shade = amb + (1.0 - amb) * ((rx.cos() * ry.cos()).abs() as f32 * (0.7 + 0.3 * la.cos() as f32));
            let srcq = [[cx - cw * 0.5, cy - chh * 0.5], [cx + cw * 0.5, cy - chh * 0.5], [cx - cw * 0.5, cy + chh * 0.5], [cx + cw * 0.5, cy + chh * 0.5]];
            cards.push((z, [proj[0], proj[1], proj[2], proj[3]], srcq, front, shade));
        }
    }
    cards.sort_by(|a, b| b.0.total_cmp(&a.0));
    let mut out = Img::new(w, h);
    for (_, dst, srcq, front, shade) in &cards {
        let Some(hm) = homography(*dst, *srcq) else { continue };
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for p in dst {
            x0 = x0.min(p[0]);
            y0 = y0.min(p[1]);
            x1 = x1.max(p[0]);
            y1 = y1.max(p[1]);
        }
        let (xa, xb) = (x0.floor().max(0.0) as usize, (x1.ceil() as isize).min(w as isize - 1));
        let (ya, yb) = (y0.floor().max(0.0) as usize, (y1.ceil() as isize).min(h as isize - 1));
        if xb < 0 || yb < 0 {
            continue;
        }
        for y in ya..=yb as usize {
            for x in xa..=xb as usize {
                let wv = hm.0[2][0] * (x as f64 + 0.5) + hm.0[2][1] * (y as f64 + 0.5) + hm.0[2][2];
                if wv <= 0.0 {
                    continue;
                }
                let s = hm.apply(x as f64 + 0.5, y as f64 + 0.5);
                if s[0] < srcq[0][0] || s[0] > srcq[1][0] || s[1] < srcq[0][1] || s[1] > srcq[2][1] {
                    continue;
                }
                let col = if *front { src.sample(s[0], s[1], true) } else { back.as_ref().map(|b| b.sample(s[0], s[1], true)).unwrap_or([0.2, 0.2, 0.2, 1.0]) };
                let i = (y * w + x) * 4;
                let k = 1.0 - col[3];
                for ch in 0..3 {
                    out.px[i + ch] = col[ch] * shade + out.px[i + ch] * k;
                }
                out.px[i + 3] = col[3] + out.px[i + 3] * k;
            }
        }
    }
    *img = out;
}

fn caustics(c: &FxCtx, img: &mut Img) {
    let (w, h) = (img.w, img.h);
    let bottom = c.other_fit("bottom", true, w, h, 1).unwrap_or_else(|| img.clone());
    let surf = c.other_fit("waterSurface", true, w, h, 1);
    let Some(surf) = surf else { return };
    let mut hf: Vec<f32> = surf.px.chunks_exact(4).map(|p| lum(p[0], p[1], p[2])).collect();
    let sm = c.f("smoothing") * c.r * 0.5;
    if sm > 0.3 {
        super::util::blur_scalar(&mut hf, w, h, sm, sm);
    }
    let wh = c.f("waveHeight") as f32 * 10.0;
    let depth = c.f("depth") as f32 * 200.0 * c.r as f32;
    let ior = c.f("ior") as f32;
    let k = depth * (1.0 - 1.0 / ior.max(1.0)) * wh;
    let sc = c.c("surfaceColor");
    let so = c.f("surfaceOpacity") as f32 / 100.0;
    let strength = c.f("strength") as f32 / 100.0;
    let la = c.f("lightDirection").to_radians();
    let (lx, ly) = (la.cos() as f32, la.sin() as f32);
    let disp: Vec<[f32; 2]> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let g = |xx: usize, yy: usize| hf[yy.min(h - 1) * w + xx.min(w - 1)];
            [(g(x + 1, y) - g(x.saturating_sub(1), y)) * k, (g(x, y + 1) - g(x, y.saturating_sub(1))) * k]
        })
        .collect();
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let i = y * w + x;
            let d = disp[i];
            let s = bottom.sample(x as f64 + 0.5 + d[0] as f64, y as f64 + 0.5 + d[1] as f64, true);
            let dxx = disp[(i + 1).min(w * h - 1)][0] - disp[i.saturating_sub(1)][0];
            let dyy = disp[(i + w).min(w * h - 1)][1] - disp[i.saturating_sub(w)][1];
            let caustic = (1.0 / (1.0 + dxx + dyy).abs().max(0.2) - 1.0).max(0.0) * strength;
            let g = |xx: usize, yy: usize| hf[yy.min(h - 1) * w + xx.min(w - 1)];
            let spec = (-((g(x + 1, y) - g(x.saturating_sub(1), y)) * lx + (g(x, y + 1) - g(x, y.saturating_sub(1))) * ly) * wh).max(0.0).powi(2);
            let p = &mut row[x * 4..x * 4 + 4];
            for ch in 0..3 {
                let v = s[ch] * (1.0 + caustic) * (1.0 - so) + sc[ch] * so * s[3] + spec * 0.5;
                p[ch] = v.max(0.0);
            }
            p[3] = s[3].max(so);
        }
    });
}

fn foam(c: &FxCtx, img: &mut Img) {
    let rate = c.f("rate");
    let life = c.f("lifespan").max(0.01);
    let t = c.t;
    if rate <= 0.0 || t < 0.0 {
        return;
    }
    let p0 = c.pt("producer");
    let px = c.px(c.f("producerX"));
    let size = c.px(c.f("size")).max(0.5);
    let sv = c.f("sizeVariance") / 100.0;
    let growth = c.f("growth");
    let sp = c.px(c.f("speed"));
    let wind = c.px(c.f("wind"));
    let wob = c.px(c.f("wobble"));
    let col = c.c("color");
    let op = c.f("opacity") as f32 / 100.0;
    let seed = c.f("seed") as u32;
    let first = ((t - life) * rate).floor().max(0.0) as i64;
    let last = (t * rate).floor() as i64;
    let mut parts = vec![];
    for i in first..=last {
        let age = t - i as f64 / rate;
        if !(0.0..=life).contains(&age) {
            continue;
        }
        let x = p0[0] + (rnd(i, 1, seed) - 0.5) * 2.0 * px + wind * age + (age * 3.0 + rnd(i, 2, seed) * TAU).sin() * wob;
        let y = p0[1] - sp * age * (0.7 + 0.6 * rnd(i, 3, seed));
        let s = size * (1.0 + (rnd(i, 4, seed) - 0.5) * 2.0 * sv) * (1.0 + growth * age);
        let fade = 1.0 - (age / life) as f32;
        parts.push((x, y, s.max(0.5),[col[0], col[1], col[2], op * fade], 4u32));
    }
    draw_particles(img, &parts, 0);
}

fn wave_world(c: &FxCtx, img: &mut Img) {
    let rect = c.layer_rect();
    let (lw, lh) = ((rect[2] - rect[0]).max(1.0), (rect[3] - rect[1]).max(1.0));
    let speed = c.f("waveSpeed") * lw;
    let damp = c.f("damping");
    let line = c.e("type") == 1;
    let pos = c.pt("position");
    let len = c.f("length") * lw;
    let amp = c.f("amplitude");
    let freq = c.f("frequency").max(0.0);
    let ph = c.f("phase").to_radians();
    let br = c.f("brightness") as f32;
    let con = c.f("contrast") as f32;
    let wire = c.e("view") == 1;
    let t = c.t;
    let w = img.w;
    img.px.par_chunks_mut(w * 4).enumerate().for_each(|(y, row)| {
        for x in 0..w {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let r = if line { super::draw::seg_dist([px, py], [pos[0] - len * 0.5, pos[1]], [pos[0] + len * 0.5, pos[1]]) } else { ((px - pos[0]).hypot(py - pos[1]) - len * 0.5).max(0.0) };
            let tt = t - r / speed.max(1e-6);
            let v = if tt < 0.0 { 0.0 } else { amp * (TAU * freq * tt + ph).sin() * (-damp * r / lw * 5.0).exp() };
            let g = (br + v as f32 * con).clamp(0.0, 1.0);
            let p = &mut row[x * 4..x * 4 + 4];
            if wire {
                let gx = ((px - rect[0]) / (lw / 40.0)).rem_euclid(1.0);
                let gy = ((py - rect[1] + v * lh * 0.05) / (lh / 30.0)).rem_euclid(1.0);
                let on = gx < 0.05 || gy < 0.05;
                let k = if on { 1.0 } else { 0.0 };
                p.copy_from_slice(&[k, k, k, 1.0]);
            } else {
                p.copy_from_slice(&[g, g, g, 1.0]);
            }
        }
    });
}
