FROM gcr.io/distroless/static@sha256:1a64f46364620465ef9e9a8f5413757dfdf22695e050bbfd7303eeaf17a59383
COPY target/x86_64-unknown-linux-musl/release/photoframe-server /usr/local/bin/photoframe-server
WORKDIR /app
CMD ["photoframe-server"]
