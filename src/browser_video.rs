use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq)]
pub struct BrowserVideoFrame {
    pub elapsed: Duration,
    pub image_type: String,
    pub base64: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserVideoChapter {
    pub elapsed_ms: u128,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BrowserVideoEncodeResult {
    pub path: PathBuf,
    pub frame_count: usize,
    pub duration_ms: u128,
    pub chapters: Vec<BrowserVideoChapter>,
}

pub fn encode_browser_video_mp4(
    output_path: PathBuf,
    frames: Vec<BrowserVideoFrame>,
    chapters: Vec<BrowserVideoChapter>,
    fps: u32,
) -> Result<BrowserVideoEncodeResult, String> {
    if frames.is_empty() {
        return Err("No browser video frames were captured".to_owned());
    }
    let fps = fps.clamp(1, 60);
    let duration_ms = frames
        .last()
        .map(|frame| frame.elapsed.as_millis())
        .unwrap_or(0)
        .max(((frames.len() as u128) * 1000) / fps as u128);
    encode_browser_video_mp4_platform(&output_path, &frames, fps)?;
    Ok(BrowserVideoEncodeResult {
        path: output_path,
        frame_count: frames.len(),
        duration_ms,
        chapters,
    })
}

#[cfg(target_os = "windows")]
fn encode_browser_video_mp4_platform(
    output_path: &Path,
    frames: &[BrowserVideoFrame],
    fps: u32,
) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt as _;

    use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
    use base64::Engine as _;
    use image::imageops::FilterType;
    use windows::core::PCWSTR;
    use windows::Win32::Media::MediaFoundation::{
        IMFAttributes, IMFByteStream, MFCreateMediaType, MFCreateSinkWriterFromURL,
        MFMediaType_Video, MFShutdown, MFStartup, MFVideoFormat_H264, MFVideoFormat_RGB32,
        MFVideoInterlace_Progressive, MFSTARTUP_FULL, MF_MT_ALL_SAMPLES_INDEPENDENT,
        MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FIXED_SIZE_SAMPLES, MF_MT_FRAME_RATE,
        MF_MT_FRAME_SIZE, MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO,
        MF_MT_SUBTYPE, MF_VERSION,
    };
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

    fn guid_attr(value: &windows::core::GUID) -> *const windows::core::GUID {
        value as *const windows::core::GUID
    }

    fn ratio_u64(numerator: u32, denominator: u32) -> u64 {
        ((numerator as u64) << 32) | denominator as u64
    }

    fn image_frame_to_bgra(
        frame: &BrowserVideoFrame,
        target: Option<(u32, u32)>,
    ) -> Result<(u32, u32, Vec<u8>), String> {
        let bytes = BASE64_STANDARD
            .decode(frame.base64.as_bytes())
            .map_err(|err| format!("Browser video frame base64 decode failed: {err}"))?;
        let image = image::load_from_memory(&bytes)
            .map_err(|err| format!("Browser video frame decode failed: {err}"))?;
        let (width, height) = target.unwrap_or_else(|| {
            (
                normalize_video_dimension(image.width()),
                normalize_video_dimension(image.height()),
            )
        });
        let image = if image.width() != width || image.height() != height {
            image.resize_exact(width, height, FilterType::Triangle)
        } else {
            image
        };
        let rgba = image.to_rgba8();
        let mut bgra = Vec::with_capacity((width as usize) * (height as usize) * 4);
        for pixel in rgba.pixels() {
            bgra.push(pixel[2]);
            bgra.push(pixel[1]);
            bgra.push(pixel[0]);
            bgra.push(pixel[3]);
        }
        Ok((width, height, bgra))
    }

    let Some(parent) = output_path.parent() else {
        return Err(format!(
            "Browser video output path has no parent: {}",
            output_path.display()
        ));
    };
    std::fs::create_dir_all(parent).map_err(|err| {
        format!(
            "Could not create browser video directory {}: {err}",
            parent.display()
        )
    })?;

    let (width, height, first_bgra) = image_frame_to_bgra(&frames[0], None)?;
    let frame_bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Browser video frame is too large".to_owned())?;
    let frame_duration_hns = 10_000_000i64 / fps as i64;
    let bitrate = (width as u64)
        .saturating_mul(height as u64)
        .saturating_mul(fps as u64)
        .saturating_mul(4)
        .clamp(1_500_000, 12_000_000) as u32;
    let wide_path: Vec<u16> = output_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_FULL)
            .map_err(|err| format!("Media Foundation startup failed: {err:?}"))?;

