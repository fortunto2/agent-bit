fn main() {
    connectrpc_build::Config::new()
        .files(&[
            "proto/bitgn/harness.proto",
            "proto/bitgn/vm/pcm.proto",
            "proto/bitgn/vm/mini.proto",
        ])
        .includes(&["proto/"])
        .include_file("_all.rs")
        .compile()
        .expect("failed to compile BitGN proto files");
}
