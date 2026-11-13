FROM rust:1.83-bookworm AS build
WORKDIR /app
COPY Cargo.toml ./
COPY src ./src
RUN cargo build --release
FROM debian:bookworm-slim
RUN useradd --system --uid 10001 routerpulse
COPY --from=build /app/target/release/routerpulse-api /usr/local/bin/routerpulse-api
USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["routerpulse-api"]

