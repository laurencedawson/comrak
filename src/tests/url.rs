use crate::nodes::NodeLink;

fn check(url: &str, expected: &str) {
    let nl = NodeLink {
        url: url.into(),
        title: String::new(),
    };
    assert_eq!(nl.cleaned_url().as_ref(), expected, "\ninput: {url:?}");
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
