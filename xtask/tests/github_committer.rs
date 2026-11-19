use xtask::privacy::{RuleId, Terms, scan_text};

fn rules(text: &str) -> Vec<RuleId> {
    scan_text("f.md", text, &Terms::generic())
        .into_iter()
        .map(|f| f.rule)
        .collect()
}

// The hosting service's own no-reply address is a real domain on purpose: the
// rule under test is about exactly that address and its look-alikes.
#[test]
fn githubs_own_committer_address_is_clean() {
    assert!(rules("committer noreply@github.com").is_empty());
}

#[test]
fn only_that_exact_address_is_allowed() {
    for bad in [
        "someone@github.com",
        "noreply@github.com.example.test",
        "xnoreply@github.com",
    ] {
        assert!(rules(bad).contains(&RuleId::Email), "{bad}");
    }
}
