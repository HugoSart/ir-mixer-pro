use image::{ImageFormat, imageops::FilterType};
use std::{
    env, fs,
    io::{Cursor, Write},
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    println!("cargo:rerun-if-changed=design/logos/desktop-icon.png");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let output = PathBuf::from(env::var("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let icon_path = output.join("ir-mixer-pro.ico");
    let resource_path = output.join("ir-mixer-pro.rc");
    let compiled_resource = output.join("ir-mixer-pro.res");
    create_ico(Path::new("design/logos/desktop-icon.png"), &icon_path)
        .expect("desktop icon should be convertible to ICO");
    fs::write(
        &resource_path,
        format!(
            "1 ICON \"{}\"\n",
            icon_path.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("Windows icon resource should be writable");

    let rc = find_resource_compiler().expect(
        "Windows SDK resource compiler (rc.exe) was not found; install the Windows SDK or set RC",
    );
    let status = Command::new(rc)
        .args(["/nologo", "/fo"])
        .arg(&compiled_resource)
        .arg(&resource_path)
        .status()
        .expect("Windows resource compiler should start");
    assert!(status.success(), "Windows resource compiler failed");
    println!("cargo:rustc-link-arg={}", compiled_resource.display());
}

fn create_ico(source: &Path, destination: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let source = image::open(source)?.to_rgba8();
    let sizes = [16_u32, 32, 48, 256];
    let mut images = Vec::with_capacity(sizes.len());
    for size in sizes {
        let resized = image::imageops::resize(&source, size, size, FilterType::Lanczos3);
        let mut png = Vec::new();
        resized.write_to(&mut Cursor::new(&mut png), ImageFormat::Png)?;
        images.push((size, png));
    }

    let directory_size = 6 + images.len() * 16;
    let mut file = Vec::with_capacity(directory_size + images.iter().map(|(_, png)| png.len()).sum::<usize>());
    file.write_all(&0_u16.to_le_bytes())?;
    file.write_all(&1_u16.to_le_bytes())?;
    file.write_all(&(images.len() as u16).to_le_bytes())?;
    let mut offset = directory_size as u32;
    for (size, png) in &images {
        file.push(if *size == 256 { 0 } else { *size as u8 });
        file.push(if *size == 256 { 0 } else { *size as u8 });
        file.extend_from_slice(&[0, 0]);
        file.write_all(&1_u16.to_le_bytes())?;
        file.write_all(&32_u16.to_le_bytes())?;
        file.write_all(&(png.len() as u32).to_le_bytes())?;
        file.write_all(&offset.to_le_bytes())?;
        offset += png.len() as u32;
    }
    for (_, png) in images {
        file.write_all(&png)?;
    }
    fs::write(destination, file)?;
    Ok(())
}

fn find_resource_compiler() -> Option<PathBuf> {
    if let Some(path) = env::var_os("RC").map(PathBuf::from).filter(|path| path.is_file()) {
        return Some(path);
    }
    let kits = env::var_os("ProgramFiles(x86)")?;
    let bin = PathBuf::from(kits).join("Windows Kits").join("10").join("bin");
    let architecture = match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86") => "x86",
        Ok("aarch64") => "arm64",
        _ => "x64",
    };
    let mut versions: Vec<_> = fs::read_dir(bin)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    versions.sort();
    versions.reverse();
    versions
        .into_iter()
        .map(|version| version.join(architecture).join("rc.exe"))
        .find(|path| path.is_file())
}
