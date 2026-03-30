FROM rust:1.91-slim-bookworm@sha256:8514999d4786ef12efe89239e86b3d0a021b94b9d35108c8efe6c79ca7dc1a65 AS chef
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

FROM gcr.io/distroless/cc-debian12@sha256:329e54034ce498f9c6b345044e8f530c6691f99e94a92446f68c0adf9baa8464 AS runtime

WORKDIR /app

COPY --from=builder --chown=65532:65532 /app/target/release/cqwu_achievement_system .
COPY --from=builder --chown=65532:65532 /app/configuration ./configuration
COPY --from=builder --chown=65532:65532 /app/migrations ./migrations

USER 65532:65532

ENV APP_ENVIRONMENT=production

ENTRYPOINT ["./cqwu_achievement_system"]