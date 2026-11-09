# Modern Signal CLI API: one image = this REST/WebSocket bridge + a pinned, stock
# signal-cli release. signal-cli is a dependency, never forked or patched here;
# moving to a newer one is a one-line change (SIGNAL_CLI_VERSION + SIGNAL_CLI_SHA256 below).

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM ubuntu:24.04

# jdk-headless, not jre-headless: jstack/jcmd (needed to catch the intermittent
# SQLITE_BUSY/receive-thread-death bug live) ship only with the JDK.
RUN apt-get update && \
    apt-get install -y --no-install-recommends openjdk-25-jdk-headless ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

# https://github.com/AsamK/signal-cli/releases - bump both together and re-run the tests.
ARG SIGNAL_CLI_VERSION=0.14.10
ARG SIGNAL_CLI_SHA256=706a57d9faae7525cf732936efdabf210146e3c83f9e8ae7e4e62f5946121c33
RUN curl -fsSL -o /tmp/signal-cli.tgz \
        "https://github.com/AsamK/signal-cli/releases/download/v${SIGNAL_CLI_VERSION}/signal-cli-${SIGNAL_CLI_VERSION}.tar.gz" && \
    echo "${SIGNAL_CLI_SHA256}  /tmp/signal-cli.tgz" | sha256sum -c - && \
    tar -xzf /tmp/signal-cli.tgz -C /opt && rm /tmp/signal-cli.tgz && \
    ln -s "/opt/signal-cli-${SIGNAL_CLI_VERSION}/bin/signal-cli" /usr/local/bin/signal-cli

COPY --from=build /src/target/release/modern-signal-cli-api /usr/local/bin/modern-signal-cli-api

ENV HOME=/var/lib/signal-cli-api
WORKDIR /var/lib/signal-cli-api

EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/modern-signal-cli-api", "--listen", "0.0.0.0:8080"]
