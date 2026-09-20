//! JSON/keyframes → measured scene → SVG/PNG/kitty. Content stays data.
mod assets;
mod check;
mod frame;
mod kitty;
mod layout;
mod raster;
mod sims_data;
mod spec;
mod svg;
mod terminal;
mod timeline;
mod typography;

use frame::Sim;
use std::error::Error;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const DEFAULT_FPS: f64 = 30.;
const MAX_EXPORT_FRAMES: u64 = 1800;

fn get_doc(name: &str) -> timeline::Doc {
    let json = sims_data::get(name).unwrap_or_else(|| sims_data::get("tcphs").unwrap());
    timeline::parse_doc(json).unwrap_or_else(|e| {
        eprintln!("sim {name:?}: {e}");
        std::process::exit(2)
    })
}

fn get_sim(name: &str) -> Box<dyn Sim> {
    Box::new(timeline::DocSim(get_doc(name)))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(2);
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref().unwrap_or("web") {
        "check" => {
            check::run();
            Ok(())
        }
        "skill" => {
            print!("{}", include_str!("../skills/dynamic-diagram/SKILL.md"));
            Ok(())
        }
        "spec" => {
            let path = args.next().ok_or(
                "usage: dynamic-diagram spec FILE [svg|png|info|frames|kitty|kitty-anim] [options]",
            )?;
            spec_mode(&path, args.collect())
        }
        "kitty" => {
            let doc = get_doc(&args.next().unwrap_or_default());
            let options = OutputOptions::parse(args.collect())?;
            play(doc, options)
        }
        "web" => {
            let doc = get_doc(&args.next().unwrap_or_default());
            web_mode(doc, OutputOptions::parse(args.collect())?)
        }
        name => web_mode(get_doc(name), OutputOptions::parse(args.collect())?),
    }
}

struct OutputOptions {
    scale: Option<f64>,
    height: Option<f64>,
    width: Option<f64>,
    density: f32,
    fps: f64,
    at: u64,
    positional: Vec<String>,
}

impl OutputOptions {
    fn parse(args: Vec<String>) -> Result<Self> {
        let density = std::env::var("DDA_SCALE").unwrap_or_else(|_| "3".into());
        let mut options = Self {
            scale: None,
            height: None,
            width: None,
            density: positive("DDA_SCALE", &density)? as f32,
            fps: DEFAULT_FPS,
            at: 0,
            positional: vec![],
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if !arg.starts_with("--") {
                options.positional.push(arg);
                continue;
            }
            let value = args
                .next()
                .ok_or_else(|| format!("{arg} requires a value"))?;
            match arg.as_str() {
                "--scale" => options.scale = Some(positive(&arg, &value)?),
                "--height" => options.height = Some(positive(&arg, &value)?),
                "--width" => options.width = Some(positive(&arg, &value)?),
                "--density" => options.density = positive(&arg, &value)? as f32,
                "--fps" => {
                    options.fps = positive(&arg, &value)?;
                    if !(1. ..=120.).contains(&options.fps) {
                        return Err("--fps must be between 1 and 120".into());
                    }
                }
                "--at" => {
                    options.at = value
                        .parse()
                        .map_err(|_| "--at must be milliseconds >= 0")?
                }
                _ => return Err(format!("unknown option {arg}").into()),
            }
        }
        Ok(options)
    }
    fn apply(&self, doc: &mut timeline::Doc) {
        if let Some(v) = self.scale {
            doc.canvas.scale = v;
        }
        if let Some(v) = self.height {
            doc.canvas.min_height = v;
        }
        if let Some(v) = self.width {
            doc.canvas.width = v;
        }
    }
}

fn positive(name: &str, value: &str) -> Result<f64> {
    let number: f64 = value
        .parse()
        .map_err(|_| format!("{name} must be a positive finite number"))?;
    if !number.is_finite() || number <= 0. {
        return Err(format!("{name} must be a positive finite number").into());
    }
    Ok(number)
}

