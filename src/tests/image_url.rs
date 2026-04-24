//! Tests for `crate::image_url::is_image_url`.

use crate::image_url::is_image_url;

#[test]
fn extensions() {
    assert!(is_image_url("https://example.com/photo.jpg"));
    assert!(is_image_url("https://example.com/photo.PNG"));
    assert!(is_image_url("https://example.com/photo.webp"));
    assert!(!is_image_url("https://example.com/page.html"));
    assert!(!is_image_url("https://example.com/"));
}

#[test]
fn known_hosts() {
    assert!(is_image_url("https://i.redd.it/abc123.jpg"));
    assert!(is_image_url("https://i.imgur.com/abc123"));
    assert!(is_image_url("https://pbs.twimg.com/media/abc123"));
    assert!(!is_image_url("https://i.imgur.com/a/abc123"));
    assert!(!is_image_url("https://i.imgur.com/gallery/abc123"));
}

#[test]
fn image_paths() {
    assert!(is_image_url("https://lemmy.ml/pictrs/image/abc123"));
    assert!(is_image_url("https://lemmy.ml/api/v3/image_proxy?url=test"));
}

#[test]
fn query_param() {
    assert!(is_image_url("https://proxy.example.com/?url=https://example.com/img.png"));
}

#[test]
fn no_scheme() {
    assert!(!is_image_url("example.com/photo.jpg"));
}
