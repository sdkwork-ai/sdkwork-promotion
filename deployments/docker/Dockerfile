FROM rust:1-bookworm AS builder
WORKDIR /src
COPY . .
RUN cargo build --release -p sdkwork-api-promotion-standalone-gateway

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/target/release/sdkwork-api-promotion-standalone-gateway /usr/local/bin/sdkwork-api-promotion-standalone-gateway
ENV PROMOTION_API_BIND=0.0.0.0:18097
EXPOSE 18097
CMD ["sdkwork-api-promotion-standalone-gateway"]
