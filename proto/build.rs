use std::io;

fn main() -> io::Result<()> {
    let mut prost_config = prost_build::Config::new();
    prost_config.extern_path(".ibc", "::ibc_proto::ibc");
    prost_config.extern_path(
        ".google.protobuf.Timestamp",
        "::tendermint_proto::google::protobuf::Timestamp",
    );
    prost_config.message_attribute(
        ".eclipse.ibc.port.v1",
        "#[allow(clippy::module_name_repetitions)]",
    );
    prost_config.type_attribute(".eclipse", "#[derive(serde::Serialize)]");

    tonic_build::configure()
        .build_server(false)
        .compile_with_config(
            prost_config,
            &[
                "proto/eclipse/ibc/admin/v1/admin.proto",
                "proto/eclipse/ibc/chain/v1/chain.proto",
                "proto/eclipse/ibc/client/v1/client.proto",
                "proto/eclipse/ibc/port/v1/port.proto",
            ],
            &["ibc-go-proto/", "proto/"],
        )?;

    Ok(())
}
