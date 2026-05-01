# Stage 1: Plan
FROM lukemathwalker/cargo-chef:latest-rust-1.78 AS chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Build
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .
RUN cargo build --release

# Stage 3: Runtime
FROM gcr.io/distroless/cc-debian12 AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/backend /app/backend
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/.env.example /app/.env

EXPOSE 3000
ENTRYPOINT ["./backend"]
