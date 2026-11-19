//! spikes #142 (spike only): the nodes at each TwoNodesOneAnchor anchor of a graph file: kind, marker reach and vertex, whether that reach's up node is the plain node there.
use planet_generators::bake::river_graph::{validate, Checks, Graph, NodeKind, Violation};
use std::collections::{BTreeMap, HashMap};

fn main() -> Result<(), String> {
    let path = std::env::args().nth(1).ok_or("graph_diag <graph.txt>")?;
    let g: Graph = planet_cache::river_gate_graph::read_graph(&path)?;
    let v = validate(&g, &Checks { adjacent: &planet_cache::river_gate_graph::adjacent, tile_of: None, open: true });
    let reach: HashMap<u64, _> = g.reaches.iter().map(|r| (r.id, r)).collect();
    let mut at: HashMap<(u8, u64), Vec<_>> = HashMap::new();
    for n in &g.nodes {
        at.entry((n.level as u8, n.anchor)).or_default().push(n);
    }
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    let mut shown = 0;
    for x in &v {
        if let Violation::TwoNodesOneAnchor { level, anchor } = x {
            let ns = &at[&(*level as u8, *anchor)];
            let mut ks: Vec<String> = ns
                .iter()
                .map(|n| match n.marker {
                    None => format!("{:?}", n.kind),
                    Some(m) => {
                        let r = reach[&m.on_reach];
                        let upn = ns.iter().find(|p| p.id == r.up).map(|p| format!("{:?}", p.kind)).unwrap_or("other".into());
                        format!(
                            "{:?}@v{}{}(up={upn})",
                            n.kind,
                            if m.at_vertex == 0 { 0 } else { 1 },
                            if m.at_vertex as usize + 1 == r.vertices.len() { "last" } else { "" }
                        )
                    }
                })
                .collect();
            ks.sort();
            let key = ks.join(" + ");
            if shown < 12 {
                println!("anchor {anchor}: {key}");
                for n in ns {
                    if let Some(m) = n.marker {
                        let r = reach[&m.on_reach];
                        println!(
                            "   {:?} on reach {} at v{} of {} (up {} down {})",
                            n.kind,
                            m.on_reach,
                            m.at_vertex,
                            r.vertices.len(),
                            r.up,
                            r.down
                        );
                    } else {
                        println!("   {:?} id {}", n.kind, n.id);
                    }
                }
                shown += 1;
            }
            *by.entry(key).or_default() += 1;
        }
    }
    for (k, c) in &by {
        println!("{c:>6} {k}");
    }
    // conservation breaks: by field, at a reach (its level, trunk image or tributary, its up node's kind) or at a node (its kind)
    let kind_of: HashMap<u64, NodeKind> = g.nodes.iter().map(|n| (n.id, n.kind)).collect();
    let mut cb: BTreeMap<String, usize> = BTreeMap::new();
    let mut shown_cb = 0;
    for x in &v {
        if let Violation::ConservationBreak { at, field } = x {
            let key = if let Some(r) = reach.get(at) {
                if shown_cb < 6 && *field == "q_w" {
                    let qw = &r.attrs.q_w;
                    let i = qw.windows(2).position(|w| w[1] < w[0] * (1.0 - 1e-9)).unwrap_or(0);
                    println!(
                        "reach {at} {:?} up {:?} down {:?} n {}: q_w falls at {i}: {:.4} -> {:.4}; q there {:.4} -> {:.4}",
                        r.level,
                        kind_of.get(&r.up),
                        kind_of.get(&r.down),
                        qw.len(),
                        qw[i],
                        qw[i + 1],
                        r.attrs.q[i],
                        r.attrs.q[i + 1]
                    );
                    shown_cb += 1;
                }
                format!(
                    "{field} along a {:?} reach ({}), up {:?}",
                    r.level,
                    if r.parent.is_some() {
                        "trunk image"
                    } else if r.level == planet_generators::bake::river_graph::Level::H1 {
                        "H1"
                    } else {
                        "tributary"
                    },
                    kind_of.get(&r.up)
                )
            } else {
                format!("{field} at a node {:?}", kind_of.get(at))
            };
            *cb.entry(key).or_default() += 1;
        }
    }
    for (k, c) in &cb {
        println!("{c:>6} {k}");
    }
    let _ = NodeKind::Exception;
    Ok(())
}
