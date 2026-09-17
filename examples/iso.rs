// isolate: is it comma-list parsing or memory loading?
fn main() {
    let mk = |fam: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 320 44\"><rect width=\"320\" height=\"44\" fill=\"#fff\"/><text x=\"8\" y=\"32\" font-family=\"{fam}\" font-size=\"24\">CLIENT BASE</text></svg>"
        )
    };
    let raster = |svg: String, db: resvg::usvg::fontdb::Database| -> Vec<u8> {
        let opts = resvg::usvg::Options { fontdb: std::sync::Arc::new(db), ..Default::default() };
        let tree = resvg::usvg::Tree::from_str(&svg, &opts).unwrap();
        let mut pm = resvg::tiny_skia::Pixmap::new(320, 44).unwrap();
        let mut pmref = pm.as_mut();
        resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pmref);
        pm.encode_png().unwrap()
    };
    let fresh = || {
        let mut db = resvg::usvg::fontdb::Database::new();
        db.load_font_data(std::fs::read("src/assets/fonts/Inconsolata.ttf").unwrap());
        db.load_font_data(std::fs::read("src/assets/fonts/LiberationSans.ttf").unwrap());
        db
    };
    let cases = [
        ("single: Liberation Sans", "Liberation Sans"),
        ("single: Inconsolata", "Inconsolata"),
        ("list: Source Sans 3, Liberation Sans", "Source Sans 3, Liberation Sans, sans-serif"),
    ];
    for (i, (name, fam)) in cases.iter().enumerate() {
        let png = raster(mk(fam), fresh());
        std::fs::write(format!("/tmp/iso_{i}.png"), &png).unwrap();
        println!("{name}: {} bytes", png.len());
    }
}
