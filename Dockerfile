# Build stage.
#
# Pinned deliberately. `rust:latest` has moved to Debian 13 (trixie) while the
# runtime stage below is bookworm, so the floating tag was one upstream bump
# away from building against a glibc the runtime image does not have. 1.98 is
# the compiler that built the image this replaces; `-bookworm` matches runtime.
# A moving toolchain also invalidates every cached crate, which is the other
# half of why this is pinned now.
FROM rust:1.98-bookworm AS builder

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

# Build for release.
#
# The cache mounts are what make an incremental build cheap: the crates.io
# registry, the git checkouts and the target directory all survive between
# builds on the CI runner, so a one-line source change recompiles one crate
# instead of the whole dependency graph. Measured 22m09s without them.
#
# A cache mount is NOT part of the resulting layer, so /app/target does not
# exist in the runtime stage -- the binary has to be copied out inside this
# same RUN. unishort's Dockerfile does the same thing with its objcopy step.
RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    set -eux; \
    cargo build --release; \
    cp target/release/barbeleuchtung /app/barbeleuchtung

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
COPY --from=builder /app/barbeleuchtung /app/barbeleuchtung

# Copy static files
COPY --from=builder /app/static ./static

# Expose the web server port
EXPOSE 8080

# Run the application
CMD ["./barbeleuchtung"]
