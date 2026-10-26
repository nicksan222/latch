ARG LATCH_RUST_IMAGE=mcr.microsoft.com/devcontainers/rust:1-trixie
FROM node:24-bookworm-slim AS node
FROM ${LATCH_RUST_IMAGE}
ENV CARGO_BUILD_JOBS=2

# Node LTS and the real Tauri/WebKitGTK desktop, plus a browser-accessible X11 seat.
COPY --from=node /usr/local/bin/node /usr/local/bin/node
COPY --from=node /usr/local/lib/node_modules /usr/local/lib/node_modules
RUN ln -s ../lib/node_modules/npm/bin/npm-cli.js /usr/local/bin/npm \
    && ln -s ../lib/node_modules/npm/bin/npx-cli.js /usr/local/bin/npx \
    && apt-get update && apt-get install -y --no-install-recommends \
      build-essential pkg-config clang lld libssl-dev \
      libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev patchelf \
      libxdo-dev xvfb xauth x11vnc openbox xdotool x11-utils scrot \
      novnc websockify dbus-x11 xterm chromium chromium-sandbox \
      fonts-dejavu fonts-liberation ca-certificates curl git gh jq ripgrep shellcheck \
    && rm -rf /var/lib/apt/lists/*
COPY rust-toolchain.toml /tmp/latch-toolchain/rust-toolchain.toml
USER vscode
RUN cd /tmp/latch-toolchain && rustup show \
    && mkdir -p /home/vscode/.cache/latch-targets
USER root
RUN printf '%s\n' '<!doctype html><meta charset="utf-8"><meta http-equiv="refresh" content="0;url=vnc.html?autoconnect=1&amp;reconnect=1&amp;resize=scale"><title>Latch desktop</title><a href="vnc.html?autoconnect=1&amp;resize=scale">Open Latch desktop</a>' > /usr/share/novnc/index.html
