FROM ubuntu:24.04

ENV USER=ubuntu

# System essentials
# jq: used by postStartCommand to inject MCP server config into the persistent .claude.json volume on container start
RUN apt-get update && apt-get upgrade -y \
&& apt-get install -y --no-install-recommends \
sudo \
curl \
unzip \
ca-certificates \
git \
openssh-client \
vim \
jq \
bash-completion \
&& apt-get clean && rm -rf /var/lib/apt/lists/*

# Docker (for spawning sibling containers via host socket)
RUN apt-get update && apt-get install -y --no-install-recommends docker.io docker-compose-v2 \
    && apt-get clean && rm -rf /var/lib/apt/lists/* \
    && usermod -aG docker ubuntu

# GitHub CLI
RUN install -m 0755 -d /etc/apt/keyrings \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg -o /etc/apt/keyrings/githubcli-archive-keyring.gpg \
    && chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends gh \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# LLVM 18 + Clang 18
# - llvm-18-dev: LLVM libraries + headers that inkwell/llvm-sys links against when
#   building the ynz compiler itself (cargo build)
# - clang-18: the system linker ynz invokes to turn .o files into native binaries
#   (ynz build uses clang-18 as its first-choice linker)
# - libclang-18-dev: libclang headers used by some inkwell binding code
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
       llvm-18-dev \
       clang-18 \
       libclang-18-dev \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# Allow ubuntu user to run commands as root
RUN echo "${USER} ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# Switch to ubuntu user for all installs
USER ${USER}
WORKDIR /home/${USER}

# Rust toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain stable
ENV PATH="/home/${USER}/.cargo/bin:${PATH}"

# Claude Code CLI
RUN curl -fsSL https://claude.ai/install.sh | bash
ENV PATH="/home/${USER}/.local/bin:${PATH}"

# Makes git trust any mounted workspace directory
RUN git config --global --add safe.directory '*'
