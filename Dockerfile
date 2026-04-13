# =============================================================================
# Stage 1: Build the Dioxus fullstack application with Informix CSDK
# =============================================================================
FROM rust:1.93-bullseye AS builder

# -- System dependencies for unixODBC -----------------------------------------
RUN apt-get update && apt-get install -y --no-install-recommends \
        libncurses5 \
        unixodbc \
        unixodbc-dev \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*

# -- WASM target for Dioxus frontend ------------------------------------------
RUN rustup target add wasm32-unknown-unknown

# -- Install Dioxus CLI -------------------------------------------------------
RUN cargo install dioxus-cli --version "=0.7.5"

# -- Informix user/group (some CSDK libs expect this) -------------------------
RUN groupadd informix \
    && useradd -g informix -d /opt/informix -m informix

# -- Extract CSDK from tarball and install to /opt/informix --------------------
COPY csdk/csdk.tar.gz /tmp/csdk.tar.gz
RUN tar xzf /tmp/csdk.tar.gz -C /tmp \
    && cp -r /tmp/csdk/lib   /opt/informix/lib \
    && cp -r /tmp/csdk/bin   /opt/informix/bin \
    && cp -r /tmp/csdk/incl  /opt/informix/incl \
    && cp -r /tmp/csdk/msg   /opt/informix/msg \
    && cp -r /tmp/csdk/gls   /opt/informix/gls \
    && rm -rf /tmp/csdk.tar.gz /tmp/csdk

# -- Informix connection configuration ----------------------------------------
COPY ifx-config/sqlhosts-docker /opt/informix/etc/sqlhosts

RUN chown -R informix:informix /opt/informix \
    && chmod 775 -R /opt/informix

# -- ODBC driver/DSN configuration --------------------------------------------
COPY ifx-config/odbcinst.ini    /etc/odbcinst.ini
COPY ifx-config/odbc-docker.ini /etc/odbc.ini

# -- Informix environment for the build ---------------------------------------
ENV INFORMIXDIR=/opt/informix
ENV INFORMIXSERVER=informix
ENV LD_LIBRARY_PATH=/opt/informix/lib:/opt/informix/lib/esql:/opt/informix/lib/cli
ENV PATH=/opt/informix/bin:${PATH}
ENV DB_LOCALE=en_US.819
ENV CLIENT_LOCALE=en_US.UTF8
ENV ODBCSYSINI=/etc
ENV ODBCINI=/etc/odbc.ini

# -- Copy workspace source and build ------------------------------------------
WORKDIR /usr/src/app
COPY Cargo.toml ./
COPY crates/ ./crates/

# Build with Dioxus CLI — bundles server binary + WASM frontend
# default_platform = "fullstack" is set in crates/app/Dioxus.toml
RUN dx bundle --package app --release

# =============================================================================
# Stage 2: Slim runtime image
# =============================================================================
# NOTE: This Dockerfile is configured for the local dev Informix instance
# (plain TCP, Protocol=onsoctcp). For a production Informix over SSL you will
# additionally need to:
#   1. Copy csdk/gskit into the builder and extract to /opt/informix/gskit
#   2. Copy SSL certificates to /opt/informix/ssl/ and config/conssl.cfg
#   3. Copy the gskit ibm libs to the runtime and symlink into /usr/lib
#   4. Set DB_LOCALE/CLIENT_LOCALE to en_US.8859-1 to match the server
#   5. Change Protocol=onsoctcp → onsocssl in db.rs connection_string()
# See cminter/Dockerfile for a reference implementation with SSL.

FROM debian:bullseye-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        libncurses5 \
        unixodbc \
        ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd informix \
    && useradd -g informix -d /opt/informix -m informix

# -- CSDK runtime libs ---------------------------------------------------------
COPY --from=builder /opt/informix/lib     /opt/informix/lib
COPY --from=builder /opt/informix/etc     /opt/informix/etc
COPY --from=builder /opt/informix/msg     /opt/informix/msg
COPY --from=builder /opt/informix/gls     /opt/informix/gls

# -- ODBC configuration -------------------------------------------------------
COPY ifx-config/odbcinst.ini    /etc/odbcinst.ini
COPY ifx-config/odbc-docker.ini /etc/odbc.ini
RUN chmod 644 /etc/odbcinst.ini /etc/odbc.ini

# -- Informix environment ------------------------------------------------------
ENV INFORMIXDIR=/opt/informix
ENV INFORMIXSERVER=informix
ENV LD_LIBRARY_PATH=/opt/informix/lib:/opt/informix/lib/esql:/opt/informix/lib/cli
ENV PATH=/opt/informix/bin:${PATH}
ENV DB_LOCALE=en_US.819
ENV CLIENT_LOCALE=en_US.UTF8
ENV ODBCSYSINI=/etc
ENV ODBCINI=/etc/odbc.ini
ENV INFORMIXSQLHOSTS=/opt/informix/etc/sqlhosts
ENV INFORMIXCONTIME=30
ENV INFORMIXCONRETRY=3

# -- Copy the bundled Dioxus output --------------------------------------------
# dx bundle outputs to target/dx/app/release/web/
COPY --from=builder /usr/src/app/target/dx/app/release/web/ /srv/app/

RUN chown -R informix:informix /opt/informix \
    && chmod 775 -R /opt/informix

USER informix
WORKDIR /srv/app

EXPOSE 8080

ENTRYPOINT ["./server"]
