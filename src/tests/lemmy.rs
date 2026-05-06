use super::*;

#[test]
fn lemmy_mention_user() {
    html_opts!(
        [extension.lemmy_mention],
        "@user@example.com\n",
        "<p><a href=\"https://example.com/u/user\">@user@example.com</a></p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_user_in_sentence() {
    html_opts!(
        [extension.lemmy_mention],
        "Hello @user@example.com how are you?\n",
        "<p>Hello <a href=\"https://example.com/u/user\">@user@example.com</a> how are you?</p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_user_subdomain() {
    html_opts!(
        [extension.lemmy_mention],
        "@user@lemmy.ml\n",
        "<p><a href=\"https://lemmy.ml/u/user\">@user@lemmy.ml</a></p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_user_underscores() {
    html_opts!(
        [extension.lemmy_mention],
        "@my_user@example.com\n",
        "<p><a href=\"https://example.com/u/my_user\">@my_user@example.com</a></p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_not_in_code() {
    html_opts!(
        [extension.lemmy_mention],
        "`@user@example.com`\n",
        "<p><code>@user@example.com</code></p>\n",
    );

    html_opts!(
        [extension.lemmy_mention],
        "```\n@user@example.com\n```\n",
        "<pre><code>@user@example.com\n</code></pre>\n",
    );

    html_opts!(
        [extension.lemmy_mention],
        "`!community@example.com`\n",
        "<p><code>!community@example.com</code></p>\n",
    );
}

#[test]
fn lemmy_mention_not_after_alphanumeric() {
    html_opts!(
        [extension.lemmy_mention],
        "email@user@example.com\n",
        "<p>email@user@example.com</p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_not_in_link() {
    html_opts!(
        [extension.lemmy_mention],
        "[@user@example.com](https://example.com)\n",
        "<p><a href=\"https://example.com\">@user@example.com</a></p>\n",
    );
}

#[test]
fn lemmy_mention_name_length() {
    // Too short (< 3 chars)
    html_opts!(
        [extension.lemmy_mention],
        "@ab@example.com\n",
        "<p>@ab@example.com</p>\n",
    );

    // Minimum (3 chars)
    html_opts!(
        [extension.lemmy_mention],
        "@abc@example.com\n",
        "<p><a href=\"https://example.com/u/abc\">@abc@example.com</a></p>\n",
        no_roundtrip,
    );

    // Too long (> 20 chars)
    html_opts!(
        [extension.lemmy_mention],
        "@abcdefghijklmnopqrstu@example.com\n",
        "<p>@abcdefghijklmnopqrstu@example.com</p>\n",
    );
}

#[test]
fn lemmy_mention_invalid_domain() {
    html_opts!(
        [extension.lemmy_mention],
        "@user\n",
        "<p>@user</p>\n",
    );

    html_opts!(
        [extension.lemmy_mention],
        "@user@localhost\n",
        "<p>@user@localhost</p>\n",
    );
}

#[test]
fn lemmy_mention_with_port() {
    html_opts!(
        [extension.lemmy_mention],
        "@user@lemmy-alpha:8541\n",
        "<p><a href=\"https://lemmy-alpha:8541/u/user\">@user@lemmy-alpha:8541</a></p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_multiple() {
    html_opts!(
        [extension.lemmy_mention],
        "@alice@one.com and @bob@two.org\n",
        "<p><a href=\"https://one.com/u/alice\">@alice@one.com</a> and <a href=\"https://two.org/u/bob\">@bob@two.org</a></p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_community() {
    html_opts!(
        [extension.lemmy_mention],
        "!community@example.com\n",
        "<p><a href=\"https://example.com/c/community\">!community@example.com</a></p>\n",
        no_roundtrip,
    );

    html_opts!(
        [extension.lemmy_mention],
        "Check out !linux@lemmy.ml for news\n",
        "<p>Check out <a href=\"https://lemmy.ml/c/linux\">!linux@lemmy.ml</a> for news</p>\n",
        no_roundtrip,
    );

    html_opts!(
        [extension.lemmy_mention],
        "!tech@discuss.tchncs.de\n",
        "<p><a href=\"https://discuss.tchncs.de/c/tech\">!tech@discuss.tchncs.de</a></p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_community_not_image() {
    html_opts!(
        [extension.lemmy_mention],
        "![alt text](https://example.com/img.png)\n",
        "<p><img src=\"https://example.com/img.png\" alt=\"alt text\" /></p>\n",
    );
}

#[test]
fn lemmy_mention_mixed() {
    html_opts!(
        [extension.lemmy_mention],
        "@user@lemmy.ml posted in !community@lemmy.world\n",
        "<p><a href=\"https://lemmy.ml/u/user\">@user@lemmy.ml</a> posted in <a href=\"https://lemmy.world/c/community\">!community@lemmy.world</a></p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_with_surrounding_markdown() {
    html_opts!(
        [extension.lemmy_mention],
        "**@user@example.com** said hello\n",
        "<p><strong><a href=\"https://example.com/u/user\">@user@example.com</a></strong> said hello</p>\n",
        no_roundtrip,
    );
}

#[test]
fn lemmy_mention_bare_triggers() {
    html_opts!(
        [extension.lemmy_mention],
        "@ not a mention\n",
        "<p>@ not a mention</p>\n",
    );

    html_opts!(
        [extension.lemmy_mention],
        "! not a mention\n",
        "<p>! not a mention</p>\n",
    );
}

#[test]
fn lemmy_mention_disabled() {
    html_opts!(
        [extension.lemmy_mention = false],
        "@user@example.com\n",
        "<p>@user@example.com</p>\n",
    );
}

#[test]
fn lemmy_spoiler() {
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            ":::spoiler Click to reveal\n",
            "Hidden content\n",
            ":::\n",
        ),
        concat!(
            "<details>\n",
            "<summary>Click to reveal</summary>\n",
            "<p>Hidden content</p>\n",
            "</details>\n",
        ),
        no_roundtrip,
    );
}

#[test]
fn lemmy_spoiler_multiline() {
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            ":::spoiler Title\n",
            "Line one\n",
            "\n",
            "Line two\n",
            ":::\n",
        ),
        concat!(
            "<details>\n",
            "<summary>Title</summary>\n",
            "<p>Line one</p>\n",
            "<p>Line two</p>\n",
            "</details>\n",
        ),
        no_roundtrip,
    );
}

#[test]
fn lemmy_spoiler_with_markdown() {
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            ":::spoiler Details\n",
            "**Bold** and *italic* inside\n",
            ":::\n",
        ),
        concat!(
            "<details>\n",
            "<summary>Details</summary>\n",
            "<p><strong>Bold</strong> and <em>italic</em> inside</p>\n",
            "</details>\n",
        ),
        no_roundtrip,
    );
}

#[test]
fn lemmy_spoiler_not_in_code_block() {
    html_opts!(
        [extension.lemmy_spoiler],
        "```\n:::spoiler Title\nContent\n:::\n```\n",
        "<pre><code>:::spoiler Title\nContent\n:::\n</code></pre>\n",
    );
}

#[test]
fn lemmy_spoiler_space_after_colons() {
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            "::: spoiler Click me\n",
            "Hidden\n",
            ":::\n",
        ),
        concat!(
            "<details>\n",
            "<summary>Click me</summary>\n",
            "<p>Hidden</p>\n",
            "</details>\n",
        ),
        no_roundtrip,
    );
}

#[test]
fn lemmy_spoiler_no_title_rejected() {
    // Lemmy's `^spoiler\s+(.*)$` requires a non-empty title; without one the
    // opener falls through and the lines render as plain text.
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            ":::spoiler\n",
            "Hidden\n",
            ":::\n",
        ),
        concat!(
            "<p>:::spoiler\n",
            "Hidden\n",
            ":::</p>\n",
        ),
        no_roundtrip,
    );
}

