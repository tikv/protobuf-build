// Copyright 2019 PingCAP, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// See the License for the specific language governing permissions and
// limitations under the License.

//! Utility functions for generating Rust code from protobuf specifications.
//!
//! These functions panic liberally, they are designed to be used from build
//! scripts, not in production.

#[cfg(feature = "prost-codec")]
pub use crate::wrapper::GenOpt;
use regex::Regex;
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

#[cfg(feature = "prost-codec")]
mod wrapper;

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
    replace_read_unknown_fields(&generated);
    let mut f = File::create(format!("{}/mod.rs", out_dir)).unwrap();
    for (module, file_name) in &modules {
        if !module.contains('.') {
            writeln!(f, "pub mod {};", module).unwrap();
            continue;
        }
        let mut level = 0;
        for part in module.split('.') {
            writeln!(f, "{:level$}pub mod {} {{", "", part, level = level).unwrap();
            level += 1;
        }
        writeln!(f, "include!(\"{}.rs\");", file_name).unwrap();
        for _ in (0..level).rev() {
            writeln!(f, "{:1$}}}", "", level).unwrap();
        }
    }
}

/// Use rust-protobuf to generate Rust files from proto files (`files`).
#[cfg(feature = "protobuf-codec")]
mod protobuf_imps {
    use regex::Regex;
    use std::env;
    use std::fmt::Debug;
    use std::path::Path;
    use std::process::Command;
    use std::str::from_utf8;

    pub fn get_protoc() -> String {
        let protoc_bin_name = match (env::consts::OS, env::consts::ARCH) {
            ("linux", "x86") => "protoc-linux-x86_32",
            ("linux", "x86_64") => "protoc-linux-x86_64",
            ("linux", "aarch64") => "protoc-linux-aarch_64",
            ("linux", "ppcle64") => "protoc-linux-ppcle_64",
            ("macos", "x86_64") => "protoc-osx-x86_64",
            ("windows", _) => "protoc-win32.exe",
            _ => return "protoc".to_owned(),
        };
        let bin_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("bin")
            .join(protoc_bin_name);
        format!("{}", bin_path.display())
    }

    /// Check that the user's installed version of the protobuf compiler is 3.1.x.
    pub fn check_protoc_version(protoc: &str) {
        let ver_re = Regex::new(r"([0-9]+)\.([0-9]+)\.[0-9]").unwrap();
        let ver = Command::new(protoc)
            .arg("--version")
            .output()
            .expect("Program `protoc` not installed (is it in PATH?).");
        let caps = ver_re.captures(from_utf8(&ver.stdout).unwrap()).unwrap();
        let major = caps.get(1).unwrap().as_str().parse::<i16>().unwrap();
        let minor = caps.get(2).unwrap().as_str().parse::<i16>().unwrap();
        if major == 3 && minor < 1 || major < 3 {
            panic!(
                "Invalid version of protoc (required at least 3.1.x, get {}.{}.x).",
                major, minor,
            );
        }
    }
    pub fn generate<T: AsRef<Path> + Debug>(includes: &[T], files: &[T], out_dir: &str) {
        check_protoc_version(&get_protoc());
        let mut cmd = Command::new(get_protoc());
        let desc_file = format!("{}/mod.desc", out_dir);
        for i in includes {
            cmd.arg(format!("-I{}", i.as_ref().display()));
        }
        cmd.arg("--include_imports")
            .arg("--include_source_info")
            .arg("-o")
            .arg(&desc_file);
        for f in files {
            cmd.arg(&format!("{}", f.as_ref().display()));
        }
        println!("executing {:?}", cmd);
        match cmd.status() {
            Ok(e) if e.success() => {}
            e => panic!("failed to generate descriptor set files: {:?}", e),
        }

        let desc_bytes = std::fs::read(&desc_file).unwrap();
        let desc: protobuf::descriptor::FileDescriptorSet =
            protobuf::parse_from_bytes(&desc_bytes).unwrap();
        let mut files_to_generate = Vec::new();
        'outer: for file in files {
            for include in includes {
                if let Some(truncated) = file.as_ref().strip_prefix(include).ok() {
                    files_to_generate.push(format!("{}", truncated.display()));
                    continue 'outer;
                }
            }

            panic!("file {:?} is not found in includes {:?}", file, includes);
        }

