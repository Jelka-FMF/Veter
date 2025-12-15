FROM rust:trixie AS builder

WORKDIR /work

COPY . .

RUN cargo build --locked --release

FROM debian:trixie-slim AS runtime

EXPOSE 3030

COPY --from=builder /work/target/release/veter /usr/local/bin/veter

CMD ["veter"]
