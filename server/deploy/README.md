# Deploying the Remota relay (rendezvous host)

The relay is the only component that needs a public address. It brokers WSS connections
between agents (which dial out from behind NAT) and the Remota app. Agents and targets need
**no** public IP and **no** inbound ports — only outbound 443.

```
 app  ──wss/443──►  Caddy (TLS)  ──http──►  remota-relay (127.0.0.1:8787)  ◄──wss/443──  agent
                    relay.privum.cloud                                                    (behind NAT)
```

## 1. Provision

A tiny VM is enough (1 vCPU / 1 GB). Keep it **separate** from the WireGuard VPN box.

- DNS: point `relay.privum.cloud` (A, and AAAA if you have IPv6) at the VM's public IP.
- Firewall / Azure NSG: allow inbound **TCP 80 and 443** only. (Port 8787 stays on loopback.)

## 2. Install Caddy (TLS termination)

```bash
sudo apt install -y debian-keyring debian-archive-keyring apt-transport-https curl
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/gpg.key' \
  | sudo gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
curl -1sLf 'https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt' \
  | sudo tee /etc/apt/sources.list.d/caddy-stable.list
sudo apt update && sudo apt install -y caddy

sudo cp Caddyfile /etc/caddy/Caddyfile
sudoedit /etc/caddy/Caddyfile              # set the domain, and the operator IP allowlist
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile
sudo systemctl reload caddy                 # Caddy fetches the Let's Encrypt cert on first hit
```

The shipped `Caddyfile` **fails closed**: `POST /session` and `GET /agents` answer `403` until you
replace the placeholder range (`192.0.2.0/24`) with your VPN / office CIDRs. `/agent/control` and
`/data/{id}` stay open — agents dial out from behind NAT and must always reach the relay.

## 3. Install the relay

On the VM (with this repo checked out), from `server/deploy/`:

```bash
./install.sh
```

This builds the release binary (or pass a prebuilt path), creates the `remota` service user,
writes `/etc/remota/relay.env` with a **freshly generated enrollment token**, installs the
systemd unit, and starts it. Note the printed token.

> No Rust on the VM? Build locally and copy the binary up:
> ```bash
> cargo build --release -p remota-relay
> scp ../target/release/remota-relay  user@vm:/tmp/remota-relay
> ./install.sh /tmp/remota-relay      # run this on the VM
> ```

## 4. Enroll an agent (on any target machine, behind NAT)

```bash
remota-agent --relay wss://relay.privum.cloud \
  --token <ENROLLMENT_TOKEN> \
  --name "servidor-cliente-x"
```

Verify it registered (from a host allowed to reach the relay):

```bash
curl https://relay.privum.cloud/agents
# [{"agent_id":"...","name":"servidor-cliente-x","os":"linux","capabilities":["cli"]}]
```

## 5. Broker a tunnel (until the app wires it in — T5)

```bash
curl -s https://relay.privum.cloud/session \
  -H 'content-type: application/json' \
  -d '{"agent_id":"<AGENT_ID>","target_port":22}'
# {"session_id":"...","token":"..."}
```

Both legs then meet at `wss://relay.privum.cloud/data/<session_id>?token=<token>&role=client`
(app/client) and `...&role=agent` (the agent connects automatically on OpenChannel). The relay
bridges raw bytes — so a TCP service like `sshd` on the target is reachable through it.

## Security (MVP limitations — read before exposing)

- **Enrollment** is a single shared token (`REMOTA_ENROLL_TOKEN`), **required**: the relay refuses
  to start if it is unset, under 16 characters, or the old `dev-enroll` placeholder. Rotate it by
  editing `/etc/remota/relay.env` and `systemctl restart remota-relay`.
- **`POST /session` and `GET /agents` are unauthenticated.** Anyone who can reach them can list
  registered agents and broker a tunnel to an agent's `target_port`. Until per-user auth lands
  (Phase 3) they are gated by source IP in `Caddyfile`, which ships denying everyone — set your
  own allowlist. Agents still reach the relay outbound regardless.
- **Session tokens are single-use** and consumed when the two legs pair. Reaching the token check
  at all requires the session's UUIDv4 id, which is only ever sent to the app and the agent.
  A brokered session whose legs never pair is currently kept until the relay restarts.
- **Enrollment and session tokens are compared in constant time**, so a failed attempt leaks
  neither the secret's contents nor its length. There is **no rate limiting** on the WebSocket
  handshakes yet — put one in front of the relay if it is exposed to the open internet.
- TLS is handled entirely by Caddy; the relay speaks plain HTTP on loopback.
