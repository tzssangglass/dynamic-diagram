//! Reusable, bounded raster output. Display size lives in SVG; density only
//! changes sampling. Buffers belong to one renderer, never a process-wide lock.

use resvg::{tiny_skia, usvg};
use std::collections::VecDeque;
use std::io;
use std::sync::Arc;

const DEFAULT_CACHE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CACHE_ENTRIES: usize = 64;
// Bound the main RGBA buffer; resvg may need additional buffers for filters.
const MAX_PIXELS: u64 = 32 * 1024 * 1024;

struct Cached {
    svg: String,
    density: f32,
    png: Arc<[u8]>,
}

pub struct Rasterizer {
    options: usvg::Options<'static>,
    pixmap: Option<tiny_skia::Pixmap>,
    cache: VecDeque<Cached>,
    cache_bytes: usize,
    budget: usize,
}

impl Rasterizer {
    pub fn new() -> Self {
        Self {
            options: usvg::Options {
                fontdb: crate::typography::fontdb(),
                ..Default::default()
            },
            pixmap: None,
            cache: VecDeque::new(),
            cache_bytes: 0,
            budget: DEFAULT_CACHE_BYTES,
        }
    }

    pub fn render(&mut self, svg: &str, density: f32) -> io::Result<Arc<[u8]>> {
        if !density.is_finite() || density <= 0.0 {
            return Err(invalid("density must be a positive finite number"));
        }
        if let Some(i) = self
            .cache
            .iter()
            .position(|e| e.density == density && e.svg == svg)
        {
            let entry = self.cache.remove(i).unwrap();
            let png = Arc::clone(&entry.png);
            self.cache.push_back(entry);
            return Ok(png);
        }
        let tree = usvg::Tree::from_str(svg, &self.options)
            .map_err(|e| invalid(format!("svg parse: {e}")))?;
        let (w, h) = pixel_size(tree.size().width(), tree.size().height(), density)?;
        if self
            .pixmap
            .as_ref()
            .is_none_or(|p| (p.width(), p.height()) != (w, h))
        {
            self.pixmap = Some(
                tiny_skia::Pixmap::new(w, h)
                    .ok_or_else(|| invalid("cannot allocate image buffer"))?,
            );
        }
        let pixmap = self.pixmap.as_mut().unwrap();
        // Required even with caching: transparent frames must not retain the
        // preceding frame's pixels when a caller supplies a partial background.
        pixmap.fill(tiny_skia::Color::TRANSPARENT);
        resvg::render(
            &tree,
            tiny_skia::Transform::from_scale(density, density),
            &mut pixmap.as_mut(),
        );
        let png: Arc<[u8]> = encode(pixmap)?.into();
        let bytes = svg.len() + png.len();
        if bytes <= self.budget {
            while self.cache_bytes + bytes > self.budget || self.cache.len() >= MAX_CACHE_ENTRIES {
                if let Some(old) = self.cache.pop_front() {
                    self.cache_bytes -= old.svg.len() + old.png.len();
                }
            }
            self.cache_bytes += bytes;
            self.cache.push_back(Cached {
                svg: svg.into(),
                density,
                png: Arc::clone(&png),
            });
        }
        Ok(png)
    }
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn pixel_size(width: f32, height: f32, density: f32) -> io::Result<(u32, u32)> {
    let w = (width as f64 * density as f64).ceil();
    let h = (height as f64 * density as f64).ceil();
    if !w.is_finite() || !h.is_finite() || w < 1. || h < 1. || w * h > MAX_PIXELS as f64 {
        return Err(invalid(format!("raster dimensions exceed the {MAX_PIXELS}-pixel budget; reduce --density or canvas size")));
    }
    Ok((w as u32, h as u32))
}

fn encode(pixmap: &tiny_skia::Pixmap) -> io::Result<Vec<u8>> {
    if pixmap.data().chunks_exact(4).all(|p| p[3] == 255) {
        // The diagram's background is opaque. Premultiplied and straight RGBA
        // coincide, so there is no need to clone/demultiply the entire image.
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, pixmap.width(), pixmap.height());
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(pixmap.data())?;
        }
        Ok(bytes)
    } else {
        pixmap
            .encode_png()
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><rect width="20" height="10" fill="red"/></svg>"#;
    const EMPTY: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"/>"#;

    #[test]
    fn reused_buffer_does_not_ghost_previous_frame() {
        let mut r = Rasterizer::new();
        r.render(RED, 1.).unwrap();
        let empty = r.render(EMPTY, 1.).unwrap();
        let pm = tiny_skia::Pixmap::decode_png(&empty).unwrap();
        assert!(pm.pixels().iter().all(|p| p.alpha() == 0));
    }

    #[test]
    fn cache_keeps_density_distinct_and_enforces_its_budget() {
        let mut r = Rasterizer::new();
        let one = r.render(RED, 1.).unwrap();
        let two = r.render(RED, 2.).unwrap();
        assert_eq!(tiny_skia::Pixmap::decode_png(&two).unwrap().width(), 40);
        assert_eq!(tiny_skia::Pixmap::decode_png(&one).unwrap().width(), 20);
        assert!(Arc::ptr_eq(&one, &r.render(RED, 1.).unwrap()));
        r.budget = 0;
        r.cache.clear();
        r.cache_bytes = 0;
        r.render(RED, 1.).unwrap();
        assert!(r.cache.is_empty());
    }

    #[test]
    fn opaque_fast_path_matches_general_encoder_pixels() {
        let mut pm = tiny_skia::Pixmap::new(20, 10).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(20, 35, 50, 255));
        let expected = tiny_skia::Pixmap::decode_png(&pm.encode_png().unwrap()).unwrap();
        let actual = tiny_skia::Pixmap::decode_png(&encode(&pm).unwrap()).unwrap();
        assert_eq!(actual.data(), expected.data());
        pm.fill(tiny_skia::Color::from_rgba8(20, 35, 50, 100));
        let actual = tiny_skia::Pixmap::decode_png(&encode(&pm).unwrap()).unwrap();
        assert_eq!(actual.data(), pm.data());
    }
}
