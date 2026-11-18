# Build environment matching the CI runners (Ubuntu 22.04, glibc 2.35).
FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential curl ca-certificates file pkg-config \
      libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev \
    && curl -fsSL https://deb.nodesource.com/setup_20.x | bash - \
    && apt-get install -y nodejs \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
ENV PATH=/root/.cargo/bin:$PATH
WORKDIR /src
