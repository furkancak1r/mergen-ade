use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

const SOURCE_LOGO: &str = "logo.png";
const OUTPUT_PNG: &str = "app-icon.png";
const OUTPUT_ICO: &str = "app-icon.ico";
const MASTER_ICON_SIZE: u32 = 1024;
const PREVIEW_SIZE: u32 = 512;
const MASK_THRESHOLD: u16 = 42;
const CROP_EXPANSION_NUMERATOR: u32 = 18;
const CROP_EXPANSION_DENOMINATOR: u32 = 100;
const MASK_MARGIN_NUMERATOR: u32 = 3;
const MASK_MARGIN_DENOMINATOR: u32 = 100;
const CORNER_RADIUS_NUMERATOR: u32 = 18;
const CORNER_RADIUS_DENOMINATOR: u32 = 100;
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

fn main() {
    println!("cargo:rerun-if-changed={SOURCE_LOGO}");
    println!("cargo:rerun-if-changed=.toolchain/llvm-mingw-20260224-ucrt-x86_64/bin/windres.exe");
    println!("cargo:rerun-if-changed=.toolchain/llvm-mingw-20260224-ucrt-x86_64/bin/ar.exe");
    println!("cargo:rerun-if-env-changed=WindowsSdkDir");
    println!("cargo:rerun-if-env-changed=WindowsSdkVersion");
    println!("cargo:rerun-if-env-changed=ProgramFiles");
    println!("cargo:rerun-if-env-changed=ProgramFiles(x86)");
    println!("cargo:rerun-if-env-changed=PATH");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));
    let source_logo = manifest_dir.join(SOURCE_LOGO);
    let icon_png = out_dir.join(OUTPUT_PNG);
    let icon_ico = out_dir.join(OUTPUT_ICO);

    let icon = prepare_icon(&source_logo);
    icon.save(&icon_png)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", icon_png.display()));
    write_ico(&icon, &icon_ico);

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target = env::var("TARGET").unwrap_or_default();
    let is_test_build = env::var_os("CARGO_CFG_TEST").is_some();
    if target_os == "windows" && !is_test_build {
        embed_windows_icon(&manifest_dir, &target, &icon_ico);
    }
}

fn prepare_icon(source_logo: &Path) -> RgbaImage {
    let source = image::open(source_logo)
        .unwrap_or_else(|err| panic!("failed to open {}: {err}", source_logo.display()))
        .into_rgba8();
    let focus = detect_focus_bounds(&source);
    let cropped = crop_focus_to_square(&source, focus);
    let resized = imageops::resize(
        &cropped,
        MASTER_ICON_SIZE,
        MASTER_ICON_SIZE,
        FilterType::CatmullRom,
    );
    apply_rounded_mask(&resized)
}

fn detect_focus_bounds(image: &RgbaImage) -> (u32, u32, u32, u32) {
    let preview = imageops::resize(image, PREVIEW_SIZE, PREVIEW_SIZE, FilterType::Triangle);
    let bg = average_corner_color(&preview);
    let width = preview.width();
    let height = preview.height();
    let mut mask = vec![false; (width * height) as usize];

    for y in 0..height {
        for x in 0..width {
            let pixel = preview.get_pixel(x, y);
            let diff = color_distance(pixel, bg);
            mask[(y * width + x) as usize] = diff > MASK_THRESHOLD;
        }
    }

    let seed = find_seed_near_center(&mask, width, height).unwrap_or((width / 2, height / 2));
    let bounds = flood_fill_bounds(&mask, width, height, seed);

    let scale_x = image.width() as f32 / width as f32;
    let scale_y = image.height() as f32 / height as f32;

    let min_x = (bounds.0 as f32 * scale_x).floor() as u32;
    let min_y = (bounds.1 as f32 * scale_y).floor() as u32;
    let max_x = ((bounds.2 + 1) as f32 * scale_x).ceil() as u32;
    let max_y = ((bounds.3 + 1) as f32 * scale_y).ceil() as u32;

    (
        min_x.min(image.width().saturating_sub(1)),
        min_y.min(image.height().saturating_sub(1)),
        max_x.min(image.width()),
        max_y.min(image.height()),
    )
}

fn crop_focus_to_square(image: &RgbaImage, bounds: (u32, u32, u32, u32)) -> RgbaImage {
    let (min_x, min_y, max_x, max_y) = bounds;
    let focus_w = max_x.saturating_sub(min_x).max(1);
    let focus_h = max_y.saturating_sub(min_y).max(1);
    let expanded_w = focus_w + focus_w * CROP_EXPANSION_NUMERATOR / CROP_EXPANSION_DENOMINATOR;
    let expanded_h = focus_h + focus_h * CROP_EXPANSION_NUMERATOR / CROP_EXPANSION_DENOMINATOR;
    let side = expanded_w
        .max(expanded_h)
        .min(image.width())
        .min(image.height());
    let center_x = min_x as i64 + focus_w as i64 / 2;
    let center_y = min_y as i64 + focus_h as i64 / 2;
    let half = side as i64 / 2;

    let mut left = center_x - half;
    let mut top = center_y - half;
    let max_left = image.width() as i64 - side as i64;
    let max_top = image.height() as i64 - side as i64;
    left = left.clamp(0, max_left.max(0));
    top = top.clamp(0, max_top.max(0));

    imageops::crop_imm(image, left as u32, top as u32, side, side).to_image()
}

