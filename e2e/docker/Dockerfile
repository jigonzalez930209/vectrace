# Isolated nested-compositor e2e image (NOT full GNOME/KDE).
# X11 via Xvfb (no parent DISPLAY needed). Sway nests under Weston headless.
FROM rust:1-bookworm

ENV DEBIAN_FRONTEND=noninteractive \
    CARGO_TERM_COLOR=always

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libx11-dev \
    libxext-dev \
    libxrender-dev \
    libwayland-dev \
    libpipewire-0.3-dev \
    libclang-dev \
    clang \
    # Nested X11 (Xvfb is the Docker path; Xephyr optional on hosts with DISPLAY)
    xvfb \
    xserver-xephyr \
    openbox \
    twm \
    x11-utils \
    x11-xserver-utils \
    xterm \
    # Nested Wayland (best-effort)
    weston \
    sway \
    xwayland \
    # DBus / portals (ScreenCast for sway via portal-wlr)
    dbus-daemon \
    dbus-x11 \
    pipewire \
    pipewire-pulse \
    wireplumber \
    xdg-desktop-portal \
    xdg-desktop-portal-wlr \
    grim \
    ca-certificates \
    curl \
    git \
    python3 \
    sudo \
    util-linux \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --shell /bin/bash --uid 1000 e2e \
    && mkdir -p /src \
    && chown -R e2e:e2e /src

WORKDIR /src

# Cache dependency builds when only sources change.
COPY --chown=e2e:e2e Cargo.toml Cargo.lock ./
COPY --chown=e2e:e2e src ./src
COPY --chown=e2e:e2e assets ./assets
COPY --chown=e2e:e2e e2e ./e2e

USER e2e
RUN cargo build --bin vectrace-e2e-probe \
    && cp target/debug/vectrace-e2e-probe /tmp/vectrace-e2e-probe

USER root
RUN cp /tmp/vectrace-e2e-probe /usr/local/bin/vectrace-e2e-probe \
    && chmod +x /usr/local/bin/vectrace-e2e-probe \
                 /src/e2e/harness.sh /src/e2e/nest/*.sh /src/e2e/docker/entrypoint.sh \
    && chown -R e2e:e2e /src

ENV PROBE_BIN=/usr/local/bin/vectrace-e2e-probe \
    E2E_PROFILE=docker \
    HOME=/home/e2e

# Start as root so entrypoint can chown bind mounts, then drop to e2e.
USER root
WORKDIR /src
ENTRYPOINT ["/src/e2e/docker/entrypoint.sh"]
CMD ["run-docker"]
