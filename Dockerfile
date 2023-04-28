# FROM alpine:latest
# RUN apk add --no-cache curl bash gcc libc-dev
# RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
# ENV PATH="/root/.cargo/bin:${PATH}"
# ENV HYDRABADGER_LOG=info

# WORKDIR /app
# COPY . /app

# RUN cargo build --release
# CMD ["./target/release/peer_node"]

# FROM alpine:latest
FROM ubuntu:latest

RUN apt-get update && \
    apt-get install -y iproute2 && \
    rm -rf /var/lib/apt/lists/*

ENV HYDRABADGER_LOG=info

COPY ./peer_node /usr/local/bin/

CMD peer_node