fn apply_rounded_mask(image: &RgbaImage) -> RgbaImage {
    let size = image.width().min(image.height());
    let margin = size * MASK_MARGIN_NUMERATOR / MASK_MARGIN_DENOMINATOR;
    let radius = size * CORNER_RADIUS_NUMERATOR / CORNER_RADIUS_DENOMINATOR;
    let mut masked = RgbaImage::new(image.width(), image.height());

    for y in 0..image.height() {
        for x in 0..image.width() {
            let inside = point_inside_rounded_rect(x, y, size, margin, radius);
            if inside {
                masked.put_pixel(x, y, *image.get_pixel(x, y));
            } else {
                masked.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }

    masked
}

fn point_inside_rounded_rect(x: u32, y: u32, size: u32, margin: u32, radius: u32) -> bool {
    let x = x as i64;
    let y = y as i64;
    let size = size as i64;
    let margin = margin as i64;
    let radius = radius.max(1) as i64;
    let left = margin;
    let top = margin;
    let right = size - margin - 1;
    let bottom = size - margin - 1;

    if x < left || x > right || y < top || y > bottom {
        return false;
    }

    if x >= left + radius && x <= right - radius {
        return true;
    }

    if y >= top + radius && y <= bottom - radius {
        return true;
    }

    let nearest_x = if x < left + radius {
        left + radius
    } else {
        right - radius
    };
    let nearest_y = if y < top + radius {
        top + radius
    } else {
        bottom - radius
    };
    let dx = x - nearest_x;
    let dy = y - nearest_y;
    dx * dx + dy * dy <= radius * radius
}

fn average_corner_color(image: &RgbaImage) -> [u8; 3] {
    let sample = (image.width().min(image.height()) / 10).max(8);
    let corners = [
        (0, 0),
        (image.width().saturating_sub(sample), 0),
        (0, image.height().saturating_sub(sample)),
        (
            image.width().saturating_sub(sample),
            image.height().saturating_sub(sample),
        ),
    ];
    let mut total = [0u64; 3];
    let mut count = 0u64;

    for (start_x, start_y) in corners {
        for y in start_y..(start_y + sample).min(image.height()) {
            for x in start_x..(start_x + sample).min(image.width()) {
                let pixel = image.get_pixel(x, y);
                total[0] += pixel[0] as u64;
                total[1] += pixel[1] as u64;
                total[2] += pixel[2] as u64;
                count += 1;
            }
        }
    }

    [
        (total[0] / count) as u8,
        (total[1] / count) as u8,
        (total[2] / count) as u8,
    ]
}

fn color_distance(pixel: &Rgba<u8>, bg: [u8; 3]) -> u16 {
    (pixel[0].abs_diff(bg[0]) as u16)
        + (pixel[1].abs_diff(bg[1]) as u16)
        + (pixel[2].abs_diff(bg[2]) as u16)
}

fn find_seed_near_center(mask: &[bool], width: u32, height: u32) -> Option<(u32, u32)> {
    let center_x = width / 2;
    let center_y = height / 2;

    for radius in 0..(width.max(height) / 2) {
        let min_x = center_x.saturating_sub(radius);
        let max_x = (center_x + radius).min(width.saturating_sub(1));
        let min_y = center_y.saturating_sub(radius);
        let max_y = (center_y + radius).min(height.saturating_sub(1));

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if mask[(y * width + x) as usize] {
                    return Some((x, y));
                }
            }
        }
    }

    None
}

fn flood_fill_bounds(
    mask: &[bool],
    width: u32,
    height: u32,
    seed: (u32, u32),
) -> (u32, u32, u32, u32) {
    let mut seen = vec![false; mask.len()];
    let mut queue = std::collections::VecDeque::from([seed]);
    let mut min_x = seed.0;
    let mut min_y = seed.1;
    let mut max_x = seed.0;
    let mut max_y = seed.1;

    while let Some((x, y)) = queue.pop_front() {
        let index = (y * width + x) as usize;
        if seen[index] || !mask[index] {
            continue;
        }
        seen[index] = true;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);

        if x > 0 {
            queue.push_back((x - 1, y));
        }
        if x + 1 < width {
            queue.push_back((x + 1, y));
        }
        if y > 0 {
            queue.push_back((x, y - 1));
        }
        if y + 1 < height {
            queue.push_back((x, y + 1));
        }
    }

    (min_x, min_y, max_x, max_y)
}

