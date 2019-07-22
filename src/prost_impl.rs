use crate::wrapper::{GenOpt, WrapperGen};
use crate::{list_rs_files, OUT_DIR};
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

    list_rs_files().for_each(|path| WrapperGen::new(path, GenOpt::all()).write());
}