fn parse_input(json: &str) -> Result<timeline::Doc> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    if !value.is_object() {
        return Err("document must be a JSON object".into());
    }
    // Values such as label="els" must not select a different document schema.
    Ok(if value.get("els").is_some() {
        timeline::parse_doc(json)?
    } else {
        spec::parse(json)?
    })
}

fn spec_mode(path: &str, args: Vec<String>) -> Result<()> {
    if path == "--list-icons" {
        for name in assets::icons::all() {
            println!("{name}");
        }
        return Ok(());
    }
    let options = OutputOptions::parse(args)?;
    let mode = options
        .positional
        .first()
        .map(String::as_str)
        .unwrap_or("png");
    if !["svg", "png", "info", "frames", "kitty", "kitty-anim"].contains(&mode) {
        return Err(format!("unknown output mode {mode:?}").into());
    }
    let mut doc = parse_input(&std::fs::read_to_string(path)?)?;
    options.apply(&mut doc);
    if mode == "kitty-anim" {
        return play(doc, options);
    }
    let renderer = svg::Renderer::new(&doc)?;
    let (width, height) = renderer.size();
    let mut raster = raster::Rasterizer::new();
    match mode {
        "info" => println!(
            "{}",
            serde_json::json!({"width":width,"height":height,"duration":doc.duration})
        ),
        "svg" => print!("{}", renderer.render(options.at)),
        "png" => {
            let png = raster.render(&renderer.render(options.at), options.density)?;
            let out = Path::new(path).with_extension("png");
            std::fs::write(&out, &png)?;
            println!("wrote {} ({} bytes)", out.display(), png.len());
        }
        "kitty" => {
            let png = raster.render(&renderer.render(options.at), options.density)?;
            let (cols, rows) = terminal::Geometry::current().fit(width, height);
            print!("{}", kitty::kitty_png(&png, cols, rows));
        }
        "frames" => {
            let duration = doc
                .duration
                .filter(|d| *d > 0)
                .ok_or("spec needs a positive duration to animate")?;
            let dir = options
                .positional
                .get(1)
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| Path::new(path).with_extension("frames"));
            let count = match options.positional.get(2) {
                Some(value) => value
                    .parse::<u64>()
                    .map_err(|_| "frame count must be an integer")?,
                None => ((duration as f64 * options.fps / 1000.).ceil() as u64)
                    .clamp(1, MAX_EXPORT_FRAMES),
            };
            if count == 0 || count > MAX_EXPORT_FRAMES {
                return Err(format!("frame count must be 1..={MAX_EXPORT_FRAMES}").into());
            }
            std::fs::create_dir_all(&dir)?;
            let mut files = Vec::with_capacity(count as usize);
            for i in 0..count {
                let t = (i as u128 * duration as u128 / count as u128) as u64;
                let png = raster.render(&renderer.render(t), options.density)?;
                let name = format!("{:05}.png", i + 1);
                std::fs::write(dir.join(&name), &png)?;
                files.push(name);
            }
            // Manifest identifies this export exactly; unrelated files in the
            // supplied directory are never silently deleted.
            let manifest = serde_json::json!({"width":width,"height":height,"duration":duration,"count":count,"fps":count as f64*1000./duration as f64,"files":files});
            std::fs::write(
                dir.join("frames.json"),
                serde_json::to_vec_pretty(&manifest)?,
            )?;
            println!(
                "wrote {count} frames to {}/ (loop {duration}ms)",
                dir.display()
            );
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn play(mut doc: timeline::Doc, options: OutputOptions) -> Result<()> {
    if doc.duration.is_none_or(|d| d == 0) {
        return Err("spec needs a positive duration to animate".into());
    }
    options.apply(&mut doc);
    let renderer = svg::Renderer::new(&doc)?;
    let mut raster = raster::Rasterizer::new();
    let quit = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&quit))?;
    let out = std::io::stdout();
    let mut out = out.lock();
    out.write_all(b"\x1b[?25l")?;
    let start = Instant::now();
    let tick = Duration::from_secs_f64(1. / options.fps);
    let mut deadline = start;
    let mut error = None;
    while !quit.load(Ordering::Relaxed) {
        let t = start.elapsed().as_millis() as u64;
        let frame = (|| -> Result<()> {
            let png = raster.render(&renderer.render(t), options.density)?;
            let (w, h) = renderer.size();
            let (cols, rows) = terminal::Geometry::current().fit(w, h);
            write!(out, "\x1b[H{}", kitty::kitty_png(&png, cols, rows))?;
            out.flush()?;
            Ok(())
        })();
        if let Err(e) = frame {
            error = Some(e);
            break;
        }
        deadline += tick;
        let now = Instant::now();
        if deadline > now {
            // Signal handlers set a flag; one long sleep would delay Ctrl+C
            // until the next frame at low frame rates.
            while !quit.load(Ordering::Relaxed) {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                std::thread::sleep(remaining.min(Duration::from_millis(25)));
            }
        } else {
            deadline = now;
        }
    }
    write!(out, "{}\x1b[?25h", kitty::KITTY_CLEAR)?;
    out.flush()?;
    if let Some(e) = error {
        return Err(e);
    }
    Ok(())
}

