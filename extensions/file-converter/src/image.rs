// Made by Nordan
use crate::target::Target;
use crate::{Error, Result};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
fn io_error(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |error| Error::io(path.display().to_string(), error)
}

fn image_error(path: &Path) -> impl Fn(image::ImageError) -> Error + '_ {
    move |error| Error::Invalid(format!("{}: {error}", path.display()))
}

// ---------------------------------------------------------------------------
// Pictures

pub fn convert(source: &Path, output: &Path, target: Target) -> Result<bool> {
    use image::{DynamicImage, ImageDecoder, ImageFormat};
    let encodable = matches!(
        target,
        Target::Png
            | Target::Jpg
            | Target::Webp
            | Target::Bmp
            | Target::Tiff
            | Target::Gif
            | Target::Ico
            | Target::Qoi
            | Target::Avif
    );
    if !encodable {
        return Ok(false);
    }
    let Ok(reader) =
        image::ImageReader::open(source).and_then(image::ImageReader::with_guessed_format)
    else {
        return Ok(false);
    };
    let Some(format) = reader.format() else {
        return Ok(false);
    };
    let same = matches!(
        (format, target),
        (ImageFormat::Png, Target::Png)
            | (ImageFormat::Jpeg, Target::Jpg)
            | (ImageFormat::WebP, Target::Webp)
            | (ImageFormat::Bmp, Target::Bmp)
            | (ImageFormat::Tiff, Target::Tiff)
            | (ImageFormat::Gif, Target::Gif)
            | (ImageFormat::Ico, Target::Ico)
            | (ImageFormat::Qoi, Target::Qoi)
            | (ImageFormat::Avif, Target::Avif)
    );
    if same {
        return Err(Error::Already(target));
    }
    if format == ImageFormat::Gif
        && matches!(target, Target::Gif | Target::Webp)
        && animated_gif(source)
    {
        return Err(Error::Reason(crate::text::Reason::Animation));
    }
    let mut decoder = match reader.into_decoder() {
        Ok(decoder) => decoder,
        Err(_) => return Ok(false),
    };
    let orientation = decoder
        .orientation()
        .unwrap_or(image::metadata::Orientation::NoTransforms);
    let mut picture = DynamicImage::from_decoder(decoder).map_err(image_error(source))?;
    picture.apply_orientation(orientation);

    let file = File::create(output).map_err(io_error(output))?;
    let mut writer = BufWriter::new(file);
    let fail = image_error(output);
    let alpha = picture.color().has_alpha();
    match target {
        Target::Png => {
            let picture = match picture {
                DynamicImage::ImageRgb32F(_) | DynamicImage::ImageRgba32F(_) => {
                    DynamicImage::ImageRgba16(picture.to_rgba16())
                }
                other => other,
            };
            picture
                .write_with_encoder(image::codecs::png::PngEncoder::new(&mut writer))
                .map_err(fail)?;
        }
        Target::Jpg => {
            let flat = flatten_on_white(&picture);
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut writer, 90)
                .encode_image(&flat)
                .map_err(fail)?;
        }
        Target::Bmp | Target::Tiff | Target::Webp => {
            let picture = if alpha {
                DynamicImage::ImageRgba8(picture.to_rgba8())
            } else {
                DynamicImage::ImageRgb8(picture.to_rgb8())
            };
            match target {
                Target::Bmp => {
                    picture.write_with_encoder(image::codecs::bmp::BmpEncoder::new(&mut writer))
                }
                Target::Tiff => {
                    picture.write_with_encoder(image::codecs::tiff::TiffEncoder::new(&mut writer))
                }
                _ => picture.write_with_encoder(image::codecs::webp::WebPEncoder::new_lossless(
                    &mut writer,
                )),
            }
            .map_err(fail)?;
        }
        Target::Gif => {
            let mut encoder = image::codecs::gif::GifEncoder::new_with_speed(&mut writer, 10);
            encoder
                .encode_frame(image::Frame::new(picture.to_rgba8()))
                .map_err(fail)?;
        }
        Target::Ico => {
            let icon = if picture.width() > 256 || picture.height() > 256 {
                picture.resize(256, 256, image::imageops::FilterType::Lanczos3)
            } else {
                picture
            };
            icon.to_rgba8()
                .write_with_encoder(image::codecs::ico::IcoEncoder::new(&mut writer))
                .map_err(fail)?;
        }
        Target::Qoi => image::DynamicImage::ImageRgba8(picture.to_rgba8())
            .write_with_encoder(image::codecs::qoi::QoiEncoder::new(&mut writer))
            .map_err(fail)?,
        Target::Avif => image::DynamicImage::ImageRgba8(picture.to_rgba8())
            .write_with_encoder(image::codecs::avif::AvifEncoder::new_with_speed_quality(
                &mut writer,
                6,
                80,
            ))
            .map_err(fail)?,
        _ => return Ok(false),
    }
    writer.flush().map_err(io_error(output))?;
    Ok(true)
}

