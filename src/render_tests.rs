use crate::{spec, svg, timeline};
use resvg::usvg;
fn texts(g: &usvg::Group, result: &mut Vec<(String, [f32; 4])>) {
    for n in g.children() {
        match n {
            usvg::Node::Group(g) => texts(g, result),
            usvg::Node::Text(t) => {
                let b = t.abs_bounding_box();
                result.push((
                    t.chunks().iter().map(|c| c.text()).collect(),
                    [b.left(), b.top(), b.right(), b.bottom()],
                ));
            }
            _ => {}
        }
    }
}
fn assert_text_geometry(svg: &str) {
    let mut db = usvg::fontdb::Database::new();
    db.load_font_data(include_bytes!("assets/fonts/Inconsolata.ttf").to_vec());
    db.load_font_data(include_bytes!("assets/fonts/LiberationSans.ttf").to_vec());
    let tree = usvg::Tree::from_str(
        svg,
        &usvg::Options {
            fontdb: std::sync::Arc::new(db),
            ..Default::default()
        },
    )
    .unwrap();
    let mut all = vec![];
    texts(tree.root(), &mut all);
    for (s, b) in &all {
        assert!(
            b[0] >= 0.
                && b[1] >= 0.
                && b[2] <= tree.size().width() + 0.01
                && b[3] <= tree.size().height() + 0.01,
            "text outside viewport: {s}: {b:?}"
        );
    }
    for i in 0..all.len() {
        for j in i + 1..all.len() {
            let (a, b) = (all[i].1, all[j].1);
            assert!(
                !(a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]),
                "overlapping rendered text: {:?} / {:?}",
                all[i],
                all[j]
            );
        }
    }
}
fn fixture(json: &str) -> timeline::Doc {
    if json.contains("\"els\"") {
        timeline::parse_doc(json).unwrap()
    } else {
        spec::parse(json).unwrap()
    }
}
#[test]
fn retained_ci_texts_do_not_overlap() {
    let d = fixture(include_str!(
        "../outputs/layout-performance-audit/ci-pipeline.json"
    ));
    assert_text_geometry(&svg::render_svg(&d.frame_at(0)));
}
#[test]
fn retained_mechanism_texts_fit_viewport() {
    let d = fixture(include_str!(
        "../outputs/layout-performance-audit/mechanism.json"
    ));
    assert_text_geometry(&svg::render_svg(&d.frame_at(0)));
}
#[test]
fn scatter_gather_texts_fit() {
    let d = fixture(include_str!(
        "../outputs/unified-layout/scatter-gather.json"
    ));
    assert_text_geometry(&svg::render_svg(&d.frame_at(0)));
}
#[test]
fn unspaced_caption_fits_viewport() {
    let d = fixture(&serde_json::json!({"note":"W".repeat(200)}).to_string());
    assert_text_geometry(&svg::render_svg(&d.frame_at(0)));
}
#[test]
fn prepared_geometry_is_stable_across_time_and_visibility() {
    let d = fixture(
        r#"{"duration":1000,"els":[{"type":"node","id":"hidden","x":15,"y":20,"label":"hidden","show":[[0,false],[600,true]]},{"type":"node","id":"main","x":50,"y":40,"icon":"dns","label":"server","status":[[0,"ok"],[500,"a much longer status message"]]}]}"#,
    );
    let r = svg::Renderer::new(&d).unwrap();
    let at = r.render(700);
    assert_eq!(r.render(100), r.render(1100));
    assert_eq!(at, r.render(700));
    fn label_bounds(s: &str) -> [f32; 4] {
        let tree = usvg::Tree::from_str(
            s,
            &usvg::Options {
                fontdb: crate::typography::fontdb(),
                ..Default::default()
            },
        )
        .unwrap();
        let mut all = vec![];
        texts(tree.root(), &mut all);
        all.into_iter().find(|(s, _)| s == "server").unwrap().1
    }
    assert_eq!(label_bounds(&r.render(0)), label_bounds(&r.render(700)));
}
#[test]
fn scale_doubles_display_without_changing_world_layout() {
    let mut d = fixture(include_str!(
        "../outputs/unified-layout/scatter-gather.json"
    ));
    let a = svg::Renderer::new(&d).unwrap();
    d.canvas.scale = 2.;
    let b = svg::Renderer::new(&d).unwrap();
    assert_eq!(b.size(), (a.size().0 * 2., a.size().1 * 2.));
    let parse = |s: &str| {
        usvg::Tree::from_str(
            s,
            &usvg::Options {
                fontdb: crate::typography::fontdb(),
                ..Default::default()
            },
        )
        .unwrap()
    };
    let (sa, sb) = (a.render(0), b.render(0));
    let (ta, tb) = (parse(&sa), parse(&sb));
    assert_eq!(tb.size().width(), ta.size().width() * 2.);
    assert_eq!(tb.size().height(), ta.size().height() * 2.);
    assert_eq!(sa.split("viewBox=").nth(1), sb.split("viewBox=").nth(1));
}
#[test]
fn requested_height_provides_more_content_room() {
    let mut d = fixture(include_str!(
        "../outputs/unified-layout/scatter-gather.json"
    ));
    let a = svg::Renderer::new(&d).unwrap();
    d.canvas.min_height = a.size().1 * 2.;
    let b = svg::Renderer::new(&d).unwrap();
    assert!(b.size().1 >= a.size().1 * 2. - 0.01);
    assert_text_geometry(&b.render(0));
}
#[test]
fn canvas_and_missing_coordinates_are_validated() {
    for json in [
        r#"{"canvas":{"scale":0}}"#,
        r#"{"canvas":{"width":-1}}"#,
        r#"{"nodes":[{"label":"missing"}]}"#,
    ] {
        assert!(spec::parse(json).is_err(), "{json}");
    }
}
#[test]
fn coordinate_free_flow_and_grid_fit_their_nodes() {
    for mode in ["flow", "grid", "columns"] {
        let d=spec::parse(&serde_json::json!({"layout":mode,"nodes":(0..8).map(|i|serde_json::json!({"id":format!("n{i}"),"icon":"dns","label":format!("service {i}"),"status":"ready"})).collect::<Vec<_>>(),"note":"automatic placement"}).to_string()).unwrap();
        assert_text_geometry(&svg::Renderer::new(&d).unwrap().render(0));
    }
}

