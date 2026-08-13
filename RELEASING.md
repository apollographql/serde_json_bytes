# Releasing `serde_json_bytes`

This crate is released to [crates.io](https://crates.io/crates/serde_json_bytes) via two
manually-triggered GitHub Actions workflows: **Prepare Release** and **Publish Release**.
Publishing uses crates.io's [Trusted Publishing](https://crates.io/docs/trusted-publishing) —
GitHub Actions OIDC authentication, no stored API token to create or rotate.

## Who can release

**Publish Release** runs under the `release` GitHub Environment, configured (in repo
Settings → Environments) to require approval from a specific list of reviewers, and
restricted to protected branches only (in practice, `main`). Anyone with write access can
*trigger* `workflow_dispatch`, but the job then pauses for reviewer approval, and can only
run at all from `main` — both enforced by GitHub itself, independent of the workflow file's
contents.

**Prepare Release** isn't environment-gated — it can't publish or tag anything on its own,
it only opens a PR, so its real backstop is the same required review that already gates
every other merge into `main`.

## Versioning

Versions are bare semver with no `v` prefix — e.g. `0.2.6`, not `v0.2.6`.

## Step by step

1. Decide the next version number based on what's merged to `main` since the last release.
2. Go to the **Actions** tab → **Prepare Release** → **Run workflow**, enter the version
   (e.g. `0.2.6`), and run it.
   - This validates the version, bumps `Cargo.toml` on a new `release/<version>` branch,
     and opens a PR titled `chore: release <version>`.
3. Review and merge that PR into `main`.
   - The PR is authored by `github-actions[bot]`, which GitHub treats specially: the
     `rust.yaml` run is created but parked as `action_required` pending manual approval,
     and CircleCI doesn't fire at all — so the required `gitleaks` and `semgrep` checks
     are missing and the PR is blocked.
   - Simplest fix is to push an empty commit to the release branch yourself. A push from
     a human actor wakes both systems and everything goes green:
     `git commit --allow-empty -m "chore: trigger CI on release/<version>" && git push`
   - You also can't approve your own release PR, so merging needs either a second
     `@apollographql/graphos` reviewer or an admin override.
4. Go to the **Actions** tab → **Publish Release** → **Run workflow** (no inputs needed)
   and run it.
   - This requires approval from a `@apollographql/graphos` reviewer before it proceeds,
     since it runs under the `release` Environment's required-reviewer protection.
   - Once approved, it reads the version from `Cargo.toml` on `main`, creates and pushes a
     matching git tag, runs `cargo publish`, and creates a GitHub Release with
     auto-generated release notes.
5. Verify: check the [crates.io page](https://crates.io/crates/serde_json_bytes), the new
   git tag, and the new GitHub Release.

## Troubleshooting

If **Publish Release** fails partway through (e.g. `cargo publish` errors out after the tag
was already pushed), it's safe to just re-run the workflow. It only refuses to proceed if
the version in `Cargo.toml` is already live on crates.io — it doesn't care whether the tag
already exists, and will happily re-point it at the current `main` HEAD on retry. Once a
version is actually published, crates.io publishes can't be undone, only
[yanked](https://doc.rust-lang.org/cargo/commands/cargo-yank.html).
