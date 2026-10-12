import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';

const room = `qa-${randomUUID()}`;
const clients = [];
async function connect(version) {
  const socket = new WebSocket('ws://localhost:8080/');
  const packets = [];
  const waiters = new Set();
  const client = {
    socket,
    packets,
    waitFor(predicate) {
      const found = packets.find(predicate);
      if (found) return Promise.resolve(found);
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => { waiters.delete(receive); reject(new Error('Packet timeout')); }, 5000);
        const receive = packet => {
          if (!predicate(packet)) return;
          clearTimeout(timer);
          waiters.delete(receive);
          resolve(packet);
        };
        waiters.add(receive);
      });
    },
  };
  clients.push(client);
  socket.addEventListener('message', event => {
    const packet = JSON.parse(event.data);
    packets.push(packet);
    for (const receive of [...waiters]) receive(packet);
  });
  await new Promise((resolve, reject) => {
    socket.addEventListener('open', resolve, { once: true });
    socket.addEventListener('error', reject, { once: true });
  });
  socket.send(JSON.stringify({ type: 'join-room', rid: room, username: 'Mesmo Nome', ...(version === 2 ? { clientVersion: 2 } : {}) }));
  await client.waitFor(packet => packet.type === 'user-list');
  return client;
}

try {
  const a = await connect(2);
  const b = await connect(2);
  const legacy = await connect(1);
  const identityA = await a.waitFor(packet => packet.type === 'room-joined');
  const identityB = await b.waitFor(packet => packet.type === 'room-joined');
  assert.notEqual(identityA.sid, identityB.sid, 'Duplicate names must have distinct identities');
  a.socket.send(JSON.stringify({ type: 'send-message', rid: room, msg: 'QA receipt', clientMessageId: 'receipt-a' }));
  const receipt = await a.waitFor(packet => packet.type === 'chat-message' && packet.msg.clientMessageId === 'receipt-a');
  assert.equal(receipt.msg.senderSocketId, identityA.sid);
  const remote = await b.waitFor(packet => packet.type === 'chat-message' && packet.msg.clientMessageId === 'receipt-a');
  assert.equal(remote.msg.senderSocketId, identityA.sid);
  const oldMessage = await legacy.waitFor(packet => packet.type === 'chat-message' && packet.msg.content === 'QA receipt');
  assert.deepEqual(Object.keys(oldMessage.msg).sort(), ['content', 'timestamp', 'username']);
  assert.equal(legacy.packets.some(packet => packet.type === 'room-joined'), false);
  legacy.socket.send(JSON.stringify({ type: 'send-message', rid: room, msg: 'from 0.5.0' }));
  await a.waitFor(packet => packet.type === 'chat-message' && packet.msg.content === 'from 0.5.0');
  legacy.socket.send(JSON.stringify({ type: 'start-camera-share', rid: room }));
  await a.waitFor(packet => packet.type === 'camera-share-started');
  legacy.socket.send(JSON.stringify({ type: 'stop-camera-share', rid: room }));
  await a.waitFor(packet => packet.type === 'camera-share-stopped');
  // 0.5.0 decodes strictly: any packet type it doesn't know breaks its session.
  const knownTo050 = ['user-connected', 'user-disconnected', 'chat-message', 'user-list', 'ice-candidate-received', 'description-received', 'screen-share-started', 'screen-share-stopped', 'user-muted', 'user-unmuted', 'camera-share-started', 'camera-share-stopped'];
  assert.deepEqual(legacy.packets.filter(packet => !knownTo050.includes(packet.type)), []);
  b.socket.close();
  await a.waitFor(packet => packet.type === 'user-disconnected' && packet.sid === identityB.sid);
  console.log('PASS: duplicate identities, sender identity, delivery receipt, legacy (0.5.0) compatibility, room exit');
} finally {
  for (const { socket } of clients) socket.close();
}
