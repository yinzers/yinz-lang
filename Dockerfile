FROM ubuntu:24.04

# Toolchain-only image — no git, gh, claude, sudo, docker, vim, jq, or any
# orchestration wrapper. Just the compiler and runtime dependencies needed to
# build and test the Yinz compiler (Rust + LLVM 18 + Node 22 for the VSCode
# extension build path).
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    llvm-18-dev \
    clang-18 \
    libclang-18-dev \
    curl \
    ca-certificates \
    unzip \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Node.js 22 — nodesource setup script requires root to register the apt source.
# Run as root here, then switch to the ubuntu user below.
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Switch to the stock ubuntu user (uid 1000) so that files written into the
# bind-mounted project directory are owned by host uid 1000 (patrick), not root.
# Root-owned build artifacts in the bind mount require sudo to remove on the host.
USER ubuntu
WORKDIR /home/ubuntu

# Rust stable toolchain. rustup installs into ~/.cargo for the ubuntu user.
# The cargo-registry named volume (declared in docker-compose.yml) is mounted at
# ~/.cargo/registry so downloaded crates persist across container rebuilds without
# being baked into the image layer.
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable
ENV PATH="/home/ubuntu/.cargo/bin:${PATH}"

# Pre-create the registry cache directory so the named volume mount (declared in
# docker-compose.yml) inherits ubuntu ownership from the image layer rather than
# being initialized as root. Docker copies image-directory contents into a newly
# created named volume on first mount, preserving owner/permissions.
RUN mkdir -p /home/ubuntu/.cargo/registry/cache \
           /home/ubuntu/.cargo/registry/src \
           /home/ubuntu/.cargo/registry/index

WORKDIR /work
