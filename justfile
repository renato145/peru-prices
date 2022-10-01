_default:
  @just --choose

watch-dev:
  cargo watch --clear -x "run"

checks:
  #!/usr/bin/env bash
  set -x
  cargo check
  cargo check --tests
  cargo clippy --all-targets
  cargo fmt --all -- --check

run-driver:
  chromedriver --port=4444 --disable-dev-shm-usage

render-site:
  #!/usr/bin/env bash
  cd quarto_site
  quarto render
  cd ..
  cp -r quarto_site/_site docs

