const fs = require('node:fs');
const path = require('node:path');
const {createHash} = require('node:crypto');
const {pipeline} = require('node:stream/promises');

async function writeChecksums(directory) {
  const files = fs.readdirSync(directory, {withFileTypes:true})
    .filter(entry => entry.isFile() && entry.name.toLowerCase().endsWith('.exe'))
    .map(entry => entry.name).sort();
  if (!files.length) throw new Error('No release executables found');
  const lines = [];
  for (const filename of files) {
    const hash = createHash('sha256');
    await pipeline(fs.createReadStream(path.join(directory, filename)), hash);
    lines.push(`${hash.digest('hex')}  ${filename}`);
  }
  fs.writeFileSync(path.join(directory, 'SHA256SUMS.txt'), lines.join('\n') + '\n', 'ascii');
  console.log(`SHA256SUMS.txt created for ${files.length} executable(s).`);
}

if (require.main === module) {
  const directory = process.argv[2];
  if (!directory) { console.error('Usage: node scripts/write-checksums.cjs <release-directory>'); process.exitCode = 1; }
  else writeChecksums(directory).catch(error => {console.error(error.message);process.exitCode = 1;});
}
module.exports = {writeChecksums};
