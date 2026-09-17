// dump what fontdb actually registered: every face's family names
fn main() {
    let mut db = resvg::usvg::fontdb::Database::new();
    db.load_system_fonts();
    db.load_font_data(std::fs::read("src/assets/fonts/SourceSans3.ttf").unwrap());
    db.load_font_data(std::fs::read("src/assets/fonts/Inconsolata.ttf").unwrap());
    let mut rows: Vec<String> = db
        .faces()
        .map(|f| format!("  {:?}", f.families))
        .collect();
    rows.sort();
    rows.dedup();
    println!("{} distinct family entries:", rows.len());
    for r in rows.iter().filter(|r| r.to_lowercase().contains("source") || r.to_lowercase().contains("incon") || r.to_lowercase().contains("dejavu") || r.to_lowercase().contains("liberation")) {
        println!("{r}");
    }
    println!("--- query test ---");
    for q in ["Source Sans 3", "Inconsolata", "DejaVu Sans", "Liberation Sans"] {
        let hit = db.query(&resvg::usvg::fontdb::Query { families: &[resvg::usvg::fontdb::Family::Name(q)], weight: Default::default(), stretch: Default::default(), style: Default::default() });
        println!("query {q:?} -> {}", if hit.is_some() { "HIT" } else { "miss" });
    }
}
