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
fn lemmy_spoiler_no_title() {
    html_opts!(
        [extension.lemmy_spoiler],
        concat!(
            ":::spoiler\n",
            "Hidden\n",
            ":::\n",
        ),
        concat!(
            "<details>\n",
            "<p>Hidden</p>\n",
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
