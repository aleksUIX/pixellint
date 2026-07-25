# Security Policy

Pixellint parses untrusted URLs and user-supplied rulepack manifests, so
parser robustness matters.

## Reporting a vulnerability

Email aleks@vastlint.org with details and a reproducing artifact if you have
one. You should get a response within a few days. Please do not open a public
issue for anything you believe is exploitable: panics on crafted input,
pathological regex behavior from a manifest, or unbounded resource use while
validating.

Crashes on malformed input that only affect the local CLI are fine to report
as regular bug reports.

## Rulepack manifests are code-adjacent

A manifest can define regular expressions that Pixellint compiles and runs.
Treat third-party manifests the way you treat any other configuration you
execute: read them before loading them with `--rulepack-file`.

## Supported versions

Only the latest released version receives fixes.
