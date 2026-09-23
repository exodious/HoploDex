# HoploDex development environment: everything needed to build the app, run
# every test suite (Rust, Vitest, WebdriverIO E2E, the quit-cleanup check) and
# take screenshots, on Debian trixie. Built for rootless podman, runs as the
# non-root user `dev`. The source isn't copied in: scripts/dev-container.sh
# bind-mounts the checkout at /workspace. See "Development container" in
# README.md.
#
#   podman build -t hoplodex-dev .        # or: scripts/dev-container.sh --build
#
# Toolchains live outside /home/dev (Rust in /opt/rust, Node in /usr/local) so
# the persistent home volume the wrapper mounts never hides an updated image.

FROM docker.io/library/debian:trixie-slim

ARG NODE_VERSION=24.21.0
ARG NPM_VERSION=12
ARG RUST_TOOLCHAIN=stable
ARG TAURI_DRIVER_VERSION=2.0.6
ARG SPEC_KIT_VERSION=v1.0.8
ARG USERNAME=dev
ARG USER_UID=1000
ARG USER_GID=1000

ENV DEBIAN_FRONTEND=noninteractive \
    LANG=C.UTF-8 \
    LC_ALL=C.UTF-8 \
    NPM_CONFIG_UPDATE_NOTIFIER=false

# System packages:
# - Tauri/WebKitGTK build deps and rusqlite's bundled SQLCipher (perl, libssl)
# - E2E: WebKitWebDriver (webkit2gtk-driver), Xvfb + xauth for xvfb-run,
#   iproute2 for `ss` (the harness clears stale driver ports)
# - human testing: a session D-Bus and gnome-keyring for the real keyring
# - python3-gi + GTK typelibs for the native drag-and-drop test technique
# - imagemagick to crop screenshots, fonts for anything the app doesn't bundle
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        dbus \
        dbus-x11 \
        file \
        fonts-dejavu-core \
        fonts-noto-core \
        fonts-noto-color-emoji \
        git \
        gir1.2-gtk-3.0 \
        gnome-keyring \
        gnupg \
        imagemagick \
        iproute2 \
        jq \
        less \
        libayatana-appindicator3-dev \
        libgtk-3-dev \
        libjavascriptcoregtk-4.1-dev \
        librsvg2-dev \
        libsecret-tools \
        libsoup-3.0-dev \
        libssl-dev \
        libwebkit2gtk-4.1-dev \
        libxdo-dev \
        openssh-client \
        perl \
        pkg-config \
        procps \
        psmisc \
        python3 \
        python3-gi \
        sudo \
        webkit2gtk-driver \
        wget \
        xauth \
        xvfb \
        xz-utils \
    && rm -rf /var/lib/apt/lists/*

# GitHub CLI, from GitHub's own apt repository (Debian's lags behind).
RUN install -d -m 0755 /etc/apt/keyrings \
    && curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
        -o /etc/apt/keyrings/githubcli-archive-keyring.gpg \
    && chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg \
    && echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
        > /etc/apt/sources.list.d/github-cli.list \
    && apt-get update \
    && apt-get install -y --no-install-recommends gh \
    && rm -rf /var/lib/apt/lists/*

# Node.js LTS (official build, checksum-verified) and npm 12, which writes the
# lockfile format this repo uses.
RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
        amd64) arch=x64 ;; \
        arm64) arch=arm64 ;; \
        *) echo "unsupported architecture" >&2; exit 1 ;; \
    esac; \
    tarball="node-v${NODE_VERSION}-linux-${arch}.tar.xz"; \
    cd /tmp; \
    curl -fsSLO "https://nodejs.org/dist/v${NODE_VERSION}/${tarball}"; \
    curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/SHASUMS256.txt" | grep " ${tarball}\$" | sha256sum -c -; \
    tar -xJf "${tarball}" -C /usr/local --strip-components=1 --no-same-owner; \
    rm "${tarball}"; \
    npm install -g "npm@${NPM_VERSION}"; \
    npm cache clean --force

# Claude Code, installed with the image rather than self-updating in $HOME.
RUN npm install -g --allow-scripts=@anthropic-ai/claude-code @anthropic-ai/claude-code \
    && npm cache clean --force
ENV DISABLE_AUTOUPDATER=1

# uv, and Spec Kit's `specify` CLI at the version .specify/ was set up with.
COPY --from=ghcr.io/astral-sh/uv:latest /uv /uvx /usr/local/bin/
RUN UV_TOOL_DIR=/opt/uv/tools UV_TOOL_BIN_DIR=/usr/local/bin UV_PYTHON_INSTALL_DIR=/opt/uv/python \
        uv tool install specify-cli --from "git+https://github.com/github/spec-kit.git@${SPEC_KIT_VERSION}" \
    && chmod -R a+rX /opt/uv \
    && rm -rf /root/.cache

# The non-root user. Passwordless sudo is only for installing extra packages
# while experimenting; nothing in the workflow needs it.
RUN groupadd --gid "${USER_GID}" "${USERNAME}" \
    && useradd --uid "${USER_UID}" --gid "${USER_GID}" --create-home --shell /bin/bash "${USERNAME}" \
    && echo "${USERNAME} ALL=(ALL) NOPASSWD:ALL" > "/etc/sudoers.d/${USERNAME}" \
    && chmod 0440 "/etc/sudoers.d/${USERNAME}" \
    && install -d -o "${USERNAME}" -g "${USERNAME}" /opt/rust /workspace

# Rust, owned by the dev user so `rustup` works from inside the container.
# CARGO_HOME points at $HOME/.cargo at run time (the crate cache persists in
# the home volume); /opt/rust/cargo/bin keeps the rustup proxies and
# tauri-driver.
USER ${USERNAME}
ENV RUSTUP_HOME=/opt/rust/rustup \
    PATH=/opt/rust/cargo/bin:/usr/local/bin:/usr/bin:/bin
RUN set -eux; \
    export CARGO_HOME=/opt/rust/cargo; \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --no-modify-path --profile minimal \
            --default-toolchain "${RUST_TOOLCHAIN}" --component rustfmt,clippy; \
    cargo install tauri-driver --version "${TAURI_DRIVER_VERSION}" --locked; \
    rm -rf /opt/rust/cargo/registry /opt/rust/cargo/git
ENV CARGO_HOME=/home/${USERNAME}/.cargo \
    PATH=/home/${USERNAME}/.cargo/bin:/home/${USERNAME}/.local/bin:/opt/rust/cargo/bin:/usr/local/bin:/usr/bin:/bin \
    HUMAN_TESTING_DIR=/home/${USERNAME}/human-testing \
    NO_AT_BRIDGE=1

COPY --chmod=0755 container/entrypoint.sh /usr/local/bin/hoplodex-entrypoint

WORKDIR /workspace
ENTRYPOINT ["/usr/local/bin/hoplodex-entrypoint"]
CMD ["bash"]
