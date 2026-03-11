use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

const SOURCE_ICON_LOGO: &str = "logo-icon.png";
const OUTPUT_PNG: &str = "app-icon.png";
const OUTPUT_ICO: &str = "app-icon.ico";
const MASTER_ICON_SIZE: u32 = 1024;
const MASK_MARGIN_NUMERATOR: u32 = 1;
const MASK_MARGIN_DENOMINATOR: u32 = 100;
const CORNER_RADIUS_NUMERATOR: u32 = 18;
const CORNER_RADIUS_DENOMINATOR: u32 = 100;
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

fn main() {
    println!("cargo:rerun-if-changed={SOURCE_ICON_LOGO}");
    println!("cargo:rerun-if-changed=.toolchain/llvm-mingw-20260224-ucrt-x86_64/bin/windres.exe");
    println!("cargo:rerun-if-changed=.toolchain/llvm-mingw-20260224-ucrt-x86_64/bin/ar.exe");
    println!("cargo:rerun-if-env-changed=WindowsSdkDir");
    println!("cargo:rerun-if-env-changed=WindowsSdkVersion");
    println!("cargo:rerun-if-env-changed=ProgramFiles");
    println!("cargo:rerun-if-env-changed=ProgramFiles(x86)");
    println!("cargo:rerun-if-env-changed=PATH");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));
    let source_logo = manifest_dir.join(SOURCE_ICON_LOGO);
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
    let cropped = center_on_square_canvas(&source);
    let resized = imageops::resize(
        &cropped,
        MASTER_ICON_SIZE,
        MASTER_ICON_SIZE,
        FilterType::CatmullRom,
    );
    apply_rounded_mask(&resized)
}

fn center_on_square_canvas(image: &RgbaImage) -> RgbaImage {
    let side = image.width().max(image.height()).max(1);
    let mut canvas = RgbaImage::from_pixel(side, side, Rgba([0, 0, 0, 0]));
    let x = (side - image.width()) / 2;
    let y = (side - image.height()) / 2;
    imageops::overlay(&mut canvas, image, i64::from(x), i64::from(y));
    canvas
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
