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

use regex::Regex;
use std::fs::read_dir;
use std::fs::File;
use std::io::{Read, Write};
use std::process::Command;
use std::str::from_utf8;

/// Check that the user's installed version of the protobuf compiler is 3.1.x.
pub fn check_protoc_version() {
    let ver_re = Regex::new(r"([0-9]+)\.([0-9]+)\.[0-9]").unwrap();
    let ver = Command::new("protoc")
        .arg("--version")
        .output()
        .expect("Program `protoc` not installed (is it in PATH?).");
    let caps = ver_re.captures(from_utf8(&ver.stdout).unwrap()).unwrap();
    let major = caps.get(1).unwrap().as_str().parse::<i16>().unwrap();
    let minor = caps.get(2).unwrap().as_str().parse::<i16>().unwrap();
    if major == 3 && minor < 1 || major < 3 {
        panic!(
            "Invalid version of protoc (required 3.1.x, get {}.{}.x).",
            major, minor,
        );
    }
}

/// Use protobuf-rs to generate Rust files from proto files (`file_names`).
///
/// Uses `["proto", "include"]` as the include lists.
pub fn generate_protobuf_files(file_names: Vec<&str>, out_dir: &str) {
    protoc_rust::run(protoc_rust::Args {
        out_dir,
        input: &file_names,
        includes: &["proto", "include"],
        customize: protoc_rust::Customize {
            ..Default::default()
        },
    })
    .unwrap();

    protoc_grpcio::compile_grpc_protos(file_names, &["proto", "include"], out_dir).unwrap();
}

/// Returns a list of module names corresponding to the Rust files in a directory.
///
/// Note that this does not read the files so will miss inline modules, it only
/// looks at filenames,
pub fn module_names_for_dir(directory_name: &str) -> Vec<String> {
    let mut mod_names: Vec<_> = read_dir(directory_name)
        .expect("Couldn't read directory")
        .filter_map(|e| {
            let file_name = e.expect("Couldn't list file").file_name();
            file_name
                .to_string_lossy()
                .split(".rs")
                .next()
                .map(|n| n.to_owned())
        })
        .collect();

    mod_names.sort();
    mod_names
}

/// Convert protobuf files to use the old way of reading protobuf enums.
// FIXME: Remove this once stepancheg/rust-protobuf#233 is resolved.
pub fn replace_read_unknown_fields(file_names: &[String]) {
    let regex =
        Regex::new(r"::protobuf::rt::read_proto3_enum_with_unknown_fields_into\(([^,]+), ([^,]+), &mut ([^,]+), [^\)]+\)\?").unwrap();
    for file_name in file_names {
        let mut text = String::new();
        {
            let mut f = File::open(file_name).unwrap();
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
        let mut out = File::create(file_name).unwrap();
        out.write_all(text.as_bytes())
            .expect("Could not write source file");
    }
}

/// Generate module declarations for `mod_names` and write it to `output_file_name`.
pub fn generate_protobuf_rs(mod_names: &[String], output_file_name: &str) {
    let mut text = String::new();
    for mod_name in mod_names {
        text.push_str("pub mod ");
        text.push_str(mod_name);
        text.push_str(";\n");
    }

    let mut lib =
        File::create(output_file_name).expect(&format!("Could not create {}", output_file_name));
    lib.write_all(text.as_bytes())
        .expect(&format!("Could not write {}", output_file_name));
}
