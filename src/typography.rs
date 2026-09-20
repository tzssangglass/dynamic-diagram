//! The same embedded font database and shaping engine serve measurement and rasterization.
use resvg::usvg;
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{Arc, OnceLock},
};
pub const FONT: &str = "Inconsolata";
pub fn valid_text(s: &str) -> bool {
    !s.chars().any(|c| {
        matches!(c, '\u{FFFE}' | '\u{FFFF}') || c.is_control() && !matches!(c, '\n' | '\t' | '\r')
    })
}
pub fn fontdb() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        db.load_font_data(include_bytes!("assets/fonts/Inconsolata.ttf").to_vec());
        db.load_font_data(include_bytes!("assets/fonts/LiberationSans.ttf").to_vec());
        db.set_sans_serif_family("Liberation Sans");
        db.set_monospace_family(FONT);
        Arc::new(db)
    })
    .clone()
}
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Metrics {
    pub width: f64,
    pub height: f64,
    pub left: f64,
    pub top: f64,
}
#[derive(Default)]
pub struct Typography {
    cache: RefCell<HashMap<(String, u64), Metrics>>,
    wraps: RefCell<HashMap<(String, u64, u64), Vec<String>>>,
}
impl Typography {
    pub fn measure(&self, text: &str, size: f64) -> Metrics {
        if text.is_empty() {
            return Metrics::default();
        }
        let key = (text.to_owned(), size.to_bits());
        if let Some(m) = self.cache.borrow().get(&key) {
            return *m;
        }
        let svg=format!("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"100000\" height=\"1000\"><text x=\"0\" y=\"100\" font-family=\"{FONT}\" font-size=\"{size}\">{}</text></svg>",escape(text));
        let tree = usvg::Tree::from_str(
            &svg,
            &usvg::Options {
                fontdb: fontdb(),
                ..Default::default()
            },
        )
        .expect("escaped measurement SVG");
        fn find(g: &usvg::Group) -> Option<Metrics> {
            for n in g.children() {
                match n {
                    usvg::Node::Text(t) => {
                        let b = t.abs_bounding_box();
                        return Some(Metrics {
                            width: b.width() as f64,
                            height: b.height() as f64,
                            left: b.left() as f64,
                            top: b.top() as f64 - 100.,
                        });
                    }
                    usvg::Node::Group(g) => {
                        if let Some(m) = find(g) {
                            return Some(m);
                        }
                    }
                    _ => {}
                }
            }
            None
        }
        let m = find(tree.root()).unwrap_or_default();
        self.cache.borrow_mut().insert(key, m);
        m
    }
    /// Width-based wrapping; split an overlong token at Unicode scalar boundaries.
    /// Breaks only at Unicode scalar boundaries. Rendering coverage is limited
    /// to the embedded fonts; wrapping does not add missing CJK glyphs.
    pub fn wrap(&self, text: &str, size: f64, width: f64) -> Vec<String> {
        let key = (text.to_owned(), size.to_bits(), width.to_bits());
        if let Some(lines) = self.wraps.borrow().get(&key) {
            return lines.clone();
        }
        let mut lines = Vec::new();
        for paragraph in text.split('\n') {
            let mut rest = paragraph.trim();
            while !rest.is_empty() {
                if self.measure(rest, size).width <= width {
                    lines.push(rest.to_owned());
                    break;
                }
                let offsets: Vec<usize> = rest
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(rest.len()))
                    .collect();
                let (mut lo, mut hi) = (1, offsets.len() - 1);
                while lo < hi {
                    let mid = (lo + hi).div_ceil(2);
                    if self.measure(&rest[..offsets[mid]], size).width <= width {
                        lo = mid;
                    } else {
                        hi = mid - 1;
                    }
                }
                let end = offsets[lo];
                let split = rest[..end]
                    .rfind(char::is_whitespace)
                    .filter(|i| *i > 0)
                    .unwrap_or(end);
                lines.push(rest[..split].trim_end().to_owned());
                rest = rest[split..].trim_start();
            }
            if paragraph.is_empty() {
                lines.push(String::new());
            }
        }
        self.wraps.borrow_mut().insert(key, lines.clone());
        lines
    }
}
