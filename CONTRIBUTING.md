# Contributing to Vectrace

Thank you for your interest in contributing to Vectrace! We welcome contributions of all kinds: bug fixes, performance optimizations, new drawing tools, UI improvements, and documentation updates.

---

## 🛠️ Development Setup

### Prerequisites
Make sure you have Rust (edition 2024 or latest stable) and system build dependencies installed:

#### Ubuntu / Debian:
```bash
sudo apt-get update
sudo apt-get install -y build-essential libx11-dev libxext-dev libxrender-dev libwayland-dev libdbus-1-dev libpipewire-0.3-dev libclang-dev
```

#### Fedora / RHEL:
```bash
sudo dnf install -y gcc libX11-devel libXext-devel libXrender-devel wayland-devel dbus-devel pipewire-devel clang-devel
```

#### Arch Linux:
```bash
sudo pacman -S --needed base-devel libx11 libxext libxrender wayland dbus pipewire clang
```

---

## 🚀 Building & Testing

1. **Clone the repository**:
   ```bash
   git clone https://github.com/jigonzalez930209/vectrace.git
   cd vectrace
   ```

2. **Check compilation**:
   ```bash
   cargo check
   ```

3. **Run unit & integration tests**:
   ```bash
   cargo test
   ```

4. **Run Vectrace locally**:
   ```bash
   cargo run
   ```
   Or start in tray mode:
   ```bash
   cargo run -- --start-in-tray
   ```

---

## 📋 Commit & Pull Request Guidelines

- **Clean Commits**: Write clear, imperative commit messages (e.g. `fix: resolve Wayland surface configure race condition` or `feat: add highlighter tool opacity control`).
- **Formatting**: Ensure your code passes standard Rust formatting checks:
  ```bash
  cargo fmt --check
  cargo clippy --all-targets
  ```
- **Updating CHANGELOG.md**: If adding a new feature or fixing a bug, please add a brief note under the `[Unreleased]` section in `CHANGELOG.md`.

---

## 📦 Release Engineering

Project releases are automated using `scripts/deploy-release.sh`.

To deploy a new release (e.g. `v0.2.4`):
```bash
./scripts/deploy-release.sh 0.2.4
```

This automated script will:
1. Update version numbers across `Cargo.toml`, `Cargo.lock`, `package.json`, `PKGBUILD`, `README.md`, and `docs/guide/installation.md`.
2. Sync and stage `CHANGELOG.md`.
3. Create a git commit `chore: release v0.2.4`.
4. Create an annotated git tag `v0.2.4`.
5. Push the commit and tag to GitHub, triggering the CI/CD pipeline (`.github/workflows/ci.yml`) to publish release packages.

Use `--dry-run` to preview changes without modifying git history:
```bash
./scripts/deploy-release.sh 0.2.4 --dry-run
```
