FROM rust:1.85.1-bookworm AS build
WORKDIR /workspace
COPY Cargo.toml ./
COPY programs ./programs
COPY backend ./backend
RUN cargo build --release -p routerpulse-api

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 routerpulse
COPY --from=build /workspace/target/release/routerpulse-api /usr/local/bin/routerpulse-api
USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["routerpulse-api"]

