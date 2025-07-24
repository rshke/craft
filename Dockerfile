FROM lukemathwalker/cargo-chef:latest-rust-1.88 as chef
WORKDIR /user/src/app

FROM chef as planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /user/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY . .

ENV SQLX_OFFLINE true
RUN cargo build --release


# FROM rust:1.88 as runtime
FROM debian:bookworm-slim as runtime

WORKDIR /user/src/app

RUN apt-get update -y \
    && apt-get install -y --no-install-recommends openssl ca-certificates \
    && apt-get autoremove -y \
    && apt-get clean -y \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /user/src/app/target/release/craft .
COPY configurations configurations

ENV RUNNING_ENV production
ENTRYPOINT [ "./craft" ]
