// Copyright 2019 PingCAP, Inc.

//! Utility functions for generating Rust code from protobuf specifications.
//!
//! These functions panic liberally, they are designed to be used from build
//! scripts, not in production.

#[cfg(feature = "prost-codec")]
mod wrapper;

#[cfg(feature = "protobuf-codec")]
mod protobuf_impl;

#[cfg(feature = "prost-codec")]
mod prost_impl;

use bitflags::bitflags;
use lazy_static::lazy_static;
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

pub struct Builder {
    files: Vec<String>,
    includes: Vec<String>,
    include_black_list: Vec<String>,
    wrapper_opts: GenOpt,
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            files: Vec::new(),
            includes: vec!["include".to_owned(), "proto".to_owned()],
            include_black_list: vec![
                "protobuf".to_owned(),
                "google".to_owned(),
                "gogoproto".to_owned(),
            ],
            wrapper_opts: GenOpt::all(),
        }
    }

    pub fn generate(&self) {
        assert!(!self.files.is_empty(), "No files specified for generation");
        prep_out_dir();
        self.generate_files();
        self.generate_mod_file();
    }

    pub fn wrapper_options(&mut self, wrapper_opts: GenOpt) -> &mut Self {
        self.wrapper_opts = wrapper_opts;
        self
    }

    /// Finds proto files to operate on in the `proto_dir` directory.
    pub fn search_dir_for_protos(&mut self, proto_dir: &str) -> &mut Self {
        self.files = fs::read_dir(proto_dir)
            .expect("Couldn't read proto directory")
            .filter_map(|e| {
                let e = e.expect("Couldn't list file");
                if e.file_type().expect("File broken").is_dir() {
                    None
                } else {
                    Some(format!("{}/{}", proto_dir, e.file_name().to_string_lossy()))
                }
            })
            .collect();
        self
    }

    pub fn files<T: Into<String> + Clone>(&mut self, files: &[T]) -> &mut Self {
        self.files = files
            .iter()
            .map(|t| t.clone().into())
            .collect::<Vec<String>>();
        self
    }

    pub fn includes(&mut self, includes: Vec<String>) -> &mut Self {
        self.includes = includes;
        self
    }

    pub fn append_include(&mut self, include: String) -> &mut Self {
        self.includes.push(include);
        self
    }

    pub fn include_black_list(&mut self, include_black_list: Vec<String>) -> &mut Self {
        self.include_black_list = include_black_list;
        self
    }

    pub fn append_black_listed_include(&mut self, include: String) -> &mut Self {
        self.include_black_list.push(include);
        self
    }

    fn generate_mod_file(&self) {
        let mut f = File::create(format!("{}/mod.rs", *OUT_DIR)).unwrap();

        let modules = list_rs_files().filter_map(|path| {
            let name = path.file_stem().unwrap().to_str().unwrap();
            if name.starts_with("wrapper_")
                || name == "mod"
                || self.include_black_list.iter().any(|i| name.contains(i))
            {
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
}

impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}

fn prep_out_dir() {
    if Path::new(&*OUT_DIR).exists() {
        fs::remove_dir_all(&*OUT_DIR).unwrap();
    }
    fs::create_dir_all(&*OUT_DIR).unwrap();
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

bitflags! {
    pub struct GenOpt: u32 {
        /// Generate implementation for trait `::protobuf::Message`.
        const MESSAGE = 0b0000_0001;
        /// Generate getters.
        const TRIVIAL_GET = 0b0000_0010;
        /// Generate setters.
        const TRIVIAL_SET = 0b0000_0100;
        /// Generate the `new_` constructors.
        const NEW = 0b0000_1000;
        /// Generate `clear_*` functions.
        const CLEAR = 0b0001_0000;
        /// Generate `has_*` functions.
        const HAS = 0b0010_0000;
        /// Generate mutable getters.
        const MUT = 0b0100_0000;
        /// Generate `take_*` functions.
        const TAKE = 0b1000_0000;
        /// Except `impl protobuf::Message`.
        const NO_MSG = Self::TRIVIAL_GET.bits
         | Self::TRIVIAL_SET.bits
         | Self::CLEAR.bits
         | Self::HAS.bits
         | Self::MUT.bits
         | Self::TAKE.bits;
        /// Except `new_` and `impl protobuf::Message`.
        const ACCESSOR = Self::TRIVIAL_GET.bits
         | Self::TRIVIAL_SET.bits
         | Self::MUT.bits
         | Self::TAKE.bits;
    }
}
