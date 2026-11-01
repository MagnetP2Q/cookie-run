use image::{DynamicImage, ImageFormat};
use std::{cell::RefCell, io::Cursor};
use windows::{
    Graphics::Imaging::{BitmapDecoder, SoftwareBitmap},
    Media::Ocr::OcrEngine,
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream},
};
pub trait Ocr {
    fn recognize(&self, image: &DynamicImage) -> Result<String, String>;
}
// Each bot session runs OCR on one dedicated worker thread. The engine is
// initialized lazily once on that thread and is released when the thread exits.
thread_local! {
    static ENGINE: RefCell<Option<OcrEngine>> = const { RefCell::new(None) };
}
fn engine_for_thread() -> windows::core::Result<OcrEngine> {
    ENGINE.with(|slot| {
        let mut engine = slot.borrow_mut();
        if engine.is_none() {
            *engine = Some(OcrEngine::TryCreateFromUserProfileLanguages().or_else(|_| {
                let language = windows::Globalization::Language::CreateLanguage(
                    &windows::core::HSTRING::from("en-US"),
                )?;
                OcrEngine::TryCreateFromLanguage(&language)
            })?);
        }
        Ok(engine.as_ref().expect("engine initialized").clone())
    })
}
// Explicitly close WinRT resources on both success and error paths.
struct OcrStreams {
    stream: InMemoryRandomAccessStream,
    writer: DataWriter,
}
impl Drop for OcrStreams {
    fn drop(&mut self) {
        let _ = self.writer.Close();
        let _ = self.stream.Close();
    }
}
struct OcrBitmap(SoftwareBitmap);
impl Drop for OcrBitmap {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
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
            let writer = match DataWriter::CreateDataWriter(&stream) {
                Ok(writer) => writer,
                Err(error) => {
                    let _ = stream.Close();
                    return Err(error);
                }
            };
            let resources = OcrStreams { stream, writer };
            resources.writer.WriteBytes(bytes.get_ref())?;
            resources.writer.StoreAsync()?.get()?;
            resources.writer.DetachStream()?;
            resources.stream.Seek(0)?;
            let decoder = BitmapDecoder::CreateAsync(&resources.stream)?.get()?;
            let bitmap = OcrBitmap(decoder.GetSoftwareBitmapAsync()?.get()?);
            let engine = engine_for_thread()?;
            Ok(engine.RecognizeAsync(&bitmap.0)?.get()?.Text()?.to_string())
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

#[cfg(test)]
mod direct_bitmap_comparison {
    use super::*;
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};

