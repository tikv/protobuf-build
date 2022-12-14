use crate::wrapper::WrapperGen;
use crate::Builder;

impl Builder {
    pub fn generate_files(&self) {
        #[cfg(feature = "grpcio-prost-codec")]
        {
            grpcio_compiler::prost_codegen::compile_protos(
                &self.files,
                &self.includes,
                &self.out_dir,
            )
            .unwrap();
        }
        #[cfg(not(feature = "grpcio-prost-codec"))]
        {
            prost_build::Config::new()
                .out_dir(&self.out_dir)
                .compile_protos(&self.files, &self.includes)
                .unwrap();
        }

        let rs_files_snapshot = self.list_rs_files().collect::<Vec<_>>;
        rs_files_snapshot.for_each(|path| WrapperGen::new(path, self.wrapper_opts).write());
    }
}
