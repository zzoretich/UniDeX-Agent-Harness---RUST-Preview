//! Finding `unidex://` links in the arguments the app was started with.
//!
//! On macOS a link reaches a running app through `Application::on_open_urls`, which must be registered before `run`.
//! On Linux and Windows GPUI stores that callback and never calls it; the desktop or the shell starts the app with the
//! link as an argument instead, and handing it to an instance that is already running is the app's own job (decision
//! 0010, E19 and E20).

/// The scheme the application declares.
pub const SCHEME: &str = "unidex";

/// Whether `arg` is a link in [`SCHEME`] (any case) with something after the colon.
pub fn is_link(arg: &str) -> bool {
    // `get` rather than slicing: an argument that is not ASCII where the scheme would end must not panic.
    let scheme = arg.get(..SCHEME.len());
    let rest = arg.get(SCHEME.len()..);
    match (scheme, rest) {
        (Some(scheme), Some(rest)) => {
            scheme.eq_ignore_ascii_case(SCHEME) && rest.len() > 1 && rest.starts_with(':')
        }
        _ => false,
    }
}

/// The links among `args`, in order.
pub fn links_in_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    args.into_iter().filter(|arg| is_link(arg)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_links_in_any_case() {
        assert!(is_link("unidex://thread/42?from=test"));
        assert!(is_link("UNIDEX://thread/42"));
        assert!(is_link("unidex:x"));
    }

    #[test]
    fn rejects_everything_else_without_panicking() {
        for arg in [
            "",
            "unidex",
            "unidex:",
            "unidexx://thread",
            "https://example.com",
            "--flag",
            // Not ASCII where the scheme ends: slicing by bytes there would panic.
            "uni😀dex://",
            "ünidex://x",
        ] {
            assert!(!is_link(arg), "{arg:?}");
        }
    }

    #[test]
    fn keeps_only_links_in_order() {
        let args = ["--verbose", "unidex://a", "file.txt", "UNIDEX://b"].map(String::from);
        assert_eq!(links_in_args(args), vec!["unidex://a", "UNIDEX://b"]);
    }
}
