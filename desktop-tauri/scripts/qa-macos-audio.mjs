// Validate measurements collected from the real macOS desktop and WebRTC receiver.
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
const directory = process.argv[2];
assert(directory, 'usage: node qa-macos-audio.mjs <evidence-directory>');
const state = name => JSON.parse(readFileSync(join(directory, `${name}.json`), 'utf8'));
const screen = name => {
  const data = state(name);
  assert.deepEqual(data.errors, [], `${name}: no receiver errors`);
  assert.equal(data.screens.length, 1, `${name}: one share`);
  const video = data.screens[0];
  assert(video.w > 0 && video.h > 0 && video.frames > 0, `${name}: real video received`);
  assert.equal(video.audioTracks, 1, `${name}: an audio track received`);
  return video;
};
assert(screen('chrome-coreaudio').rms > 0.001, 'Chrome helper audio must reach the receiver');
assert(screen('isolated').rms < 0.0001, 'other app audio must be excluded');
assert(screen('isolation-system-control').rms > 0.001, 'other app was actually audible in the system control');
assert.deepEqual(state('closed').screens, [], 'closing the source must end the share');
const converted = screen('rate44100');
assert(converted.rms > 0.001, '44.1 kHz output must remain audible');
assert(Math.abs(converted.frequencyHz - 440) < 8, '44.1 kHz output must preserve the 440 Hz tone');
console.log('PASS: Chrome helper audio, app isolation, system control, source-close cleanup, 44.1 kHz pitch');
