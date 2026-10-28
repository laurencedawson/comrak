//! From dom_smoothie 0.19.0's `glob.rs`: the numbers and word lists, its selectors left behind.

pub(crate) const N_TOP_CANDIDATES: usize = 5;
pub(crate) const CHAR_THRESHOLD: usize = 500;
pub(crate) const MIN_SCORE_TO_ADJUST: f32 = 5.0;
pub(crate) const MIN_COMMON_ANCESTORS: usize = 3;

pub(crate) const DATA_URL_PFX: &str = "data:";
pub(crate) const BASE64_MARKER: &str = ";base64,";

pub(crate) const PROTOCOL_PFX: &str = "//";
pub(crate) const WWW_PFX: &str = "//www.";

pub(crate) const META_TITLE_KEYS: &[&str] = &[
    "dc:title",
    "dcterms:title",
    "og:title",
    "weibo:article:title",
    "weibo:webpage:title",
    "title",
    "twitter:title",
    "parsely-title",
];

pub(crate) const META_IMAGE_KEYS: &[&str] = &["og:image", "image", "twitter:image"];

pub(crate) const META_BYLINE_KEYS: &[&str] =
    &["dc:creator", "dcterms:creator", "author", "parsely-author"];

pub(crate) const BYLINE_PATTERNS: &[&str] =
    &["byline", "author", "dateline", "writtenby", "p-author"];

#[rustfmt::skip]
pub(crate) const UNLIKELY_CANDIDATES: &[&str] = &[
    "-ad-", "ai2html", "banner", "breadcrumbs", "combx", "comment", "community",
    "cover-wrap", "disqus", "extra", "footer", "gdpr", "header", "legends", "menu",
    "related", "remark", "replies", "rss", "shoutbox", "sidebar", "skyscraper",
    "social", "sponsor", "supplemental", "ad-break", "agegate", "pagination",
    "pager", "popup", "yom-remote",
];

pub(crate) const MAYBE_CANDIDATES: &[&str] = &[
    "and", "article", "body", "column", "content", "layout", "main", "mathjax", "shadow",
];

pub(crate) const VIDEO_DOMAINS: &[&str] = &[
    "dailymotion.com",
    "youtube.com",
    "youtube-nocookie.com",
    "player.vimeo.com",
    "v.qq.com",
    "archive.org",
    "upload.wikimedia.org",
    "player.twitch.tv",
    "bilibili.com",
    "live.bilibili.com",
];

pub(crate) const COMMAS: &[char] = &[
    '\u{002C}', '\u{060C}', '\u{FE50}', '\u{FE10}', '\u{FE11}', '\u{2E41}', '\u{2E34}', '\u{2E32}',
    '\u{FF0C}',
];

pub(crate) const TITLE_SEPARATORS: &[char] = &['|', '-', '–', '—', '\\', '/', '>', '»'];
pub(crate) const TITLE_HIERARCHY_SEP: &[char] = &['\\', '/', '>', '»'];
pub(crate) const IMG_EXT: &[&str] = &[".jpg", ".jpeg", ".png", ".webp", ".avif", ".gif"];

pub(crate) const SHARE_WORDS: &[&str] = &["share", "sharedaddy"];

#[rustfmt::skip]
pub(crate) const CLASSES_NEGATIVE: &[&str] = &[
    "-ad-", "hidden", "banner", "combx", "comment", "com-", "contact", "footer",
    "gdpr", "masthead", "media", "meta", "outbrain", "promo", "related", "scroll",
    "share", "shoutbox", "sidebar", "skyscraper", "sponsor", "shopping", "tags",
    "widget"
];

#[rustfmt::skip]
pub(crate) const CLASSES_POSITIVE: &[&str] = &[
    "article", "body", "content", "entry", "hentry", "h-entry", "main", "page",
    "post", "text", "blog", "story",
];

pub(crate) const CLASSES_NEGATIVE_WORDS: &[&str] = &["hid"];

pub(crate) const ALTER_TO_DIV_EXCEPTIONS: &[&str] = &["article", "section", "p", "ol", "ul"];
pub(crate) const DEFAULT_TAGS_TO_SCORE: &[&str] =
    &["section", "h2", "h3", "h4", "h5", "h6", "p", "td", "pre"];
pub(crate) const TAGS_WITH_CONTENT: &[&str] = &[
    "div", "section", "header", "h1", "h2", "h3", "h4", "h5", "h6",
];
pub(crate) const BLOCK_ELEMS: &[&str] = &[
    "blockquote",
    "dl",
    "div",
    "img",
    "ol",
    "p",
    "pre",
    "table",
    "ul",
];
pub(crate) const EMBED_ELEMENTS: &[&str] = &["object", "embed", "iframe"];

#[rustfmt::skip]
pub(crate) const UNLIKELY_ROLES: &[&str] = &[
    "menu", "menubar", "complementary", "navigation", "alert", "alertdialog", "dialog"
];

#[rustfmt::skip]
pub(crate) const PHRASING_ELEMS: &[&str] = &[
    "abbr", "audio", "b", "bdo", "br", "button", "cite", "code", "data", "datalist", "dfn", "em",
    "embed", "i", "img", "input", "kbd", "label", "mark", "math", "meter", "noscript", "object",
    "output", "progress", "q", "ruby", "samp", "script", "select", "small", "span", "strong",
    "sub", "sup", "textarea", "time", "var", "wbr"
];

#[rustfmt::skip]
pub(crate) const AD_WORDS: &[&str] = &[
    "ad", "advertising", "advertisement", "pub", "publicité",
    "werb", "werbung", "广告", "реклама", "anuncio"
];

#[rustfmt::skip]
pub(crate) const LOADING_WORDS: &[&str] = &[
    "loading", "正在加载", "загрузка", "chargement", "cargando"
];

pub(crate) const CLEAN: &[&str] = &[
    "object", "embed", "footer", "link", "aside", "iframe", "input", "textarea", "select", "button",
];
pub(crate) const TEXTISH: &[&str] = &[
    "blockquote",
    "dl",
    "div",
    "img",
    "ol",
    "p",
    "pre",
    "table",
    "ul",
    "span",
    "li",
    "td",
];
pub(crate) const H1_H6: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];
pub(crate) const TABLE_MEMBERS: &[&str] = &["caption", "col", "colgroup", "tfoot", "thead", "th"];
