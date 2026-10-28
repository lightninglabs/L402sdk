//! Generate clients from the vendored upstream protobuf definitions.
//! Source distributions also carry matching stubs for builds without protoc.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-env-changed=L402_UPDATE_PROTO_STUBS");
    if std::env::var_os("CARGO_FEATURE_GRPC").is_none() {
        return Ok(());
    }
    println!("cargo:rerun-if-changed=proto");
    println!("cargo:rerun-if-changed=src/gen");
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR")?);
    let protoc = std::env::var_os("PROTOC").unwrap_or_else(|| "protoc".into());
    let missing_protoc = matches!(
        std::process::Command::new(protoc).arg("--version").output(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound
    );
    let generated = ["lnrpc.rs", "routerrpc.rs"];
    if missing_protoc {
        if std::env::var_os("L402_UPDATE_PROTO_STUBS").is_some() {
            return Err("protoc is required to regenerate stubs".into());
        }
        println!("cargo:warning=protoc not found; using checked-in gRPC stubs");
        for name in generated {
            std::fs::copy(format!("src/gen/{name}"), out_dir.join(name))?;
        }
    } else {
        tonic_prost_build::configure()
            .build_server(false)
            .compile_protos(&["proto/lightning.proto", "proto/router.proto"], &["proto"])?;
        if std::env::var_os("L402_UPDATE_PROTO_STUBS").is_some() {
            for name in generated {
                std::fs::copy(out_dir.join(name), format!("src/gen/{name}"))?;
            }
        }
    }
    Ok(())
}
