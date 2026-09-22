//! Stable, dependency-free policy engine. The optional collector uses pinned rustc.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub symbol: String,
    pub location: String,
    pub kind: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Graph {
    pub crate_name: String,
    pub modules: BTreeSet<String>,
    pub edges: BTreeSet<Edge>,
}

fn within(module: &str, prefix: &str, root: &str) -> bool {
    module == prefix || (prefix != root && module.starts_with(&format!("{prefix}::")))
}

fn hex(text: &str) -> String {
    text.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Result<String, String> {
    if text.len() % 2 != 0 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("Invalid graph field encoding".into());
    }
    let bytes = (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

impl Graph {
    pub fn validate(&self) -> Result<(), String> {
        if self.crate_name.is_empty() || !self.modules.contains(&self.crate_name) {
            return Err("Graph must contain its crate root".into());
        }
        for module in &self.modules {
            if module != &self.crate_name
                && !module.starts_with(&format!("{}::", self.crate_name))
            {
                return Err(format!("Module outside collected crate: {module}"));
            }
        }
        for edge in &self.edges {
            if !self.modules.contains(&edge.from) || !self.modules.contains(&edge.to)
                || edge.symbol.is_empty() || edge.location.is_empty() || edge.kind.is_empty()
            {
                return Err(format!("Invalid edge: {edge:?}"));
            }
        }
        Ok(())
    }

    /// Versioned, escaped, deterministic interchange. END rejects partial output.
    pub fn encode(&self) -> String {
        let mut output = format!("layer-graph-v1\nC\t{}\n", hex(&self.crate_name));
        for module in &self.modules {
            output.push_str(&format!("M\t{}\n", hex(module)));
        }
        for edge in &self.edges {
            let fields = [&edge.from, &edge.to, &edge.symbol, &edge.location, &edge.kind];
            output.push_str(&format!("E\t{}\n", fields.iter().map(|s| hex(s)).collect::<Vec<_>>().join("\t")));
        }
        output.push_str("END\n");
        output
    }

    pub fn decode(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some("layer-graph-v1") {
            return Err("Unsupported or missing graph schema".into());
        }
        let mut graph = Self::default();
        let mut has_crate = false;
        let mut ended = false;
        for line in lines {
            if ended {
                return Err("Data after graph terminator".into());
            }
            let parts: Vec<_> = line.split('\t').collect();
            match parts.as_slice() {
                ["C", name] if !has_crate => {
                    graph.crate_name = unhex(name)?;
                    has_crate = true;
                }
                ["M", name] => {
                    if !graph.modules.insert(unhex(name)?) {
                        return Err("Duplicate module in graph".into());
                    }
                }
                ["E", from, to, symbol, location, kind] => {
                    if !graph.edges.insert(Edge {
                        from: unhex(from)?, to: unhex(to)?, symbol: unhex(symbol)?,
                        location: unhex(location)?, kind: unhex(kind)?,
                    }) {
                        return Err("Duplicate edge in graph".into());
                    }
                }
                ["END"] => ended = true,
                _ => return Err(format!("Invalid graph record: {line}")),
            }
        }
        if !has_crate || !ended {
            return Err("Incomplete graph".into());
        }
        graph.validate()?;
        Ok(graph)
    }

    /// Module graph for Graphviz. It includes same-layer module dependencies.
    pub fn dot(&self) -> String {
        fn quote(s: &str) -> String {
            format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
        }
        let mut out = String::from("digraph modules {\n");
        for module in &self.modules {
            out.push_str(&format!("  {};\n", quote(module)));
        }
        let edges: BTreeSet<_> = self.edges.iter().map(|e| (&e.from, &e.to)).collect();
        for (from, to) in edges {
            out.push_str(&format!("  {} -> {};\n", quote(from), quote(to)));
        }
        out.push_str("}\n");
        out
    }
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub name: &'static str,
    pub module: &'static str,
    pub allows: &'static [&'static str],
}

#[derive(Debug, Clone)]
pub struct Policy {
    pub layers: &'static [Layer],
    pub acyclic: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Finding {
    Invalid(String),
    MissingLayer(String),
    Unclassified(String),
    Forbidden { from: String, to: String, edge: Edge },
    Cycle(Vec<String>),
}

impl Policy {
    /// Each module has exactly one owner. Root matches ONLY the crate root.
    pub fn check(&self, graph: &Graph) -> Result<(), Vec<Finding>> {
        if let Err(error) = graph.validate() {
            return Err(vec![Finding::Invalid(error)]);
        }
        let mut names = BTreeSet::new();
        if self.layers.is_empty() {
            return Err(vec![Finding::Invalid("No layers declared".into())]);
        }
        for layer in self.layers {
            if layer.name.is_empty() || layer.module.is_empty() || !names.insert(layer.name) {
                return Err(vec![Finding::Invalid("Empty/duplicate layer declaration".into())]);
            }
        }
        for (i, layer) in self.layers.iter().enumerate() {
            for other in &self.layers[i + 1..] {
                if within(layer.module, other.module, &graph.crate_name)
                    || within(other.module, layer.module, &graph.crate_name)
                {
                    return Err(vec![Finding::Invalid(format!("Overlapping layer selectors: {} / {}", layer.name, other.name))]);
                }
            }
            let mut seen = BTreeSet::new();
            for allowed in layer.allows {
                if !names.contains(allowed) || !seen.insert(*allowed) {
                    return Err(vec![Finding::Invalid(format!("Unknown/duplicate allowed layer: {allowed}"))]);
                }
            }
        }
        let mut findings = Vec::new();
        let mut owners = BTreeMap::new();
        for module in &graph.modules {
            match self.layers.iter().position(|l| within(module, l.module, &graph.crate_name)) {
                Some(index) => { owners.insert(module.as_str(), index); }
                None => findings.push(Finding::Unclassified(module.clone())),
            }
        }
        for (i, layer) in self.layers.iter().enumerate() {
            if !owners.values().any(|owner| *owner == i) {
                findings.push(Finding::MissingLayer(layer.name.into()));
            }
        }
        let mut adjacency = vec![BTreeSet::new(); self.layers.len()];
        for edge in &graph.edges {
            let (Some(&from), Some(&to)) = (owners.get(edge.from.as_str()), owners.get(edge.to.as_str())) else {
                continue;
            };
            if from == to { continue; }
            adjacency[from].insert(to);
            if !self.layers[from].allows.contains(&self.layers[to].name) {
                findings.push(Finding::Forbidden {
                    from: self.layers[from].name.into(), to: self.layers[to].name.into(), edge: edge.clone(),
                });
            }
        }
        if self.acyclic {
            if let Some(cycle) = first_cycle(&adjacency) {
                findings.push(Finding::Cycle(cycle.into_iter().map(|i| self.layers[i].name.into()).collect()));
            }
        }
        if findings.is_empty() { Ok(()) } else { Err(findings) }
    }

    pub fn report(&self, graph: &Graph) -> String {
        let mut report = format!("# Resolved layer contracts\n\nModules: {}. References: {}.\n\n", graph.modules.len(), graph.edges.len());
        match self.check(graph) {
            Ok(()) => report.push_str("PASS\n"),
            Err(findings) => {
                report.push_str("FAIL\n\n");
                for finding in findings {
                    // Fenced diagnostics avoid interpreting source paths as Markdown.
                    report.push_str(&format!("    {finding:?}\n\n"));
                }
            }
        }
        report
    }
}

fn first_cycle(adjacency: &[BTreeSet<usize>]) -> Option<Vec<usize>> {
    fn visit(node: usize, adjacency: &[BTreeSet<usize>], state: &mut [u8], stack: &mut Vec<usize>) -> Option<Vec<usize>> {
        state[node] = 1;
        stack.push(node);
        for &next in &adjacency[node] {
            if state[next] == 1 {
                let start = stack.iter().position(|n| *n == next)?;
                let mut cycle = stack[start..].to_vec();
                cycle.push(next);
                return Some(cycle);
            }
            if state[next] == 0 {
                if let Some(cycle) = visit(next, adjacency, state, stack) { return Some(cycle); }
            }
        }
        stack.pop();
        state[node] = 2;
        None
    }
    let mut state = vec![0; adjacency.len()];
    for node in 0..adjacency.len() {
        if state[node] == 0 {
            if let Some(cycle) = visit(node, adjacency, &mut state, &mut Vec::new()) { return Some(cycle); }
        }
    }
    None
}

/// Declare layer ownership and allowed directions. Unknown names fail validation.
#[macro_export]
macro_rules! layer_rules {
    ($vis:vis $name:ident {
        acyclic: $acyclic:literal,
        $($layer:ident ($module:literal) => [$($allowed:ident),* $(,)?]),+ $(,)?
    }) => {
        $vis const $name: $crate::Policy = $crate::Policy {
            acyclic: $acyclic,
            layers: &[$($crate::Layer {
                name: stringify!($layer), module: $module,
                allows: &[$(stringify!($allowed)),*],
            }),+],
        };
    };
}
