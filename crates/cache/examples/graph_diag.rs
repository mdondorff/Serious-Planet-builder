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
                        format!("{:?}@v{}{}(up={upn})", n.kind, if m.at_vertex == 0 { 0 } else { 1 }, if m.at_vertex as usize + 1 == r.vertices.len() { "last" } else { "" })
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
                        println!("   {:?} on reach {} at v{} of {} (up {} down {})", n.kind, m.on_reach, m.at_vertex, r.vertices.len(), r.up, r.down);
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
    let _ = NodeKind::Exception;
    Ok(())
}