#[test]
fn lemmy_spoiler_keyword_must_have_whitespace_boundary() {
    // `:::spoilers x` looks like "spoiler" followed by `s x`; without a
    // whitespace boundary after the keyword Lemmy rejects it.
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            "::: spoilers something\n",
            "Hidden\n",
            ":::\n",
        ),
        concat!(
            "<p>::: spoilers something\n",
            "Hidden\n",
            ":::</p>\n",
        ),
        no_roundtrip,
    );
}

#[test]
fn lemmy_spoiler_whitespace_only_title_rejected() {
    // Title that's only whitespace trims to empty -> reject.
    // (The trailing run of spaces becomes a markdown hard break.)
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            "::: spoiler   \n",
            "Hidden\n",
            ":::\n",
        ),
        concat!(
            "<p>::: spoiler<br />\n",
            "Hidden\n",
            ":::</p>\n",
        ),
        no_roundtrip,
    );
}

#[test]
fn lemmy_spoiler_title_with_trailing_colon() {
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            "::: spoiler twitter bio now:\n",
            "Hidden\n",
            ":::\n",
        ),
        concat!(
            "<details>\n",
            "<summary>twitter bio now:</summary>\n",
            "<p>Hidden</p>\n",
            "</details>\n",
        ),
        no_roundtrip,
    );
}

#[test]
fn lemmy_spoiler_title_with_internal_colon() {
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            "::: spoiler about: stuff\n",
            "Hidden\n",
            ":::\n",
        ),
        concat!(
            "<details>\n",
            "<summary>about: stuff</summary>\n",
            "<p>Hidden</p>\n",
            "</details>\n",
        ),
        no_roundtrip,
    );
}

// Real-world comment from lemmy.world/post/46436930/23571125: spoiler with a
// title that ends in `:`, followed by an image whose alt text spans many lines.
#[test]
fn lemmy_spoiler_realworld_trailing_colon_with_multiline_image() {
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            "::: spoiler and it's also his twitter bio now:\n",
            "![screenshot of twitter profile\n",
            "Aaron Abernethy\n",
            "@theronster\n",
            "](https://lemmy.ml/pictrs/image/a7e4edb1-8b21-46a2-b248-f69c04cf481c.png)\n",
            ":::\n",
        ),
        concat!(
            "<details>\n",
            "<summary>and it's also his twitter bio now:</summary>\n",
            "<p><img src=\"https://lemmy.ml/pictrs/image/a7e4edb1-8b21-46a2-b248-f69c04cf481c.png\" alt=\"screenshot of twitter profile Aaron Abernethy @theronster \" /></p>\n",
            "</details>\n",
        ),
        no_roundtrip,
    );
}

#[test]
fn lemmy_spoiler_not_other_directives() {
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            ":::warning\n",
            "Not a spoiler\n",
            ":::\n",
        ),
        concat!(
            "<p>:::warning\n",
            "Not a spoiler\n",
            ":::</p>\n",
        ),
    );
}
