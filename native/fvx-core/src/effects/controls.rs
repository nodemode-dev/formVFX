use super::{EffectSpec, FxCtx, angle, check, color, layer_param, no_pad, p, point, popup};
use crate::model::{Ui, Value};
use crate::render::image::Img;

const CAT: &str = "Expression Controls";

fn pass(_: &FxCtx, _: &mut Img) {}

fn spec(id: &'static str, name: &'static str, params: Vec<super::ParamSpec>) -> EffectSpec {
    EffectSpec { id, name, category: CAT, params, render: pass, pad: no_pad }
}

pub fn is_control(id: &str) -> bool {
    id.starts_with("fx.control.")
}

pub fn specs() -> Vec<EffectSpec> {
    vec![
        spec("fx.control.slider", "Slider Control", vec![p("slider", "Slider", Value::Num(0.0), Ui::slider(-1_000_000.0, 1_000_000.0, 0.0, 100.0, 2))]),
        spec("fx.control.point", "Point Control", vec![point("point", "Point", 0.5, 0.5)]),
        spec("fx.control.point3d", "3D Point Control", vec![p("point", "3D Point", Value::Vec3([0.0; 3]), Ui::Point3)]),
        spec("fx.control.angle", "Angle Control", vec![angle("angle", "Angle", 0.0)]),
        spec("fx.control.checkbox", "Checkbox Control", vec![check("checkbox", "Checkbox", false)]),
        spec("fx.control.color", "Color Control", vec![color("color", "Color", [1.0, 0.0, 0.0])]),
        spec("fx.control.layer", "Layer Control", vec![layer_param("layer", "Layer")]),
        spec("fx.control.dropdown", "Dropdown Menu Control", vec![popup("menu", "Menu", 0, &["Item 1", "Item 2", "Item 3"])]),
    ]
}
