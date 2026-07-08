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

crates.io publishing is intentionally manual. It requires:

- crates.io crate ownership for `prompt-pantry`
- `cargo login` locally, or a release-only `CARGO_REGISTRY_TOKEN`

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
4. Publish the crate when the version is ready:

   ```sh
   cargo publish --locked
   ```

5. Tag the merged commit with the matching semver tag:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

The release workflow creates GitHub Release artifacts, installer scripts, and
a Homebrew formula. Homebrew users install with:

```sh
brew tap EricGrill/tap
brew trust EricGrill/tap
brew install prompt-pantry
```

Homebrew 6 requires trust before loading formulae from third-party taps. Older
Homebrew versions that do not have `brew trust` can skip that line.

Rust users can install from crates.io after `cargo publish` completes:

```sh
cargo install prompt-pantry --locked
```

After both crates.io and the matching GitHub Release exist, cargo-binstall can
use the crate metadata to discover the repository and fetch cargo-dist release
artifacts:

```sh
cargo binstall prompt-pantry
```

Do not run `cargo publish` or push release tags from routine CI. Registry
publishing and tags are permanent release actions.

## cargo-binstall Compatibility

No `[package.metadata.binstall]` block is currently needed. cargo-binstall's
default strategy reads crates.io metadata, follows the repository link, and
looks for matching GitHub Release artifacts before falling back to source
compilation. The cargo-dist release artifacts use the expected
`prompt-pantry-{target}` archive naming shape.

Add explicit binstall metadata only if a future release changes archive names,
archive formats, or binary locations in a way cargo-binstall cannot infer.