fn animated_gif(path: &Path) -> bool {
    use image::AnimationDecoder;
    let Ok(file) = File::open(path) else {
        return false;
    };
    image::codecs::gif::GifDecoder::new(BufReader::new(file))
        .is_ok_and(|decoder| decoder.into_frames().take(2).count() > 1)
}

/// JPEG has no transparency; see-through pixels go on white, as viewers show them.
fn flatten_on_white(picture: &image::DynamicImage) -> image::RgbImage {
    if !picture.color().has_alpha() {
        return picture.to_rgb8();
    }
    let rgba = picture.to_rgba8();
    image::RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [r, g, b, a] = rgba.get_pixel(x, y).0;
        let blend = |channel: u8| {
            ((u32::from(channel) * u32::from(a) + 255 * (255 - u32::from(a)) + 127) / 255) as u8
        };
        image::Rgb([blend(r), blend(g), blend(b)])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn convert_to(source: &Path, target: Target) -> std::path::PathBuf {
        let out = source.with_extension(format!("out.{}", target.extension()));
        assert!(convert(source, &out, target).unwrap());
        out
    }
    #[test]
    fn pictures_convert_and_keep_their_size() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("p.png");
        let picture = image::RgbaImage::from_fn(300, 120, |x, _| {
            image::Rgba([(x % 256) as u8, 40, 200, if x < 150 { 0 } else { 255 }])
        });
        picture.save(&png).unwrap();
        for target in [
            Target::Jpg,
            Target::Webp,
            Target::Bmp,
            Target::Tiff,
            Target::Gif,
            Target::Ico,
            Target::Qoi,
        ] {
            let output = convert_to(&png, target);
            let back = image::open(&output).unwrap();
            let expected = if target == Target::Ico {
                (256, 102)
            } else {
                (300, 120)
            };
            assert_eq!((back.width(), back.height()), expected, "{target:?}");
        }
        let jpg = png.with_extension("out.jpg");
        let back = image::open(&jpg).unwrap().to_rgb8();
        assert!(back.get_pixel(10, 10).0.iter().all(|c| *c > 240));
        let back = image::open(convert_to(&jpg, Target::Png)).unwrap();
        assert_eq!((back.width(), back.height()), (300, 120));
    }
    #[test]
    fn avif_encoding_writes_the_expected_dimensions() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("p.png");
        image::RgbImage::new(32, 24).save(&p).unwrap();
        let bytes = std::fs::read(convert_to(&p, Target::Avif)).unwrap();
        assert_eq!(&bytes[4..12], b"ftypavif");
        let at = bytes.windows(4).position(|w| w == b"ispe").unwrap();
        assert_eq!(
            u32::from_be_bytes(bytes[at + 8..at + 12].try_into().unwrap()),
            32
        );
        assert_eq!(
            u32::from_be_bytes(bytes[at + 12..at + 16].try_into().unwrap()),
            24
        );
    }
    #[test]
    fn exif_orientation_is_applied() {
        let dir = tempfile::tempdir().unwrap();
        let jpg = dir.path().join("turned.jpg");
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut bytes)
            .encode_image(&image::RgbImage::new(40, 20))
            .unwrap();
        let mut exif =
            b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0".to_vec();
        let mut segment = vec![0xff, 0xe1];
        segment.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        segment.append(&mut exif);
        bytes.splice(2..2, segment);
        std::fs::write(&jpg, bytes).unwrap();
        let back = image::open(convert_to(&jpg, Target::Png)).unwrap();
        assert_eq!((back.width(), back.height()), (20, 40));
    }
    #[test]
    fn rejects_animation_and_same_format() {
        let dir = tempfile::tempdir().unwrap();
        let gif = dir.path().join("a.gif");
        let file = File::create(&gif).unwrap();
        let mut enc = image::codecs::gif::GifEncoder::new(file);
        for _ in 0..2 {
            enc.encode_frame(image::Frame::new(image::RgbaImage::new(4, 4)))
                .unwrap();
        }
        drop(enc);
        let out = dir.path().join("out.webp");
        assert!(matches!(
            convert(&gif, &out, Target::Gif),
            Err(Error::Already(_))
        ));
        assert!(matches!(
            convert(&gif, &out, Target::Webp),
            Err(Error::Reason(crate::text::Reason::Animation))
        ));
        let input = dir.path().join("p.png");
        image::RgbImage::new(4, 4).save(&input).unwrap();
        assert!(matches!(
            convert(&input, &out, Target::Png),
            Err(Error::Already(_))
        ));
    }
}
