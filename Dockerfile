FROM rust:1.75-slim AS build
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends build-essential pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src/
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
LABEL org.opencontainers.image.source=https://github.com/OrkWard/rspb
WORKDIR /app

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tzdata \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir db

COPY --from=build /app/target/release/rspb /app/rspb
COPY ./config.yaml config.yaml
COPY ./README.md README.md

VOLUME ["/app/db"]
EXPOSE 3030

ENV RUST_LOG=info

CMD ["/app/rspb"]
