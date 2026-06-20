# Nginx Reverse Proxy

The provided configuration exposes nli-server as `https://nli-api.muyucloud.cool`, including WebSocket signaling at
`wss://nli-api.muyucloud.cool/v1/signaling/ws`.

## 1. DNS and certificate

Point the `nli-api.muyucloud.cool` A/AAAA record to the Nginx host. Obtain a certificate if the domain is not already
covered by an existing wildcard certificate:

```bash
sudo apt update
sudo apt install nginx certbot python3-certbot-nginx
sudo certbot certonly --nginx -d nli-api.muyucloud.cool
```

If an existing wildcard certificate covers `*.muyucloud.cool`, edit `ssl_certificate` and `ssl_certificate_key` in the
template to use that certificate's actual paths.

## 2. Install the configuration

```bash
sudo cp deploy/nginx/nli-api.conf.example /etc/nginx/conf.d/nli-api.conf
sudoedit /etc/nginx/conf.d/nli-api.conf
sudo nginx -t
sudo systemctl reload nginx
```

The template assumes Nginx and nli-server run on the same host. Use these nli-server settings:

```dotenv
NLI_ENV=production
NLI_BIND_ADDR=127.0.0.1:8080
NLI_TRUST_PROXY_HEADERS=true
NLI_METRICS_TOKEN=<strong-random-token>
```

`NLI_TRUST_PROXY_HEADERS=true` is safe here only because nli-server listens on loopback and all external traffic must
pass through this trusted proxy.

For a separate proxy host, replace the upstream server with the nli-server private address:

```nginx
server 10.0.0.20:8080;
```

Set `NLI_BIND_ADDR` to that private interface and allow port 8080 only from the proxy host. Do not proxy to nli-server
over plaintext HTTP across the public Internet.

## 3. Verify

```bash
curl -i http://nli-api.muyucloud.cool/health
curl -i https://nli-api.muyucloud.cool/health
curl -i https://nli-api.muyucloud.cool/metrics
curl -i -H 'Authorization: Bearer <metrics-token>' https://nli-api.muyucloud.cool/metrics
```

Expected results:

- HTTP redirects to HTTPS.
- `/health` returns `200` when PostgreSQL and Redis are healthy.
- `/metrics` without its token returns `401` when `NLI_METRICS_TOKEN` is configured.
- `/metrics` with the token returns Prometheus text.

The WebSocket endpoint requires a valid runtime instance token. Test it with the NetherLink client or another WebSocket
client that can send the `Authorization` header.