fn index_html(width: f64, height: f64, fps: f64) -> String {
    format!(
        r#"<!doctype html>
<meta charset="utf-8"><title>dynamic-diagram</title>
<body style="margin:0;background:#fafafa;display:grid;justify-items:center;min-height:100vh">
<img id="sim" width="{width}" height="{height}" style="max-width:96vw;height:auto;align-self:start">
<script>
const img=document.getElementById('sim'),start=performance.now(),interval=1000/{fps};
let next=0;
async function frame(){{
 const t=performance.now()-start;
 try{{ const response=await fetch('/frame.svg?t='+Math.floor(t));
 if(response.ok){{const url=URL.createObjectURL(await response.blob());const previous=img.src;img.src=url;await img.decode().catch(()=>{{}});if(previous.startsWith('blob:'))URL.revokeObjectURL(previous);}}
 }}finally{{next=Math.max(next+interval,performance.now()-start);setTimeout(frame,Math.max(0,next-(performance.now()-start)));}}
}}
frame();
</script>"#
    )
}

fn respond(mut stream: std::net::TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let head = format!("HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncache-control: no-store\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body);
}

fn web_mode(mut doc: timeline::Doc, options: OutputOptions) -> Result<()> {
    options.apply(&mut doc);
    let renderer = svg::Renderer::new(&doc)?;
    let (width, height) = renderer.size();
    let html = index_html(width, height, options.fps);
    let listener = std::net::TcpListener::bind("127.0.0.1:8080")?;
    println!("open http://localhost:8080  (Ctrl+C to stop)");
    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(s) => s,
            Err(_) => continue,
        };
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        let mut buf = [0u8; 4096];
        let n = match stream.read(&mut buf) {
            Ok(0) | Err(_) => continue,
            Ok(n) => n,
        };
        let req = String::from_utf8_lossy(&buf[..n]);
        let path = req
            .lines()
            .next()
            .unwrap_or("")
            .split_whitespace()
            .nth(1)
            .unwrap_or("/");
        if path == "/" {
            respond(
                stream,
                "200 OK",
                "text/html; charset=utf-8",
                html.as_bytes(),
            );
        } else if path.starts_with("/frame.svg") {
            let t = path
                .split("t=")
                .nth(1)
                .and_then(|s| s.split('&').next())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            respond(
                stream,
                "200 OK",
                "image/svg+xml",
                renderer.render(t).as_bytes(),
            );
        } else {
            respond(stream, "404 Not Found", "text/plain", b"not found");
        }
    }
    Ok(())
}
