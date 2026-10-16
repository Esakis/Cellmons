#!/usr/bin/env bash
set -Eeuo pipefail
cd /srv/spellgambit/deploy
compose() { docker compose --env-file private/slots.env -f compose.production.yml "$@"; }
slot=$(cat private/active.slot)
[[ $slot == blue || $slot == green ]] || exit 2
case "${1:-}" in
  maintenance) compose exec -T "game-$slot" /server maintenance ;;
  backup)
    compose exec -T -u postgres postgres pgbackrest --stanza=game --type=full backup
    compose exec -T -u postgres postgres pgbackrest --stanza=game check
    mkdir -p private/textfile
    printf 'game_backup_success_seconds %s\n' "$(date +%s)" >private/textfile/backup.prom.next
    mv private/textfile/backup.prom.next private/textfile/backup.prom
    ;;
  telemetry)
    mkdir -p private/reports
    compose exec -T "game-$slot" /server telemetry-report >"private/reports/weekly-$(date -u +%F).json"
    ;;
  *) exit 2 ;;
esac
