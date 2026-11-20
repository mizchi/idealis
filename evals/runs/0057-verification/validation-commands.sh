#!/bin/zsh
set -eu
just check > .artifacts/0057-final-check.log 2>&1
just browser > .artifacts/0057-final-browser.log 2>&1
just portable-test > .artifacts/0057-final-portable.log 2>&1
just safety-test > .artifacts/0057-final-safety.log 2>&1
just criterion-test > .artifacts/0057-final-criterion-test.log 2>&1
node --test dist/evals/*/contract-test.js > .artifacts/0057-final-eval-contracts.log 2>&1
just runtime-build > .artifacts/0057-final-runtime-build.log 2>&1
just runtime-test > .artifacts/0057-final-runtime-test.log 2>&1
just evaluate > .artifacts/0057-final-evaluate.log 2>&1
just replay-evaluation > .artifacts/0057-final-replay-evaluation.log 2>&1
just package > .artifacts/0057-final-package.log 2>&1
just tlc-diff-test .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar > .artifacts/0057-final-tlc-tests.log 2>&1
just tlc-diff .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar development evals/runs/0057-tlc > .artifacts/0057-final-tlc.log 2>&1
just tlc-diff .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar confirmation evals/runs/0057-tlc-confirmation > .artifacts/0057-final-tlc-confirmation.log 2>&1
just tlc-diff-replay .artifacts/tlc/arm64/jdk-21.0.13+9-jre/Contents/Home/bin/java .artifacts/tlc/tla2tools.jar evals/runs/0057-tlc evals/runs/0057-tlc-replay > .artifacts/0057-final-tlc-replay.log 2>&1
