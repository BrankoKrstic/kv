use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    tonic_build::compile_protos("proto/kv.proto")?;
    // let mut config = tonic_build::Config::new();
    // config.service_generator(service_generator)
    // // ...
    // config.protoc_arg("--experimental_allow_proto3_optional");
    // config.compile_protos(&["proto/entries.proto"], &["proto"])?;
    Ok(())
}
