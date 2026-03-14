//! Build script for bolt402-lnd: compiles LND proto files.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .compile_protos(&["proto/lightning.proto", "proto/router.proto"], &["proto"])?;

    Ok(())
}
