// same test but through the binary's actual raster() path: render a spec frame
// twice with different font-families injected into the svg, compare
fn main() {
    let mk = |family: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 60\"><text x=\"10\" y=\"40\" font-family=\"{family}\" font-size=\"32\">MMMM mmmm iiii</text></svg>"
        )
    };
    let render = |svg: String| -> Vec<u8> {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_system_fonts();
        db.load_font_data(std::fs::read("src/assets/fonts/Inconsolata.ttf").unwrap());
        let opts = resvg::usvg::Options { fontdb: std::sync::Arc::new(db), ..Default::default() };
        let tree = resvg::usvg::Tree::from_str(&svg, &opts).unwrap();
        let mut pm = resvg::tiny_skia::Pixmap::new(200, 60).unwrap();
        let mut pmref = pm.as_mut();
        resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pmref);
        pm.encode_png().unwrap()
    };
    let a = render(mk("Inconsolata"));
    let b = render(mk("ThisFontDoesNotExist12345"));
    println!("{}", if a == b { "STILL IDENTICAL — font STILL not found" } else { "DIFFER — embedded font active" });
}
