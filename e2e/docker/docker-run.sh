#!/usr/bin/env bash
# Build and run the Docker nested e2e matrix from the host.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
COMPOSE_FILE="$ROOT/e2e/docker/docker-compose.yml"

die() { echo "error: $*" >&2; exit 1; }

if command -v docker >/dev/null 2>&1; then
  DOCKER=(docker)
elif command -v podman >/dev/null 2>&1; then
  DOCKER=(podman)
else
  die "Docker/Podman not found. Install docker.io (or podman) to run nested e2e in a container.

Host attach still works without Docker:
  ./e2e/harness.sh run gnome-wayland"
fi

# Catch the common post-install case: user is in group docker in /etc/group
# but this shell session was started before that membership applied.
if [[ "${DOCKER[0]}" == "docker" ]]; then
  if ! docker info >/dev/null 2>&1; then
    in_docker_group_now=0
    in_docker_group_passwd=0
    id -nG 2>/dev/null | grep -qw docker && in_docker_group_now=1
    getent group docker 2>/dev/null | grep -qw "$USER" && in_docker_group_passwd=1

    if [[ -S /var/run/docker.sock && "$in_docker_group_passwd" -eq 1 && "$in_docker_group_now" -eq 0 ]]; then
      die "Docker daemon OK, but this terminal does not have group 'docker' yet
(you were added to the group after this session started).

Fix (pick one):
  1) Close this terminal and open a new one, then:
       ./e2e/docker/docker-run.sh build
       ./e2e/docker/docker-run.sh run
  2) Log out of GNOME and log back in
  3) Temporary in this terminal:
       newgrp docker
       ./e2e/docker/docker-run.sh build
  4) Or once with sudo:
       sudo docker compose -f e2e/docker/docker-compose.yml build
       sudo docker compose -f e2e/docker/docker-compose.yml run --rm e2e run-docker"
    fi
    if [[ -S /var/run/docker.sock && "$in_docker_group_passwd" -eq 0 ]]; then
      die "Cannot access Docker socket (permission denied).

Add yourself to the docker group, then open a NEW terminal:
  sudo usermod -aG docker \"\$USER\"
  # then log out/in (or: newgrp docker)

Or run once with:
  sudo docker compose -f e2e/docker/docker-compose.yml build"
    fi
    die "Docker is installed but the daemon is not usable (is docker.service running?).
Try: sudo systemctl start docker"
  fi
fi

if "${DOCKER[@]}" compose version >/dev/null 2>&1; then
  COMPOSE=("${DOCKER[@]}" compose -f "$COMPOSE_FILE")
elif command -v docker-compose >/dev/null 2>&1; then
  COMPOSE=(docker-compose -f "$COMPOSE_FILE")
else
  die "docker compose plugin not found"
fi

mkdir -p "$ROOT/e2e/reports" "$ROOT/e2e/goldens"

cmd="${1:-run}"
shift || true

case "$cmd" in
  build)
    "${COMPOSE[@]}" build "$@"
    ;;
  run|run-docker)
    "${COMPOSE[@]}" run --rm e2e run-docker "$@"
    ;;
  run-one)
    id="${1:-}"
    [[ -n "$id" ]] || die "usage: docker-run.sh run-one <scenario_id>"
    shift || true
    "${COMPOSE[@]}" run --rm e2e run "$id" "$@"
    ;;
  list)
    "${COMPOSE[@]}" run --rm e2e list
    ;;
  shell)
    "${COMPOSE[@]}" run --rm --entrypoint bash e2e
    ;;
  *)
    cat <<EOF
Usage:
  $0 build              # build vectrace-e2e image
  $0 run                # run docker-safe nested matrix (Xephyr/Weston/Sway)
  $0 run-one <id>       # run a single scenario inside the container
  $0 list
  $0 shell

Note: gnome-wayland / kde-* need your real host session:
  ./e2e/harness.sh run gnome-wayland
EOF
    exit 1
    ;;
esac
