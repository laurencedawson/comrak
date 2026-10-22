//! A site's comment HTML into the blob, with no markdown between: a walk over the HTML in place of
//! the walk over comrak's tree, writing through the same [`BlobWriter`](super::BlobWriter). Not
//! [`crate::html`], which writes markdown out as HTML.

mod mdream;
