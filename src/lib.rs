// Copyright 2019 PingCAP, Inc.

//! Utility functions for generating Rust code from protobuf specifications.
//!
//! These functions panic liberally, they are designed to be used from build
//! scripts, not in production.

#[cfg(feature = "prost-codec")]
mod wrapper;

#[cfg(feature = "prost-codec")]
pub use crate::wrapper::GenOpt;

#[cfg(feature = "protobuf-codec")]
mod protobuf_impl;
#[cfg(feature = "protobuf-codec")]
pub use protobuf_impl::*;

#[cfg(feature = "prost-codec")]
mod prost_impl;
#[cfg(feature = "prost-codec")]
pub use prost_impl::*;

use std::fmt::Debug;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

/// Generate Rust files from proto files (`files`).
pub fn generate_files<T: AsRef<Path> + Debug>(includes: &[T], files: &[T], out_dir: &str) {
    if Path::new(&out_dir).exists() {
        fs::remove_dir_all(&out_dir).unwrap();
    }
    fs::create_dir_all(&out_dir).unwrap();
    generate(includes, files, out_dir);
    let mut generated = Vec::new();
    let modules: Vec<_> = fs::read_dir(out_dir)
        .unwrap()
        .filter_map(|res| {
            let path = match res {
                Ok(e) => e.path(),
                Err(e) => panic!("failed to list {}: {:?}", out_dir, e),
            };
            if path.extension() == Some(std::ffi::OsStr::new("rs")) {
                generated.push(format!("{}", path.display()));
                let name = path.file_stem().unwrap().to_str().unwrap();
                Some((name.replace('-', "_"), name.to_owned()))
            } else {
                None
            }
        })
        .collect();
    let mut f = File::create(format!("{}/mod.rs", out_dir)).unwrap();
    for (module, file_name) in &modules {
        if cfg!(feature = "protobuf-codec") {
            writeln!(f, "pub mod {};", module).unwrap();
            continue;
        }
        if module.starts_with("wrapper_") {
            continue;
        }
        let mut level = 0;
        for part in module.split('.') {
            writeln!(f, "{:level$}pub mod {} {{", "", part, level = level).unwrap();
            level += 1;
        }
        writeln!(
            f,
            "{:level$}include!(\"{}.rs\");",
            "",
            file_name,
            level = level
        )
        .unwrap();
        if Path::new(&format!("{}/wrapper_{}.rs", out_dir, file_name)).exists() {
            writeln!(
                f,
                "{:level$}include!(\"wrapper_{}.rs\");",
                "",
                file_name,
                level = level
            )
            .unwrap();
        }
        for l in (0..level).rev() {
            writeln!(f, "{:1$}}}", "", l).unwrap();
        }
    }
}

/// Use prost to generate Rust files from proto files (`files`).
#[cfg(feature = "prost-codec")]
pub fn generate<T: AsRef<Path>>(includes: &[T], files: &[T], out_dir: &str) {
    #[cfg(feature = "grpcio-prost-codec")]
    {
        grpcio_compiler::prost_codegen::compile_protos(files, includes, out_dir).unwrap();
    }
    #[cfg(not(feature = "grpcio-prost-codec"))]
    {
        prost_build::Config::new()
            .out_dir(out_dir)
            .compile_protos(files, includes)
            .unwrap();
    }

    let mod_names = module_names_for_dir(out_dir);
    generate_wrappers(
        &mod_names
            .iter()
            .map(|m| format!("{}/{}.rs", out_dir, m))
            .collect::<Vec<_>>(),
        out_dir,
        GenOpt::all(),
    );
}
