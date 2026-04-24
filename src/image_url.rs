//! Heuristic URL-to-image detection.
//!
//! Used by the blob renderer to convert bare autolinks pointing at image
//! resources into inline image embeds, and exposed to callers so they can
//! make the same decision at their own layer.

/// Strip `http://` / `https://`, returning the rest of the URL.
fn strip_scheme(url: &str) -> Option<&str> {
    url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))
}

/// Returns `true` if the URL points to an image (by known host, path pattern,
/// or file extension). Conservative: may return false for actual images with
/// unusual hosting, never true for non-image URLs.
pub fn is_image_url(url: &str) -> bool {
    let rest = match strip_scheme(url) {
        Some(r) => r,
        None => return false,
    };

    if IMAGE_HOSTS.iter().any(|h| rest.starts_with(h)) {
        return !IMAGE_HOST_EXCLUDED.iter().any(|p| rest.starts_with(p));
    }

    if IMAGE_PATHS.iter().any(|p| rest.contains(p)) {
        return true;
    }

    let path = rest.split(['?', '#']).next().unwrap_or(rest);
    for segment in path.split('/') {
        if let Some((_, ext)) = segment.rsplit_once('.') {
            if IMAGE_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
                return true;
            }
        }
    }

    if let Some(qs) = rest.split_once('?').map(|(_, q)| q) {
        for pair in qs.split('&') {
            let value = pair.split_once('=').map(|(_, v)| v).unwrap_or(pair);
            let v = value.split(['?', '#']).next().unwrap_or(value);
            if let Some((_, ext)) = v.rsplit_once('.') {
                if IMAGE_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)) {
                    return true;
                }
            }
        }
    }

    false
}

const IMAGE_HOSTS: &[&str] = &[
    "i.redd.it/",
    "preview.redd.it/",
    "i.imgur.com/",
    "upload.wikimedia.org/",
    "s.yimg.com/",
    "encrypted-tbn0.gstatic.com/",
    "pbs.twimg.com/",
    "cdn.bsky.app/img/",
];

const IMAGE_HOST_EXCLUDED: &[&str] = &[
    "i.imgur.com/a/",
    "i.imgur.com/gallery/",
];

const IMAGE_PATHS: &[&str] = &[
    "/pictrs/image/",
    "/api/v3/image_proxy",
    "/api/v4/image/proxy",
];

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "bmp", "avif",
    "svg", "ico", "tif", "tiff", "heic", "heif", "jfif",
];
