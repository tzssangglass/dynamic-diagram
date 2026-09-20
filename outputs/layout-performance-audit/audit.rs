#![allow(dead_code)]
#[path="../../src/frame.rs"] mod frame;
#[path="../../src/timeline.rs"] mod timeline;
#[path="../../src/spec.rs"] mod spec;
#[path="../../src/svg.rs"] mod svg;
#[path="../../src/kitty.rs"] mod kitty;
#[path="../../src/assets/mod.rs"] mod assets;
const FONT_INCONSOLATA: &[u8] = include_bytes!("../../src/assets/fonts/Inconsolata.ttf");
const FONT_LIBERATION: &[u8] = include_bytes!("../../src/assets/fonts/LiberationSans.ttf");

// parse the fontdb once for the whole process (was rebuilt every frame: ~2MB churn)
fn fontdb() -> std::sync::Arc<resvg::usvg::fontdb::Database> {
    static DB: std::sync::OnceLock<std::sync::Arc<resvg::usvg::fontdb::Database>> = std::sync::OnceLock::new();
    DB.get_or_init(|| {
        // ponytail: resvg renders Adobe/Google "Source Sans 3" TTFs blank/broken
        // (fontdb bug); Liberation Sans is the proven-good embedded sans.
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_font_data(FONT_INCONSOLATA.to_vec());
        db.load_font_data(FONT_LIBERATION.to_vec());
        std::sync::Arc::new(db)
    })
    .clone()
}

use std::time::Instant;
fn read_doc(path: &str) -> timeline::Doc {
    let json = std::fs::read_to_string(path).unwrap();
    if json.contains("\"els\"") { timeline::parse_doc(&json).unwrap() } else { spec::parse(&json).unwrap() }
}
fn collect_text(g: &resvg::usvg::Group, out: &mut Vec<(String, [f32;4])>) {
    for n in g.children() {
        match n {
            resvg::usvg::Node::Group(g) => collect_text(g,out),
            resvg::usvg::Node::Text(t) => {
                let b=t.abs_bounding_box();
                out.push((t.chunks().iter().map(|c|c.text()).collect::<String>(),[b.left(),b.top(),b.right(),b.bottom()]));
            }, _=>{}
        }
    }
}
fn main() {
    let args: Vec<String>=std::env::args().collect();
    if args[1]=="geometry" {
        let doc=read_doc(&args[2]);
        let opts=resvg::usvg::Options { fontdb:fontdb(), ..Default::default() };
        let f=doc.frame_at(0);
        let svg=svg::render_svg(&f);
        let tree=resvg::usvg::Tree::from_str(&svg,&opts).unwrap();
        let mut texts=vec![];collect_text(tree.root(),&mut texts);
        let mut collisions=vec![];
        for i in 0..texts.len() {for j in i+1..texts.len() {
            let (a,b)=(texts[i].1,texts[j].1);
            if a[0]<b[2]&&b[0]<a[2]&&a[1]<b[3]&&b[1]<a[3] {collisions.push((&texts[i],&texts[j]));}
        }}
        let overflow:Vec<_>=texts.iter().filter(|(_,b)|b[0]<0.||b[1]<0.||b[2]>760.||b[3]>330.).collect();
        println!("{}",serde_json::json!({"path":args[2],"text_collisions":collisions,"text_overflow":overflow,"layouts":format!("{:?}",svg::plan_node_layout(&f.nodes)),"unspaced_caption_line_chars":svg::wrap(&"W".repeat(200),92).iter().map(|s|s.chars().count()).collect::<Vec<_>>() }));
        return;
    }
    let path=&args[2];let scale: f32=args[3].parse().unwrap();
    let n: usize=args.get(4).map(|s|s.parse().unwrap()).unwrap_or(96);
    let doc=read_doc(path);let duration=doc.duration.unwrap_or(1000);
    let opts=resvg::usvg::Options { fontdb:fontdb(), ..Default::default() };
    let w=(svg::W as f32*scale).ceil() as u32;let h=(svg::H as f32*scale).ceil() as u32;
    let mut pm=resvg::tiny_skia::Pixmap::new(w,h).unwrap();
    let mut samples:Vec<[f64;7]>=vec![];let mut sizes:Vec<usize>=vec![];
    for i in 0..n+8 {
        let t=(i%24) as u64*duration/24;
        let a=Instant::now();let f=doc.frame_at(t);
        let b=Instant::now();let s=svg::render_svg(&f);
        let c=Instant::now();let tree=resvg::usvg::Tree::from_str(&s,&opts).unwrap();
        let d=Instant::now();resvg::render(&tree,resvg::tiny_skia::Transform::from_scale(scale,scale),&mut pm.as_mut());
        let e=Instant::now();let png=pm.encode_png().unwrap();
        let g=Instant::now();let b64=kitty::base64_encode(&png);
        std::hint::black_box(b64);
        let end=Instant::now();
        if i>=8 {
            samples.push([(b-a).as_secs_f64()*1000.,(c-b).as_secs_f64()*1000.,(d-c).as_secs_f64()*1000.,(e-d).as_secs_f64()*1000.,(g-e).as_secs_f64()*1000.,(end-g).as_secs_f64()*1000.,(end-a).as_secs_f64()*1000.]);sizes.push(png.len());
        }
    }
    let mut avg=[0.;7];for s in &samples {for i in 0..7 {avg[i]+=s[i]/n as f64;}}
    let mut totals:Vec<f64>=samples.iter().map(|s|s[6]).collect();totals.sort_by(|a,b|a.total_cmp(b));
    println!("{}",serde_json::json!({"path":path,"scale":scale,"pixels":[w,h],"n":n,"mean_ms":{"frame_at":avg[0],"svg":avg[1],"parse_shape":avg[2],"raster":avg[3],"png":avg[4],"base64":avg[5],"total":avg[6]},"p50_ms":totals[n/2],"p95_ms":totals[(n*95/100).min(n-1)],"mean_png_bytes":sizes.iter().sum::<usize>()/n,"rgba_bytes":u64::from(w)*u64::from(h)*4 }));
}
