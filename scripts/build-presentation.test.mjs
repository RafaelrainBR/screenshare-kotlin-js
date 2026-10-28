import test from 'node:test';
import assert from 'node:assert/strict';
import { renderPage } from './build-presentation.mjs';

test('escapes metadata and refuses missing translations rather than publishing placeholders', () => {
  assert.equal(renderPage('<title>{{title}}</title>', { title: 'A & "B" <C>' }), '<title>A &amp; &quot;B&quot; &lt;C&gt;</title>');
  assert.throws(() => renderPage('{{missing}}', {}));
});
