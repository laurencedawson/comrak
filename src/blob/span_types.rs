// @generated
pub const QUOTE: i32 = 0;
pub const LIST_ITEM: i32 = 1;
pub const TASK_ITEM: i32 = 2;
pub const HEADING_1: i32 = 3;
pub const HEADING_2: i32 = 4;
pub const HEADING_3: i32 = 5;
pub const HEADING_4: i32 = 6;
pub const HEADING_5: i32 = 7;
pub const HEADING_6: i32 = 8;
pub const CENTERED: i32 = 9;
pub const CODE: i32 = 10;
pub const CODE_BLOCK: i32 = 11;
pub const IMAGE: i32 = 12;
pub const TABLE: i32 = 13;
pub const HRULE: i32 = 14;
pub const BOLD: i32 = 15;
pub const ITALIC: i32 = 16;
pub const STRIKETHROUGH: i32 = 17;
pub const SUPERSCRIPT: i32 = 18;
pub const SUBSCRIPT: i32 = 19;
pub const SUPERSCRIPT_SIZE: i32 = 20;
pub const SUBSCRIPT_SIZE: i32 = 21;
pub const GREENTEXT: i32 = 22;
pub const LINK: i32 = 23;
pub const APP_LINK: i32 = 24;
pub const LINK_SIZE: i32 = 25;
pub const READER_BLOCK: i32 = 26;
pub const LEMMY_SPOILER_TITLE: i32 = 27;
pub const LEMMY_SPOILER_CONTENT: i32 = 28;

pub const HEADINGS: [i32; 6] = [
    HEADING_1, HEADING_2, HEADING_3, HEADING_4, HEADING_5, HEADING_6,
];

/// The data of a `READER_BLOCK` span: what its lines are, which sets their height.
pub mod reader_block {
    pub const LEVEL_SHIFT: i32 = 8;
    pub const PARAGRAPH: i32 = 0;
    pub const HEADING: i32 = 1;
    pub const QUOTE: i32 = 2;
    pub const GAP: i32 = 3;
    pub const LIST: i32 = 4;
    pub const KEEP: i32 = 5;
    pub const PICTURE: i32 = 6;
}

/// The blob's layout, what the writer and the reader must agree on beyond the types.
pub mod format {
    pub const HEADER_SIZE: usize = 8;
    pub const TEXT_LENGTH_AT: usize = 0;
    pub const COUNT_AT: usize = 4;
    pub const TEXT_ALIGN: usize = 4;
    pub const RECORD: usize = 16;
    pub const RECORD_SHIFT: usize = 4;
    pub const COUNT_BITS: usize = 24;
    pub const ASCII_BIT: usize = 0;
    pub const SITE_SHIFT: usize = 2;
    pub const SITE_BITS: usize = 6;
    pub const PICTURE: &str = "\u{0001}";
    pub const RULE: &str = "\u{0002}";
    pub const LINK_LENGTH_BITS: usize = 12;
    pub const LIST_INDENT_SHIFT: usize = 16;
    pub const LIST_BULLET: usize = 65535;
    pub const TASK_CHECKED_BIT: usize = 0;
    pub const TABLE_DATA_BIAS: usize = 1;
    pub const TABLE_ROWS_SHIFT: usize = 16;
    pub const TABLE_CELLS: usize = 4;
    pub const PICTURE_WIDTH: usize = 1;
    pub const PICTURE_HEIGHT: usize = 3;
    pub const PICTURE_URL_LENGTH: usize = 5;
    pub const PICTURE_ALT_LENGTH: usize = 7;
    pub const PICTURE_URL: usize = 9;
}
