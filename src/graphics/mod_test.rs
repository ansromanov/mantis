use super::*;

fn encode(w: u32, h: u32, fmt: image::ImageFormat) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(w, h, image::Rgb([10, 20, 30]));
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut buf), fmt)
        .unwrap();
    buf
}

#[test]
fn from_bytes_reads_png_dimensions_and_passes_bytes_through() {
    let src = encode(64, 48, image::ImageFormat::Png);
    let img = ContentImage::from_bytes(&src).expect("png decodes");
    assert_eq!((img.width, img.height), (64, 48));
    assert_eq!(&*img.png, &src, "already-PNG data must not be re-encoded");
}

#[test]
fn from_bytes_reencodes_non_png_to_png() {
    let src = encode(32, 20, image::ImageFormat::Bmp);
    let img = ContentImage::from_bytes(&src).expect("bmp decodes");
    assert_eq!((img.width, img.height), (32, 20));
    assert_eq!(
        &img.png[..8],
        &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
        "output is PNG-encoded"
    );
}

#[test]
fn from_bytes_rejects_non_image_data() {
    assert!(ContentImage::from_bytes(b"this is definitely not an image").is_none());
}