        let encode_result = (|| -> Result<(), String> {
            let output_type = MFCreateMediaType()
                .map_err(|err| format!("Media Foundation output type failed: {err:?}"))?;
            output_type
                .SetGUID(guid_attr(&MF_MT_MAJOR_TYPE), guid_attr(&MFMediaType_Video))
                .map_err(|err| format!("Media Foundation output major type failed: {err:?}"))?;
            output_type
                .SetGUID(guid_attr(&MF_MT_SUBTYPE), guid_attr(&MFVideoFormat_H264))
                .map_err(|err| format!("Media Foundation H.264 subtype failed: {err:?}"))?;
            output_type
                .SetUINT32(guid_attr(&MF_MT_AVG_BITRATE), bitrate)
                .map_err(|err| format!("Media Foundation bitrate failed: {err:?}"))?;
            output_type
                .SetUINT64(guid_attr(&MF_MT_FRAME_SIZE), ratio_u64(width, height))
                .map_err(|err| format!("Media Foundation frame size failed: {err:?}"))?;
            output_type
                .SetUINT64(guid_attr(&MF_MT_FRAME_RATE), ratio_u64(fps, 1))
                .map_err(|err| format!("Media Foundation frame rate failed: {err:?}"))?;
            output_type
                .SetUINT64(guid_attr(&MF_MT_PIXEL_ASPECT_RATIO), ratio_u64(1, 1))
                .map_err(|err| format!("Media Foundation pixel aspect failed: {err:?}"))?;
            output_type
                .SetUINT32(
                    guid_attr(&MF_MT_INTERLACE_MODE),
                    MFVideoInterlace_Progressive.0 as u32,
                )
                .map_err(|err| format!("Media Foundation interlace mode failed: {err:?}"))?;

            let writer = MFCreateSinkWriterFromURL(
                PCWSTR(wide_path.as_ptr()),
                None::<&IMFByteStream>,
                None::<&IMFAttributes>,
            )
            .map_err(|err| {
                format!(
                    "Media Foundation MP4 writer failed for {}: {err:?}",
                    output_path.display()
                )
            })?;
            let stream_index = writer
                .AddStream(&output_type)
                .map_err(|err| format!("Media Foundation stream setup failed: {err:?}"))?;

            let input_type = MFCreateMediaType()
                .map_err(|err| format!("Media Foundation input type failed: {err:?}"))?;
            input_type
                .SetGUID(guid_attr(&MF_MT_MAJOR_TYPE), guid_attr(&MFMediaType_Video))
                .map_err(|err| format!("Media Foundation input major type failed: {err:?}"))?;
            input_type
                .SetGUID(guid_attr(&MF_MT_SUBTYPE), guid_attr(&MFVideoFormat_RGB32))
                .map_err(|err| format!("Media Foundation RGB32 subtype failed: {err:?}"))?;
            input_type
                .SetUINT64(guid_attr(&MF_MT_FRAME_SIZE), ratio_u64(width, height))
                .map_err(|err| format!("Media Foundation input frame size failed: {err:?}"))?;
            input_type
                .SetUINT64(guid_attr(&MF_MT_FRAME_RATE), ratio_u64(fps, 1))
                .map_err(|err| format!("Media Foundation input frame rate failed: {err:?}"))?;
            input_type
                .SetUINT64(guid_attr(&MF_MT_PIXEL_ASPECT_RATIO), ratio_u64(1, 1))
                .map_err(|err| format!("Media Foundation input pixel aspect failed: {err:?}"))?;
            input_type
                .SetUINT32(
                    guid_attr(&MF_MT_INTERLACE_MODE),
                    MFVideoInterlace_Progressive.0 as u32,
                )
                .map_err(|err| format!("Media Foundation input interlace failed: {err:?}"))?;
            input_type
                .SetUINT32(guid_attr(&MF_MT_DEFAULT_STRIDE), width.saturating_mul(4))
                .map_err(|err| format!("Media Foundation input stride failed: {err:?}"))?;
            input_type
                .SetUINT32(guid_attr(&MF_MT_FIXED_SIZE_SAMPLES), 1)
                .map_err(|err| format!("Media Foundation fixed samples flag failed: {err:?}"))?;
            input_type
                .SetUINT32(guid_attr(&MF_MT_ALL_SAMPLES_INDEPENDENT), 1)
                .map_err(|err| {
                    format!("Media Foundation independent samples flag failed: {err:?}")
                })?;
            writer
                .SetInputMediaType(stream_index, &input_type, None::<&IMFAttributes>)
                .map_err(|err| format!("Media Foundation input media type failed: {err:?}"))?;
            writer
                .BeginWriting()
                .map_err(|err| format!("Media Foundation begin writing failed: {err:?}"))?;

            write_bgra_sample(
                &writer,
                stream_index,
                0,
                frame_duration_hns,
                &first_bgra,
                frame_bytes,
            )?;
            for (index, frame) in frames.iter().enumerate().skip(1) {
                let (_, _, bgra) = image_frame_to_bgra(frame, Some((width, height)))?;
                write_bgra_sample(
                    &writer,
                    stream_index,
                    (index as i64) * frame_duration_hns,
                    frame_duration_hns,
                    &bgra,
                    frame_bytes,
                )?;
            }

            writer
                .Finalize()
                .map_err(|err| format!("Media Foundation finalize failed: {err:?}"))?;
            Ok(())
        })();

