#!/usr/bin/env bash
# No host executable or Docker socket is exposed to the scanner. Its networked DB
# update sees only an empty cache; scanning is offline against one read-only export.
set -Eeuo pipefail
(( $# > 0 )) || exit 2
scanner='anchore/grype:v0.120.1@sha256:e4a44ef45d285b829ce6efe2642980329661bd2d18eab5fc539138d4adaebbbe'
mkdir -p target/grype/db
cache="$PWD/target/grype/db"
docker pull "$scanner"
docker run --rm --cap-drop ALL --security-opt no-new-privileges --read-only \
  --tmpfs /tmp:rw,noexec,nosuid,size=512m \
  -e GRYPE_DB_CACHE_DIR=/cache -e GRYPE_CHECK_FOR_APP_UPDATE=false \
  -v "$cache:/cache" "$scanner" db update
index=0
status=0
for image in "$@"; do
  archive="$PWD/target/grype/image-$index.tar"
  docker image save --output "$archive" "${image#docker:}"
  docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges --read-only \
    --tmpfs /tmp:rw,noexec,nosuid,size=512m \
    -e GRYPE_DB_CACHE_DIR=/cache -e GRYPE_DB_AUTO_UPDATE=false \
    -e GRYPE_CHECK_FOR_APP_UPDATE=false -v "$cache:/cache:ro" \
    -v "$archive:/image.tar:ro" "$scanner" docker-archive:/image.tar \
    --fail-on high --output json >"target/grype/image-$index.json" || status=1
  rm -- "$archive"
  index=$((index+1))
done
exit "$status"
