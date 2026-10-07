use serde_json::json;

use super::Engine;
use crate::model::Project;

pub fn build() -> Project {
    let mut e = Engine::new();
    let steps = vec![
        json!({"command": "comp.new", "params": {"name": "formvfx Intro", "width": 1920, "height": 1080, "frameRate": 30, "duration": 6, "background": "#0b0b12"}}),
        json!({"command": "layer.newSolid", "params": {"name": "Backdrop", "color": "#141428"}}),
        json!({"command": "effect.apply", "params": {"effect": "Gradient Ramp", "layers": ["$2.layer"]}}),
        json!({"command": "prop.set", "params": {"layer": "$2.layer", "path": "effects/#1/startColor", "value": "#1b1640"}}),
        json!({"command": "prop.set", "params": {"layer": "$2.layer", "path": "effects/#1/endColor", "value": "#05050a"}}),
        json!({"command": "prop.set", "params": {"layer": "$2.layer", "path": "effects/#1/shape", "value": 1}}),
        json!({"command": "prop.set", "params": {"layer": "$2.layer", "path": "effects/#1/start", "value": [960, 540]}}),
        json!({"command": "prop.set", "params": {"layer": "$2.layer", "path": "effects/#1/end", "value": [960, 1400]}}),
        json!({"command": "layer.newShape", "params": {"kind": "ellipse", "name": "Orb", "size": [360, 360], "position": [960, 500], "fill": "#f97316"}}),
        json!({"command": "prop.addKey", "params": {"layer": "$9.layer", "path": "transform/scale", "time": 0, "value": [0, 0]}}),
        json!({"command": "prop.addKey", "params": {"layer": "$9.layer", "path": "transform/scale", "time": 1.0, "value": [100, 100]}}),
        json!({"command": "effect.apply", "params": {"effect": "Glow", "layers": ["$9.layer"]}}),
        json!({"command": "prop.set", "params": {"layer": "$9.layer", "path": "effects/#1/radius", "value": 60}}),
        json!({"command": "prop.set", "params": {"layer": "$9.layer", "path": "effects/#1/threshold", "value": 40}}),
        json!({"command": "layer.newShape", "params": {"kind": "ellipse", "name": "Ring", "size": [520, 520], "position": [960, 500], "fill": null, "stroke": "#ffffff", "strokeWidth": 6}}),
        json!({"command": "shape.add", "params": {"layer": "$15.layer", "kind": "trim"}}),
        json!({"command": "prop.addKey", "params": {"layer": "$15.layer", "path": "contents/#2/end", "time": 0.3, "value": 0}}),
        json!({"command": "prop.addKey", "params": {"layer": "$15.layer", "path": "contents/#2/end", "time": 1.6, "value": 100}}),
        json!({"command": "prop.addKey", "params": {"layer": "$15.layer", "path": "transform/rotation", "time": 0, "value": 0}}),
        json!({"command": "prop.addKey", "params": {"layer": "$15.layer", "path": "transform/rotation", "time": 6, "value": 180}}),
        json!({"command": "layer.newText", "params": {"text": "formvfx", "size": 150, "fill": "#ffffff", "position": [960, 840], "justify": "center", "style": "Bold"}}),
        json!({"command": "prop.addKey", "params": {"layer": "$21.layer", "path": "transform/opacity", "time": 0.8, "value": 0}}),
        json!({"command": "prop.addKey", "params": {"layer": "$21.layer", "path": "transform/opacity", "time": 1.6, "value": 100}}),
        json!({"command": "prop.addKey", "params": {"layer": "$21.layer", "path": "transform/position", "time": 0.8, "value": [960, 900]}}),
        json!({"command": "prop.addKey", "params": {"layer": "$21.layer", "path": "transform/position", "time": 1.6, "value": [960, 840]}}),
        json!({"command": "layer.newText", "params": {"text": "motion graphics and visual effects", "size": 42, "fill": "#b9b9c8", "position": [960, 920], "justify": "center"}}),
        json!({"command": "prop.addKey", "params": {"layer": "$26.layer", "path": "transform/opacity", "time": 1.4, "value": 0}}),
        json!({"command": "prop.addKey", "params": {"layer": "$26.layer", "path": "transform/opacity", "time": 2.2, "value": 100}}),
    ];
    let _ = e.batch(&json!({"steps": steps, "label": "Demo", "atomic": false}));
    let ids: Vec<u64> = e.project.comp(e.state.active_comp.unwrap_or(0)).map(|c| c.layers.iter().map(|l| l.id).collect()).unwrap_or_default();
    for lid in ids {
        let Some(cid) = e.state.active_comp else { break };
        let keys: Vec<serde_json::Value> = e
            .project
            .comp(cid)
            .and_then(|c| c.layer(lid))
            .map(|l| {
                let mut v = vec![];
                l.props.for_each_prop(&mut |p| {
                    for k in &p.keys {
                        v.push(json!({"layer": lid, "prop": p.uid, "time": k.time}));
                    }
                });
                v
            })
            .unwrap_or_default();
        if keys.is_empty() {
            continue;
        }
        let _ = e.execute("keys.select", json!({"keys": keys}));
        let _ = e.execute("keys.easyEase", json!({}));
    }
    let _ = e.execute("keys.select", json!({"keys": []}));
    (*e.project).clone()
}
