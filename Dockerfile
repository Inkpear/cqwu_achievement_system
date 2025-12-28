FROM rust:1.91-slim-bookworm AS chef
WORKDIR /app
RUN apt-get update && apt-get install -y lld clang
RUN cargo install cargo-chef

FROM chef AS planner
WORKDIR /app
COPY . .

RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .

ENV SQLX_OFFLINE=true
RUN cargo build --release --bin cqwu_achievement_system

FROM gcr.io/distroless/cc-debian12 AS runtime

USER 65532:65532

WORKDIR /app

COPY --from=builder --chown=65532:65532 /app/target/release/cqwu_achievement_system .
COPY --from=builder --chown=65532:65532 /app/configuration ./configuration
COPY --from=builder --chown=65532:65532 /app/migrations ./migrations

ENV APP_ENVIRONMENT=production

ENTRYPOINT ["./cqwu_achievement_system"]