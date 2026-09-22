//! sims/ is a repo-local test corpus — loaded from disk at runtime, never
//! embedded in the binary and never shipped in releases. sims/*.json stay the
//! on-disk source of truth; sim commands work from a dynamic-diagram checkout.

pub fn get(name: &str) -> Option<String> {
    if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return None; // keep lookups inside sims/
    }
    std::fs::read_to_string(std::path::Path::new("sims").join(format!("{name}.json"))).ok()
}
