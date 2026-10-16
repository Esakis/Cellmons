#!/usr/bin/env python3
"""Render firewall and Caddy proxy trust together. Apply nft atomically only after validation."""
import ipaddress
import json
import pathlib
import subprocess
import sys
import urllib.request

directory = pathlib.Path('/srv/spellgambit/deploy/private')
def apply(config):
    exists = subprocess.run(['nft', 'list', 'table', 'inet', 'spellgambit'], capture_output=True).returncode == 0
    payload = ('delete table inet spellgambit\n' if exists else '') + config
    subprocess.run(['nft', '--check', '-f', '-'], input=payload, text=True, check=True)
    subprocess.run(['nft', '-f', '-'], input=payload, text=True, check=True)

if sys.argv[1:] == ['--restore']:
    apply((directory / 'firewall.nft').read_text())
    raise SystemExit(0)
if sys.argv[1:]:
    raise SystemExit('update-firewall.py [--restore]')
admins = [ipaddress.ip_network(value) for value in json.loads((directory / 'admin-networks.json').read_text())]
if not admins or any(network.prefixlen == 0 for network in admins):
    raise SystemExit('Configure bounded owner SSH networks first')
cloudflare = []
for family in ('v4', 'v6'):
    with urllib.request.urlopen('https://www.cloudflare.com/ips-' + family, timeout=15) as response:
        values = response.read(16384).decode('ascii').splitlines()
    networks = [ipaddress.ip_network(value) for value in values if value.strip()]
    if not 1 <= len(networks) <= 100 or any(not n.is_global or n.prefixlen < (8 if n.version==4 else 16) for n in networks):
        raise SystemExit('Invalid Cloudflare network list')
    cloudflare.extend(networks)
def rule(networks, version, port):
    values = ', '.join(str(n) for n in networks if n.version == version)
    return f'{"ip" if version == 4 else "ip6"} saddr {{ {values} }} tcp dport {port} accept' if values else ''
rules = '\n'.join(filter(None, [rule(admins, v, '22') for v in (4, 6)] + [rule(cloudflare, v, '{ 80, 443 }') for v in (4, 6)]))
# Host-network Caddy is the only public listener. Database and game containers publish loopback only.
config = f'''table inet spellgambit {{
chain input {{ type filter hook input priority -10; policy drop;
iifname "lo" accept
ct state established,related accept
ct state invalid drop
ip protocol icmp accept
ip6 nexthdr ipv6-icmp accept
{rules}
}}
}}
'''
apply(config)
(directory / 'trusted-proxies.caddy.next').write_text('trusted_proxies static ' + ' '.join(map(str, cloudflare)) + '\n')
(directory / 'trusted-proxies.caddy.next').replace(directory / 'caddy' / 'trusted-proxies.caddy')
(directory / 'firewall.nft').write_text(config)
print(f'Applied {len(cloudflare)} Cloudflare and {len(admins)} administrator networks')
subprocess.run(['docker','compose','--env-file',str(directory/'slots.env'),'-f',str(directory.parent/'compose.production.yml'),'exec','-T','caddy','caddy','reload','--config','/etc/caddy/Caddyfile'],check=True)
