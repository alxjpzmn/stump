# ------------------------------------------------------------------------------
# Frontend Build Stage
# ------------------------------------------------------------------------------

FROM node:16-alpine3.14 as frontend

WORKDIR /app

COPY apps/client/ .

RUN npm install
RUN npm run build

# ------------------------------------------------------------------------------
# Cargo Build Stage
# ------------------------------------------------------------------------------

FROM --platform=$BUILDPLATFORM rust:1.60.0 AS builder

RUN dpkg --add-architecture arm64

RUN apt-get update -y && apt-get -y install clang llvm pkg-config build-essential musl-dev musl-tools musl:arm64 libssl-dev openssl sqlite3 libsqlite3-dev gcc-aarch64-linux-gnu g++-aarch64-linux-gnu libc6-dev:arm64 libstdc++-devel

# Note: This is a workaround for gcc compilation issues when cross compiling for arm64.
# I need to ALSO do this for g++ comilation issues when cross compiling for arm64.
# TODO: I have not yet found a way to fix that.
ENV CC_aarch64_unknown_linux_musl=clang

WORKDIR /app

COPY .cargo .cargo
COPY core/ .

RUN rustup target add aarch64-unknown-linux-musl

ARG TARGETPLATFORM
RUN case "$TARGETPLATFORM" in \
  "linux/arm64") rustup target add aarch64-unknown-linux-musl && \
  cargo build --release --target aarch64-unknown-linux-musl && \ 
  cp target/aarch64-unknown-linux-musl/release/stump . ;; \
  "linux/amd64") rustup target add x86_64-unknown-linux-musl && \
  cargo build --release --target=x86_64-unknown-linux-musl && \
  cp target/x86_64-unknown-linux-musl/release/stump . ;; \
  *) exit 1 ;; \
  esac

# FROM rust:1-alpine3.15 as builder

# ENV RUSTFLAGS="-C target-feature=-crt-static"

# RUN apk add --no-cache --verbose musl-dev build-base sqlite openssl-dev

# WORKDIR /

# COPY core/ .

# RUN cargo build --release --target=x86_64-unknown-linux-musl

# ------------------------------------------------------------------------------
# Final Stage
# ------------------------------------------------------------------------------

FROM alpine:latest

RUN apk add --no-cache libstdc++

RUN addgroup -g 1000 stump

RUN adduser -D -s /bin/sh -u 1000 -G stump stump

WORKDIR /

# create the config, data and app directories
RUN mkdir -p config
RUN mkdir -p data
RUN mkdir -p app

# copy the binary
COPY --from=builder /app/stump ./app/stump

# copy the react build
COPY --from=frontend /app/build ./app/client

# *sigh* Rocket requires the toml file at runtime
COPY core/Rocket.toml ./app/Rocket.toml

RUN chown stump:stump ./app/stump

USER stump

# Default Stump environment variables
ENV STUMP_CONFIG_DIR=/config
ENV STUMP_CLIENT_DIR=/app/client

# Default Rocket environment variables
ENV ROCKET_PROFILE=release
ENV ROCKET_LOG_LEVEL=normal
ENV ROCKET_PORT=10801

CMD ["./app/stump"]