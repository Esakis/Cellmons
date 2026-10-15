fn main() -> std::io::Result<()> {
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path().expect("bundled protoc"));
    config.compile_protos(
        &[
            "../../proto/game/v1/training.proto",
            "../../proto/game/v1/envelope.proto",
        ],
        &["../../proto"],
    )?;
    println!("cargo:rerun-if-changed=../../proto");
    Ok(())
}
