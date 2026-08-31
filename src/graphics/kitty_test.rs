use super::*;

fn s(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

// ── fit ──────────────────────────────────────────────────────────────────

#[test]
fn fit_landscape_fills_width() {
    // 2:1 pixel aspect -> 4:1 in cell units; width-bound.
    assert_eq!(fit(1000, 500, 80, 40), (80, 20));
}

#[test]
fn fit_portrait_fills_height() {
    assert_eq!(fit(500, 1000, 80, 40), (40, 40));
}

#[test]
fn fit_shrinks_to_a_small_box() {
    assert_eq!(fit(100, 100, 10, 10), (10, 5));
}

#[test]
fn fit_never_returns_zero() {
    let (c, r) = fit(10_000, 1, 5, 5);
    assert!(c >= 1 && r >= 1, "got {c}x{r}");
    assert!(c <= 5 && r <= 5);
}

#[test]
fn fit_zero_dimensions_fall_back_to_the_box() {
    assert_eq!(fit(0, 0, 12, 8), (12, 8));
}

// ── transmit_and_place ───────────────────────────────────────────────────

#[test]
fn transmit_single_chunk_has_full_control_data_and_base64_payload() {
    let out = s(&transmit_and_place(
        42,
        b"hi",
        ImagePlacement {
            cols: 5,
            rows: 3,
            col: 2,
            row: 4,
        },
    ));
    assert_eq!(
        out,
        "\x1b[4;2H\x1b_Ga=T,f=100,i=42,p=1,c=5,r=3,C=1,q=2,m=0;aGk=\x1b\\"
    );
}

#[test]
fn transmit_chunks_large_payloads_and_flags_continuations() {
    // 3100 raw bytes -> 4136 base64 chars -> two 4096-max chunks.
    let out = s(&transmit_and_place(
        7,
        &vec![0xABu8; 3100],
        ImagePlacement {
            cols: 10,
            rows: 6,
            col: 1,
            row: 1,
        },
    ));
    assert_eq!(out.matches("\x1b_G").count(), 2, "expected two chunks");
    let (first, rest) = out.split_once("\x1b\\").unwrap();
    assert!(first.contains("a=T,f=100,i=7,p=1,c=10,r=6"));
    assert!(first.contains(",m=1;"), "first chunk must flag more data");
    assert!(rest.contains("\x1b_Gm=0;"), "last chunk closes the stream");
    assert!(
        !rest.contains("a=T"),
        "continuation carries no control data"
    );
}

#[test]
fn transmit_empty_payload_is_just_a_cursor_move() {
    let out = transmit_and_place(
        1,
        b"",
        ImagePlacement {
            cols: 1,
            rows: 1,
            col: 3,
            row: 9,
        },
    );
    assert_eq!(s(&out), "\x1b[9;3H");
}

// ── place / delete ───────────────────────────────────────────────────────

#[test]
fn place_repositions_without_retransmitting() {
    let out = s(&place(
        7,
        ImagePlacement {
            cols: 20,
            rows: 12,
            col: 40,
            row: 2,
        },
    ));
    assert_eq!(out, "\x1b[2;40H\x1b_Ga=p,i=7,p=1,c=20,r=12,C=1,q=2\x1b\\");
}

#[test]
fn delete_targets_one_image_by_id() {
    assert_eq!(s(&delete(9)), "\x1b_Ga=d,d=i,i=9,q=2\x1b\\");
}

#[test]
fn delete_all_clears_everything() {
    assert_eq!(s(&delete_all()), "\x1b_Ga=d,d=A,q=2\x1b\\");
}

// ── base64 ───────────────────────────────────────────────────────────────

#[test]
fn base64_matches_known_vectors() {
    assert_eq!(base64_encode(b""), "");
    assert_eq!(base64_encode(b"f"), "Zg==");
    assert_eq!(base64_encode(b"fo"), "Zm8=");
    assert_eq!(base64_encode(b"foo"), "Zm9v");
    assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
}
