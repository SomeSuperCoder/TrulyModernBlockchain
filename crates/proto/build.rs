fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &[
                "proto/consensus.proto",
                "proto/execution.proto",
                "proto/networking.proto",
            ],
            &["proto"],
        )?;
    Ok(())
}
