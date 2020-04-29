// Copyright 2019 PingCAP, Inc.

use std::env;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::process::Command;
use std::str::from_utf8;

use regex::Regex;

use crate::Builder;

// We use system protoc when its version matches,
// otherwise use the protoc from bin which we bundle with the crate.
fn get_protoc() -> String {
    // $PROTOC overrides everything; if it isn't a useful version then fail.
    if let Ok(s) = env::var("PROTOC") {
        check_protoc_version(&s).expect("PROTOC version not usable");
        return s;
    }
    if let Ok(s) = check_protoc_version("protoc") {
        return s;
    }

    // The bundled protoc should always match the version
    let protoc_bin_name = match (env::consts::OS, env::consts::ARCH) {
        ("linux", "x86") => "protoc-linux-x86_32",
        ("linux", "x86_64") => "protoc-linux-x86_64",
        ("linux", "aarch64") => "protoc-linux-aarch_64",
        ("linux", "powerpc64") => "protoc-linux-ppcle_64",
        ("macos", "x86_64") => "protoc-osx-x86_64",
        ("windows", _) => "protoc-win32.exe",
        _ => panic!("No suitable `protoc` (>= 3.1.0) found in PATH"),
    };
    let bin_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("bin")
        .join(protoc_bin_name);
    bin_path.display().to_string()
}

fn check_protoc_version(protoc: &str) -> Result<String, ()> {
    let ver_re = Regex::new(r"([0-9]+)\.([0-9]+)\.[0-9]").unwrap();
    let output = Command::new(protoc).arg("--version").output();
    match output {
        Ok(o) => {
            let caps = ver_re.captures(from_utf8(&o.stdout).unwrap()).unwrap();
            let major = caps.get(1).unwrap().as_str().parse::<i16>().unwrap();
            let minor = caps.get(2).unwrap().as_str().parse::<i16>().unwrap();
            if major == 3 && minor >= 1 {
                return Ok(protoc.to_owned());
            }
            println!("The system `protoc` version mismatch, require >= 3.1.0, got {}.{}.x, fallback to the bundled `protoc`", major, minor);
        }
        Err(_) => println!("`protoc` not in PATH, try using the bundled protoc"),
    };

    Err(())
}

impl Builder {
    pub fn generate_files(&self) {
        let mut cmd = Command::new(get_protoc());
        let desc_file = format!("{}/mod.desc", self.out_dir);
        for i in &self.includes {
            cmd.arg(format!("-I{}", i));
        }
        cmd.arg("--include_imports")
            .arg("--include_source_info")
            .arg("-o")
            .arg(&desc_file);
        for f in &self.files {
            cmd.arg(f);
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
        'outer: for file in &self.files {
            for include in &self.includes {
                if let Ok(truncated) = Path::new(file).strip_prefix(include) {
                    files_to_generate.push(format!("{}", truncated.display()));
                    continue 'outer;
                }
            }

            panic!(
                "file {:?} is not found in includes {:?}",
                file, self.includes
            );
        }

        protobuf_codegen::gen_and_write(
            desc.get_file(),
            &files_to_generate,
            &Path::new(&self.out_dir),
            &protobuf_codegen::Customize::default(),
        )
        .unwrap();
        self.generate_grpcio(&desc.get_file(), &files_to_generate);
        self.import_grpcio();
        self.replace_read_unknown_fields();
    }

    /// Convert protobuf files to use the old way of reading protobuf enums.
    // FIXME: Remove this once stepancheg/rust-protobuf#233 is resolved.
    fn replace_read_unknown_fields(&self) {
        let regex =
            Regex::new(r"::protobuf::rt::read_proto3_enum_with_unknown_fields_into\(([^,]+), ([^,]+), &mut ([^,]+), [^\)]+\)\?").unwrap();
        self.list_rs_files().for_each(|path| {
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
        });
    }

    #[cfg(feature = "grpcio-protobuf-codec")]
    fn import_grpcio(&self) {
        use std::collections::BTreeMap;
        use std::fs::OpenOptions;

        if !self.re_export_services {
            return;
        }

        // TODO should be behind an option
        let paths: BTreeMap<_, _> = self
            .list_rs_files()
            .map(|path| (path.file_stem().unwrap().to_str().unwrap().to_owned(), path))
            .collect();
        for (name, path) in &paths {
            if name.starts_with("wrapper_")
                || *name == "mod"
                || name.ends_with("_grpc")
                || !paths.contains_key(&*format!("{}_grpc", name))
            {
                continue;
            }

            let mut out = OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("Couldn't open source file");
            writeln!(out, "pub use super::{}_grpc::*;", name).expect("Could not write source file");
        }
    }

    #[cfg(not(feature = "grpcio-protobuf-codec"))]
    fn import_grpcio(&self) {}

    #[cfg(feature = "grpcio-protobuf-codec")]
    fn generate_grpcio(
        &self,
        desc: &[protobuf::descriptor::FileDescriptorProto],
        files_to_generate: &[String],
    ) {
        let output_dir = std::path::Path::new(&self.out_dir);
        let results = grpcio_compiler::codegen::gen(desc, &files_to_generate);
        for res in results {
            let out_file = output_dir.join(&res.name);
            let mut f = std::fs::File::create(&out_file).unwrap();
            f.write_all(&res.content).unwrap();
        }
    }

    #[cfg(not(feature = "grpcio-protobuf-codec"))]
    fn generate_grpcio(&self, _: &[protobuf::descriptor::FileDescriptorProto], _: &[String]) {}
}
