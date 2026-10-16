#!/usr/bin/env bash
# Run on the prepared Linux host. Credentials/config stay in deploy/private, never command output.
set -Eeuo pipefail
cd /srv/spellgambit/deploy
exec 9>private/release.lock
flock -n 9 || { echo 'Another release is running' >&2; exit 1; }
sha=${1:-}; image=${2:-}
[[ $sha =~ ^[0-9a-f]{40}$ ]] || exit 2
[[ $image =~ ^ghcr\.io/[a-z0-9._/-]+@sha256:[0-9a-f]{64}$ ]] || exit 2
test -f "/srv/spellgambit/web/releases/$sha/.complete"
compose() { docker compose --env-file private/slots.env -f compose.production.yml "$@"; }
old=$(cat private/active.slot)
[[ $old == blue || $old == green ]] || exit 2
if [[ $old == blue ]]; then next=green; port=18082; else next=blue; port=18081; fi
# Do not take ownership of an operator's existing incident/drain barrier.
compose exec -T "game-$old" /server drain status | python3 -c 'import json,sys; assert not json.load(sys.stdin)["draining"], "Existing deployment drain requires operator review"'
backup=$(compose exec -T -u postgres postgres pgbackrest --stanza=game info --output=json)
printf '%s' "$backup" | python3 -c 'import json,sys,time; d=json.load(sys.stdin)[0]; b=[x for x in d["backup"] if x["type"]=="full"]; assert d["status"]["code"]==0 and b and time.time()-max(x["timestamp"]["stop"] for x in b)<93600, "No recent healthy full backup"'
compose exec -T -u postgres postgres pgbackrest --stanza=game check
cp private/slots.env private/slots.previous.env
cp private/caddy/active.caddy private/caddy/active.previous.caddy
previous_web=$(readlink /srv/spellgambit/web/current)
committed=0
drain_owned=0
rollback() {
    code=$?
    if (( code != 0 && committed == 0 )); then
        cp private/slots.previous.env private/slots.env
        cp private/caddy/active.previous.caddy private/caddy/active.caddy
        ln -s "$previous_web" /srv/spellgambit/web/current.rollback
        mv -Tf /srv/spellgambit/web/current.rollback /srv/spellgambit/web/current
        if compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile >/dev/null; then
            if (( drain_owned == 1 )); then compose exec -T "game-$old" /server drain cancel || true; fi
            compose stop "game-$next" || true
        else
            echo 'Proxy rollback failed: both slots retained, admission remains blocked; use runbook R2.' >&2
        fi
        echo 'Release cancelled; old server retained. Expand migrations are retained.' >&2
    fi
    exit "$code"
}
trap rollback EXIT
python3 - "$next" "$image" <<'PY'
import pathlib,sys
path=pathlib.Path('private/slots.env')
lines=[line for line in path.read_text().splitlines() if not line.startswith((sys.argv[1].upper()+'_IMAGE=', 'CANDIDATE_IMAGE='))]
lines += [sys.argv[1].upper()+'_IMAGE='+sys.argv[2], 'CANDIDATE_IMAGE='+sys.argv[2]]
path.write_text('\n'.join(lines)+'\n')
PY
compose pull "game-$next" migrate
drain_owned=1
compose exec -T "game-$old" /server drain start
compose run --rm --no-deps migrate
compose run --rm --no-deps "game-$next" check-config
idle=0
for ((i=0;i<240;i++)); do
    if compose exec -T "game-$old" /server drain idle >/dev/null 2>&1; then idle=1; break; fi
    sleep 2
done
(( idle == 1 )) || { echo 'Activities still running after 8 minutes; deferring release' >&2; exit 1; }
compose up -d --no-deps "game-$next"
ready=0
for ((i=0;i<60;i++)); do
    if curl --fail --silent --max-time 2 "http://127.0.0.1:$port/readyz" >/dev/null; then ready=1; break; fi
    sleep 1
done
(( ready == 1 )) || exit 1
curl --fail --silent --max-time 5 "http://127.0.0.1:$port/healthz" | python3 -c 'import json,sys,pathlib; expected=json.loads((pathlib.Path("/srv/spellgambit/web/releases")/sys.argv[1]/".vite/release.json").read_text()); actual=json.load(sys.stdin); assert all(actual.get(k)==expected[k] for k in ("release","engine","content")), "Server and browser release differ"' "$sha"
ln -s "releases/$sha" /srv/spellgambit/web/current.next
mv -Tf /srv/spellgambit/web/current.next /srv/spellgambit/web/current
printf 'reverse_proxy 127.0.0.1:%s {\n header_up X-Forwarded-For {client_ip}\n header_up -Forwarded\n stream_close_delay 10m\n}\n' "$port" >private/caddy/active.caddy
compose exec -T caddy caddy validate --config /etc/caddy/Caddyfile
compose exec -T caddy caddy reload --config /etc/caddy/Caddyfile
printf '%s\n' "$next" >private/active.slot
committed=1
# Keep the shared barrier until old WebSockets and background writers are gone.
# Otherwise an old socket could start an incompatible fight just before shutdown.
compose stop "game-$old"
# From this point rollback must use this same drain procedure, because new fights may start.
compose exec -T "game-$next" /server drain cancel
echo "Activated $sha on $next"
