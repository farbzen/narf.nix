# Deployment

The packages are published as prebuilt store paths in the Cachix binary cache
[farbzen.cachix.org](https://farbzen.cachix.org) by the workflow
[`.github/workflows/cache.yml`](../.github/workflows/cache.yml).

## Triggers

| Event               | Behaviour                                                             |
| ------------------- | --------------------------------------------------------------------- |
| Schedule (every 6h) | runs `nix flake update nixpkgs`; builds only if the lock file changed |
| Push to `main`      | on changes to `flake.nix`, `flake.lock`, `packages/` or the workflow  |
| Manual              | `workflow_dispatch` from the Actions tab, always builds               |

## Flow

1. **Update nixpkgs** – updates only the `nixpkgs` input. A changed
   `flake.lock` is committed to `main` as `chore(deps): update nixpkgs` by
   `github-actions[bot]`. Pushes made with the workflow token do not trigger
   the workflow again.
2. **Build and push** – builds `narf` and `narf-tui` on native runners for
   `x86_64-linux` (`ubuntu-24.04`) and `aarch64-linux` (`ubuntu-24.04-arm`) at
   the commit from step 1. `cachix/cachix-action` pushes every newly built
   store path to the cache.

Runs never overlap (`concurrency: binary-cache`).

## Setup

| Setting                               | Value                                            |
| ------------------------------------- | ------------------------------------------------ |
| Repository secret `CACHIX_AUTH_TOKEN` | Cachix auth token with write access to `farbzen` |
| Branch protection on `main`           | must allow pushes by GitHub Actions              |

Public signing key of the cache:

```text
farbzen.cachix.org-1:8KQga60LYPZVg0E4iKZgQhTmI5dV38Mb9iSuryp8BoY=
```

> [!NOTE]
> GitHub disables scheduled workflows in public repositories after 60 days
> without repository activity; re-enable it in the Actions tab if needed.
