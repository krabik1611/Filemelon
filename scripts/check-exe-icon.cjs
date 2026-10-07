const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const executable = process.argv[2];
if (!executable) throw new Error('Usage: node scripts/check-exe-icon.cjs <executable>');
const binary = fs.readFileSync(executable);
const icon = fs.readFileSync(path.resolve(__dirname, '../src-tauri/icons/icon.ico'));
assert.equal(binary.toString('ascii', 0, 2), 'MZ', 'Expected Windows executable');
const pe = binary.readUInt32LE(60);
assert.equal(binary.readUInt32LE(pe), 0x4550, 'Invalid PE signature');
const optional = pe + 24;
const sectionTable = optional + binary.readUInt16LE(pe + 20);
const sectionCount = binary.readUInt16LE(pe + 6);
const magic = binary.readUInt16LE(optional);
assert.ok(magic === 0x20b || magic === 0x10b, 'Unsupported PE format');
function offset(rva) {
  for (let i = 0; i < sectionCount; i++) {
    const section = sectionTable + i * 40;
    const address = binary.readUInt32LE(section + 12);
    const size = Math.max(binary.readUInt32LE(section + 8), binary.readUInt32LE(section + 16));
    if (rva >= address && rva < address + size) return binary.readUInt32LE(section + 20) + rva - address;
  }
  throw new Error('PE resource address is outside sections');
}
const resourceRva = binary.readUInt32LE(optional + (magic === 0x20b ? 112 : 96) + 16);
assert.ok(resourceRva, 'Executable has no resources');
const root = offset(resourceRva);
const images = [];
function walk(relative, depth, type) {
  assert.ok(depth <= 3, 'Invalid resource tree depth');
  const directory = root + relative;
  const count = binary.readUInt16LE(directory + 12) + binary.readUInt16LE(directory + 14);
  for (let i = 0; i < count; i++) {
    const entry = directory + 16 + i * 8;
    const id = binary.readUInt32LE(entry);
    const pointer = binary.readUInt32LE(entry + 4);
    const resourceType = depth === 0 ? id : type;
    if (depth === 0 && resourceType !== 3) continue;
    if (pointer >>> 31) walk(pointer & 0x7fffffff, depth + 1, resourceType);
    else if (resourceType === 3) {
      const data = root + pointer;
      const start = offset(binary.readUInt32LE(data));
      const length = binary.readUInt32LE(data + 4);
      assert.ok(start + length <= binary.length, 'Invalid icon resource size');
      images.push(binary.subarray(start, start + length));
    }
  }
}
walk(0, 0, 0);
const count = icon.readUInt16LE(4);
assert.ok(count > 0, 'Source ICO has no images');
for (let i = 0; i < count; i++) {
  const entry = 6 + i * 16;
  const size = icon.readUInt32LE(entry + 8);
  const start = icon.readUInt32LE(entry + 12);
  assert.ok(images.some(image => image.equals(icon.subarray(start, start + size))), `EXE does not contain Filemelon icon frame ${i}`);
}
console.log(`Verified Filemelon icon in executable (${count} sizes).`);
