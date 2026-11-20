#!/bin/zsh
set -eu
just check > .artifacts/0058-final-check.log 2>&1
just browser > .artifacts/0058-final-browser.log 2>&1
just portable-test > .artifacts/0058-final-portable.log 2>&1
just safety-test > .artifacts/0058-final-safety.log 2>&1
just criterion-test > .artifacts/0058-final-criterion-test.log 2>&1
node --test dist/evals/*/contract-test.js > .artifacts/0058-final-eval-contracts.log 2>&1
just runtime-build > .artifacts/0058-final-runtime-build.log 2>&1
just runtime-test > .artifacts/0058-final-runtime-test.log 2>&1
just evaluate > .artifacts/0058-final-evaluate.log 2>&1
just replay-evaluation > .artifacts/0058-final-replay-evaluation.log 2>&1
just package > .artifacts/0058-final-package.log 2>&1
just tlc-diff-test .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar > .artifacts/0058-final-tlc-tests.log 2>&1
just tlc-diff .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar development evals/runs/0058-tlc > .artifacts/0058-final-tlc.log 2>&1
just tlc-diff .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar confirmation evals/runs/0058-tlc-confirmation > .artifacts/0058-final-tlc-confirmation.log 2>&1
just tlc-diff-replay .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar evals/runs/0058-tlc evals/runs/0058-tlc-replay > .artifacts/0058-final-tlc-replay.log 2>&1