#[test]
fn heterogeneous_structural_nodes_fit_real_columns() {
    for mode in ["flow", "grid", "columns"] {
        for width in [100, 380, 760] {
            let doc = spec::parse(&serde_json::json!({
                "canvas": {"width": width}, "layout": {"mode": mode, "columns": 3},
                "nodes": (0..9).map(|i| serde_json::json!({
                    "label": if i % 2 == 0 {"W".repeat(70)} else {format!("service {i}")},
                    "icon": if i % 3 == 0 {Some("dns")} else {None},
                    "status": if i % 2 == 0 {Some("waiting for downstream response")} else {None},
                })).collect::<Vec<_>>()
            }).to_string());
            let doc = match doc {
                Ok(doc) => doc,
                Err(e) => {
                    assert!(width == 100 && e.contains("width"), "{mode}/{width}: {e}");
                    continue;
                }
            };
            // A width too narrow for the requested columns must be diagnosed.
            match svg::Renderer::new(&doc) {
                Ok(renderer) => assert_text_geometry(&renderer.render(0)),
                Err(e) => assert!(width == 100 && e.contains("width"), "{mode}/{width}: {e}"),
            }
        }
    }
}

#[test]
fn close_fixed_nodes_wrap_or_report_unsatisfiable_geometry() {
    let doc = spec::parse(r#"{"nodes":[{"label":"a very long first label", "x":40,"y":50},{"label":"a very long second label","x":55,"y":50}]}"#).unwrap();
    assert_text_geometry(&svg::Renderer::new(&doc).unwrap().render(0));
    let doc = spec::parse(r#"{"nodes":[{"icon":"dns","label":"one","x":50,"y":50},{"icon":"dns","label":"two","x":50,"y":50}]}"#).unwrap();
    assert!(
        svg::Renderer::new(&doc).is_err(),
        "identical fixed anchors cannot fit"
    );
}

#[test]
fn invalid_timelines_are_rejected_before_sampling() {
    for json in [
        r#"{"els":[{"type":"node","label":"bad","x":[],"y":50}]}"#,
        r#"{"header":[]}"#,
        r#"{"note":"\u0000"}"#,
        r#"{"note":"\uffff"}"#,
        r#"{"els":[{"type":"text","text":"bad","x":1e300,"y":50}]}"#,
        r#"{"els":[{"type":"polyline","points":[[0,[0,0]],[10,[0,0,10,10]]]}]}"#,
        r#"{"els":[{"type":"node","x":[[10,20],[0,40]],"y":50}]}"#,
    ] {
        assert!(timeline::parse_doc(json).is_err(), "{json}");
    }
}

#[test]
fn structural_specs_reject_invalid_xml_text_before_measurement() {
    assert!(spec::parse(r#"{"layout":"flow","nodes":[{"label":"\u0000"}]}"#).is_err());
}

#[test]
fn moving_annotation_reserves_the_complete_timeline_envelope() {
    let doc = timeline::parse_doc(r#"{"duration":1000,"els":[{"type":"node","x":70,"y":50,"label":"stationary"},{"type":"text","text":"moving annotation","x":[[0,0],[999,100]],"y":[[0,20],[999,100]]}]}"#).unwrap();
    let renderer = svg::Renderer::new(&doc).unwrap();
    for t in [0, 250, 500, 750, 999] {
        assert_text_geometry(&renderer.render(t));
    }
}

#[test]
fn prepared_renderer_accepts_the_entire_seed_corpus() {
    for (name, json) in crate::sims_data::SIMS {
        let doc = timeline::parse_doc(json).unwrap_or_else(|e| panic!("{name}: {e}"));
        let renderer = svg::Renderer::new(&doc).unwrap_or_else(|e| panic!("{name}: {e}"));
        for t in [
            0,
            doc.duration.unwrap_or(0) / 2,
            doc.duration.unwrap_or(1).saturating_sub(1),
        ] {
            let output = renderer.render(t);
            let tree = usvg::Tree::from_str(
                &output,
                &usvg::Options {
                    fontdb: crate::typography::fontdb(),
                    ..Default::default()
                },
            )
            .unwrap();
            let mut all = vec![];
            texts(tree.root(), &mut all);
            for (s, b) in all {
                assert!(
                    b[0] >= 0.
                        && b[1] >= 0.
                        && b[2] <= tree.size().width() + 0.01
                        && b[3] <= tree.size().height() + 0.01,
                    "{name}/{t}: clipped text {s} {b:?}"
                );
            }
        }
    }
}

#[test]
fn columns_keep_document_order_with_a_partial_last_column() {
    let doc = spec::parse(r#"{"layout":{"mode":"columns","columns":3},"nodes":[{"label":"zero"},{"label":"one"},{"label":"two"},{"label":"three"}]}"#).unwrap();
    let output = svg::Renderer::new(&doc).unwrap().render(0);
    let tree = usvg::Tree::from_str(
        &output,
        &usvg::Options {
            fontdb: crate::typography::fontdb(),
            ..Default::default()
        },
    )
    .unwrap();
    let mut all = vec![];
    texts(tree.root(), &mut all);
    let center = |label: &str| {
        let (_, b) = all.iter().find(|(s, _)| s == label).unwrap();
        ((b[0] + b[2]) / 2., (b[1] + b[3]) / 2.)
    };
    let (a, b, c, d) = (
        center("zero"),
        center("one"),
        center("two"),
        center("three"),
    );
    assert!(
        (a.0 - b.0).abs() < 0.01 && (c.0 - d.0).abs() < 0.01,
        "column order: {a:?} {b:?} {c:?} {d:?}"
    );
    assert!(a.1 < b.1 && c.1 < d.1 && a.0 < c.0);
}

#[test]
fn extra_structural_height_is_distributed_between_rows() {
    let mut doc = spec::parse(
        r#"{"layout":{"mode":"grid","columns":1},"nodes":[{"label":"top"},{"label":"bottom"}]}"#,
    )
    .unwrap();
    let separation = |doc: &timeline::Doc| {
        let output = svg::Renderer::new(doc).unwrap().render(0);
        let tree = usvg::Tree::from_str(
            &output,
            &usvg::Options {
                fontdb: crate::typography::fontdb(),
                ..Default::default()
            },
        )
        .unwrap();
        let mut all = vec![];
        texts(tree.root(), &mut all);
        let y = |label| all.iter().find(|(s, _)| s == label).unwrap().1[1];
        y("bottom") - y("top")
    };
    let normal = separation(&doc);
    doc.canvas.min_height *= 2.;
    assert!(
        separation(&doc) > normal,
        "height must add layout room between rows"
    );
}

#[test]
fn referenced_packets_share_clipped_routes_as_nodes_move() {
    let doc = timeline::parse_doc(r#"{"duration":1000,"els":[{"type":"node","id":"a","x":[[0,10],[999,30]],"y":50,"label":"source","status":"ready"},{"type":"node","id":"b","x":90,"y":50,"label":"target","status":"ready"},{"type":"line","from":"a","to":"b"},{"type":"packet","label":"GET","from":"a","to":"b","x1":10,"y1":50,"x2":90,"y2":50,"p":0.5}]}"#).unwrap();
    let renderer = svg::Renderer::new(&doc).unwrap();
    for t in [0, 500, 999] {
        let output = renderer.render(t);
        let tree = usvg::Tree::from_str(
            &output,
            &usvg::Options {
                fontdb: crate::typography::fontdb(),
                ..Default::default()
            },
        )
        .unwrap();
        let bbox = |id| tree.node_by_id(id).unwrap().abs_bounding_box();
        let (a, b, route, packet) = (
            bbox("node-0"),
            bbox("node-1"),
            bbox("route-0"),
            bbox("packet-0"),
        );
        assert!(route.left() >= a.right() - 1. && route.right() <= b.left() + 1.);
        assert!(((packet.left() + packet.right()) - (route.left() + route.right())).abs() < 0.01);
        assert!(((packet.top() + packet.bottom()) - (route.top() + route.bottom())).abs() < 0.01);
    }
}

#[test]
fn short_routes_keep_packet_labels_visible_without_node_collisions() {
    let doc = spec::parse(include_str!(
        "../outputs/layout-performance-audit/ci-pipeline.json"
    ))
    .unwrap();
    let renderer = svg::Renderer::new(&doc).unwrap();
    for (packet, time) in [
        (0, 1080),
        (1, 2820),
        (2, 4740),
        (3, 7080),
        (4, 7560),
        (5, 10320),
    ] {
        let output = renderer.render(time);
        assert!(
            output.contains(&format!("id=\"packet-{packet}\"")),
            "packet {packet} vanished at mid-flight"
        );
        assert_text_geometry(&output);
    }
}