        let shutdown_result = MFShutdown();
        if let Err(err) = shutdown_result {
            log::warn!("Media Foundation shutdown failed: {:?}", err);
        }
        encode_result
    }
}

#[cfg(target_os = "windows")]
unsafe fn write_bgra_sample(
    writer: &windows::Win32::Media::MediaFoundation::IMFSinkWriter,
    stream_index: u32,
    sample_time: i64,
    sample_duration: i64,
    bgra: &[u8],
    frame_bytes: u32,
) -> Result<(), String> {
    use std::ptr;

    use windows::Win32::Media::MediaFoundation::{MFCreateMemoryBuffer, MFCreateSample};

    if bgra.len() != frame_bytes as usize {
        return Err(format!(
            "Browser video frame byte length mismatch: expected {}, got {}",
            frame_bytes,
            bgra.len()
        ));
    }
    let buffer = MFCreateMemoryBuffer(frame_bytes)
        .map_err(|err| format!("Media Foundation buffer allocation failed: {err:?}"))?;
    let mut destination: *mut u8 = ptr::null_mut();
    buffer
        .Lock(&mut destination, None, None)
        .map_err(|err| format!("Media Foundation buffer lock failed: {err:?}"))?;
    ptr::copy_nonoverlapping(bgra.as_ptr(), destination, bgra.len());
    buffer
        .Unlock()
        .map_err(|err| format!("Media Foundation buffer unlock failed: {err:?}"))?;
    buffer
        .SetCurrentLength(frame_bytes)
        .map_err(|err| format!("Media Foundation buffer length failed: {err:?}"))?;
    let sample = MFCreateSample()
        .map_err(|err| format!("Media Foundation sample allocation failed: {err:?}"))?;
    sample
        .AddBuffer(&buffer)
        .map_err(|err| format!("Media Foundation sample buffer attach failed: {err:?}"))?;
    sample
        .SetSampleTime(sample_time)
        .map_err(|err| format!("Media Foundation sample time failed: {err:?}"))?;
    sample
        .SetSampleDuration(sample_duration)
        .map_err(|err| format!("Media Foundation sample duration failed: {err:?}"))?;
    writer
        .WriteSample(stream_index, &sample)
        .map_err(|err| format!("Media Foundation sample write failed: {err:?}"))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn encode_browser_video_mp4_platform(
    output_path: &Path,
    frames: &[BrowserVideoFrame],
    fps: u32,
) -> Result<(), String> {
    let _ = (output_path, frames, fps);
    Err("Browser MP4 recording is currently Windows-only".to_owned())
}

fn normalize_video_dimension(value: u32) -> u32 {
    let value = value.max(2);
    if value % 2 == 0 {
        value
    } else {
        value + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_video_dimension_returns_even_minimum() {
        assert_eq!(normalize_video_dimension(0), 2);
        assert_eq!(normalize_video_dimension(1), 2);
        assert_eq!(normalize_video_dimension(101), 102);
        assert_eq!(normalize_video_dimension(200), 200);
    }

    #[test]
    fn encode_browser_video_rejects_empty_frames() {
        let err = encode_browser_video_mp4(PathBuf::from("empty.mp4"), Vec::new(), Vec::new(), 10)
            .unwrap_err();

        assert!(err.contains("No browser video frames"));
    }
}
