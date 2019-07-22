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
use protobuf_impl::generate;

#[cfg(feature = "prost-codec")]
mod prost_impl;
#[cfg(feature = "prost-codec")]
use prost_impl::generate;

use lazy_static::lazy_static;
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

lazy_static! {
    static ref OUT_DIR: String = {
        format!(
            "{}/protos",
            std::env::var("OUT_DIR").expect("No OUT_DIR defined")
        )
    };
}

/// Generate Rust files from proto files (`files`).
pub fn generate_files<T: AsRef<Path> + Debug>(includes: &[T], files: &[T]) {
    prep_out_dir();
    generate(includes, files);
    generate_mod_files();
}

fn prep_out_dir() {
    if Path::new(&*OUT_DIR).exists() {
        fs::remove_dir_all(&*OUT_DIR).unwrap();
    }
    fs::create_dir_all(&*OUT_DIR).unwrap();
}

fn generate_mod_files() {
    let mut f = File::create(format!("{}/mod.rs", *OUT_DIR)).unwrap();

    let modules = list_rs_files().filter_map(|path| {
        let name = path.file_stem().unwrap().to_str().unwrap();
        if name.starts_with("wrapper_") {
            return None;
        }
        Some((name.replace('-', "_"), name.to_owned()))
    });

    for (module, file_name) in modules {
        if cfg!(feature = "protobuf-codec") {
            writeln!(f, "pub mod {};", module).unwrap();
            continue;
        }

        let mut level = 0;
        for part in module.split('.') {
            writeln!(f, "pub mod {} {{", part).unwrap();
            level += 1;
        }
        writeln!(f, "include!(\"{}.rs\");", file_name,).unwrap();
        if Path::new(&format!("{}/wrapper_{}.rs", *OUT_DIR, file_name)).exists() {
            writeln!(f, "include!(\"wrapper_{}.rs\");", file_name,).unwrap();
        }
        writeln!(f, "{}", "}\n".repeat(level)).unwrap();
    }
}

// List all `.rs` files in `OUT_DIR`.
fn list_rs_files() -> impl Iterator<Item = PathBuf> {
    fs::read_dir(&*OUT_DIR)
        .expect("Couldn't read directory")
        .filter_map(|e| {
            let path = e.expect("Couldn't list file").path();
            if path.extension() == Some(std::ffi::OsStr::new("rs")) {
                Some(path)
            } else {
                None
            }
        })
}
