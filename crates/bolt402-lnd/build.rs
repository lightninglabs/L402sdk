//! Build script for bolt402-lnd: compiles LND proto files when the grpc feature is enabled.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "grpc")]
    {
        tonic_build::configure()
            .build_server(false)
            .compile_protos(&["proto/lightning.proto", "proto/router.proto"], &["proto"])?;
    }

    Ok(())
}
