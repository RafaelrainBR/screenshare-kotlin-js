const assert = require('node:assert/strict');
const braces = require('../../build/js/node_modules/braces');

assert.deepEqual(braces.expand('src/{a,b}/{1..3}.kt'), [
  'src/a/1.kt', 'src/a/2.kt', 'src/a/3.kt',
  'src/b/1.kt', 'src/b/2.kt', 'src/b/3.kt'
]);
assert.equal(braces.compile('a/{b,c}/d'), 'a/(b|c)/d');
assert.equal(braces.stringify(braces.parse('a/{b,c}/d')), 'a/{b,c}/d');
assert.equal(braces.compile('\\{literal\\}'), '{literal}');

const depthError = { name: 'RangeError', message: 'braces nesting depth exceeds limit' };
for (const [open, close] of [['{', '}'], ['(', ')']]) {
  for (const pattern of [open.repeat(4000) + 'a' + close.repeat(4000), open.repeat(4000)]) {
    for (const operation of ['parse', 'compile', 'expand']) {
      assert.throws(() => braces[operation](pattern), depthError);
    }
  }
}

let ast = { type: 'text', value: 'a' };
for (let i = 0; i < 4000; i++) ast = { type: 'root', nodes: [ast] };
for (const operation of ['compile', 'expand', 'stringify']) {
  assert.throws(() => braces[operation](ast), depthError);
}
console.log('Installed braces override: normal patterns pass; deep patterns and ASTs are bounded.');
