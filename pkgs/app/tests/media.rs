use irodoru_desktop::audio::{import_audio, import_icon};
use std::{io::Cursor, path::Path};
#[test]
fn supported_audio_formats_decode_downmix_and_resample() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for extension in ["wav", "mp3", "flac", "ogg", "m4a", "aac"] {
        let wav = import_audio(&fixtures.join(format!("reference.{extension}")))
            .unwrap_or_else(|e| panic!("{extension}: {e:#}"));
        let mut reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().sample_rate, 48000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert!(
            (48000..60000).contains(&reader.duration()),
            "{extension}: {}",
            reader.duration()
        );
        assert!(reader.samples::<i16>().any(|s| s.unwrap().abs() > 10));
    }
}
#[test]
fn invalid_and_oversized_images_fail() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("bad.png");
    std::fs::write(&p, b"not a png").unwrap();
    assert!(import_icon(&p).is_err());
    std::fs::File::create(&p)
        .unwrap()
        .set_len(10 * 1024 * 1024 + 1)
        .unwrap();
    assert!(import_icon(&p).unwrap_err().to_string().contains("10 MB"));
}
#[test]
fn supported_raster_formats_convert_to_square_png() {
    use base64::Engine as _;
    let dir = tempfile::tempdir().unwrap();
    for format in [
        image::ImageFormat::Png,
        image::ImageFormat::Jpeg,
        image::ImageFormat::WebP,
        image::ImageFormat::Gif,
        image::ImageFormat::Bmp,
        image::ImageFormat::Tiff,
    ] {
        let path = dir
            .path()
            .join(format!("icon.{}", format.extensions_str()[0]));
        image::RgbImage::from_pixel(180, 100, image::Rgb([40, 80, 120]))
            .save_with_format(&path, format)
            .unwrap();
        let data = import_icon(&path).unwrap();
        let png = base64::engine::general_purpose::STANDARD
            .decode(data.strip_prefix("data:image/png;base64,").unwrap())
            .unwrap();
        let image = image::load_from_memory(&png).unwrap();
        assert_eq!((image.width(), image.height()), (128, 128));
    }
}
