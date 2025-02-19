fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_client(true)
        .build_server(true)
        .compile(
            &["../../../proto/node.proto", "../../../proto/core.proto"],
            &["../../../proto"],
        )?;
    Ok(())
}
