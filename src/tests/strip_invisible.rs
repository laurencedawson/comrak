use super::*;

#[test]
fn strip_invisible_zero_width() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{200b}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_bom() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{feff}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_word_joiner() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{2060}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_math_operators() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{2061}\u{2062}\u{2063}\u{2064}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_soft_hyphen() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{00ad}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_combining_grapheme_joiner() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{034f}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_mongolian_vowel_separator() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{180e}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_bidi_embedding_controls() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{202a}\u{202b}\u{202c}\u{202d}\u{202e}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_bidi_isolate_controls() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{2066}\u{2067}\u{2068}\u{2069}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_zwnj() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{200c}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_arabic_letter_mark() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{061c}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_ltr_rtl_marks() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{200e}\u{200f}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_variation_selectors() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{fe00}\u{fe0e}b\n",
        "<p>ab</p>\n",
        no_roundtrip,
    );
}

#[test]
fn strip_invisible_preserves_zwj() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{200d}b\n",
        "<p>a\u{200d}b</p>\n",
    );
}

#[test]
fn strip_invisible_preserves_vs16() {
    html_opts!(
        [parse.strip_invisible],
        "a\u{fe0f}b\n",
        "<p>a\u{fe0f}b</p>\n",
    );
}

#[test]
fn strip_invisible_clean_input() {
    html_opts!(
        [parse.strip_invisible],
        "hello world\n",
        "<p>hello world</p>\n",
    );
}

#[test]
fn strip_invisible_disabled() {
    html_opts!(
        [parse.strip_invisible = false],
        "he\u{200b}llo\n",
        "<p>he\u{200b}llo</p>\n",
    );
}

#[test]
fn strip_invisible_massive_payload() {
    html_opts!(
        [parse.strip_invisible],
        "\u{2063}\u{feff}\u{2064}\u{2062}T\u{200b}\u{2061}e\u{fe00}st\n",
        "<p>Test</p>\n",
        no_roundtrip,
    );
}
