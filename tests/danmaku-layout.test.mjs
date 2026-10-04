import test from 'node:test';
import assert from 'node:assert/strict';
import { playbackTime, scheduleComments, commentX, firstVisibleIndex } from '../src/frontend/lib/danmakuLayout.ts';

const comment = (time, mode = 1, text = '测试') => ({ id: `${time}-${mode}`, time, mode, size: 25, color: 0xffffff, text });
const measure = text => text.length * 25;
test('playback clock follows pause, buffering, seek and speed, and bounds disconnected extrapolation', () => {
  const p = { loaded: true, paused: false, buffering: false, ended: false, position: 10, duration: 60, speed: 2, sampledAtMs: 1000 };
  assert.equal(playbackTime(p, 1500), 11);
  assert.equal(playbackTime({ ...p, paused: true }, 1500), 10);
  assert.equal(playbackTime({ ...p, buffering: true }, 1500), 10);
  assert.equal(playbackTime({ ...p, position: 2, sampledAtMs: 1500 }, 1500), 2);
  assert.equal(playbackTime(p, 99999), 12.4);
});
test('reverse, top and bottom modes position correctly and seeking is deterministic', () => {
  const items = scheduleComments([comment(0), comment(1, 6), comment(2, 5), comment(3, 4), comment(20)], 562.5, 1, measure);
  assert.equal(commentX(items[0], 0), 1000);
  assert.equal(commentX(items[1], 1), -50);
  assert.equal(commentX(items[2], 3), 475);
  assert.ok(items[3].y > items[2].y);
  assert.equal(firstVisibleIndex(items, 20), 4);
  assert.equal(firstVisibleIndex(items, 2), 0);
});
test('dense comments drop only from render schedule and faster long comments cannot catch earlier comments', () => {
  const input = Array.from({ length: 100 }, () => comment(1));
  const items = scheduleComments(input, 562.5, 1, measure);
  assert.ok(items.length < input.length);
  assert.equal(input.length, 100);
  const sequence = scheduleComments([comment(0), comment(1, 1, '很长的弹幕'.repeat(20))], 180, 1, measure);
  assert.equal(sequence.length, 1);
});
