use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Get the output directory from cargo
    let out_dir = env::var("OUT_DIR").unwrap();
    let _profile = env::var("PROFILE").unwrap();

    let config_path = Path::new("config.toml");
    let assets = Path::new("assets");
    let auth = Path::new("auth");
    let ai = Path::new("ai");

    let top_path = Path::new(&out_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();

    let mut support_path_string = top_path.to_str().unwrap().to_owned();
    support_path_string.push_str("/autohmjvis-support");

    let mut new_assets_path_string = support_path_string.clone();
    new_assets_path_string.push_str("/assets");

    let mut auth_path_string = support_path_string.clone();
    auth_path_string.push_str("/auth");

    let mut ai_path_string = support_path_string.clone();
    ai_path_string.push_str("/ai");

    // Create support subdirectory for this project
    fs::create_dir_all(&support_path_string).unwrap();

    // Copy config file
    let new_config_path = Path::new(&support_path_string).join("config.toml");
    fs::copy(config_path, new_config_path).unwrap();

    // Copy assets
    let new_assets_path = Path::new(&new_assets_path_string);
    copy_dir_all(assets, new_assets_path).unwrap();

    // Copy auth
    let new_auth_path = Path::new(&auth_path_string);
    copy_dir_all(auth, new_auth_path).unwrap();

    // Copy ai
    let new_ai_path = Path::new(&ai_path_string);
    copy_dir_all(ai, new_ai_path).unwrap();
}

fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}
