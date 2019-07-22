use super::wrapper::GenOpt;
use crate::OUT_DIR;
use std::fs;
use std::path::Path;

pub fn generate<T: AsRef<Path>>(includes: &[T], files: &[T]) {
    #[cfg(feature = "grpcio-prost-codec")]
    {
        grpcio_compiler::prost_codegen::compile_protos(files, includes, &*OUT_DIR).unwrap();
    }
    #[cfg(not(feature = "grpcio-prost-codec"))]
    {
        prost_build::Config::new()
            .out_dir(&*OUT_DIR)
            .compile_protos(files, includes)
            .unwrap();
    }

    let mod_names = module_names_for_dir(&*OUT_DIR);
    generate_wrappers(
        &mod_names
            .iter()
            .map(|m| format!("{}/{}.rs", *OUT_DIR, m))
            .collect::<Vec<_>>(),
        GenOpt::all(),
    );
}

fn generate_wrappers<T: AsRef<str>>(file_names: &[T], gen_opt: GenOpt) {
    for file in file_names {
        let gen = super::wrapper::WrapperGen::new(file.as_ref(), gen_opt);
        gen.write();
    }
}

/// Returns a list of module names corresponding to the Rust files in a directory.
///
/// Note that this does not read the files so will miss inline modules, it only
/// looks at filenames,
fn module_names_for_dir(directory_name: &str) -> Vec<String> {
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
