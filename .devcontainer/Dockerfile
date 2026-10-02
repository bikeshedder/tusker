FROM mcr.microsoft.com/devcontainers/base:ubuntu-24.04

# Add extra APT repositories
COPY apt/ /etc/apt

# Include lld linker to improve build times either by using environment variable
# RUSTFLAGS="-C link-arg=-fuse-ld=lld" or with Cargo's configuration file (i.e see .cargo/config.toml).
ARG POSTGRESQL_VERSION=17
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
      && apt-get -y install clang lld libpq-dev pkg-config gh git-delta iputils-ping postgresql-client-${POSTGRESQL_VERSION} \
      && apt-get autoremove -y && apt-get clean -y

USER vscode

ARG RUST_VERSION=1.96
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain ${RUST_VERSION} --no-modify-path -y

# Nightly is only used for `cargo +nightly update -Z direct-minimal-versions`
# to reproduce the MSRV check of the CI.
RUN /home/vscode/.cargo/bin/rustup toolchain install nightly --profile minimal

RUN /home/vscode/.cargo/bin/cargo install cargo-release
RUN /home/vscode/.cargo/bin/cargo install --locked cargo-outdated
