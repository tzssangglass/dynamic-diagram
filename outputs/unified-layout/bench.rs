//! Reproducible same-image comparison: existing resvg + tiny-skia PNG pipeline
//! versus the reusable production rasterizer. Preparation is reported separately.
#![allow(dead_code)]
#[path = "../../src/assets/mod.rs"]
mod assets;
#[path = "../../src/frame.rs"]
mod frame;
#[path = "../../src/kitty.rs"]
mod kitty;
#[path = "../../src/layout.rs"]
mod layout;
#[path = "../../src/raster.rs"]
mod raster;
#[path = "../../src/spec.rs"]
mod spec;
#[path = "../../src/svg.rs"]
mod svg;
#[path = "../../src/timeline.rs"]
mod timeline;
#[path = "../../src/typography.rs"]
mod typography;
use resvg::{tiny_skia, usvg};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let json = std::fs::read_to_string(&args[1]).unwrap();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    let doc = if v.get("els").is_some() {
        timeline::parse_doc(&json).unwrap()
    } else {
        spec::parse(&json).unwrap()
    };
    let density: f32 = args[2].parse().unwrap();
    let _ = typography::fontdb();
    let start = Instant::now();
    let renderer = svg::Renderer::new(&doc).unwrap();
    let prepare_ms = start.elapsed().as_secs_f64() * 1000.;
    let (w, h) = renderer.size();
    let mut pm = tiny_skia::Pixmap::new(
        (w * density as f64).ceil() as u32,
        (h * density as f64).ceil() as u32,
    )
    .unwrap();
    let options = usvg::Options {
        fontdb: typography::fontdb(),
        ..Default::default()
    };
    let mut results = vec![];
    let duration = doc.duration.unwrap_or(1000);
    for workload in ["first_pass", "repeat_24", "static"] {
        for optimized in [false, true] {
            let mut raster = raster::Rasterizer::new();
            let mut samples = vec![];
            let mut png_bytes = 0;
            let warmup = if workload == "repeat_24" { 24 } else { 8 };
            for i in 0..warmup + 96 {
                let t = match workload {
                    "repeat_24" => (i % 24) as u64 * duration / 24,
                    "static" => 0,
                    _ => i as u64 * duration / (warmup + 96) as u64,
                };
                let start = Instant::now();
                let s = renderer.render(t);
                let png = if optimized {
                    raster.render(&s, density).unwrap().to_vec()
                } else {
                    let tree = usvg::Tree::from_str(&s, &options).unwrap();
                    pm.fill(tiny_skia::Color::TRANSPARENT);
                    resvg::render(
                        &tree,
                        tiny_skia::Transform::from_scale(density, density),
                        &mut pm.as_mut(),
                    );
                    pm.encode_png().unwrap()
                };
                std::hint::black_box(kitty::base64_encode(&png));
                if i >= warmup {
                    samples.push(start.elapsed().as_secs_f64() * 1000.);
                    png_bytes += png.len();
                }
            }
            samples.sort_by(f64::total_cmp);
            results.push(serde_json::json!({"workload":workload,"pipeline":if optimized{"production"}else{"reference_same_svg"},"p50_ms":samples[48],"p95_ms":samples[91],"mean_ms":samples.iter().sum::<f64>()/96.,"mean_png_bytes":png_bytes/96}));
        }
    }
    println!(
        "{}",
        serde_json::json!({"fixture":args[1],"density":density,"logical_size":[w,h],"pixel_size":[pm.width(),pm.height()],"prepare_ms_fonts_warm":prepare_ms,"samples_per_case":96,"results":results})
    );
}
