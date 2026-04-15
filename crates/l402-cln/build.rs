//! Build script for l402-cln: compiles CLN proto files when the grpc feature is enabled.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("CARGO_FEATURE_GRPC").is_some() {
        tonic_build::configure()
            .build_server(false)
            .compile_protos(&["proto/node.proto"], &["proto"])?;
    }

    Ok(())
}
