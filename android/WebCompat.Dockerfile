FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update -qq \
    && apt-get install -y --no-install-recommends build-essential pkg-config ca-certificates liblzma-dev \
    && rm -rf /var/lib/apt/lists/*
RUN ln -s /usr/local/bin/bun /usr/local/bin/bunx
WORKDIR /src
