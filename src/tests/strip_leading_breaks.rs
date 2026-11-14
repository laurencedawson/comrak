use super::*;

#[test]
fn strip_leading_breaks_basic() {
    html_opts!(
        [parse.sanitize_input],
        "\\\n![alt](url)\n",
        "<p><img src=\"url\" alt=\"alt\" /></p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_leading_breaks_multiple() {
    html_opts!(
        [parse.sanitize_input],
        "\\\n\\\n![alt](url)\n",
        "<p><img src=\"url\" alt=\"alt\" /></p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_leading_breaks_with_whitespace() {
    html_opts!(
        [parse.sanitize_input],
        "  \\\n  ![alt](url)\n",
        "<p><img src=\"url\" alt=\"alt\" /></p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_leading_breaks_crlf() {
    html_opts!(
        [parse.sanitize_input],
        "\\\r\nfoo\n",
        "<p>foo</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_leading_breaks_preserves_inner() {
    html_opts!(
        [parse.sanitize_input],
        "foo\\\nbar\n",
        "<p>foo<br />\nbar</p>\n",
    );
}

#[test]
fn strip_leading_breaks_preserves_escape() {
    html_opts!(
        [parse.sanitize_input],
        "\\*not italic\\*\n",
        "<p>*not italic*</p>\n",
    );
}

#[test]
fn strip_leading_breaks_disabled() {
    html_opts!(
        [parse.sanitize_input = false],
        "\\\nfoo\n",
        "<p><br />\nfoo</p>\n",
    );
}

#[test]
fn strip_leading_breaks_clean_input() {
    html_opts!(
        [parse.sanitize_input],
        "hello world\n",
        "<p>hello world</p>\n",
    );
}
