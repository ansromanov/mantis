mod diff;
mod draw;
mod scrollbar;
mod search;
mod selection;

pub(super) use draw::draw_content;

#[cfg(test)]
#[path = "content_test.rs"]
mod tests;
