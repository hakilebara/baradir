mod config;
mod db;
mod executor;
mod registry;
mod routes;
mod runtime;
mod server;
mod slug;
mod wit;

pub use config::Config;
pub use db::Db;
pub use server::run;

pub(crate) const MANAGER: &str = "manager";

pub(crate) fn is_valid_segment(segment: &str) -> bool {
    !segment.is_empty()
        && segment.len() <= 63
        && !segment.starts_with('-')
        && !segment.ends_with('-')
        && segment
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[derive(Clone, Copy, Debug)]
pub enum Access {
    Public,
    Private,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn validates_segments() {
        assert!(is_valid_segment("bookshelf"));
        assert!(is_valid_segment("frosty-canyon-2115"));

        assert!(!is_valid_segment(""));
        assert!(!is_valid_segment("../etc"));
        assert!(!is_valid_segment("Bookshelf"));
        assert!(!is_valid_segment("book_shelf"));
        assert!(!is_valid_segment("-bookshelf"));
        assert!(!is_valid_segment("bookshelf-"));
    }
}
