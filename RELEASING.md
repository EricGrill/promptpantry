# Releasing Prompt Pantry

Prompt Pantry uses cargo-dist to build versioned GitHub Releases.

## Required Repository Setup

GitHub Releases use the repository `GITHUB_TOKEN` that Actions provides.

Homebrew publishing also requires:

- Tap repository: `EricGrill/homebrew-tap`
- Repository secret on `EricGrill/promptpantry`: `HOMEBREW_TAP_TOKEN`
- Token permissions: write access to `EricGrill/homebrew-tap`

Without `HOMEBREW_TAP_TOKEN`, tagged releases can build GitHub Release assets
but the Homebrew formula publish job will fail.

## Cut a Release

1. Update `version` in `Cargo.toml`.
2. Run local checks:

   ```sh
   cargo fmt --check
   cargo test --locked
   cargo clippy --all-targets --locked -- -D warnings
   cargo publish --dry-run --locked
   dist plan --no-local-paths
   ```

3. Commit and merge the version change.
4. Tag the merged commit with the matching semver tag:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

The release workflow creates GitHub Release artifacts, installer scripts, and a
Homebrew formula. Homebrew users install with:

```sh
brew install EricGrill/tap/prompt-pantry
```

Do not run `cargo publish` or push release tags from routine CI. Registry
publishing and tags are permanent release actions.
