use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(json: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "diagram-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("scene.json"), json).unwrap();
        Self(dir)
    }
    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_dynamic-diagram"))
            .env_remove("DDA_SCALE")
            .args(["spec", self.0.join("scene.json").to_str().unwrap()])
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    (
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    )
}
fn svg_size(out: &Output) -> (f32, f32) {
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = resvg::usvg::Tree::from_str(
        std::str::from_utf8(&out.stdout).unwrap(),
        &Default::default(),
    )
    .unwrap();
    (tree.size().width(), tree.size().height())
}

#[test]
fn display_scale_doubles_svg_and_density_only_changes_png() {
    let f = Fixture::new(r#"{"header":"test","nodes":[]}"#);
    let normal = svg_size(&f.run(&["svg"]));
    let doubled = svg_size(&f.run(&["svg", "--scale", "2"]));
    assert_eq!(doubled, (normal.0 * 2., normal.1 * 2.));
    let png = f.run(&["png", "--scale", "2", "--density", "1.5"]);
    assert!(
        png.status.success(),
        "{}",
        String::from_utf8_lossy(&png.stderr)
    );
    assert_eq!(
        png_size(&f.0.join("scene.png")),
        (
            (doubled.0 * 1.5).ceil() as u32,
            (doubled.1 * 1.5).ceil() as u32
        )
    );
}

#[test]
fn invalid_and_oversized_output_options_return_diagnostics() {
    let f = Fixture::new(r#"{"nodes":[]}"#);
    for args in [
        vec!["png", "--density", "NaN"],
        vec!["svg", "--scale", "0"],
        vec!["svg", "--height", "-1"],
        vec!["png", "--density", "1000000"],
        vec!["kitty-anim", "--fps", "1e-300"],
        vec!["kitty-anim", "--fps", "0.01"],
        vec!["svg", "--scale"],
        vec!["typo"],
    ] {
        let result = f.run(&args);
        assert!(!result.status.success(), "invalid {args:?} accepted");
        let diagnostic = String::from_utf8_lossy(&result.stderr);
        assert!(!diagnostic.contains("panicked"), "{diagnostic}");
        assert!(!diagnostic.is_empty());
    }
}

#[test]
fn info_reports_display_size_and_duration_without_writing_a_poster() {
    let f = Fixture::new(r#"{"duration":1000,"canvas":{"min_height":660,"scale":2},"nodes":[]}"#);
    let result = f.run(&["info"]);
    assert!(result.status.success());
    let info: serde_json::Value =
        serde_json::from_slice(&result.stdout).expect("info must be JSON");
    assert_eq!(info["width"], 1520.);
    assert!(info["height"].as_f64().unwrap() >= 1320.);
    assert_eq!(info["duration"], 1000);
    assert!(!f.0.join("scene.png").exists());
}

#[test]
fn frames_respect_explicit_count_and_default_to_time_based_sampling() {
    let f = Fixture::new(r#"{"duration":1000,"nodes":[]}"#);
    for (dirname, count, expected) in [("explicit", Some("3"), 3), ("automatic", None, 30)] {
        let dir = f.0.join(dirname);
        let mut args = vec!["frames", dir.to_str().unwrap()];
        if let Some(n) = count {
            args.push(n);
        }
        args.extend(["--density", "1"]);
        let result = f.run(&args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let files = std::fs::read_dir(dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|s| s == "png")
            })
            .count();
        assert_eq!(files, expected);
    }
    assert!(!f
        .run(&["frames", f.0.join("bad").to_str().unwrap(), "0"])
        .status
        .success());
}

#[test]
fn document_dispatch_inspects_root_fields_not_text_content() {
    let f = Fixture::new(r#"{"nodes":[{"id":"a","label":"els","x":50,"y":50}]}"#);
    let result = f.run(&["svg"]);
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains(">els</text>"));
}

#[cfg(unix)]
#[test]
fn low_frame_rate_playback_exits_promptly_on_interrupt() {
    use std::io::Read;
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let f = Fixture::new(r#"{"duration":1000,"nodes":[]}"#);
    let mut child = Command::new(env!("CARGO_BIN_EXE_dynamic-diagram"))
        .args([
            "spec",
            f.0.join("scene.json").to_str().unwrap(),
            "kitty-anim",
            "--fps",
            "1",
            "--density",
            "0.1",
        ])
        .env_remove("DDA_SCALE")
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    // First output means the handler is installed and playback has started.
    child
        .stdout
        .as_mut()
        .unwrap()
        .read_exact(&mut [0u8; 1])
        .unwrap();
    let start = Instant::now();
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGINT) }, 0);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if start.elapsed() > Duration::from_millis(750) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("playback did not respond to SIGINT before its next frame");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
