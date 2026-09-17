// verify resvg picks up Inconsolata: rasters of the same text with
// font-family "Inconsolata" vs a bogus family must differ
fn main() {
    let mk = |family: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 60\"><text x=\"10\" y=\"40\" font-family=\"{family}\" font-size=\"32\">MMMM mmmm iiii</text></svg>"
        )
    };
    let render = |svg: String| -> Vec<u8> {
        let opts = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_str(&svg, &opts).unwrap();
        let mut pm = resvg::tiny_skia::Pixmap::new(200, 60).unwrap();
        let mut pmref = pm.as_mut();
        resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pmref);
        pm.encode_png().unwrap()
    };
    let a = render(mk("Inconsolata"));
    let b = render(mk("ThisFontDoesNotExist12345"));
    println!(
        "inconsolata {}B vs fallback {}B -> {}",
        a.len(),
        b.len(),
        if a == b { "IDENTICAL (font NOT found)" } else { "differ (font found)" }
    );
}
