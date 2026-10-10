use crate::eval::Eval;
use crate::model::{Layer, Project, Prop};

pub const GROUP: &str = "essentialProperties";

pub fn overridden(pr: &Prop) -> bool {
    !pr.keys.is_empty() || pr.expr.as_ref().is_some_and(|x| x.enabled && !x.text.trim().is_empty()) || pr.value != pr.default
}

pub fn patched(project: &Project, ev: &Eval, l: &Layer, src: u64) -> Option<Project> {
    let g = l.props.sub(GROUP)?;
    let sub = project.comp(src)?;
    let changes: Vec<(u64, u64, crate::model::Value)> = sub
        .essential
        .iter()
        .filter(|c| c.kind == "property")
        .filter_map(|c| {
            let ip = g.get(&c.id).filter(|ip| overridden(ip))?;
            Some((c.layer, c.prop, ev.value(l, ip)))
        })
        .collect();
    let media: Vec<(u64, u64)> = sub
        .essential
        .iter()
        .filter(|c| c.kind == "media")
        .filter_map(|c| {
            let ip = g.get(&c.id).filter(|ip| overridden(ip))?;
            let item = ev.value(l, ip).num().round() as u64;
            project.item(item).is_some().then_some((c.layer, item))
        })
        .collect();
    if changes.is_empty() && media.is_empty() {
        return None;
    }
    let mut p = project.clone();
    let comp = p.comp_mut(src)?;
    for (lid, item) in media {
        if let Some(x) = comp.layer_mut(lid) {
            x.source = Some(item);
        }
    }
    for (lid, uid, v) in changes {
        if let Some(pr) = comp.layer_mut(lid).and_then(|x| x.props.find_mut(uid)) {
            pr.value = v;
            pr.keys.clear();
            pr.expr = None;
        }
    }
    Some(p)
}
