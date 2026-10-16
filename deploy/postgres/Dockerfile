FROM postgres:18.6-alpine3.24@sha256:77f585114c32fbca283dc835b0596f4e52b51b4c6662d7810b2f4084f60a1873 AS base
RUN apk upgrade --no-cache

FROM base AS backup-build
RUN apk add --no-cache build-base meson ninja pkgconf curl curl-dev ca-certificates \
    libxml2-dev openssl-dev bzip2-dev lz4-dev zstd-dev zlib-dev libpq-dev
WORKDIR /source
# Official distribution, checksum from the matching upstream release asset.
# Only S3/local repositories are used here; omit the vulnerable, unused SFTP library.
RUN curl --fail --location --proto '=https' --tlsv1.2 \
      https://github.com/pgbackrest/pgbackrest/releases/download/release/2.59.3/pgbackrest-2.59.3.tar.gz -o source.tar.gz \
    && echo '14037901db002e5536a948bf9f0fc0ff6cde31f4e675d3e9b46f129071bf2e5f  source.tar.gz' | sha256sum -c - \
    && tar -xzf source.tar.gz --strip-components=1 \
    && meson setup build -Dlibssh2=disabled -Dlibsystemd=disabled -Dlibzstd=enabled \
    && meson compile -C build -j 4 \
    && meson test -C build --print-errorlogs

FROM base
RUN apk add --no-cache libbz2 lz4-libs zstd-libs libpq libxml2
COPY --from=backup-build /source/build/src/pgbackrest /usr/local/bin/pgbackrest
COPY --from=backup-build /source/LICENSE /usr/share/licenses/pgbackrest/LICENSE
RUN pgbackrest version
