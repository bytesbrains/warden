# Warden node (`wardend`) image for fly.io — one share-holding federation member.
# Build context = the warden/ workspace root (so the workspace + its members resolve).
#   fly deploy -c deploy/fly/fly.node.toml   (run from warden/)
FROM rust:1.83-slim AS build
WORKDIR /warden
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core ./core
COPY dealer ./dealer
COPY node ./node
COPY cli ./cli
# ffi is a workspace member (#214) — cargo must load its manifest to parse the workspace,
# even though we only build wardend here.
COPY ffi ./ffi
RUN cargo build --release -p warden-node

FROM debian:bookworm-slim
# ca-certificates so the node can reach an HTTPS Base RPC.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /warden/target/release/wardend /usr/local/bin/wardend
COPY deploy/fly/entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh
# wardend serves WARDEN_LISTEN (default 0.0.0.0:8080) — matches fly internal_port.
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
