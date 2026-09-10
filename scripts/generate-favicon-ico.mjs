import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

const req = createRequire(import.meta.url);
const sharp = req('sharp');

const svgPath = path.resolve(rootDir, 'public/favicon.svg');
const svgBuffer = fs.readFileSync(svgPath);

// Standard sizes for Windows ICO
const sizes = [16, 32, 48];
const pngBuffers = await Promise.all(
  sizes.map((size) => sharp(svgBuffer).resize(size, size).png().toBuffer())
);

// Construct ICO binary (ICONDIR + ICONDIRENTRY[] + PNG blobs)
function createIco(pngs, dims) {
  const count = pngs.length;
  const headerSize = 6;
  const dirEntrySize = 16;
  const dirSize = headerSize + count * dirEntrySize;

  let offset = dirSize;
  const entries = [];
  for (let i = 0; i < count; i++) {
    const size = dims[i];
    const buf = pngs[i];
    entries.push({
      width: size >= 256 ? 0 : size,
      height: size >= 256 ? 0 : size,
      colorCount: 0,
      reserved: 0,
      planes: 1,
      bitCount: 32,
      bytesInRes: buf.length,
      imageOffset: offset,
    });
    offset += buf.length;
  }

  const out = Buffer.alloc(offset);

  // Write ICONDIR
  out.writeUInt16LE(0, 0); // reserved
  out.writeUInt16LE(1, 2); // image type: 1 = ICO
  out.writeUInt16LE(count, 4); // number of images

  // Write ICONDIRENTRY for each image
  let p = headerSize;
  for (let i = 0; i < count; i++) {
    const e = entries[i];
    out.writeUInt8(e.width, p);
    out.writeUInt8(e.height, p + 1);
    out.writeUInt8(e.colorCount, p + 2);
    out.writeUInt8(e.reserved, p + 3);
    out.writeUInt16LE(e.planes, p + 4);
    out.writeUInt16LE(e.bitCount, p + 6);
    out.writeUInt32LE(e.bytesInRes, p + 8);
    out.writeUInt32LE(e.imageOffset, p + 12);
    p += dirEntrySize;
  }

  // Write image payloads
  for (let i = 0; i < count; i++) {
    pngs[i].copy(out, entries[i].imageOffset);
  }

  return out;
}

const icoBuffer = createIco(pngBuffers, sizes);

// Write to both src/app/favicon.ico and public/favicon.ico
const targets = [
  path.resolve(rootDir, 'src/app/favicon.ico'),
  path.resolve(rootDir, 'public/favicon.ico'),
];

for (const target of targets) {
  fs.writeFileSync(target, icoBuffer);
  console.log('Updated:', target, `(${icoBuffer.length} bytes)`);
}
