use assert_cmd::Command;

#[test]
fn verify_mdbook_cli_supports_html() {
    let mut cmd = Command::cargo_bin("mdbook-classy").unwrap();
    cmd.args(["supports", "html"]).assert().success();
}

#[test]
fn verify_mdbook_cli_no_support_pdf() {
    let mut cmd = Command::cargo_bin("mdbook-classy").unwrap();
    cmd.args(["supports", "pdf"]).assert().failure();
}

#[test]
fn verify_mdbook_cli_preprocessor_no_book() {
    let mut cmd = Command::cargo_bin("mdbook-classy").unwrap();
    cmd.assert().failure();
}
