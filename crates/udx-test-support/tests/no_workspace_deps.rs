//! A 2.2: `udx-test-support` depends on no workspace crate, so `udx-core`'s dev-dependency on it closes no cycle.

#[test]
fn test_support_depends_on_no_workspace_crate() {
    let manifest =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap();
    let mut table = String::new();
    let mut named = Vec::new();
    for line in manifest.lines() {
        // Quotes and spaces never change which crate a line names.
        let unquoted: String = line
            .chars()
            .filter(|c| !matches!(c, '"' | '\'' | ' ' | '\t'))
            .collect();
        let l = unquoted.as_str();
        if l.starts_with('[') {
            table = l.to_string();
        } else if table.contains("dependencies")
            && (l.starts_with("udx-") || l.contains("package=udx-"))
        {
            named.push(l.to_string());
        }
        if table.contains("dependencies.udx-") {
            named.push(table.clone());
        }
    }
    assert!(
        named.is_empty(),
        "workspace crates named as dependencies: {named:?}"
    );
}