fn write_ico(icon: &RgbaImage, output_path: &Path) {
    let mut icon_dir = IconDir::new(ResourceType::Icon);
    for size in ICO_SIZES {
        let resized = imageops::resize(icon, size, size, FilterType::Lanczos3);
        let icon_image = IconImage::from_rgba_data(size, size, resized.into_raw());
        let entry = IconDirEntry::encode(&icon_image)
            .unwrap_or_else(|err| panic!("failed to encode {size}x{size} icon: {err}"));
        icon_dir.add_entry(entry);
    }

    let mut file = File::create(output_path)
        .unwrap_or_else(|err| panic!("failed to create {}: {err}", output_path.display()));
    icon_dir
        .write(&mut file)
        .unwrap_or_else(|err| panic!("failed to write {}: {err}", output_path.display()));
}

fn embed_windows_icon(manifest_dir: &Path, target: &str, icon_ico: &Path) {
    let mut resource = winres::WindowsResource::new();
    if target.contains("-windows-gnu") {
        if !configure_gnu_toolkit(&mut resource, manifest_dir) {
            println!(
                "cargo:warning=Skipping GNU exe icon embedding because repo-local windres.exe/ar.exe were not found."
            );
            return;
        }
    } else if target.contains("-windows-msvc") {
        if !configure_msvc_toolkit(&mut resource) {
            println!(
                "cargo:warning=Skipping MSVC exe icon embedding because rc.exe was not found in the current shell or installed Windows SDK locations."
            );
            return;
        }
    }
    resource.set_icon(icon_ico.to_string_lossy().as_ref());
    resource
        .compile()
        .unwrap_or_else(|err| panic!("failed to compile Windows icon resource: {err}"));
}

fn configure_gnu_toolkit(resource: &mut winres::WindowsResource, manifest_dir: &Path) -> bool {
    let toolkit_bin = manifest_dir.join(".toolchain/llvm-mingw-20260224-ucrt-x86_64/bin");
    let windres_path = toolkit_bin.join("windres.exe");
    let ar_path = toolkit_bin.join("ar.exe");

    if !windres_path.exists() || !ar_path.exists() {
        return false;
    }

    resource
        .set_toolkit_path(toolkit_bin.to_string_lossy().as_ref())
        .set_windres_path(windres_path.to_string_lossy().as_ref())
        .set_ar_path(ar_path.to_string_lossy().as_ref());
    true
}

fn configure_msvc_toolkit(resource: &mut winres::WindowsResource) -> bool {
    let Some(toolkit_path) =
        resolve_msvc_toolkit_path().or_else(resolve_msvc_toolkit_path_from_path)
    else {
        return false;
    };

    resource.set_toolkit_path(toolkit_path.to_string_lossy().as_ref());
    true
}

fn resolve_msvc_toolkit_path() -> Option<PathBuf> {
    resolve_msvc_toolkit_path_from_env().or_else(resolve_msvc_toolkit_path_from_windows_kits)
}

fn resolve_msvc_toolkit_path_from_env() -> Option<PathBuf> {
    let sdk_dir = env::var_os("WindowsSdkDir").map(PathBuf::from)?;
    if let Some(version) = env::var_os("WindowsSdkVersion") {
        let trimmed = version
            .to_string_lossy()
            .trim_matches(['\\', '/'])
            .to_owned();
        if !trimmed.is_empty() {
            let candidate = sdk_dir.join("bin").join(trimmed).join("x64");
            if candidate.join("rc.exe").exists() {
                return Some(candidate);
            }
        }
    }

    let candidate = sdk_dir.join("bin").join("x64");
    candidate.join("rc.exe").exists().then_some(candidate)
}

fn resolve_msvc_toolkit_path_from_windows_kits() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    for root in windows_kits_roots() {
        let bin_root = root.join("Windows Kits").join("10").join("bin");
        let Ok(entries) = std::fs::read_dir(&bin_root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let candidate = path.join("x64");
            if candidate.join("rc.exe").exists() {
                candidates.push(candidate);
            }
        }
    }

    candidates.sort();
    candidates.pop()
}

fn windows_kits_roots() -> Vec<PathBuf> {
    [
        env::var_os("ProgramFiles(x86)").map(PathBuf::from),
        env::var_os("ProgramFiles").map(PathBuf::from),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn resolve_msvc_toolkit_path_from_path() -> Option<PathBuf> {
    let path_value = env::var_os("PATH")?;
    for entry in env::split_paths(&path_value) {
        let candidate = entry.join("rc.exe");
        if candidate.exists() {
            return Some(entry);
        }
    }

    None
}
