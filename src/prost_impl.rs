use super::wrapper::GenOpt;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn rustfmt(file_path: &Path) {
    let output = Command::new("rustfmt")
        .arg(file_path.to_str().unwrap())
        .output();
    if !output.map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("Rustfmt failed");
    }
}

pub fn generate_wrappers<T: AsRef<str>>(file_names: &[T], out_dir: &str, gen_opt: GenOpt) {
    for file in file_names {
        let gen = super::wrapper::WrapperGen::new(file.as_ref(), gen_opt);
        gen.write(out_dir);
    }
}

/// Returns a list of module names corresponding to the Rust files in a directory.
///
/// Note that this does not read the files so will miss inline modules, it only
/// looks at filenames,
pub fn module_names_for_dir(directory_name: &str) -> Vec<String> {
    let mut mod_names: Vec<_> = fs::read_dir(directory_name)
        .expect("Couldn't read directory")
        .filter_map(|e| {
            let file_name = e.expect("Couldn't list file").file_name();
            let file_name = file_name.to_string_lossy();
            if !file_name.ends_with(".rs") {
                return None;
            }
            file_name
                .split(".rs")
                .next()
                .filter(|n| !n.starts_with("wrapper_"))
                .map(ToOwned::to_owned)
        })
        .collect();

    mod_names.sort();
    mod_names
}
