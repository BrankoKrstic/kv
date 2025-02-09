# Use a Rust base image with Cargo installed
FROM rust:1.84.0 AS builder

# Set the working directory inside the container
WORKDIR /usr/src/app

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock build.rs ./

# Now copy the source code
COPY ./src ./src
COPY ./proto ./proto

RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

# Build your application
RUN cargo build --release

# Start a new stage to create a smaller image without unnecessary build dependencies
# FROM debian:buster-slim
# Set the working directory
# WORKDIR /usr/src/app

# Copy the built binary from the previous stage
COPY ./target/release/ ./
# RUN apt-get update && apt-get upgrade -y libc6
# Command to run the application
CMD ["./server"]