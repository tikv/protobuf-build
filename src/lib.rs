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

use lazy_static::lazy_static;
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

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
    if Path::new(&*OUT_DIR).exists() {
        fs::remove_dir_all(&*OUT_DIR).unwrap();
    }
    fs::create_dir_all(&*OUT_DIR).unwrap();
    generate(includes, files);
    let mut generated = Vec::new();
    let modules: Vec<_> = fs::read_dir(&*OUT_DIR)
        .unwrap()
        .filter_map(|res| {
            let path = match res {
                Ok(e) => e.path(),
                Err(e) => panic!("failed to list {}: {:?}", *OUT_DIR, e),
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
    let mut f = File::create(format!("{}/mod.rs", *OUT_DIR)).unwrap();
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
        if Path::new(&format!("{}/wrapper_{}.rs", *OUT_DIR, file_name)).exists() {
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
