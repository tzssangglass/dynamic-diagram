// minimal glyph repro: which font-family garbles Latin into Greek?
fn main() {
    let families = [
        "Inconsolata",
        "Source Sans 3",
        "sans-serif",
        "DejaVu Sans",
        "Liberation Sans",
    ];
    let mut rows = String::new();
    for (i, fam) in families.iter().enumerate() {
        rows.push_str(&format!(
            "<text x=\"10\" y=\"{}\" font-family=\"{fam}\" font-size=\"22\">CLIENT PICKS {i}</text>\n",
            30 + i * 34
        ));
    }
    rows.push_str(&format!(
        "<text x=\"10\" y=\"{}\" font-size=\"22\">CLIENT PICKS default</text>\n",
        30 + families.len() * 34
    ));
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 420 {}\" font-family=\"Source Sans 3, ui-sans-serif, system-ui, sans-serif\"><rect width=\"420\" height=\"{}\" fill=\"#fff\"/>{rows}</svg>",
        40 + (families.len() + 1) * 34,
        40 + (families.len() + 1) * 34
    );
    let mut db = resvg::usvg::fontdb::Database::new();
    db.load_system_fonts();
    db.load_font_data(std::fs::read("src/assets/fonts/Inconsolata.ttf").unwrap());
    db.load_font_data(std::fs::read("src/assets/fonts/SourceSans3.ttf").unwrap());
    let opts = resvg::usvg::Options { fontdb: std::sync::Arc::new(db), ..Default::default() };
    let tree = resvg::usvg::Tree::from_str(&svg, &opts).unwrap();
    let h = 40 + (families.len() + 1) * 34;
    let mut pm = resvg::tiny_skia::Pixmap::new(420, h as u32).unwrap();
    let mut pmref = pm.as_mut();
    resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pmref);
    std::fs::write("/tmp/glyphtest.png", pm.encode_png().unwrap()).unwrap();
    println!("wrote /tmp/glyphtest.png");
}
