FROM debian:bookworm-slim

ARG BIN_FILE
ARG API_KEY_CHATGPT
ARG GRAPHQL_PARENT_PATH
ARG PANLEX_SQLITE_DB_PATH
ARG SQLITE_SPELLFIX_PREBUILT_PATH

RUN test $BIN_FILE
RUN test $API_KEY_CHATGPT
RUN test $GRAPHQL_PARENT_PATH
RUN test $PANLEX_SQLITE_DB_PATH
RUN test $SQLITE_SPELLFIX_PREBUILT_PATH

ENV API_KEY_CHATGPT $API_KEY_CHATGPT
ENV GRAPHQL_PARENT_PATH $GRAPHQL_PARENT_PATH
ENV PANLEX_SQLITE_DB_PATH $PANLEX_SQLITE_DB_PATH
ENV SQLITE_SPELLFIX_PREBUILT_PATH $SQLITE_SPELLFIX_PREBUILT_PATH

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates libssl3 \
 && apt-get clean \
 && rm -rf /var/lib/apt/lists/*

COPY ${BIN_FILE} /app/backend
RUN chmod +x /app/backend

EXPOSE 8080

CMD /app/backend \
  --graphql-parent-path "$GRAPHQL_PARENT_PATH" \
  --api-key-chatgpt "$API_KEY_CHATGPT" \
  --panlex-sqlite-db-path "$PANLEX_SQLITE_DB_PATH" \
  --sqlite-spellfix-prebuilt-path "$SQLITE_SPELLFIX_PREBUILT_PATH"
