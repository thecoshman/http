FROM rust:latest

WORKDIR /app/http
COPY . /app/http
RUN cargo build
RUN mkdir /dir

CMD [ "/app/http/target/debug/http", "/dir" ]

EXPOSE 8000