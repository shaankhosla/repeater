#!/usr/bin/env bash
set -euo pipefail

version="$(rg --max-count 1 '^version = ' Cargo.toml | sed -E 's/version = "(.+)"/\1/')"

if [[ -z "${version}" ]]; then
  echo "Unable to detect package version from Cargo.toml" >&2
  exit 1
fi

echo "version: ${version}"
git cliff --config cliff.toml --tag "v${version}" --unreleased --output CHANGELOG.md
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "chore(release): v${version}"
git tag -a "v${version}" -m "v${version}"
git push origin HEAD
git push origin "v${version}"
