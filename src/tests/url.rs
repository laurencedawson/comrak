use crate::parser::url::resolve_url;

fn check(url: &str, expected: &str) {
    assert_eq!(resolve_url(url).as_ref(), expected, "\ninput: {url:?}");
}

fn unchanged(url: &str) {
    check(url, url);
}

#[test]
fn google_redirect() {
    check(
        "https://www.google.com/url?q=https%3A%2F%2Fexample.com&sa=U",
        "https://example.com",
    );
}

#[test]
fn google_amp() {
    check(
        "https://www.google.com/amp/s/example.com/article",
        "https://example.com/article",
    );
}

#[test]
fn youtube_redirect() {
    check(
        "https://www.youtube.com/redirect?q=https%3A%2F%2Fexample.com",
        "https://example.com",
    );
}

#[test]
fn youtube_short() {
    check(
        "https://youtu.be/dQw4w9WgXcQ",
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ",
    );
}

#[test]
fn youtube_mobile() {
    check(
        "https://m.youtube.com/watch?v=abc123",
        "https://www.youtube.com/watch?v=abc123",
    );
}

#[test]
fn facebook_redirect() {
    check(
        "https://l.facebook.com/l.php?u=https%3A%2F%2Fexample.com&h=abc",
        "https://example.com",
    );
}

#[test]
fn ddg_image_proxy() {
    check(
        "https://external-content.duckduckgo.com/iu/?u=https%3A%2F%2Fexample.com%2Fimg.png",
        "https://example.com/img.png",
    );
}

#[test]
fn discord_image_proxy() {
    check(
        "https://images-ext-1.discordapp.net/external/abc?url=https%3A%2F%2Fexample.com%2Fimg.png",
        "https://example.com/img.png",
    );
}

#[test]
fn skimresources() {
    check(
        "https://go.skimresources.com/?id=123&url=https%3A%2F%2Fexample.com",
        "https://example.com",
    );
}

#[test]
fn voyager() {
    check(
        "https://vger.to/lemmy.ml/post/123",
        "https://lemmy.ml/post/123",
    );
}

#[test]
fn lemmy_image_proxy() {
    check(
        "https://lemmy.ml/api/v3/image_proxy?url=https%3A%2F%2Fexample.com%2Fimg.png",
        "https://example.com/img.png",
    );
}

#[test]
fn lemmy_v4_image_proxy() {
    check(
        "https://lemmy.ml/api/v4/image/proxy?url=https%3A%2F%2Fexample.com%2Fimg.png",
        "https://example.com/img.png",
    );
}

#[test]
fn image_proxy_preserves_literal_plus() {
    // RFC 3986 decoding: a raw + in the query is a literal +, not a space.
    check(
        "https://lemmy.ml/api/v3/image_proxy?url=https://files.catbox.moe/a+b.png",
        "https://files.catbox.moe/a+b.png",
    );
    check(
        "https://lemmy.ml/api/v3/image_proxy?url=https%3A%2F%2Ffiles.catbox.moe%2Fa%2Bb.png",
        "https://files.catbox.moe/a+b.png",
    );
}

#[test]
fn image_proxy_unusable_inner_url_kept_wrapped() {
    // %20 decodes to a raw space — not a loadable URL. Keep the proxy URL,
    // which still serves the image.
    unchanged("https://lemmy.ml/api/v3/image_proxy?url=https%3A%2F%2Fx.com%2Fmy%20pic.png");
    // Control char (%00) likewise.
    unchanged("https://lemmy.ml/api/v3/image_proxy?url=https%3A%2F%2Fx.com%2Fa%00b.png");
}

#[test]
fn image_proxy_undecodable_inner_url_kept_wrapped() {
    // Malformed percent sequence: strict RFC 3986 decoding rejects it.
    unchanged("https://lemmy.ml/api/v3/image_proxy?url=https%3A%2F%2Fx.com%2Fa%2Gb.png");
    // Decodes to invalid UTF-8.
    unchanged("https://lemmy.ml/api/v3/image_proxy?url=https%3A%2F%2Fx.com%2Fa%FFb.png");
}

#[test]
fn redirect_preserves_literal_plus() {
    check(
        "https://www.google.com/url?q=https://example.com/a+b",
        "https://example.com/a+b",
    );
}

#[test]
fn pictrs_thumbnailed() {
    check(
        "https://lemmy.world/pictrs/image/abc.jpeg",
        "https://lemmy.world/pictrs/image/abc.jpeg?thumbnail=400&format=webp",
    );
}

#[test]
fn pictrs_existing_query_replaced() {
    check(
        "https://x.tld/pictrs/image/abc.png?thumbnail=800&format=webp",
        "https://x.tld/pictrs/image/abc.png?thumbnail=400&format=webp",
    );
}

#[test]
fn pictrs_gif_unchanged() {
    unchanged("https://x.tld/pictrs/image/abc.gif");
    unchanged("https://x.tld/pictrs/image/abc.gif?thumbnail=800");
}

#[test]
fn proxied_pictrs_unwrapped_then_thumbnailed() {
    check(
        "https://lemmy.zip/api/v3/image_proxy?url=https%3A%2F%2Fother.tld%2Fpictrs%2Fimage%2Fabc.jpeg",
        "https://other.tld/pictrs/image/abc.jpeg?thumbnail=400&format=webp",
    );
}

#[test]
fn pictrs_video_not_thumbnailed() {
    unchanged("https://lemmy.world/pictrs/image/abc.mp4");
    unchanged("https://lemmy.world/pictrs/image/abc.webm");
}

#[test]
fn passthrough() {
    unchanged("https://example.com");
}

#[test]
fn no_scheme() {
    unchanged("example.com/page");
}

#[test]
fn plain_google() {
    unchanged("https://www.google.com/search?q=rust");
}
