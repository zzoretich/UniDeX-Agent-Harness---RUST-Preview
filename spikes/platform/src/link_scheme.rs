//! The `unidex://` link scheme: the metadata each OS needs before it hands such a link to the app, and how the app
//! finds a link it was started with.
//!
//! Nothing here registers anything. `--write-metadata <dir>` writes the three files; the committed copies live in
//! `bundle/` and a test keeps them equal to what this module generates.
//!
//! How a link reaches the app, from the pinned GPUI source (decision 0010 has the references):
//! - macOS: LaunchServices reads `CFBundleURLTypes` from the bundle's Info.plist. A click on a link starts the app,
//!   or, if it is already running, sends the link to the running instance; either way AppKit calls the delegate's
//!   `application:openURLs:`, which GPUI turns into the `App::on_open_urls` callback.
//! - Linux: the desktop entry's `MimeType=x-scheme-handler/unidex;` makes it a handler; the desktop starts
//!   `Exec=... %u` with the link as an argument. GPUI stores an `on_open_urls` callback on Linux but never calls it,
//!   so the app reads its arguments, and a second launch must hand the link to the running instance itself.
//! - Windows: `HKCU\Software\Classes\unidex` with an empty `URL Protocol` value and a `shell\open\command` makes the
//!   app the handler; Windows starts it with the link as an argument. As on Linux, GPUI never calls
//!   `on_open_urls`, and the hand-off to a running instance is the app's job.

use std::path::Path;

pub const SCHEME: &str = "unidex";
pub const BUNDLE_ID: &str = "com.example.unidex-platform-trial";
pub const APP_NAME: &str = "UniDeX Platform Trial";
pub const EXECUTABLE: &str = "platform-trial";

pub const INFO_PLIST_FILE: &str = "Info.plist";
pub const DESKTOP_FILE: &str = "unidex-platform-trial.desktop";
pub const REG_FILE: &str = "unidex-scheme.reg";

/// macOS: the bundle's Info.plist. `CFBundleURLTypes` is what declares the scheme.
pub fn info_plist() -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleExecutable</key>
	<string>{EXECUTABLE}</string>
	<key>CFBundleIdentifier</key>
	<string>{BUNDLE_ID}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>{APP_NAME}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>0.0.0</string>
	<key>CFBundleVersion</key>
	<string>0</string>
	<key>LSMinimumSystemVersion</key>
	<string>13.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>CFBundleURLTypes</key>
	<array>
		<dict>
			<key>CFBundleURLName</key>
			<string>{BUNDLE_ID}</string>
			<key>CFBundleTypeRole</key>
			<string>Viewer</string>
			<key>CFBundleURLSchemes</key>
			<array>
				<string>{SCHEME}</string>
			</array>
		</dict>
	</array>
</dict>
</plist>
"#
    )
}

/// Linux: a desktop entry. `MimeType=x-scheme-handler/unidex;` declares the scheme and `%u` passes the link.
/// `StartupWMClass` matches the app id GPUI sets on the window, so the desktop groups the window with this entry.
pub fn desktop_entry() -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.5\n\
         Name={APP_NAME}\n\
         Comment=Throwaway trial of platform integration\n\
         Exec={EXECUTABLE} %u\n\
         Terminal=false\n\
         NoDisplay=true\n\
         Categories=Development;\n\
         MimeType=x-scheme-handler/{SCHEME};\n\
         StartupWMClass={EXECUTABLE}\n"
    )
}

/// Windows: the per-user registry keys an installer would write. The install path is a placeholder.
pub fn registry_file() -> String {
    let exe = r#"C:\\Program Files\\UniDeX Platform Trial\\platform-trial.exe"#;
    format!(
        "Windows Registry Editor Version 5.00\r\n\
         \r\n\
         [HKEY_CURRENT_USER\\Software\\Classes\\{SCHEME}]\r\n\
         @=\"URL:UniDeX link\"\r\n\
         \"URL Protocol\"=\"\"\r\n\
         \r\n\
         [HKEY_CURRENT_USER\\Software\\Classes\\{SCHEME}\\DefaultIcon]\r\n\
         @=\"\\\"{exe}\\\",0\"\r\n\
         \r\n\
         [HKEY_CURRENT_USER\\Software\\Classes\\{SCHEME}\\shell\\open\\command]\r\n\
         @=\"\\\"{exe}\\\" \\\"%1\\\"\"\r\n"
    )
}

pub fn write_all(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join(INFO_PLIST_FILE), info_plist())?;
    std::fs::write(dir.join(DESKTOP_FILE), desktop_entry())?;
    std::fs::write(dir.join(REG_FILE), registry_file())?;
    Ok(())
}

/// The links among the command-line arguments: how Linux and Windows deliver one.
pub fn links_in_args<I: IntoIterator<Item = String>>(args: I) -> Vec<String> {
    args.into_iter().filter(|a| is_link(a)).collect()
}

pub fn is_link(s: &str) -> bool {
    s.len() > SCHEME.len() + 1
        && s[..SCHEME.len()].eq_ignore_ascii_case(SCHEME)
        && s[SCHEME.len()..].starts_with(':')
}

/// Checks that need no window, run by `--headless` (and so by CI on every platform).
pub fn headless_checks() -> Vec<(String, bool, String)> {
    let mut out = Vec::new();
    let plist = info_plist();
    out.push((
        "Info.plist declares the scheme under CFBundleURLTypes".to_string(),
        plist.contains("<key>CFBundleURLSchemes</key>")
            && plist.contains(&format!("<string>{SCHEME}</string>")),
        format!("bundle id {BUNDLE_ID}"),
    ));
    let desktop = desktop_entry();
    out.push((
        "desktop entry declares x-scheme-handler/unidex and passes %u".to_string(),
        desktop.contains("MimeType=x-scheme-handler/unidex;") && desktop.contains(" %u\n"),
        "validated with desktop-file-validate in CI on Linux".to_string(),
    ));
    let reg = registry_file();
    out.push((
        "registry file has URL Protocol and shell\\open\\command with %1".to_string(),
        reg.contains("\"URL Protocol\"=\"\"")
            && reg.contains("shell\\open\\command]")
            && reg.contains("%1"),
        "documentation of the keys an installer writes; not imported".to_string(),
    ));
    let args = [
        "--flag",
        "unidex://thread/42?x=1",
        "UNIDEX:open",
        "https://example.com",
        "unidex",
    ];
    let links = links_in_args(args.iter().map(|s| s.to_string()));
    out.push((
        "links are found among the arguments (Linux and Windows delivery)".to_string(),
        links == ["unidex://thread/42?x=1", "UNIDEX:open"],
        format!("{links:?}"),
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_metadata_matches_the_generator() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("bundle");
        for (file, expected) in [
            (INFO_PLIST_FILE, info_plist()),
            (DESKTOP_FILE, desktop_entry()),
            (REG_FILE, registry_file()),
        ] {
            let committed = std::fs::read_to_string(dir.join(file)).expect(file);
            assert_eq!(
                committed, expected,
                "{file} differs; run --write-metadata bundle"
            );
        }
    }

    #[test]
    fn headless_checks_pass() {
        for (name, ok, detail) in headless_checks() {
            assert!(ok, "{name}: {detail}");
        }
    }
}