    fn recognize_direct(image: &DynamicImage) -> windows::core::Result<String> {
        let rgba = image.to_rgba8();
        let writer = DataWriter::new()?;
        writer.WriteBytes(rgba.as_raw())?;
        let buffer = writer.DetachBuffer()?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
            &buffer,
            BitmapPixelFormat::Rgba8,
            rgba.width() as i32,
            rgba.height() as i32,
            BitmapAlphaMode::Straight,
        )?;
        let result = engine_for_thread()?
            .RecognizeAsync(&bitmap)?
            .get()?
            .Text()?
            .to_string();
        let _ = bitmap.Close();
        let _ = writer.Close();
        Ok(result)
    }

    fn recognize_uncached(image: &DynamicImage) -> windows::core::Result<String> {
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, ImageFormat::Png).unwrap();
        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream)?;
        writer.WriteBytes(bytes.get_ref())?;
        writer.StoreAsync()?.get()?;
        writer.DetachStream()?;
        stream.Seek(0)?;
        let decoder = BitmapDecoder::CreateAsync(&stream)?.get()?;
        let bitmap = decoder.GetSoftwareBitmapAsync()?.get()?;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages().or_else(|_| {
            let lang = windows::Globalization::Language::CreateLanguage(
                &windows::core::HSTRING::from("en-US"),
            )?;
            OcrEngine::TryCreateFromLanguage(&lang)
        })?;
        Ok(engine.RecognizeAsync(&bitmap)?.get()?.Text()?.to_string())
    }
    #[test]
    #[ignore = "requires Windows OCR; compares prior and cached engine"]
    fn compare_cached_to_original_and_measure() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/ocr-fixtures");
        let cfg: crate::config::Config =
            serde_json::from_str(include_str!("../../../config/cookie-run.default.json")).unwrap();
        let mut crops = Vec::new();
        for rule in &cfg.rules {
            let img = image::open(dir.join(format!("{}.png", rule.id))).unwrap();
            let [l, t, r, b] = rule.rect;
            crops.push((
                rule.id.clone(),
                img.crop_imm(l, t, r - l, b - t).resize_exact(
                    (r - l) * cfg.scale,
                    (b - t) * cfg.scale,
                    image::imageops::FilterType::Lanczos3,
                ),
            ));
        }
        let mut old_total = std::time::Duration::ZERO;
        let mut cached_total = std::time::Duration::ZERO;
        for _ in 0..5 {
            for (id, crop) in &crops {
                let start = std::time::Instant::now();
                let old = recognize_uncached(crop).unwrap();
                old_total += start.elapsed();
                let start = std::time::Instant::now();
                let cached = WindowsOcr.recognize(crop).unwrap();
                cached_total += start.elapsed();
                assert_eq!(old, cached, "OCR mismatch for {id}");
            }
        }
        println!(
            "{} scans: prior={old_total:?} cached={cached_total:?}",
            crops.len() * 5
        );
    }

    #[test]
    #[ignore = "requires Windows OCR; measures real fixture conversions"]
    fn compare_direct_to_png_and_measure() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/ocr-fixtures");
        let cfg: crate::config::Config =
            serde_json::from_str(include_str!("../../../config/cookie-run.default.json")).unwrap();
        let mut crops = Vec::new();
        for rule in &cfg.rules {
            let img = image::open(dir.join(format!("{}.png", rule.id))).unwrap();
            let [l, t, r, b] = rule.rect;
            crops.push((
                rule.id.clone(),
                img.crop_imm(l, t, r - l, b - t).resize_exact(
                    (r - l) * cfg.scale,
                    (b - t) * cfg.scale,
                    image::imageops::FilterType::Lanczos3,
                ),
            ));
        }
        if let Some(frame_path) = std::env::var_os("COOKIE_RUN_PARITY_FRAME") {
            let frame = image::open(frame_path).expect("open real emulator frame");
            assert_eq!((frame.width(), frame.height()), (1600, 900));
            for rule in &cfg.rules {
                let [l, t, r, b] = rule.rect;
                crops.push((
                    format!("real-{}", rule.id),
                    frame.crop_imm(l, t, r - l, b - t).resize_exact(
                        (r - l) * cfg.scale,
                        (b - t) * cfg.scale,
                        image::imageops::FilterType::Lanczos3,
                    ),
                ));
            }
        }
        for (id, crop) in &crops {
            let png = WindowsOcr.recognize(crop).unwrap();
            let direct = recognize_direct(crop).unwrap();
            println!("{id}: png={png:?} direct={direct:?}");
            assert_eq!(png, direct, "OCR output mismatch for {id}");
        }
        // Warm both paths before the timed runs; include all allocations and OCR.
        let mut png_total = std::time::Duration::ZERO;
        let mut direct_total = std::time::Duration::ZERO;
        for _ in 0..5 {
            for (_, crop) in &crops {
                let start = std::time::Instant::now();
                std::hint::black_box(WindowsOcr.recognize(crop).unwrap());
                png_total += start.elapsed();
                let start = std::time::Instant::now();
                std::hint::black_box(recognize_direct(crop).unwrap());
                direct_total += start.elapsed();
            }
        }
        println!(
            "{} OCR scans per method: png={png_total:?} direct={direct_total:?}",
            crops.len() * 5
        );
    }
}
