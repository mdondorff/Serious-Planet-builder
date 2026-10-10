//! A small render graph (report §8): passes declare the resources they read and write; compiling checks that every read
//! has a producer, drops passes nobody needs, and computes transient resource lifetimes so memory can be aliased.
//! GPU-free declaration and analysis; executors turn the compiled order into wgpu passes.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PassDesc {
    pub name: &'static str,
    pub reads: Vec<&'static str>,
    pub writes: Vec<&'static str>,
    /// Passes with side effects (presenting, readback) are never culled.
    pub side_effect: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum GraphError {
    MissingProducer { pass: &'static str, resource: &'static str },
    DuplicatePass(&'static str),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::MissingProducer { pass, resource } => {
                write!(f, "pass '{pass}' reads '{resource}', which no earlier pass writes and which is not an external resource")
            }
            GraphError::DuplicatePass(p) => write!(f, "pass '{p}' is declared twice"),
        }
    }
}

impl std::error::Error for GraphError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lifetime {
    pub resource: &'static str,
    /// Positions in the compiled order of the first write and the last read.
    pub first: usize,
    pub last: usize,
    /// Resources are only aliased with others of the same description (format, size class).
    pub class: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompiledGraph {
    /// Indices into the declared passes, in execution order.
    pub order: Vec<usize>,
    pub lifetimes: Vec<Lifetime>,
    /// Groups of transient resources that never live at the same time and can share memory.
    pub alias_groups: Vec<Vec<&'static str>>,
    pub culled: Vec<&'static str>,
}

#[derive(Default)]
pub struct RenderGraph {
    passes: Vec<PassDesc>,
    external: BTreeSet<&'static str>,
    classes: BTreeMap<&'static str, &'static str>,
}

impl RenderGraph {
    /// `external` resources exist before the graph runs (the swapchain image, persistent buffers).
    pub fn new(external: &[&'static str]) -> Self {
        Self { external: external.iter().copied().collect(), ..Default::default() }
    }

    /// Declare the memory class of a transient resource (for example `rgba8-1x`, `depth32-1x`).
    pub fn describe(&mut self, resource: &'static str, class: &'static str) -> &mut Self {
        self.classes.insert(resource, class);
        self
    }

    pub fn add_pass(&mut self, name: &'static str, reads: &[&'static str], writes: &[&'static str], side_effect: bool) -> &mut Self {
        self.passes.push(PassDesc { name, reads: reads.to_vec(), writes: writes.to_vec(), side_effect });
        self
    }

    pub fn compile(&self) -> Result<CompiledGraph, GraphError> {
        let mut names = BTreeSet::new();
        for p in &self.passes {
            if !names.insert(p.name) {
                return Err(GraphError::DuplicatePass(p.name));
            }
        }
        // Producers: the latest earlier pass writing each resource a pass reads.
        let mut producer: Vec<Vec<usize>> = vec![Vec::new(); self.passes.len()];
        for (i, p) in self.passes.iter().enumerate() {
            for r in &p.reads {
                match (0..i).rev().find(|&j| self.passes[j].writes.contains(r)) {
                    Some(j) => producer[i].push(j),
                    None if self.external.contains(r) => {}
                    None => return Err(GraphError::MissingProducer { pass: p.name, resource: r }),
                }
            }
        }
        // Keep passes with side effects and everything they depend on.
        let mut keep = vec![false; self.passes.len()];
        let mut stack: Vec<usize> = (0..self.passes.len()).filter(|&i| self.passes[i].side_effect).collect();
        while let Some(i) = stack.pop() {
            if !std::mem::replace(&mut keep[i], true) {
                stack.extend(producer[i].iter().copied());
            }
        }
        let order: Vec<usize> = (0..self.passes.len()).filter(|&i| keep[i]).collect();
        let culled = (0..self.passes.len()).filter(|&i| !keep[i]).map(|i| self.passes[i].name).collect();
        // Lifetimes of transient resources over the compiled order.
        let mut first: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut last: BTreeMap<&'static str, usize> = BTreeMap::new();
        for (pos, &i) in order.iter().enumerate() {
            for r in &self.passes[i].writes {
                first.entry(r).or_insert(pos);
                last.entry(r).and_modify(|l| *l = (*l).max(pos)).or_insert(pos);
            }
            for r in &self.passes[i].reads {
                last.entry(r).and_modify(|l| *l = (*l).max(pos)).or_insert(pos);
            }
        }
        let lifetimes: Vec<Lifetime> = first
            .iter()
            .filter(|(r, _)| !self.external.contains(*r))
            .map(|(r, &f)| Lifetime { resource: r, first: f, last: last[r], class: self.classes.get(r).copied().unwrap_or("unclassified") })
            .collect();
        // Greedy aliasing: a resource joins the first group of its class whose members all ended before it starts.
        let mut groups: Vec<(&'static str, Vec<&Lifetime>)> = Vec::new();
        let mut sorted: Vec<&Lifetime> = lifetimes.iter().collect();
        sorted.sort_by_key(|l| (l.first, l.resource));
        for l in sorted {
            if l.class == "unclassified" {
                groups.push((l.class, vec![l]));
                continue;
            }
            match groups.iter_mut().find(|(c, members)| *c == l.class && members.iter().all(|m| m.last < l.first)) {
                Some((_, members)) => members.push(l),
                None => groups.push((l.class, vec![l])),
            }
        }
        let alias_groups = groups.into_iter().map(|(_, m)| m.into_iter().map(|l| l.resource).collect()).collect();
        Ok(CompiledGraph { order, lifetimes, alias_groups, culled })
    }
}
