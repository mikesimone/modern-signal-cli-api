# Provenance

Modern Signal CLI API is a continuation of [h4x0r/signal-cli-api](https://github.com/h4x0r/signal-cli-api)
(MIT), forked in 2026 after upstream stopped merging pull requests. The fork's history is preserved.
The Docker image bundles a stock, unmodified release of
[AsamK/signal-cli](https://github.com/AsamK/signal-cli) (GPL-3.0); it is downloaded at build time from the
official release, pinned by version and SHA-256, and is not part of this repository's source. Its licence and
source are at that project. signal-cli is a dependency here, not a fork.

# Not affiliated

This project is an independent, unofficial tool. It is not affiliated with, endorsed by, or sponsored by the
Signal Foundation, Signal Messenger, or the authors of signal-cli. "Signal" is used only to describe what the
software talks to.

# Container images

Only the Dockerfile is published here. It downloads signal-cli (GPL-3.0) from its official release at build
time. If you publish an image that contains signal-cli, you take on the GPL-3.0 obligations for that
component (source offer: https://github.com/AsamK/signal-cli).
