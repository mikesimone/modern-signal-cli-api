# Provenance

Modern Signal CLI API is a continuation of [h4x0r/signal-cli-api](https://github.com/h4x0r/signal-cli-api)
(MIT), forked in 2026 after upstream stopped merging pull requests. The fork's history is preserved.
The Docker image bundles a stock, unmodified release of
[AsamK/signal-cli](https://github.com/AsamK/signal-cli) (GPL-3.0); it is downloaded at build time from the
official release, pinned by version and SHA-256, and is not part of this repository's source. Its licence and
source are at that project. signal-cli is a dependency here, not a fork.
