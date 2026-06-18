//! On-device OCR using Apple's Vision framework (VNRecognizeTextRequest).
//! Fully local, no network, no special entitlement. Runs synchronously, so
//! callers should invoke it off the main thread.

#[cfg(target_os = "macos")]
pub fn ocr_image_png(png_bytes: &[u8]) -> Option<String> {
    use objc2::rc::{autoreleasepool, Retained};
    use objc2::runtime::AnyObject;
    use objc2::AllocAnyThread;
    use objc2_foundation::{NSArray, NSData, NSDictionary, NSString};
    use objc2_vision::{
        VNImageOption, VNImageRequestHandler, VNRecognizeTextRequest, VNRequest,
        VNRequestTextRecognitionLevel,
    };

    if png_bytes.is_empty() {
        return None;
    }

    autoreleasepool(|_pool| {
        let data = NSData::with_bytes(png_bytes);

        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(VNRequestTextRecognitionLevel::Accurate);
        request.setUsesLanguageCorrection(true);
        // ru-RU requires macOS 13+; harmlessly ignored on older systems.
        let langs = NSArray::from_retained_slice(&[
            NSString::from_str("ru-RU"),
            NSString::from_str("en-US"),
        ]);
        request.setRecognitionLanguages(&langs);

        let options: Retained<NSDictionary<VNImageOption, AnyObject>> = NSDictionary::new();
        let handler = VNImageRequestHandler::initWithData_options(
            VNImageRequestHandler::alloc(),
            &data,
            &options,
        );

        let req_base: &VNRequest = &request;
        let requests = NSArray::from_slice(&[req_base]);
        if handler.performRequests_error(&requests).is_err() {
            return None;
        }

        let results = request.results()?;
        let mut lines: Vec<String> = Vec::new();
        for i in 0..results.count() {
            let obs = results.objectAtIndex(i);
            let candidates = obs.topCandidates(1);
            if candidates.count() > 0 {
                let best = candidates.objectAtIndex(0);
                lines.push(best.string().to_string());
            }
        }

        if lines.is_empty() {
            None
        } else {
            Some(lines.join("\n"))
        }
    })
}

#[cfg(not(target_os = "macos"))]
pub fn ocr_image_png(_png_bytes: &[u8]) -> Option<String> {
    None
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    /// Smoke test: the Vision FFI path must execute without panicking on a
    /// valid (blank) PNG. A blank image has no text, so we expect None.
    #[test]
    fn ocr_blank_image_runs_without_panic() {
        use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
        use std::io::Cursor;

        let img = ImageBuffer::from_pixel(120, 40, Rgba([255u8, 255, 255, 255]));
        let dynimg = DynamicImage::ImageRgba8(img);
        let mut buf = Cursor::new(Vec::new());
        dynimg.write_to(&mut buf, ImageFormat::Png).unwrap();
        let png = buf.into_inner();

        let result = ocr_image_png(&png);
        assert!(result.is_none(), "blank image should yield no text");
    }

    #[test]
    fn ocr_empty_bytes_returns_none() {
        assert!(ocr_image_png(&[]).is_none());
    }
}
