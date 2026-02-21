# Build stage
FROM rust:latest AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Copy source code
COPY src ./src
COPY static ./static

# Build for release
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies (if needed for Art-Net networking)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Use Berlin timezone for scheduler
ENV TZ=Europe/Berlin

# Copy the binary from builder stage
COPY --from=builder /app/target/release/barbeleuchtung /app/barbeleuchtung

# Copy static files
COPY --from=builder /app/static ./static

# Expose the web server port
EXPOSE 8080

# Run the application
CMD ["./barbeleuchtung"]
