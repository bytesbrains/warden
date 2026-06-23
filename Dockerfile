# wardend node image (Phase 0 PoC). Build context is the warden/ workspace root.
FROM rust:1.83-slim AS build
WORKDIR /warden
# Workspace manifests + lockfile + toolchain pin, then sources. Cargo must see
# EVERY workspace member's manifest to resolve the workspace, even though we only
# build `-p warden-node` — so copy all members (core, dealer, node, cli, ffi).
# (wasm is a separate workspace and is not a member here.)
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY core ./core
COPY dealer ./dealer
COPY node ./node
COPY cli ./cli
COPY ffi ./ffi
RUN cargo build --release -p warden-node

FROM debian:bookworm-slim
# ureq verifies TLS against bundled webpki-roots; ca-certificates is belt-and-suspenders.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /warden/target/release/wardend /usr/local/bin/wardend
EXPOSE 8080
ENTRYPOINT ["wardend"]
