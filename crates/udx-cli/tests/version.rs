use assert_cmd::Command;

#[test]
fn prints_plain_version() {
    Command::cargo_bin("unidex")
        .unwrap()
        .arg("--version")
        .assert()
        .success()
        .stdout("unidex 0.0.1\n");
}

#[test]
fn prints_json_version() {
    Command::cargo_bin("unidex")
        .unwrap()
        .args(["--version", "--json"])
        .assert()
        .success()
        .stdout("{\"name\":\"unidex\",\"version\":\"0.0.1\"}\n");
}

#[test]
fn unknown_flag_is_a_usage_error() {
    Command::cargo_bin("unidex")
        .unwrap()
        .arg("--nope")
        .assert()
        .code(2);
}
