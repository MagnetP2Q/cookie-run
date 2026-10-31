use image::{DynamicImage, ImageFormat};
use std::io::Cursor;
use windows::{
    Graphics::Imaging::BitmapDecoder,
    Media::Ocr::OcrEngine,
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
};
pub trait Ocr {
    fn recognize(&self, image: &DynamicImage) -> Result<String, String>;
}
pub struct WindowsOcr;
impl Ocr for WindowsOcr {
    fn recognize(&self, image: &DynamicImage) -> Result<String, String> {
        (move || -> windows::core::Result<String> {
            let mut bytes = Cursor::new(Vec::new());
            image.write_to(&mut bytes, ImageFormat::Png).map_err(|e| {
                windows::core::Error::new(
                    windows::core::HRESULT(0x80004005u32 as i32),
                    e.to_string(),
                )
            })?;
            let stream = InMemoryRandomAccessStream::new()?;
            let writer = DataWriter::CreateDataWriter(&stream)?;
            writer.WriteBytes(bytes.get_ref())?;
            writer.StoreAsync()?.get()?;
            writer.DetachStream()?;
            stream.Seek(0)?;
            let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
            let bitmap = decoder.GetSoftwareBitmapAsync()?.get()?;
            let engine = OcrEngine::TryCreateFromUserProfileLanguages().or_else(|_| {
                let language = windows::Globalization::Language::CreateLanguage(
                    &windows::core::HSTRING::from("en-US"),
                )?;
                OcrEngine::TryCreateFromLanguage(&language)
            })?;
            Ok(engine.RecognizeAsync(&bitmap)?.get()?.Text()?.to_string())
        })()
        .map_err(|e| format!("Windows OCR ใช้งานไม่ได้: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires Windows OCR and COOKIE_RUN_FRAME path"]
    fn read_play_from_emulator_frame() {
        let path = std::env::var("COOKIE_RUN_FRAME")
            .expect("set COOKIE_RUN_FRAME to a 1600x900 frame showing Play!");
        let frame = image::open(path).unwrap();
        assert_eq!((frame.width(), frame.height()), (1600, 900));
        let crop = frame.crop_imm(1091, 767, 204, 80).resize_exact(
            612,
            240,
            image::imageops::FilterType::Lanczos3,
        );
        let text = WindowsOcr.recognize(&crop).expect("Windows OCR failed");
        println!("OCR Play region: {text:?}");
        assert!(crate::games::cookie_run::matches("Play!", &text));
    }
}
#[cfg(test)]
mod fixture_tests {
    use super::*;
    #[test]
    #[ignore = "requires Windows OCR and COOKIE_RUN_FIXTURES"]
    fn every_configured_region_is_recognized() {
        let directory = std::env::var_os("COOKIE_RUN_FIXTURES")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/ocr-fixtures")
            });
        let cfg: crate::config::Config =
            serde_json::from_str(include_str!("../../../config/cookie-run.default.json")).unwrap();
        for rule in &cfg.rules {
            let img = image::open(directory.join(format!("{}.png", rule.id))).unwrap();
            let [l, t, r, b] = rule.rect;
            let crop = img.crop_imm(l, t, r - l, b - t).resize_exact(
                (r - l) * cfg.scale,
                (b - t) * cfg.scale,
                image::imageops::FilterType::Lanczos3,
            );
            let text = WindowsOcr
                .recognize(&crop)
                .unwrap_or_else(|e| panic!("{}: {e}", rule.id));
            println!("{} target={:?} recognized={:?}", rule.id, rule.label, text);
            assert!(
                crate::games::cookie_run::matches(&rule.label, &text),
                "{} OCR did not match",
                rule.id
            );
        }
    }
}
