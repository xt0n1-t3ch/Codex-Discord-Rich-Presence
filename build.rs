#[cfg(target_os = "windows")]
use std::{
    env,
    fs::File,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-changed=assets/branding/codex-app.png");

    #[cfg(target_os = "windows")]
    {
        if let Err(err) = configure_windows_icon() {
            panic!("failed to configure Windows executable icon: {err}");
        }
    }
}

#[cfg(target_os = "windows")]
fn configure_windows_icon() -> Result<(), String> {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").map_err(|err| err.to_string())?);
    let source_png = manifest_dir
        .join("assets")
        .join("branding")
        .join("codex-app.png");
    if !source_png.exists() {
        return Err(format!(
            "missing icon source image at {}",
            source_png.display()
        ));
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").map_err(|err| err.to_string())?);
    let icon_path = out_dir.join("codex-app.ico");
    write_multi_size_ico(&source_png, &icon_path)?;

    let mut resources = winres::WindowsResource::new();
    resources.set_icon(icon_path.to_string_lossy().as_ref());
    resources.compile().map_err(|err| err.to_string())
}

#[cfg(target_os = "windows")]
fn write_multi_size_ico(source_png: &Path, icon_path: &Path) -> Result<(), String> {
    let image = image::ImageReader::open(source_png)
        .map_err(|err| err.to_string())?
        .decode()
        .map_err(|err| err.to_string())?
        .into_rgba8();

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [256u32, 128, 64, 48, 32, 24, 16] {
        let resized = if image.width() == size && image.height() == size {
            image.clone()
        } else {
            image::imageops::resize(&image, size, size, image::imageops::FilterType::Lanczos3)
        };
        let icon_image = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        let entry = ico::IconDirEntry::encode(&icon_image).map_err(|err| err.to_string())?;
        icon_dir.add_entry(entry);
    }

    let mut file = File::create(icon_path).map_err(|err| err.to_string())?;
    icon_dir.write(&mut file).map_err(|err| err.to_string())
}