        protobuf_codegen::gen_and_write(
            desc.get_file(),
            &files_to_generate,
            &Path::new(out_dir),
            &protobuf_codegen::Customize::default(),
        )
        .unwrap();
        generate_grpcio(&desc.get_file(), &files_to_generate, out_dir);
    }

    #[cfg(feature = "grpcio-protobuf-codec")]
    pub fn generate_grpcio(
        desc: &[protobuf::descriptor::FileDescriptorProto],
        files_to_generate: &[String],
        out_dir: &str,
    ) {
        use std::io::Write;

        let output_dir = std::path::Path::new(out_dir);
        let results = grpcio_compiler::codegen::gen(desc, &files_to_generate);
        for res in results {
            let out_file = output_dir.join(&res.name);
            let mut f = std::fs::File::create(&out_file).unwrap();
            f.write_all(&res.content).unwrap();
        }
    }

    #[cfg(all(feature = "protobuf-codec", not(feature = "grpcio-protobuf-codec")))]
    pub fn generate_grpcio(_: &[protobuf::descriptor::FileDescriptorProto], _: &[String], _: &str) {
    }
}

#[cfg(feature = "protobuf-codec")]
pub use protobuf_imps::*;

/// Use prost to generate Rust files from proto files (`files`).
#[cfg(all(feature = "prost-codec", not(feature = "grpcio-prost-codec")))]
pub fn generate<T: AsRef<Path>>(includes: &[T], files: &[T], out_dir: &str) {
    prost_build::Config::new()
        .out_dir(out_dir)
        .compile_protos(files, includes)
        .unwrap();
    let mod_names = module_names_for_dir(out_dir);
    generate_wrappers(
        &mod_names
            .iter()
            .map(|m| format!("{}/{}.rs", out_dir, m))
            .collect::<Vec<_>>(),
        out_dir,
        GenOpt::MUT
            | GenOpt::HAS
            | GenOpt::TAKE
            | GenOpt::CLEAR
            | GenOpt::MESSAGE
            | GenOpt::TRIVIAL_SET,
    );
}

/// TODO: merge this with prost-codec.
#[cfg(feature = "grpcio-prost-codec")]
pub fn generate<T: AsRef<Path>>(includes: &[T], files: &[T], out_dir: &str) {
    let packages =
        grpcio_compiler::prost_codegen::compile_protos(files, includes, out_dir).unwrap();
    for package in &packages {
        let mut file_name = std::path::PathBuf::new();
        file_name.push(out_dir);
        file_name.push(&format!("{}.rs", package));
        rustfmt(&file_name);
    }
}

#[cfg(feature = "prost-codec")]
mod prost_imps {
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
}

#[cfg(feature = "prost-codec")]
pub use prost_imps::*;

/// Convert protobuf files to use the old way of reading protobuf enums.
// FIXME: Remove this once stepancheg/rust-protobuf#233 is resolved.
pub fn replace_read_unknown_fields<T: AsRef<str>>(file_names: &[T]) {
    let regex =
        Regex::new(r"::protobuf::rt::read_proto3_enum_with_unknown_fields_into\(([^,]+), ([^,]+), &mut ([^,]+), [^\)]+\)\?").unwrap();
    for file_name in file_names {
        let mut text = String::new();
        {
            let mut f = File::open(file_name.as_ref()).unwrap();
            f.read_to_string(&mut text)
                .expect("Couldn't read source file");
        }

        // FIXME Rustfmt bug in string literals
        #[rustfmt::skip]
            let text = {
            regex.replace_all(
                &text,
                "if $1 == ::protobuf::wire_format::WireTypeVarint {\
                    $3 = $2.read_enum()?;\
                 } else {\
                    return ::std::result::Result::Err(::protobuf::rt::unexpected_wire_type(wire_type));\
                 }",
            )
        };
        let mut out = File::create(file_name.as_ref()).unwrap();
        out.write_all(text.as_bytes())
            .expect("Could not write source file");
    }
}
