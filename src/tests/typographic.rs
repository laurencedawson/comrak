use super::*;

#[test]
fn smart_copyright() {
    html_opts!(
        [parse.smart],
        "(c) and (C)\n",
        "<p>\u{a9} and \u{a9}</p>\n",
        no_roundtrip,
    );
}

#[test]
fn smart_registered() {
    html_opts!(
        [parse.smart],
        "(r) and (R)\n",
        "<p>\u{ae} and \u{ae}</p>\n",
        no_roundtrip,
    );
}

#[test]
fn smart_trademark() {
    html_opts!(
        [parse.smart],
        "(tm) and (TM)\n",
        "<p>\u{2122} and \u{2122}</p>\n",
        no_roundtrip,
    );
}

#[test]
fn smart_plus_minus() {
    html_opts!(
        [parse.smart],
        "5+-2\n",
        "<p>5\u{b1}2</p>\n",
        no_roundtrip,
    );
}

#[test]
fn smart_cap_question_marks() {
    html_opts!(
        [parse.smart],
        "what????\n",
        "<p>what???</p>\n",
        no_roundtrip,
    );
}

#[test]
fn smart_cap_exclamation_marks() {
    html_opts!(
        [parse.smart],
        "wow!!!!\n",
        "<p>wow!!!</p>\n",
        no_roundtrip,
    );
}

#[test]
fn smart_cap_commas() {
    html_opts!(
        [parse.smart],
        "no,,really\n",
        "<p>no,really</p>\n",
        no_roundtrip,
    );
}

#[test]
fn smart_no_cap_below_threshold() {
    html_opts!(
        [parse.smart],
        "what??? and wow!!! and ok,\n",
        "<p>what??? and wow!!! and ok,</p>\n",
        no_roundtrip,
    );
}

#[test]
fn smart_typographic_in_sentence() {
    html_opts!(
        [parse.smart],
        "Copyright (c) 2024\n",
        "<p>Copyright \u{a9} 2024</p>\n",
        no_roundtrip,
    );
}

#[test]
fn smart_typographic_not_in_code() {
    html_opts!(
        [parse.smart],
        "`(c)` and `+-`\n",
        "<p><code>(c)</code> and <code>+-</code></p>\n",
    );
}

#[test]
fn smart_typographic_not_in_code_block() {
    html_opts!(
        [parse.smart],
        "```\n(c) (r) (tm) +-\n```\n",
        "<pre><code>(c) (r) (tm) +-\n</code></pre>\n",
    );
}

#[test]
fn smart_typographic_disabled() {
    html_opts!(
        [parse.smart = false],
        "(c) (r) (tm) +-\n",
        "<p>(c) (r) (tm) +-</p>\n",
    );
}

#[test]
fn smart_typographic_with_multibyte() {
    html_opts!(
        [parse.smart],
        "\u{597d}(c)\u{597d}\n",
        "<p>\u{597d}\u{a9}\u{597d}</p>\n",
        no_roundtrip,
    );
}
