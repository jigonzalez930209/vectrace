# CI/CD & GitHub Pages Deployment

## GitHub Actions Workflows

Vectrace utilizes automated GitHub Actions for Continuous Integration, release packaging, and VitePress site deployment:

1. **`ci.yml`**: Runs `cargo check` and unit tests. On git tags (`v*`), builds release artifacts (`.AppImage`, `.deb`, `.rpm`, `.tar.gz`) and publishes GitHub Releases automatically.
2. **`deploy-docs.yml`**: Builds VitePress documentation and deploys automatically to **GitHub Pages**.

## Setting Up GitHub Pages

To enable documentation hosting on GitHub:
1. Go to repository **Settings** ➔ **Pages**.
2. Under **Source**, select **GitHub Actions**.
3. Pushing changes to the `main` branch automatically deploys the latest docs to `https://jigonzalez930209.github.io/vectrace/`.
