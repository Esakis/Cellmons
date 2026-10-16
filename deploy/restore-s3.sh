#!/usr/bin/env bash
# Dedicated restore host only. Reads the configured encrypted S3 repository; never mounts live PGDATA.
set -Eeuo pipefail
cd /srv/spellgambit-restore
test -f private/pgbackrest.conf
[[ ${RESTORE_IMAGE:-} =~ ^ghcr\.io/[a-z0-9._/-]+@sha256:[0-9a-f]{64}$ ]] || exit 2
id="spellgambit-restore-$(openssl rand -hex 8)"
volume="$id-data"
started=$(date +%s)
target=$(date -u -d '2 minutes ago' '+%Y-%m-%d %H:%M:%S+00')
cleanup() {
    docker rm -f "$id" >/dev/null 2>&1 || true
    [[ $volume =~ ^spellgambit-restore-[0-9a-f]{16}-data$ ]] && docker volume rm "$volume" >/dev/null || true
}
trap cleanup EXIT
docker volume create "$volume" >/dev/null
docker run --rm -u 0 -v "$volume:/data" "$RESTORE_IMAGE" sh -c 'mkdir -p /data/18/docker && chown -R postgres:postgres /data'
docker run --rm -u postgres -v "$volume:/var/lib/postgresql" -v "$PWD/private/pgbackrest.conf:/etc/pgbackrest/pgbackrest.conf:ro" "$RESTORE_IMAGE" pgbackrest --stanza=game --type=time "--target=$target" --target-action=promote restore
docker run -d --name "$id" -v "$volume:/var/lib/postgresql" -v "$PWD/private/pgbackrest.conf:/etc/pgbackrest/pgbackrest.conf:ro" "$RESTORE_IMAGE" postgres -c archive_mode=off >/dev/null
ready=0
for ((i=0;i<600;i++)); do
    if docker exec "$id" pg_isready -U postgres -d game >/dev/null 2>&1; then ready=1; break; fi
    sleep 2
done
(( ready == 1 )) || exit 1
mkdir -p private/reports private/textfile
docker exec -i -u postgres "$id" psql -U postgres -d game -v ON_ERROR_STOP=1 -qAt <deploy/postgres/restore-check.sql >private/reports/latest.json
python3 - <<'PY'
import json,pathlib
report=json.loads(pathlib.Path('private/reports/latest.json').read_text())
assert report['unbalanced_transactions']==0 and report['incorrect_wallets']==0 and report['incorrect_escrows']==0, 'Restored ledger mismatch'
assert report['migrations']>0, 'No application migrations'
PY
elapsed=$(( $(date +%s)-started ))
(( elapsed < 3600 )) || { echo 'Restore exceeded one hour' >&2; exit 1; }
printf 'game_restore_success_seconds %s\ngame_restore_duration_seconds %s\n' "$(date +%s)" "$elapsed" >private/textfile/restore.prom.next
mv private/textfile/restore.prom.next private/textfile/restore.prom
echo "Isolated encrypted S3 restore and ledger check passed in $elapsed seconds."
