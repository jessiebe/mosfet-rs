use std::path::PathBuf;

fn main() {
    std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path().unwrap());
    let mut protospec_path = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    protospec_path.push("opamp-spec");
    protospec_path.push("proto");

    // Protobuf generation
    let include_path = protospec_path.clone();
    protospec_path.push("opamp.proto");

    prost_build::compile_protos(&[protospec_path], &[include_path]).unwrap();
}
