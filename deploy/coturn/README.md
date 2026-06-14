# coturn Deployment

This directory configures the STUN/TURN relay used by NetherLink WebRTC connections. nli-server issues temporary
credentials; coturn performs NAT discovery and relays traffic when a direct peer connection cannot be established.

## 1. DNS and addresses

Create an A record such as `turn.example.com` pointing to the server public IP. Determine both addresses:

```bash
curl -4 https://ifconfig.me
hostname -I
```

Alibaba Cloud ECS normally requires both values in `external-ip=PUBLIC_IP/PRIVATE_IP`, because the public address is
NAT-mapped and is not directly assigned to the VM network interface.

## 2. Install and configure

```bash
sudo apt update
sudo apt install coturn
openssl rand -base64 48
sudo cp deploy/coturn/turnserver.conf.example /etc/turnserver.conf
sudoedit /etc/turnserver.conf
```

Replace `PRIVATE_IP`, `PUBLIC_IP`, `TURN_DOMAIN`, and `TURN_SHARED_SECRET`. The shared secret must exactly match the
`TURN_SHARED_SECRET` used by nli-server. Do not commit the populated configuration.

On Ubuntu releases that ship `/etc/default/coturn`, ensure the service is enabled there:

```dotenv
TURNSERVER_ENABLED=1
```

Then start it:

```bash
sudo systemctl enable --now coturn
sudo systemctl restart coturn
sudo systemctl status coturn --no-pager
sudo journalctl -u coturn -n 100 --no-pager
```

## 3. Firewall

Allow these inbound ports in both the Alibaba Cloud security group and the host firewall:

```text
3478/udp             STUN and TURN
3478/tcp             TURN over TCP
5349/tcp             TURN over TLS, after certificates are enabled
49160-49200/udp      TURN relay allocation range
```

The UDP relay range is intentionally small for initial deployment. Increase it when concurrent relay usage grows.
Do not expose PostgreSQL or Redis publicly.

## 4. nli-server environment

Without TURN TLS:

```dotenv
TURN_URLS=stun:turn.example.com:3478,turn:turn.example.com:3478?transport=udp,turn:turn.example.com:3478?transport=tcp
TURN_SHARED_SECRET=<same-secret-as-static-auth-secret>
TURN_CREDENTIAL_TTL_SECONDS=600
```

After installing a valid certificate, remove `no-tls` and `no-dtls`, then enable `tls-listening-port`, `cert`, and
`pkey` in coturn:

```dotenv
TURN_URLS=stun:turn.example.com:3478,turn:turn.example.com:3478?transport=udp,turn:turn.example.com:3478?transport=tcp,turns:turn.example.com:5349?transport=tcp
```

Restart nli-server after changing its environment.

## 5. Verify

Check listeners and service logs:

```bash
sudo ss -lntup | grep -E ':(3478|5349)'
sudo journalctl -u coturn -f
```

Use the Trickle ICE test at `https://webrtc.github.io/samples/src/content/peerconnection/trickle-ice/`. Obtain a
temporary username and credential from `POST /v1/turn`, enter one of the returned TURN URLs, and confirm that a
candidate with type `relay` is produced. A `srflx` candidate proves STUN works; only `relay` proves TURN works.

For command-line troubleshooting, install `coturn` on a test host and run:

```bash
turnutils_uclient -v -t -u '<temporary-username>' -w '<temporary-credential>' turn.example.com
```

Run this test from a different network. Testing only from the TURN server itself does not validate public NAT and
firewall behavior.
