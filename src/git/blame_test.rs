use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use super::*;

#[test]
fn single_line_single_commit() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-mail <alice@example.com>\n\
         author-time 1000000\n\
         author-tz +0000\n\
         committer Alice\n\
         committer-mail <alice@example.com>\n\
         committer-time 1000000\n\
         committer-tz +0000\n\
         summary init\n\
         filename src/foo.rs\n\
         \tfn main() {}\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].line_no, 1);
    assert_eq!(result[0].author, "Alice");
    assert_eq!(result[0].short_hash, "aaaaaaa");
    assert_eq!(
        result[0].commit_hash,
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
}

#[test]
fn multi_line_same_commit() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 3\n\
         author Alice\n\
         author-time 1000000\n\
         filename src/foo.rs\n\
         \tline one\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2 2\n\
         \tline two\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3 3\n\
         \tline three\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].line_no, 1);
    assert_eq!(result[1].line_no, 2);
    assert_eq!(result[2].line_no, 3);
    for b in &result {
        assert_eq!(b.author, "Alice");
    }
}

#[test]
fn multiple_commits_interleaved() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-time 1000000\n\
         filename src/foo.rs\n\
         \tline one\n\
         bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb 2 2 1\n\
         author Bob\n\
         author-time 2000000\n\
         filename src/foo.rs\n\
         \tline two\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 3 3\n\
         \tline three\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 3);
    assert_eq!(result[0].author, "Alice");
    assert_eq!(result[0].line_no, 1);
    assert_eq!(result[1].author, "Bob");
    assert_eq!(result[1].line_no, 2);
    assert_eq!(result[2].author, "Alice");
    assert_eq!(result[2].line_no, 3);
}

#[test]
fn empty_input() {
    assert!(parse_blame_porcelain("").is_empty());
}

// -- subject field -----------------------------------------------------------

#[test]
fn blame_porcelain_parses_subject() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-time 1000000\n\
         summary fix the thing\n\
         filename src/foo.rs\n\
         \tcode here\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].subject, "fix the thing");
}

#[test]
fn blame_porcelain_subject_shared_across_lines_same_commit() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 2\n\
         author Alice\n\
         author-time 1000000\n\
         summary shared subject\n\
         filename src/foo.rs\n\
         \tline one\n\
         aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 2 2\n\
         \tline two\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].subject, "shared subject");
    assert_eq!(result[1].subject, "shared subject");
}

#[test]
fn blame_porcelain_missing_summary_gives_empty_subject() {
    let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa 1 1 1\n\
         author Alice\n\
         author-time 1000000\n\
         filename src/foo.rs\n\
         \tcode here\n";
    let result = parse_blame_porcelain(input);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].subject, "");
}

// -- blame cache (bounded LRU) -----------------------------------------------

#[test]
fn blame_cache_evicts_oldest_when_full() {
    let mut cache = BlameCache::new();
    assert_eq!(BLAME_CACHE_CAPACITY, 16);

    let paths: Vec<PathBuf> = (0..BLAME_CACHE_CAPACITY + 1)
        .map(|i| PathBuf::from(format!("/tmp/test_blame_cache_{i}.rs")))
        .collect();

    for (i, p) in paths.iter().enumerate() {
        cache.insert(
            p.clone(),
            CachedBlame {
                mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(i as u64),
                lines: Vec::new(),
            },
        );
    }

    // Cache should be at capacity.
    assert_eq!(cache.map.len(), BLAME_CACHE_CAPACITY);
    assert_eq!(cache.order.len(), BLAME_CACHE_CAPACITY);

    // The first-inserted entry should be evicted.
    assert!(
        !cache.map.contains_key(&paths[0]),
        "oldest entry should be evicted"
    );

    // The last (most recent) entry should survive.
    assert!(
        cache.map.contains_key(&paths[BLAME_CACHE_CAPACITY]),
        "most recent entry should survive"
    );
}

#[test]
fn blame_cache_re_get_promotes_entry() {
    let mut cache = BlameCache::new();

    let a = PathBuf::from("/tmp/a.rs");
    let b = PathBuf::from("/tmp/b.rs");
    let c = PathBuf::from("/tmp/c.rs");

    cache.insert(
        a.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH,
            lines: Vec::new(),
        },
    );
    cache.insert(
        b.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            lines: Vec::new(),
        },
    );
    cache.insert(
        c.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(2),
            lines: Vec::new(),
        },
    );

    // Order is [a, b, c]. Get 'a' to promote it.
    let _ = cache.get(&a);

    // Now order should be [b, c, a].
    assert_eq!(cache.order[0], b);
    assert_eq!(cache.order[1], c);
    assert_eq!(cache.order[2], a);
}

#[test]
fn blame_cache_update_promotes_entry() {
    let mut cache = BlameCache::new();

    let a = PathBuf::from("/tmp/a.rs");
    let b = PathBuf::from("/tmp/b.rs");

    cache.insert(
        a.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH,
            lines: Vec::new(),
        },
    );
    cache.insert(
        b.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
            lines: Vec::new(),
        },
    );

    // Re-insert 'a' as if its mtime changed.
    cache.insert(
        a.clone(),
        CachedBlame {
            mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(10),
            lines: Vec::new(),
        },
    );

    // 'a' should be promoted to the back (most recent).
    assert_eq!(cache.order[0], b);
    assert_eq!(cache.order[1], a);
}
