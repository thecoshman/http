# HTTP Builder Stage
FROM rust:latest AS builder
WORKDIR /app/http
COPY . /app/http
RUN cargo build

# HTTP Runner Stage
FROM rust:latest AS runner
WORKDIR /app
COPY --from=builder /app/http/target/debug/http /app/http
RUN mkdir /dir

CMD [ "/app/http", "/dir" ]
EXPOSE 8000