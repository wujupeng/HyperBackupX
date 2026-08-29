use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protoc_path = protoc_bin_vendored::protoc_bin_path()
        .map_err(|e| format!("Failed to find vendored protoc: {e}"))?;
    std::env::set_var("PROTOC", &*protoc_path);

    let proto_root = PathBuf::from("../../../proto");
    let badou_proto = proto_root.join("badou.proto");
    let third_party = proto_root.join("third_party");

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&[badou_proto], &[proto_root, third_party])?;

    Ok(())
}
