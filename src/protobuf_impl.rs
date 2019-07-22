// Copyright 2019 PingCAP, Inc.

use regex::Regex;
use std::env;
use std::fmt::Debug;
use std::fs::{self, File};
use std::io::{Read, Write};
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
            if let Ok(truncated) = file.as_ref().strip_prefix(include) {
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
    replace_read_unknown_fields(out_dir);
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
pub fn generate_grpcio(_: &[protobuf::descriptor::FileDescriptorProto], _: &[String], _: &str) {}

/// Convert protobuf files to use the old way of reading protobuf enums.
// FIXME: Remove this once stepancheg/rust-protobuf#233 is resolved.
pub fn replace_read_unknown_fields(out_dir: &str) {
    let regex =
        Regex::new(r"::protobuf::rt::read_proto3_enum_with_unknown_fields_into\(([^,]+), ([^,]+), &mut ([^,]+), [^\)]+\)\?").unwrap();
    for f in fs::read_dir(out_dir).unwrap() {
        let path = match f {
            Ok(p) => p.path(),
            Err(e) => panic!("failed to list {}: {:?}", out_dir, e),
        };
        if path.extension() != Some(std::ffi::OsStr::new("rs")) {
            continue;
        }

        let mut text = String::new();
        let mut f = File::open(&path).unwrap();
        f.read_to_string(&mut text)
            .expect("Couldn't read source file");

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
        let mut out = File::create(&path).unwrap();
        out.write_all(text.as_bytes())
            .expect("Could not write source file");
    }
}
