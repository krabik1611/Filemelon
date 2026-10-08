const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const test = require('node:test');
const {writeChecksums} = require('../scripts/write-checksums.cjs');

test('release checksums match known SHA-256 values, sort names and exclude other files', async t => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'filemelon-checksums-'));
  t.after(() => fs.rmSync(directory, {recursive:true,force:true}));
  fs.writeFileSync(path.join(directory, 'portable.exe'), 'abc');
  fs.writeFileSync(path.join(directory, 'installer.exe'), '');
  fs.writeFileSync(path.join(directory, 'notes.txt'), 'ignored');
  await writeChecksums(directory);
  assert.equal(fs.readFileSync(path.join(directory, 'SHA256SUMS.txt'), 'ascii'),
    'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855  installer.exe\n' +
    'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad  portable.exe\n');
  fs.unlinkSync(path.join(directory, 'installer.exe'));
  fs.unlinkSync(path.join(directory, 'portable.exe'));
  await assert.rejects(writeChecksums(directory), /No release executables/);
});
