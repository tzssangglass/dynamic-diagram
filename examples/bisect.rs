// bisect: which db contents make "Source Sans 3" / "DejaVu Sans" garble?
fn render(svg: &str, db: resvg::usvg::fontdb::Database) -> bool {
    // returns true if rendered text differs from a known-good reference render
    // (we compare against the same text in Inconsolata which is proven correct)
    let _ = db;
    false
}

fn main() {
    let text = "CLIENT";
    let mk = |fam: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 300 60\"><rect width=\"300\" height=\"60\" fill=\"#fff\"/><text x=\"10\" y=\"42\" font-family=\"{fam}\" font-size=\"28\">{text}</text></svg>"
        )
    };
    let raster = |svg: String, db: resvg::usvg::fontdb::Database| -> Vec<u8> {
        let opts = resvg::usvg::Options { fontdb: std::sync::Arc::new(db), ..Default::default() };
        let tree = resvg::usvg::Tree::from_str(&svg, &opts).unwrap();
        let mut pm = resvg::tiny_skia::Pixmap::new(300, 60).unwrap();
        let mut pmref = pm.as_mut();
        resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pmref);
        pm.encode_png().unwrap()
    };

    // case A: system fonts only
    let mut db_a = resvg::usvg::fontdb::Database::new();
    db_a.load_system_fonts();
    // case B: system + embedded ss3
    let mut db_b = resvg::usvg::fontdb::Database::new();
    db_b.load_system_fonts();
    db_b.load_font_data(std::fs::read("src/assets/fonts/SourceSans3.ttf").unwrap());
    // case C: ONLY embedded ss3
    let mut db_c = resvg::usvg::fontdb::Database::new();
    db_c.load_font_data(std::fs::read("src/assets/fonts/SourceSans3.ttf").unwrap());
    // case D: ONLY embedded inconsolata (known good)
    let mut db_d = resvg::usvg::fontdb::Database::new();
    db_d.load_font_data(std::fs::read("src/assets/fonts/Inconsolata.ttf").unwrap());

    let incon_d = raster(mk("Inconsolata"), db_d); // reference: correct

    for (name, db, fam) in [
        ("A system-only + DejaVu Sans", db_a, "DejaVu Sans"),
        ("B system+ss3 + Source Sans 3", db_b, "Source Sans 3"),
        ("C ss3-only + Source Sans 3", db_c, "Source Sans 3"),
    ] {
        // reuse db by reconstructing since it's moved
        let png = raster(mk(fam), db);
        println!("{name}: rendered {} bytes", png.len());
        let _ = &incon_d;
        std::fs::write(format!("/tmp/bisect_{}.png", name.split(' ').next().unwrap()), png).unwrap();
    }
    std::fs::write("/tmp/bisect_ref.png", &incon_d).unwrap();
    println!("ref (Inconsolata-only db): {} bytes", incon_d.len());
}
