//! Image import and normalization, independent of audio decoding.
use anyhow::{Context, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{io::Cursor, path::Path};
#[cfg(feature = "desktop")]
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "bmp", "tif", "tiff", "svg",
];
pub fn import_icon(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > 10 * 1024 * 1024 {
        bail!("10 MB以下の画像を選択してください");
    }
    let bytes = std::fs::read(path)?;
    let image = if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("svg"))
    {
        let tree = resvg::usvg::Tree::from_data(&bytes, &resvg::usvg::Options::default())?;
        let size = tree.size();
        let scale = 128.0 / size.width().min(size.height());
        let mut pixmap = resvg::tiny_skia::Pixmap::new(128, 128).context("画像を読み込めません")?;
        let transform = resvg::tiny_skia::Transform::from_row(
            scale,
            0.0,
            0.0,
            scale,
            (128.0 - size.width() * scale) / 2.0,
            (128.0 - size.height() * scale) / 2.0,
        );
        resvg::render(&tree, transform, &mut pixmap.as_mut());
        return Ok(format!(
            "data:image/png;base64,{}",
            STANDARD.encode(pixmap.encode_png()?)
        ));
    } else {
        let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(128 * 1024 * 1024);
        limits.max_image_width = Some(16384);
        limits.max_image_height = Some(16384);
        reader.limits(limits);
        reader.decode()?
    };
    let side = image.width().min(image.height());
    if side == 0 {
        bail!("画像を読み込めません");
    }
    let image = image
        .crop_imm(
            (image.width() - side) / 2,
            (image.height() - side) / 2,
            side,
            side,
        )
        .resize_exact(128, 128, image::imageops::FilterType::Lanczos3);
    let mut png = Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png)?;
    Ok(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(png.into_inner())
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn image_crop_and_svg_are_png() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("icon.png");
        image::RgbImage::new(320, 180).save(&p).unwrap();
        for path in [p, dir.path().join("icon.svg")] {
            if path.extension().unwrap() == "svg" {
                std::fs::write(&path,r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="200" height="100" fill="red"/></svg>"#).unwrap();
            }
            let data = import_icon(&path).unwrap();
            let image =
                image::load_from_memory(&STANDARD.decode(data.split_once(',').unwrap().1).unwrap())
                    .unwrap();
            assert_eq!((image.width(), image.height()), (128, 128));
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
}
