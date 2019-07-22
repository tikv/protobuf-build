use crate::wrapper::WrapperGen;
use crate::{list_rs_files, Builder, OUT_DIR};

impl Builder {
    pub fn generate_files(&self) {
        #[cfg(feature = "grpcio-prost-codec")]
        {
            grpcio_compiler::prost_codegen::compile_protos(&self.files, &self.includes, &*OUT_DIR)
                .unwrap();
        }
        #[cfg(not(feature = "grpcio-prost-codec"))]
        {
            prost_build::Config::new()
                .out_dir(&*OUT_DIR)
                .compile_protos(&self.files, &self.includes)
                .unwrap();
        }

        list_rs_files().for_each(|path| WrapperGen::new(path, self.wrapper_opts).write());
    }
}
