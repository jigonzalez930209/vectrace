# CI/CD & GitHub Pages Deployment

## GitHub Actions Workflows

Vectrace utilizes automated GitHub Actions for Continuous Integration, release packaging, and VitePress site deployment:

1. **`ci.yml`**: Runs `cargo check` and unit tests. On git tags (`v*`), builds release artifacts (`.AppImage`, `.deb`, `.rpm`, `.tar.gz`) and publishes GitHub Releases automatically.
2. **`deploy-docs.yml`**: Builds VitePress documentation (Node **24** + pnpm) and deploys automatically to **GitHub Pages**.

All JavaScript-based Actions in these workflows target the **Node 24** runtime (`actions/checkout@v5`, `actions/setup-node@v5`, `softprops/action-gh-release@v3`, etc.).

## Setting Up GitHub Pages

To enable documentation hosting on GitHub:
1. Go to repository **Settings** ➔ **Pages**.
2. Under **Source**, select **GitHub Actions**.
3. Pushing changes to the `main` branch automatically deploys the latest docs to `https://jigonzalez930209.github.io/vectrace/`.

## GitHub Releases (tag `v*`)

The release job needs permission to create releases with `GITHUB_TOKEN`:

1. In the workflow, `build-release-packages` sets `permissions: contents: write` (already configured).
2. Also check repo **Settings → Actions → General → Workflow permissions**:
   - Prefer **Read and write permissions**, **or**
   - Keep read-only at org/repo default and rely on the per-job `permissions:` block above.

Without `contents: write`, `softprops/action-gh-release` fails with:

`403 Resource not accessible by integration`
