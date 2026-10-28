# Self-hosting

[Documentation](README.md) · [Build from source](development.md)

The JVM server serves the browser app and a WebSocket endpoint at `/`. It stores rooms and chat in memory and relays signaling, not media. A TURN server may separately relay media when direct WebRTC connectivity is unavailable.

## Run a JAR

Install JDK 21 and build from the repository root:

```bash
./gradlew :server-java:buildFatJar
java -jar server-java/build/libs/server-java-all.jar
```

The default port is 8080; set the `PORT` environment variable to override it. Open [localhost:8080](http://localhost:8080) for local testing.

For remote use, terminate TLS at your reverse proxy and serve HTTPS/WSS. Forward HTTP and WebSocket upgrades to the same server, preserving long-lived connections. Browser capture APIs require a secure context outside `localhost`.

The deployed browser client connects to its own page's host. Packaged desktop releases connect to `screen-share.fly.dev` by default; hosting another web instance does not redirect those releases. The desktop app reads another server from the `SCREENSHARE_WS` environment variable (for example `wss://example.com/`).

## Docker

Build the JAR **before** building the image: the [Dockerfile](../server-java/Dockerfile) copies an existing JAR rather than compiling source.

```bash
./gradlew :server-java:buildFatJar
cd server-java
docker compose up --build -d
```

The image uses Amazon Corretto 21 Alpine and exposes port 8080. See [`docker-compose.yml`](../server-java/docker-compose.yml) for port mapping and restart policy. To configure ICE servers, explicitly pass `ICE_SERVERS` into the container's environment; the checked-in Compose file does not forward it automatically.

## STUN and TURN

Set `ICE_SERVERS` on the **server** to a JSON array before starting it:

```json
[
  { "urls": ["stun:stun.example.com:3478"] },
  {
    "urls": ["turn:turn.example.com:3478", "turn:turn.example.com:3478?transport=tcp"],
    "username": "your-turn-user",
    "credential": "your-turn-credential"
  }
]
```

Replace these placeholders with a relay you operate or are authorized to use. For example, in PowerShell:

```powershell
$env:ICE_SERVERS = '[{"urls":["stun:stun.example.com:3478"]}]'
.\gradlew.bat :server-java:run
```

The server sends this list to clients supporting protocol version 3 or later. Invalid JSON is logged and treated as an empty list. Current clients use a legacy fallback list when no server list is received; configure your own non-empty list instead of depending on that external service. Older desktop clients may need an update to use server-provided ICE settings.

Keep credentials out of Git and store deployment values as environment secrets. WebRTC clients receive TURN credentials to connect to the relay; an environment secret does not make them invisible to room participants. Use credentials and relay limits appropriate for that exposure.

## Fly.io

The checked-in configs use the app names `screen-share` and `screen-share-beta`, in region `gru`. Create your own uniquely named Fly app and update the config's `app` field before deploying.

Build the JAR, then run Fly from the directory containing the Dockerfile and config:

```bash
./gradlew :server-java:buildFatJar
cd server-java
fly deploy -c fly.toml
```

The production config forces HTTPS, enables machine auto-stop/start and sets connection-based concurrency for WebSockets. Review its sizing, capacity and startup settings for your workload. Supply `ICE_SERVERS` through Fly secrets when needed.

For a separate test instance, use [`fly.beta.toml`](../server-java/fly.beta.toml). The existing [`deploy-beta.sh`](../server-java/deploy-beta.sh) creates/deploys the fixed `screen-share-beta` app; update both the script's app name and the config if using your own instance. A beta instance serves its own client and keeps rooms separate from production.

## Operational limits

- There is no account system or room password. A room code grants access to the room.
- Rooms and chat are process-local. Restarts lose that state; independent server instances do not share rooms.
- WebRTC uses peer connections between participants. Bandwidth and encoding cost increase with more peers; this is not an SFU conferencing server.
- Signaling and chat pass through the server. Review server logging and retention before hosting sensitive sessions.
- Capture support still depends on client OS/browser permissions and source restrictions.
