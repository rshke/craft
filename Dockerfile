FROM rust:1.88

WORKDIR /usr/src/craft
COPY . .

ENV SQLX_OFFLINE true
RUN cargo install --path .

CMD ["craft"]
