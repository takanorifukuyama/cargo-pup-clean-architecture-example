use layer_contracts::{layer_rules, Edge, Finding, Graph, Layer, Policy};

layer_rules! {
    POLICY {
        acyclic: true,
        root("app") => [inner, outer],
        inner("app::inner") => [],
        outer("app::outer") => [inner],
    }
}

fn graph() -> Graph {
    Graph {
        crate_name: "app".into(),
        modules: ["app", "app::inner", "app::outer"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ..Graph::default()
    }
}

fn edge(from: &str, to: &str) -> Edge {
    Edge {
        from: from.into(),
        to: to.into(),
        symbol: format!("{to}::Thing"),
        location: "src/lib.rs:3:5".into(),
        kind: "path".into(),
    }
}

#[test]
fn allowed_inward_edge_passes() {
    let mut graph = graph();
    graph.edges.insert(edge("app::outer", "app::inner"));
    assert_eq!(POLICY.check(&graph), Ok(()));
}

#[test]
fn forbidden_edge_keeps_symbol_and_location() {
    let mut graph = graph();
    let edge = edge("app::inner", "app::outer");
    graph.edges.insert(edge.clone());
    assert_eq!(
        POLICY.check(&graph),
        Err(vec![Finding::Forbidden {
            from: "inner".into(),
            to: "outer".into(),
            edge,
        }])
    );
}

#[test]
fn nested_modules_are_owned_by_their_layer() {
    let mut graph = graph();
    graph.modules.insert("app::inner::nested".into());
    graph.edges.insert(edge("app::inner", "app::inner::nested"));
    graph.edges.insert(edge("app::inner::nested", "app::inner"));
    assert_eq!(POLICY.check(&graph), Ok(()));
}

#[test]
fn root_is_not_a_catch_all_and_prefixes_are_segment_bounded() {
    let mut graph = graph();
    graph.modules.insert("app::inner_extra".into());
    assert_eq!(
        POLICY.check(&graph),
        Err(vec![Finding::Unclassified("app::inner_extra".into())])
    );
}

#[test]
fn absent_layer_is_not_a_vacuous_success() {
    let mut graph = graph();
    graph.modules.remove("app::inner");
    assert_eq!(
        POLICY.check(&graph),
        Err(vec![Finding::MissingLayer("inner".into())])
    );
}

#[test]
fn unknown_allowance_fails_even_without_edges() {
    layer_rules! {
        BAD {
            acyclic: true,
            root("app") => [typo],
        }
    }
    assert!(matches!(BAD.check(&graph()), Err(v) if matches!(v[0], Finding::Invalid(_))));
}

#[test]
fn duplicate_names_and_overlapping_prefixes_are_rejected() {
    for layers in [
        &[
            Layer { name: "one", module: "app", allows: &[] },
            Layer { name: "one", module: "app::inner", allows: &[] },
        ][..],
        &[
            Layer { name: "one", module: "app::inner", allows: &[] },
            Layer { name: "two", module: "app::inner::nested", allows: &[] },
        ][..],
    ] {
        let policy = Policy { layers, acyclic: false };
        assert!(matches!(policy.check(&graph()), Err(v) if matches!(v[0], Finding::Invalid(_))));
    }
}

#[test]
fn empty_policy_is_invalid() {
    let policy = Policy { layers: &[], acyclic: true };
    assert!(matches!(policy.check(&graph()), Err(v) if matches!(v[0], Finding::Invalid(_))));
}

#[test]
fn wire_roundtrip_preserves_unicode_and_control_characters() {
    let mut graph = graph();
    let mut edge = edge("app::outer", "app::inner");
    edge.symbol = "app::inner::型".into();
    edge.location = "file\twith\ncontrols\\and\"quotes".into();
    graph.edges.insert(edge);
    assert_eq!(Graph::decode(&graph.encode()), Ok(graph));
}

#[test]
fn incomplete_unknown_and_invalid_wire_formats_fail_closed() {
    for text in [
        "",
        "layer-graph-v2\nEND\n",
        "layer-graph-v1\nC\t617070\nM\t617070\n",
        "layer-graph-v1\nC\t617070\nM\t617070\nEND\nM\t617070\n",
        "layer-graph-v1\nC\tzx\nEND\n",
        "layer-graph-v1\nC\tff\nEND\n",
        "layer-graph-v1\nC\t6\nEND\n",
        "layer-graph-v1\nC\t617070\nM\t617070\nM\t617070\nEND\n",
    ] {
        assert!(Graph::decode(text).is_err(), "Accepted {text:?}");
    }
}

#[test]
fn dangling_edges_and_wrong_crate_modules_are_invalid() {
    let mut missing = graph();
    missing.edges.insert(edge("app::inner", "app::missing"));
    assert!(missing.validate().is_err());
    let mut wrong = graph();
    wrong.modules.insert("another::inner".into());
    assert!(wrong.validate().is_err());
}

#[test]
fn graph_exports_are_deterministic() {
    let mut graph = graph();
    graph.edges.insert(edge("app::outer", "app::inner"));
    graph.edges.insert(edge("app::outer", "app::inner"));
    assert_eq!(graph.edges.len(), 1);
    let reconstructed = Graph::decode(&graph.encode()).unwrap();
    assert_eq!(graph.dot(), reconstructed.dot());
    assert_eq!(POLICY.report(&graph), POLICY.report(&reconstructed));
    assert!(graph.dot().contains("\"app::outer\" -> \"app::inner\""));
}
