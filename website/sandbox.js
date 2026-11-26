// Pure browser/Node subset evaluator & strict type diagnostic engine for DOG v0.3.0
// Mirrors DOG's English + classic syntax, strict typing (no implicit text + number), and block rules.

const MAX_STEPS = 8000;

export class DogRuntimeError extends Error {
  constructor({ code, title, line, why, fix, tip, severity = "error", stack = null }) {
    super(`${code}: ${title}`);
    this.code = code;
    this.severity = severity;
    this.title = title;
    this.line = line || 1;
    this.why = why || "";
    this.fix = fix || "";
    this.tip = tip || "";
    this.stackFrames = Array.isArray(stack) ? stack : [];
  }

  toJSON() {
    return {
      code: this.code,
      severity: this.severity,
      line: this.line,
      title: this.title,
      why: this.why,
      fix: this.fix,
      tip: this.tip,
      stack: this.stackFrames,
    };
  }

  format() {
    const stackLines =
      this.stackFrames.length > 0
        ? [
            "STACK TRACE:",
            ...this.stackFrames.map((f) => `  at ${f.name}() [line ${f.line}]`),
          ]
        : [];
    return [
      `[${this.code}] line ${this.line}: ${this.title}`,
      this.why ? `WHY: ${this.why}` : "",
      this.fix ? `FIX: ${this.fix}` : "",
      this.tip ? `TIP: ${this.tip}` : "",
      ...stackLines,
    ]
      .filter(Boolean)
      .join("\n");
  }
}

// Pure-JS UTF-8, Hex, Base64, SHA-256, HMAC-SHA256 & PBKDF2-SHA256 (works identically in browser & Node)
const _te = new TextEncoder();
const _td = new TextDecoder();

function sha256Bytes(bytes) {
  const K = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ];
  let h0 = 0x6a09e667, h1 = 0xbb67ae85, h2 = 0x3c6ef372, h3 = 0xa54ff53a;
  let h4 = 0x510e527f, h5 = 0x9b05688c, h6 = 0x1f83d9ab, h7 = 0x5be0cd19;
  const bitLen = bytes.length * 8;
  const padLen = ((bytes.length + 9 + 63) >> 6) << 6;
  const msg = new Uint8Array(padLen);
  msg.set(bytes);
  msg[bytes.length] = 0x80;
  const dv = new DataView(msg.buffer);
  dv.setUint32(padLen - 8, Math.floor(bitLen / 0x100000000), false);
  dv.setUint32(padLen - 4, bitLen >>> 0, false);
  const w = new Int32Array(64);
  for (let i = 0; i < padLen; i += 64) {
    for (let t = 0; t < 16; t++) w[t] = dv.getInt32(i + t * 4, false);
    for (let t = 16; t < 64; t++) {
      const s0 = ((w[t - 15] >>> 7) | (w[t - 15] << 25)) ^ ((w[t - 15] >>> 18) | (w[t - 15] << 14)) ^ (w[t - 15] >>> 3);
      const s1 = ((w[t - 2] >>> 17) | (w[t - 2] << 15)) ^ ((w[t - 2] >>> 19) | (w[t - 2] << 13)) ^ (w[t - 2] >>> 10);
      w[t] = (w[t - 16] + s0 + w[t - 7] + s1) | 0;
    }
    let a = h0, b = h1, c = h2, d = h3, e = h4, f = h5, g = h6, h = h7;
    for (let t = 0; t < 64; t++) {
      const S1 = ((e >>> 6) | (e << 26)) ^ ((e >>> 11) | (e << 21)) ^ ((e >>> 25) | (e << 7));
      const ch = (e & f) ^ (~e & g);
      const t1 = (h + S1 + ch + K[t] + w[t]) | 0;
      const S0 = ((a >>> 2) | (a << 30)) ^ ((a >>> 13) | (a << 19)) ^ ((a >>> 22) | (a << 10));
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const t2 = (S0 + maj) | 0;
      h = g; g = f; f = e; e = (d + t1) | 0;
      d = c; c = b; b = a; a = (t1 + t2) | 0;
    }
    h0 = (h0 + a) | 0; h1 = (h1 + b) | 0; h2 = (h2 + c) | 0; h3 = (h3 + d) | 0;
    h4 = (h4 + e) | 0; h5 = (h5 + f) | 0; h6 = (h6 + g) | 0; h7 = (h7 + h) | 0;
  }
  const out = new Uint8Array(32);
  const odv = new DataView(out.buffer);
  [h0, h1, h2, h3, h4, h5, h6, h7].forEach((v, idx) => odv.setUint32(idx * 4, v >>> 0, false));
  return out;
}

function bytesToHex(bytes) {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

function hexToBytes(hexStr, lineNum) {
  const clean = String(hexStr || "").trim();
  if (clean.length % 2 !== 0 || /[^0-9a-fA-F]/.test(clean)) {
    throw new DogRuntimeError({ code: "E030", title: "Invalid hexadecimal input.", line: lineNum });
  }
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  return out;
}

function sha1Bytes(bytes) {
  const len = bytes.length;
  const bitLen = len * 8;
  const padLen = ((len + 9 + 63) >> 6) << 6;
  const msg = new Uint8Array(padLen);
  msg.set(bytes);
  msg[len] = 0x80;
  const dv = new DataView(msg.buffer);
  dv.setUint32(padLen - 8, Math.floor(bitLen / 0x100000000), false);
  dv.setUint32(padLen - 4, bitLen >>> 0, false);
  let h0 = 0x67452301, h1 = 0xefcdab89, h2 = 0x98badcfe, h3 = 0x10325476, h4 = 0xc3d2e1f0;
  const w = new Int32Array(80);
  const rotl = (n, c) => (n << c) | (n >>> (32 - c));
  for (let i = 0; i < padLen; i += 64) {
    for (let t = 0; t < 16; t++) w[t] = dv.getInt32(i + t * 4, false);
    for (let t = 16; t < 80; t++) w[t] = rotl(w[t - 3] ^ w[t - 8] ^ w[t - 14] ^ w[t - 16], 1);
    let a = h0, b = h1, c = h2, d = h3, e = h4;
    for (let t = 0; t < 80; t++) {
      let f = 0, k = 0;
      if (t < 20) { f = (b & c) | (~b & d); k = 0x5a827999; }
      else if (t < 40) { f = b ^ c ^ d; k = 0x6ed9eba1; }
      else if (t < 60) { f = (b & c) | (b & d) | (c & d); k = 0x8f1bbcdc; }
      else { f = b ^ c ^ d; k = 0xca62c1d6; }
      const temp = (rotl(a, 5) + f + e + k + w[t]) | 0;
      e = d; d = c; c = rotl(b, 30); b = a; a = temp;
    }
    h0 = (h0 + a) | 0; h1 = (h1 + b) | 0; h2 = (h2 + c) | 0; h3 = (h3 + d) | 0; h4 = (h4 + e) | 0;
  }
  const out = new Uint8Array(20);
  const odv = new DataView(out.buffer);
  [h0, h1, h2, h3, h4].forEach((v, idx) => odv.setUint32(idx * 4, v >>> 0, false));
  return out;
}

const SHA512_K = [
  0x428a2f98d728ae22n, 0x7137449123ef65cdn, 0xb5c0fbcfec4d3b2fn, 0xe9b5dba58189dbbcn,
  0x3956c25bf348b538n, 0x59f111f1b605d019n, 0x923f82a4af194f9bn, 0xab1c5ed5da6d8118n,
  0xd807aa98a3030242n, 0x12835b0145706fben, 0x243185be4ee4b28cn, 0x550c7dc3d5ffb4e2n,
  0x72be5d74f27b896fn, 0x80deb1fe3b1696b1n, 0x9bdc06a725c71235n, 0xc19bf174cf692694n,
  0xe49b69c19ef14ad2n, 0xefbe4786384f25e3n, 0x0fc19dc68b8cd5b5n, 0x240ca1cc77ac9c65n,
  0x2de92c6f592b0275n, 0x4a7484aa6ea6e483n, 0x5cb0a9dcbd41fbd4n, 0x76f988da831153b5n,
  0x983e5152ee66dfabn, 0xa831c66d2db43210n, 0xb00327c898fb213fn, 0xbf597fc7beef0ee4n,
  0xc6e00bf33da88fc2n, 0xd5a79147930aa725n, 0x06ca6351e003826fn, 0x142929670a0e6e70n,
  0x27b70a8546d22ffcn, 0x2e1b21385c26c926n, 0x4d2c6dfc5ac42aedn, 0x53380d139d95b3dfn,
  0x650a73548baf63den, 0x766a0abb3c77b2a8n, 0x81c2c92e47edaee6n, 0x92722c851482353bn,
  0xa2bfe8a14cf10364n, 0xa81a664bbc423001n, 0xc24b8b70d0f89791n, 0xc76c51a30654be30n,
  0xd192e819d6ef5218n, 0xd69906245565a910n, 0xf40e35855771202an, 0x106aa07032bbd1b8n,
  0x19a4c116b8d2d0c8n, 0x1e376c085141ab53n, 0x2748774cdf8eeb99n, 0x34b0bcb5e19b48a8n,
  0x391c0cb3c5c95a63n, 0x4ed8aa4ae3418acbn, 0x5b9cca4f7763e373n, 0x682e6ff3d6b2b8a3n,
  0x748f82ee5defb2fcn, 0x78a5636f43172f60n, 0x84c87814a1f0ab72n, 0x8cc702081a6439ecn,
  0x90befffa23631e28n, 0xa4506cebde82bde9n, 0xbef9a3f7b2c67915n, 0xc67178f2e372532bn,
  0xca273eceea26619cn, 0xd186b8c721c0c207n, 0xeada7dd6cde0eb1en, 0xf57d4f7fee6ed178n,
  0x06f067aa72176fban, 0x0a637dc5a2c898a6n, 0x113f9804bef90daen, 0x1b710b35131c471bn,
  0x28db77f523047d84n, 0x32caab7b40c72493n, 0x3c9ebe0a15c9bebcn, 0x431d67c49c100d4cn,
  0x4cc5d4becb3e42b6n, 0x597f299cfc657e2an, 0x5fcb6fab3ad6faecn, 0x6c44198c4a475817n,
];
const M64 = 0xffffffffffffffffn;
const rotr64 = (x, n) => ((x >> BigInt(n)) | (x << BigInt(64 - n))) & M64;

function sha512Bytes(bytes) {
  const len = bytes.length;
  const padLen = ((len + 17 + 127) >> 7) << 7;
  const msg = new Uint8Array(padLen);
  msg.set(bytes);
  msg[len] = 0x80;
  const dv = new DataView(msg.buffer);
  dv.setBigUint64(padLen - 8, BigInt(len) * 8n, false);
  let h = [
    0x6a09e667f3bcc908n, 0xbb67ae8584caa73bn, 0x3c6ef372fe94f82bn, 0xa54ff53a5f1d36f1n,
    0x510e527fade682d1n, 0x9b05688c2b3e6c1fn, 0x1f83d9abfb41bd6bn, 0x5be0cd19137e2179n,
  ];
  const w = new Array(80).fill(0n);
  for (let i = 0; i < padLen; i += 128) {
    for (let t = 0; t < 16; t++) w[t] = dv.getBigUint64(i + t * 8, false);
    for (let t = 16; t < 80; t++) {
      const s0 = rotr64(w[t - 15], 1) ^ rotr64(w[t - 15], 8) ^ (w[t - 15] >> 7n);
      const s1 = rotr64(w[t - 2], 19) ^ rotr64(w[t - 2], 61) ^ (w[t - 2] >> 6n);
      w[t] = (w[t - 16] + s0 + w[t - 7] + s1) & M64;
    }
    let [a, b, c, d, e, f, g, hh] = h;
    for (let t = 0; t < 80; t++) {
      const S1 = rotr64(e, 14) ^ rotr64(e, 18) ^ rotr64(e, 41);
      const ch = (e & f) ^ ((~e & M64) & g);
      const t1 = (hh + S1 + ch + SHA512_K[t] + w[t]) & M64;
      const S0 = rotr64(a, 28) ^ rotr64(a, 34) ^ rotr64(a, 39);
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const t2 = (S0 + maj) & M64;
      hh = g; g = f; f = e; e = (d + t1) & M64;
      d = c; c = b; b = a; a = (t1 + t2) & M64;
    }
    h = [(h[0] + a) & M64, (h[1] + b) & M64, (h[2] + c) & M64, (h[3] + d) & M64, (h[4] + e) & M64, (h[5] + f) & M64, (h[6] + g) & M64, (h[7] + hh) & M64];
  }
  const out = new Uint8Array(64);
  const odv = new DataView(out.buffer);
  h.forEach((v, idx) => odv.setBigUint64(idx * 8, v, false));
  return out;
}

function hmacSha256Bytes(keyBytes, msgBytes) {
  let k = keyBytes.length > 64 ? sha256Bytes(keyBytes) : keyBytes;
  const ipad = new Uint8Array(64 + msgBytes.length);
  const opad = new Uint8Array(64 + 32);
  for (let i = 0; i < 64; i++) {
    const kb = i < k.length ? k[i] : 0;
    ipad[i] = kb ^ 0x36;
    opad[i] = kb ^ 0x5c;
  }
  ipad.set(msgBytes, 64);
  opad.set(sha256Bytes(ipad), 64);
  return sha256Bytes(opad);
}

function constantTimeStrEq(a, b) {
  const ba = _te.encode(String(a || ""));
  const bb = _te.encode(String(b || ""));
  const len = Math.max(ba.length, bb.length);
  let diff = ba.length ^ bb.length;
  for (let i = 0; i < len; i++) {
    diff |= (ba[i] || 0) ^ (bb[i] || 0);
  }
  return diff === 0;
}

function pbkdf2Sha256Hex(password, salt, iterations = 1000) {
  const passBytes = _te.encode(String(password || ""));
  const saltBytes = _te.encode(String(salt || ""));
  const iters = Math.min(Math.max(1, Math.trunc(iterations)), 20000);
  const input = new Uint8Array(saltBytes.length + 4);
  input.set(saltBytes);
  input[saltBytes.length + 3] = 1;
  let u = hmacSha256Bytes(passBytes, input);
  const t = new Uint8Array(u);
  for (let c = 1; c < iters; c++) {
    u = hmacSha256Bytes(passBytes, u);
    for (let j = 0; j < 32; j++) t[j] ^= u[j];
  }
  return bytesToHex(t);
}

function randomBytesArr(n) {
  const buf = new Uint8Array(n);
  if (globalThis.crypto?.getRandomValues) globalThis.crypto.getRandomValues(buf);
  else for (let i = 0; i < n; i++) buf[i] = Math.floor(Math.random() * 256);
  return buf;
}

function encryptAes256Auth(keyStr, plaintext) {
  const keyHash = sha256Bytes(_te.encode(String(keyStr || "")));
  const encKey = sha256Bytes(_te.encode(`enc:${bytesToHex(keyHash)}`));
  const macKey = sha256Bytes(_te.encode(`mac:${bytesToHex(keyHash)}`));
  const nonce = randomBytesArr(12);
  const pt = _te.encode(String(plaintext || ""));
  const ct = new Uint8Array(pt.length);
  for (let off = 0, ctr = 0; off < pt.length; off += 32, ctr++) {
    const blockIn = new Uint8Array(32 + 12 + 4);
    blockIn.set(encKey, 0);
    blockIn.set(nonce, 32);
    new DataView(blockIn.buffer).setUint32(44, ctr, false);
    const ks = sha256Bytes(blockIn);
    for (let j = 0; j < 32 && off + j < pt.length; j++) ct[off + j] = pt[off + j] ^ ks[j];
  }
  const macIn = new Uint8Array(nonce.length + ct.length);
  macIn.set(nonce, 0);
  macIn.set(ct, nonce.length);
  const tag = hmacSha256Bytes(macKey, macIn);
  return `v1:${bytesToHex(nonce)}:${bytesToHex(ct)}:${bytesToHex(tag)}`;
}

function decryptAes256Auth(keyStr, token, lineNum) {
  const parts = String(token || "").split(":");
  if (parts.length !== 4 || parts[0] !== "v1") {
    throw new DogRuntimeError({ code: "E030", title: "Invalid encrypted payload format.", line: lineNum });
  }
  const keyHash = sha256Bytes(_te.encode(String(keyStr || "")));
  const encKey = sha256Bytes(_te.encode(`enc:${bytesToHex(keyHash)}`));
  const macKey = sha256Bytes(_te.encode(`mac:${bytesToHex(keyHash)}`));
  const nonce = hexToBytes(parts[1], lineNum);
  const ct = hexToBytes(parts[2], lineNum);
  const macIn = new Uint8Array(nonce.length + ct.length);
  macIn.set(nonce, 0);
  macIn.set(ct, nonce.length);
  const expectedTag = bytesToHex(hmacSha256Bytes(macKey, macIn));
  if (!constantTimeStrEq(expectedTag, parts[3].toLowerCase())) {
    throw new DogRuntimeError({ code: "E030", title: "Ciphertext authentication failed (wrong key or tampered data).", line: lineNum });
  }
  const pt = new Uint8Array(ct.length);
  for (let off = 0, ctr = 0; off < ct.length; off += 32, ctr++) {
    const blockIn = new Uint8Array(32 + 12 + 4);
    blockIn.set(encKey, 0);
    blockIn.set(nonce, 32);
    new DataView(blockIn.buffer).setUint32(44, ctr, false);
    const ks = sha256Bytes(blockIn);
    for (let j = 0; j < 32 && off + j < ct.length; j++) pt[off + j] = ct[off + j] ^ ks[j];
  }
  return _td.decode(pt);
}

// Genuine RFC 8032 Ed25519 digital signatures over Curve25519 (pure BigInt + SHA-512)
const ED_P = (1n << 255n) - 19n;
const ED_L = (1n << 252n) + 27742317777372353535851937790883648493n;
const edMod = (a, m = ED_P) => ((a % m) + m) % m;
function edPow(base, exp, m = ED_P) {
  let r = 1n, b = edMod(base, m), e = exp;
  while (e > 0n) {
    if (e & 1n) r = (r * b) % m;
    b = (b * b) % m;
    e >>= 1n;
  }
  return r;
}
const edInv = (a) => edPow(a, ED_P - 2n, ED_P);
const ED_D = edMod(-121665n * edInv(121666n));
const ED_I = edPow(2n, (ED_P - 1n) / 4n);
const ED_BY = edMod(4n * edInv(5n));
function edXRecover(y) {
  const xx = edMod((y * y - 1n) * edInv(ED_D * y * y + 1n));
  let x = edPow(xx, (ED_P + 3n) / 8n);
  if (edMod(x * x - xx) !== 0n) x = edMod(x * ED_I);
  if ((x & 1n) !== 0n) x = ED_P - x;
  return x;
}
const ED_G = [edXRecover(ED_BY), ED_BY, 1n, edMod(edXRecover(ED_BY) * ED_BY)];
function edPtAdd(P, Q) {
  const [X1, Y1, Z1, T1] = P, [X2, Y2, Z2, T2] = Q;
  const A = edMod((Y1 - X1) * (Y2 - X2));
  const B = edMod((Y1 + X1) * (Y2 + X2));
  const C = edMod(2n * ED_D * T1 * T2);
  const D = edMod(2n * Z1 * Z2);
  const E = edMod(B - A), F = edMod(D - C), G = edMod(D + C), H = edMod(B + A);
  return [edMod(E * F), edMod(G * H), edMod(F * G), edMod(E * H)];
}
function edScalarMul(s, Pt = ED_G) {
  let acc = [0n, 1n, 1n, 0n], cur = Pt, k = edMod(s, ED_L);
  while (k > 0n) {
    if (k & 1n) acc = edPtAdd(acc, cur);
    cur = edPtAdd(cur, cur);
    k >>= 1n;
  }
  return acc;
}
function edPtEncode(Pt) {
  const zi = edInv(Pt[2]);
  const x = edMod(Pt[0] * zi);
  const y = edMod(Pt[1] * zi);
  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i++) out[i] = Number((y >> BigInt(i * 8)) & 0xffn);
  if (x & 1n) out[31] |= 0x80;
  return out;
}
function edPtDecode(bytes) {
  let y = 0n;
  for (let i = 0; i < 32; i++) y |= BigInt(bytes[i] & (i === 31 ? 0x7f : 0xff)) << BigInt(i * 8);
  if (y >= ED_P) return null;
  let x = edXRecover(y);
  if ((x & 1n) !== BigInt((bytes[31] >> 7) & 1)) x = ED_P - x;
  if (edMod(-(x * x) + y * y) !== edMod(1n + ED_D * x * x * y * y)) return null;
  return [x, y, 1n, edMod(x * y)];
}
function edLeToInt(bytes) {
  let r = 0n;
  for (let i = 0; i < bytes.length; i++) r |= BigInt(bytes[i]) << BigInt(i * 8);
  return r;
}
function edIntToLe32(n) {
  const out = new Uint8Array(32);
  for (let i = 0; i < 32; i++) out[i] = Number((n >> BigInt(i * 8)) & 0xffn);
  return out;
}
function edExpandSecret(seedBytes) {
  const h = sha512Bytes(seedBytes);
  const aBytes = new Uint8Array(h.slice(0, 32));
  aBytes[0] &= 248;
  aBytes[31] &= 127;
  aBytes[31] |= 64;
  return { a: edLeToInt(aBytes), prefix: h.slice(32, 64) };
}
function edSignHex(privHex, message) {
  const seed = hexToBytes(privHex, 1);
  if (seed.length !== 32) throw new DogRuntimeError({ code: "E030", title: "Ed25519 private key must be 32 bytes (64 hex chars).", line: 1 });
  const { a, prefix } = edExpandSecret(seed);
  const pubBytes = edPtEncode(edScalarMul(a));
  const msgBytes = _te.encode(String(message || ""));
  const rIn = new Uint8Array(prefix.length + msgBytes.length);
  rIn.set(prefix); rIn.set(msgBytes, prefix.length);
  const r = edMod(edLeToInt(sha512Bytes(rIn)), ED_L);
  const RBytes = edPtEncode(edScalarMul(r));
  const kIn = new Uint8Array(32 + 32 + msgBytes.length);
  kIn.set(RBytes, 0); kIn.set(pubBytes, 32); kIn.set(msgBytes, 64);
  const k = edMod(edLeToInt(sha512Bytes(kIn)), ED_L);
  const S = edMod(r + k * a, ED_L);
  const sig = new Uint8Array(64);
  sig.set(RBytes, 0); sig.set(edIntToLe32(S), 32);
  return bytesToHex(sig);
}
function edVerifyHex(pubHex, message, sigHex) {
  try {
    const pubBytes = hexToBytes(pubHex, 1);
    const sigBytes = hexToBytes(sigHex, 1);
    if (pubBytes.length !== 32 || sigBytes.length !== 64) return false;
    const A = edPtDecode(pubBytes);
    const R = edPtDecode(sigBytes.slice(0, 32));
    if (!A || !R) return false;
    const S = edLeToInt(sigBytes.slice(32, 64));
    if (S >= ED_L) return false;
    const msgBytes = _te.encode(String(message || ""));
    const kIn = new Uint8Array(32 + 32 + msgBytes.length);
    kIn.set(sigBytes.slice(0, 32), 0); kIn.set(pubBytes, 32); kIn.set(msgBytes, 64);
    const k = edMod(edLeToInt(sha512Bytes(kIn)), ED_L);
    const lhs = edPtEncode(edScalarMul(S));
    const rhs = edPtEncode(edPtAdd(R, edScalarMul(k, A)));
    return constantTimeStrEq(bytesToHex(lhs), bytesToHex(rhs));
  } catch {
    return false;
  }
}

// Pure-JS In-Memory Relational SQL Engine (SQLite-compatible subset for std.data)
function createSqlDb(name = ":memory:") {
  const db = {
    name,
    tables: new Map(), // table -> { columns: string[], rows: object[], nextId: number }
    txStack: [],
  };
  return { __dogType: "db", __db: db };
}

function splitSqlValues(raw) {
  const vals = [];
  let cur = "", inStr = false, quote = "";
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i];
    if (inStr) {
      if (ch === quote && raw[i + 1] === quote) { cur += quote; i++; continue; }
      if (ch === quote) { inStr = false; continue; }
      cur += ch;
    } else if (ch === "'" || ch === '"') {
      inStr = true; quote = ch; cur += ch;
    } else if (ch === ",") {
      vals.push(cur.trim()); cur = "";
    } else {
      cur += ch;
    }
  }
  if (cur.trim()) vals.push(cur.trim());
  return vals;
}

function bindSqlLiteralOrParam(rawToken, params, pIdxRef) {
  const t = rawToken.trim();
  if (t === "?") {
    if (pIdxRef.i >= params.length) throw new Error("Missing SQL '?' parameter value.");
    return params[pIdxRef.i++];
  }
  if ((t.startsWith("'") && t.endsWith("'")) || (t.startsWith('"') && t.endsWith('"'))) return t.slice(1, -1);
  if (/^-?\d+$/.test(t)) return parseInt(t, 10);
  if (/^-?\d+\.\d+$/.test(t)) return makeTypedVal("decimal", Number(t));
  if (/^true$/i.test(t)) return true;
  if (/^false$/i.test(t)) return false;
  if (/^null$/i.test(t)) return null;
  return t;
}

function evalSqlWhereClause(row, whereRaw, params, pIdxRef) {
  if (!whereRaw) return true;
  const parts = whereRaw.split(/\s+AND\s+/i);
  for (const part of parts) {
    const m = part.trim().match(/^([a-zA-Z_][a-zA-Z0-9_]*)\s*(=|!=|>=|<=|>|<|LIKE)\s*(.+)$/i);
    if (!m) throw new Error(`Unsupported WHERE clause: ${part}`);
    const [, col, op, rhsRaw] = m;
    const rhs = bindSqlLiteralOrParam(rhsRaw, params, pIdxRef);
    const lhs = row[col];
    const lv = unwrapNum(lhs)?.n ?? formatDogValue(lhs);
    const rv = unwrapNum(rhs)?.n ?? formatDogValue(rhs);
    if (op === "=" && String(lhs) !== String(rhs)) return false;
    if (op === "!=" && String(lhs) === String(rhs)) return false;
    if (op === ">" && !(lv > rv)) return false;
    if (op === "<" && !(lv < rv)) return false;
    if (op === ">=" && !(lv >= rv)) return false;
    if (op === "<=" && !(lv <= rv)) return false;
    if (op.toUpperCase() === "LIKE") {
      const re = new RegExp("^" + String(rhs).replace(/%/g, ".*").replace(/_/g, ".") + "$", "i");
      if (!re.test(String(lhs ?? ""))) return false;
    }
  }
  return true;
}

function runSqlStatement(dbHandle, rawSql, params = [], lineNum = 1) {
  const db = dbHandle?.__db;
  if (!db) throw new DogRuntimeError({ code: "E030", title: "Expected a valid db handle from db_open().", line: lineNum });
  const sql = String(rawSql || "").trim().replace(/;+$/, "");
  const pIdxRef = { i: 0 };

  const createM = sql.match(/^CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s*\((.+)\)$/i);
  if (createM) {
    const tName = createM[1];
    if (!db.tables.has(tName)) {
      const cols = splitSqlValues(createM[2]).map((c) => c.trim().split(/\s+/)[0]);
      db.tables.set(tName, { columns: cols, rows: [], nextId: 1 });
    }
    return { changes: 0, rows: [] };
  }

  const insM = sql.match(/^INSERT\s+INTO\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]+)\)\s*VALUES\s*\((.+)\)$/i);
  if (insM) {
    const tName = insM[1];
    const tbl = db.tables.get(tName);
    if (!tbl) throw new DogRuntimeError({ code: "E030", title: `SQL table '${tName}' does not exist.`, line: lineNum });
    const cols = splitSqlValues(insM[2]);
    const vals = splitSqlValues(insM[3]).map((v) => bindSqlLiteralOrParam(v, params, pIdxRef));
    const row = {};
    cols.forEach((c, idx) => { row[c] = vals[idx] ?? null; });
    if (tbl.columns.includes("id") && row.id === undefined) row.id = tbl.nextId++;
    tbl.rows.push(row);
    return { changes: 1, rows: [row] };
  }

  const selM = sql.match(/^SELECT\s+(.+?)\s+FROM\s+([a-zA-Z_][a-zA-Z0-9_]*)(?:\s+WHERE\s+(.+?))?(?:\s+ORDER\s+BY\s+([a-zA-Z_][a-zA-Z0-9_]*)(?:\s+(ASC|DESC))?)?(?:\s+LIMIT\s+(\d+))?$/i);
  if (selM) {
    const [, proj, tName, whereRaw, orderCol, orderDir, limitRaw] = selM;
    const tbl = db.tables.get(tName);
    if (!tbl) throw new DogRuntimeError({ code: "E030", title: `SQL table '${tName}' does not exist.`, line: lineNum });
    let rows = tbl.rows.filter((r) => evalSqlWhereClause(r, whereRaw, params, { i: pIdxRef.i }));
    if (orderCol) {
      const desc = String(orderDir || "ASC").toUpperCase() === "DESC";
      rows = [...rows].sort((a, b) => {
        const av = unwrapNum(a[orderCol])?.n ?? String(a[orderCol] ?? "");
        const bv = unwrapNum(b[orderCol])?.n ?? String(b[orderCol] ?? "");
        return av < bv ? (desc ? 1 : -1) : av > bv ? (desc ? -1 : 1) : 0;
      });
    }
    if (limitRaw) rows = rows.slice(0, parseInt(limitRaw, 10));
    const cols = proj.trim() === "*" ? null : splitSqlValues(proj);
    return {
      changes: 0,
      rows: rows.map((r) => {
        if (!cols) return { ...r };
        const sub = {};
        cols.forEach((c) => { sub[c] = r[c]; });
        return sub;
      }),
    };
  }

  const updM = sql.match(/^UPDATE\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+SET\s+(.+?)(?:\s+WHERE\s+(.+))?$/i);
  if (updM) {
    const [, tName, setRaw, whereRaw] = updM;
    const tbl = db.tables.get(tName);
    if (!tbl) throw new DogRuntimeError({ code: "E030", title: `SQL table '${tName}' does not exist.`, line: lineNum });
    const assigns = splitSqlValues(setRaw).map((a) => {
      const eq = a.indexOf("=");
      return [a.slice(0, eq).trim(), bindSqlLiteralOrParam(a.slice(eq + 1), params, pIdxRef)];
    });
    let changes = 0;
    for (const r of tbl.rows) {
      if (evalSqlWhereClause(r, whereRaw, params, { i: pIdxRef.i })) {
        assigns.forEach(([k, v]) => { r[k] = v; });
        changes++;
      }
    }
    return { changes, rows: [] };
  }

  const delM = sql.match(/^DELETE\s+FROM\s+([a-zA-Z_][a-zA-Z0-9_]*)(?:\s+WHERE\s+(.+))?$/i);
  if (delM) {
    const [, tName, whereRaw] = delM;
    const tbl = db.tables.get(tName);
    if (!tbl) throw new DogRuntimeError({ code: "E030", title: `SQL table '${tName}' does not exist.`, line: lineNum });
    const kept = [];
    let changes = 0;
    for (const r of tbl.rows) {
      if (evalSqlWhereClause(r, whereRaw, params, { i: pIdxRef.i })) changes++;
      else kept.push(r);
    }
    tbl.rows = kept;
    return { changes, rows: [] };
  }

  throw new DogRuntimeError({ code: "E030", title: `Unsupported SQL statement: ${sql}`, line: lineNum });
}

function stripComment(raw) {
  let inStr = false;
  let escape = false;
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i];
    if (escape) {
      escape = false;
      continue;
    }
    if (inStr) {
      if (ch === "\\") escape = true;
      else if (ch === '"') inStr = false;
      continue;
    }
    if (ch === '"') {
      inStr = true;
      continue;
    }
    if (ch === "#" || (ch === "/" && raw[i + 1] === "/")) {
      return raw.slice(0, i);
    }
  }
  return raw;
}

export function tokenizeExpression(src, lineNum = 1) {
  const tokens = [];
  let i = 0;
  while (i < src.length) {
    const ch = src[i];
    if (ch === " " || ch === "\t" || ch === "\r") {
      i++;
      continue;
    }
    if (ch === "$" && src[i + 1] === '"') {
      i += 2;
      const parts = [];
      let lit = "";
      let closed = false;
      while (i < src.length) {
        const c = src[i];
        if (c === '"') {
          i++;
          closed = true;
          break;
        }
        if (c === "\\") {
          const next = src[i + 1];
          i += 2;
          if (next === "n") lit += "\n";
          else if (next === "t") lit += "\t";
          else if (next !== undefined) lit += next;
          continue;
        }
        if (c === "{" && src[i + 1] === "{") {
          lit += "{";
          i += 2;
          continue;
        }
        if (c === "}" && src[i + 1] === "}") {
          lit += "}";
          i += 2;
          continue;
        }
        if (c === "{") {
          if (lit.length > 0) {
            parts.push({ type: "lit", value: lit });
            lit = "";
          }
          i++;
          let depth = 1;
          let exprStr = "";
          let innerStr = false;
          while (i < src.length && depth > 0) {
            const ec = src[i];
            if (innerStr) {
              exprStr += ec;
              if (ec === "\\") {
                exprStr += src[i + 1] || "";
                i += 2;
                continue;
              }
              if (ec === '"') innerStr = false;
              i++;
              continue;
            }
            if (ec === '"') {
              innerStr = true;
              exprStr += ec;
              i++;
              continue;
            }
            if (ec === "{") depth++;
            else if (ec === "}") {
              depth--;
              if (depth === 0) {
                i++;
                break;
              }
            }
            exprStr += ec;
            i++;
          }
          if (depth !== 0) {
            throw new DogRuntimeError({
              code: "E002",
              title: "Unclosed '{' inside interpolated string.",
              line: lineNum,
              why: "Interpolation placeholder `{...}` was not closed with '}'.",
              fix: 'Close braces inside $"...", e.g. $"Hi {name}".',
            });
          }
          parts.push({ type: "expr", value: exprStr.trim() });
          continue;
        }
        lit += c;
        i++;
      }
      if (!closed) {
        throw new DogRuntimeError({
          code: "E002",
          title: "Unterminated interpolated string literal.",
          line: lineNum,
          why: 'String opened with $" was not closed before the end of the line.',
          fix: 'Add a closing `"` at the end of the string.',
        });
      }
      if (lit.length > 0 || parts.length === 0) {
        parts.push({ type: "lit", value: lit });
      }
      tokens.push({ kind: "interp", value: parts });
      continue;
    }
    if (ch === '"') {
      i++;
      let s = "";
      let closed = false;
      while (i < src.length) {
        const c = src[i];
        if (c === '"') {
          i++;
          closed = true;
          break;
        }
        if (c === "\\") {
          const next = src[i + 1];
          i += 2;
          if (next === "n") s += "\n";
          else if (next === "t") s += "\t";
          else if (next !== undefined) s += next;
          continue;
        }
        s += c;
        i++;
      }
      if (!closed) {
        throw new DogRuntimeError({
          code: "E002",
          title: "Unterminated string literal.",
          line: lineNum,
          why: 'A string opened with `"` was not closed.',
          fix: 'Add a matching closing `"` quote.',
        });
      }
      tokens.push({ kind: "string", value: s });
      continue;
    }
    if (/[0-9]/.test(ch)) {
      if (ch === "0" && (src[i + 1] === "x" || src[i + 1] === "X")) {
        i += 2;
        let hexRaw = "";
        while (i < src.length && /[0-9a-fA-F_]/.test(src[i])) {
          if (src[i] !== "_") hexRaw += src[i];
          i++;
        }
        if (!hexRaw) {
          throw new DogRuntimeError({ code: "E003", title: "Invalid hexadecimal literal.", line: lineNum, why: "Expected hex digits after `0x`.", fix: "Write e.g. `0xFF` or `0x10_00`." });
        }
        tokens.push({ kind: "number", value: parseInt(hexRaw, 16) });
        continue;
      }
      if (ch === "0" && (src[i + 1] === "b" || src[i + 1] === "B")) {
        i += 2;
        let binRaw = "";
        while (i < src.length && /[01_]/.test(src[i])) {
          if (src[i] !== "_") binRaw += src[i];
          i++;
        }
        if (!binRaw) {
          throw new DogRuntimeError({ code: "E003", title: "Invalid binary literal.", line: lineNum, why: "Expected binary digits after `0b`.", fix: "Write e.g. `0b1010` or `0b1010_0101`." });
        }
        tokens.push({ kind: "number", value: parseInt(binRaw, 2) });
        continue;
      }
      let raw = "";
      let isDec = false;
      while (i < src.length && (/[0-9_]/.test(src[i]) || (src[i] === "." && !isDec && /[0-9]/.test(src[i + 1] || "")))) {
        if (src[i] === ".") isDec = true;
        raw += src[i];
        i++;
      }
      const clean = raw.replace(/_/g, "");
      tokens.push({ kind: isDec ? "decimal" : "number", value: isDec ? Number(clean) : parseInt(clean, 10) });
      continue;
    }
    if (/[a-zA-Z_]/.test(ch)) {
      let ident = "";
      while (i < src.length && /[a-zA-Z0-9_]/.test(src[i])) {
        ident += src[i];
        i++;
      }
      if (ident === "true" || ident === "false") {
        tokens.push({ kind: "bool", value: ident === "true" });
      } else if (ident === "null") {
        tokens.push({ kind: "null", value: null });
      } else {
        tokens.push({ kind: "ident", value: ident });
      }
      continue;
    }
    const two = src.slice(i, i + 2);
    if (["==", "!=", ">=", "<=", "..", "=>", "->", "|>", "??"].includes(two)) {
      tokens.push({ kind: "op", value: two, start: i, end: i + 2 });
      i += 2;
      continue;
    }
    if (ch === "!" && src[i + 1] !== "=") {
      throw new DogRuntimeError({
        code: "E001",
        title: "Unexpected character '!'.",
        line: lineNum,
        why: "DOG uses English logical words instead of C-style '!' negation.",
        fix: "Write `not condition` instead of `!condition` (keep `!=` for not-equal).",
        tip: "DOG reads like English: `if not ready` or `if name is not \"\"`.",
      });
    }
    if (ch === "&") {
      throw new DogRuntimeError({
        code: "E001",
        title: "Unexpected character '&'.",
        line: lineNum,
        why: "DOG uses the English word 'and' instead of '&' or '&&'.",
        fix: "Replace `&&` with `and`, e.g. `if a > 0 and b > 0`.",
      });
    }
    if (ch === "|") {
      throw new DogRuntimeError({
        code: "E001",
        title: "Unexpected character '|'.",
        line: lineNum,
        why: "DOG uses the English word 'or' instead of '|' or '||'.",
        fix: "Replace `||` with `or`, e.g. `if role is \"admin\" or role is \"mod\"`.",
      });
    }
    if (ch === ";") {
      throw new DogRuntimeError({
        code: "E001",
        title: "Unexpected character ';'.",
        line: lineNum,
        why: "DOG uses newlines to end statements and does not require semicolons.",
        fix: "Remove the trailing `;` at the end of the line.",
      });
    }
    if ("+-*/%><()[],.:{}".includes(ch)) {
      tokens.push({ kind: "op", value: ch, start: i, end: i + 1 });
      i++;
      continue;
    }
    throw new DogRuntimeError({
      code: "E001",
      title: `Unexpected character '${ch}'.`,
      line: lineNum,
      why: "Encountered syntax not valid in DOG.",
      fix: "Remove stray punctuation or wrap text in double quotes.",
    });
  }
  return tokens;
}

function typeNameOf(val) {
  if (val === undefined || val === null) return "void";
  if (typeof val === "string") return "text";
  if (typeof val === "boolean") return "bool";
  if (typeof val === "number") return Number.isInteger(val) ? "number" : "decimal";
  if (Array.isArray(val)) return "list";
  if (typeof val === "object" && val.__dogLambda) return "fn";
  if (typeof val === "object" && val.__dogType) return val.__dogType;
  if (typeof val === "object") return "map";
  return "text";
}

function makeTypedVal(kind, value) {
  if (kind === "decimal") {
    return { __dogType: "decimal", value: Number(value) };
  }
  return value;
}

function unwrapNum(v) {
  if (v && typeof v === "object" && v.__dogType === "decimal") return { isDecimal: true, n: v.value };
  if (typeof v === "number") return { isDecimal: !Number.isInteger(v), n: v };
  return null;
}

export function formatDogValue(v) {
  if (v && typeof v === "object" && v.__dogType === "decimal") {
    const s = String(v.value);
    return s.includes(".") ? s : `${s}.0`;
  }
  if (v && typeof v === "object" && v.__enum) {
    const pay = v.__payload?.length ? `(${v.__payload.map(formatDogValue).join(", ")})` : "";
    return `${v.__enum}.${v.__variant}${pay}`;
  }
  if (v && typeof v === "object" && v.__structName) {
    const fields = Object.entries(v)
      .filter(([k]) => !k.startsWith("__"))
      .map(([k, val]) => `${k}: ${formatDogValue(val)}`)
      .join(", ");
    return `${v.__structName} { ${fields} }`;
  }
  if (v && typeof v === "object" && v.__dogLambda) {
    return `fn(${(v.params || []).map((p) => p.name).join(", ")})`;
  }
  if (Array.isArray(v)) return `[${v.map(formatDogValue).join(", ")}]`;
  if (v && typeof v === "object") return JSON.stringify(v);
  return String(v);
}

function dogEquals(a, b, lineNum) {
  const ta = typeNameOf(a);
  const tb = typeNameOf(b);
  if (ta !== tb) {
    throw new DogRuntimeError({
      code: "E021",
      title: "Type mismatch detected.",
      line: lineNum,
      why: `Cannot compare '${ta}' with '${tb}' without explicit conversion.`,
      fix: "Convert one side with `text()`, `number()`, or `decimal()` so both sides match.",
      tip: "DOG prevents silent comparisons across mismatched types.",
    });
  }
  const na = unwrapNum(a);
  const nb = unwrapNum(b);
  if (na && nb) return na.n === nb.n;
  return JSON.stringify(a) === JSON.stringify(b);
}

function evalExprString(exprSrc, ctx, lineNum) {
  const tokens = tokenizeExpression(exprSrc, lineNum);
  // Annotate char offsets for ident/string/num tokens if missing so lambda body slicing works reliably
  let scanIdx = 0;
  for (const tok of tokens) {
    if (tok.start === undefined) {
      const needle = tok.kind === "bool" ? String(tok.value) : tok.kind === "ident" ? tok.value : null;
      if (needle) {
        const found = exprSrc.indexOf(needle, scanIdx);
        if (found >= 0) {
          tok.start = found;
          tok.end = found + needle.length;
          scanIdx = tok.end;
        }
      }
    } else {
      scanIdx = tok.end;
    }
  }
  let pos = 0;
  const peek = (offset = 0) => tokens[pos + offset];
  const peekIdent = (offset = 0) => {
    const t = tokens[pos + offset];
    return t && t.kind === "ident" ? t.value : null;
  };
  const consume = () => tokens[pos++];

  function parseNullish() {
    let left = parseOr();
    while (peek() && peek().kind === "op" && peek().value === "??") {
      consume();
      const right = parseOr();
      if (left === null || left === undefined) {
        left = right;
      }
    }
    return left;
  }

  function parsePipe() {
    let left = parseNullish();
    while (peek() && peek().kind === "op" && peek().value === "|>") {
      consume();
      const t = peek();
      if (t && t.kind === "ident" && t.value !== "fn" && t.value !== "do") {
        const parts = [consume().value];
        while (peek() && peek().kind === "op" && peek().value === ".") {
          consume();
          const f = consume();
          if (!f || f.kind !== "ident") {
            throw new DogRuntimeError({ code: "E004", title: "Expected identifier after '.' in pipe target.", line: lineNum });
          }
          parts.push(f.value);
        }
        const calleeName = parts.join(".");
        if (peek() && peek().kind === "op" && peek().value === "(") {
          const restArgs = parseCallArgs();
          const bound = parts.length === 1 ? ctx.lookup(calleeName) : null;
          if (bound && bound.value && bound.value.__dogLambda) {
            left = invokeLambdaObject(bound.value, [left, ...restArgs], ctx, lineNum);
          } else {
            left = callNamespaceOrBuiltin(calleeName, [left, ...restArgs], ctx, lineNum);
          }
          continue;
        }
        if (parts.length === 1) {
          const bound = ctx.lookup(calleeName);
          if (bound && bound.value && bound.value.__dogLambda) {
            left = invokeLambdaObject(bound.value, [left], ctx, lineNum);
            continue;
          }
          if (ctx.functions.has(calleeName)) {
            left = callNamespaceOrBuiltin(calleeName, [left], ctx, lineNum);
            continue;
          }
          left = callNamespaceOrBuiltin(calleeName, [left], ctx, lineNum);
          continue;
        }
        left = callNamespaceOrBuiltin(calleeName, [left], ctx, lineNum);
        continue;
      }
      const fnTarget = parseNullish();
      if (fnTarget && typeof fnTarget === "object" && fnTarget.__dogLambda) {
        left = invokeLambdaObject(fnTarget, [left], ctx, lineNum);
      } else {
        throw new DogRuntimeError({ code: "E021", title: "Pipe '|>' requires a function or lambda on the right side.", line: lineNum });
      }
    }
    return left;
  }

  function parseOr() {
    let left = parseAnd();
    while (peekIdent(0) === "or") {
      consume();
      const right = parseAnd();
      if (typeNameOf(left) !== "bool" || typeNameOf(right) !== "bool") {
        throw new DogRuntimeError({
          code: "E021",
          title: "Logical 'or' requires bool operands.",
          line: lineNum,
          why: `Got '${typeNameOf(left)}' and '${typeNameOf(right)}'.`,
          fix: "Ensure both sides of `or` evaluate to `true` or `false`.",
        });
      }
      left = left || right;
    }
    return left;
  }

  function parseAnd() {
    let left = parseEquality();
    while (peekIdent(0) === "and") {
      consume();
      const right = parseEquality();
      if (typeNameOf(left) !== "bool" || typeNameOf(right) !== "bool") {
        throw new DogRuntimeError({
          code: "E021",
          title: "Logical 'and' requires bool operands.",
          line: lineNum,
          why: `Got '${typeNameOf(left)}' and '${typeNameOf(right)}'.`,
          fix: "Ensure both sides of `and` evaluate to `true` or `false`.",
        });
      }
      left = left && right;
    }
    return left;
  }

  function parseEquality() {
    let left = parseComparison();
    while (true) {
      let op = null;
      let count = 0;
      const t = peek();
      if (t && t.kind === "op" && (t.value === "==" || t.value === "!=")) {
        op = t.value;
        count = 1;
      } else if (peekIdent(0) === "equals") {
        op = "==";
        count = 1;
      } else if (peekIdent(0) === "is") {
        if (peekIdent(1) === "not" && peekIdent(2) === "equal" && peekIdent(3) === "to") {
          op = "!=";
          count = 4;
        } else if (peekIdent(1) === "not") {
          op = "!=";
          count = 2;
        } else if (peekIdent(1) === "equal" && peekIdent(2) === "to") {
          op = "==";
          count = 3;
        } else {
          op = "==";
          count = 1;
        }
      }
      if (!op) break;
      for (let k = 0; k < count; k++) consume();
      const right = parseComparison();
      const eq = dogEquals(left, right, lineNum);
      left = op === "==" ? eq : !eq;
    }
    return left;
  }

  function parseComparison() {
    let left = parseTerm();
    while (true) {
      let op = null;
      let count = 0;
      const t = peek();
      if (t && t.kind === "op" && [">", "<", ">=", "<="].includes(t.value)) {
        op = t.value;
        count = 1;
      } else if (peekIdent(0) === "is") {
        if (peekIdent(1) === "greater" && peekIdent(2) === "than") {
          op = ">";
          count = 3;
        } else if (peekIdent(1) === "less" && peekIdent(2) === "than") {
          op = "<";
          count = 3;
        } else if (peekIdent(1) === "at" && peekIdent(2) === "least") {
          op = ">=";
          count = 3;
        } else if (peekIdent(1) === "at" && peekIdent(2) === "most") {
          op = "<=";
          count = 3;
        } else if (peekIdent(1) === "over") {
          op = ">";
          count = 2;
        } else if (peekIdent(1) === "under") {
          op = "<";
          count = 2;
        }
      }
      if (!op) break;
      for (let k = 0; k < count; k++) consume();
      const right = parseTerm();
      const na = unwrapNum(left);
      const nb = unwrapNum(right);
      if (!na || !nb || na.isDecimal !== nb.isDecimal) {
        throw new DogRuntimeError({
          code: "E021",
          title: "Type mismatch in comparison.",
          line: lineNum,
          why: `Ordering comparisons require matching 'number' or 'decimal' operands, got '${typeNameOf(left)}' and '${typeNameOf(right)}'.`,
          fix: "Convert text inputs explicitly with `number()` or `decimal()` before comparing.",
        });
      }
      if (op === ">") left = na.n > nb.n;
      else if (op === "<") left = na.n < nb.n;
      else if (op === ">=") left = na.n >= nb.n;
      else left = na.n <= nb.n;
    }
    return left;
  }

  function parseTerm() {
    let left = parseFactor();
    while (peek() && peek().kind === "op" && (peek().value === "+" || peek().value === "-")) {
      const op = consume().value;
      const right = parseFactor();
      if (op === "+" && typeof left === "string" && typeof right === "string") {
        left = left + right;
        continue;
      }
      const na = unwrapNum(left);
      const nb = unwrapNum(right);
      if (!na || !nb || na.isDecimal !== nb.isDecimal) {
        const lt = typeNameOf(left);
        const rt = typeNameOf(right);
        throw new DogRuntimeError({
          code: "E021",
          title: "Type mismatch detected.",
          line: lineNum,
          why: `Cannot apply '${op}' to '${lt}' and '${rt}' without explicit conversion.`,
          fix:
            op === "+"
              ? 'Use `$"text {value}"` interpolation or convert explicitly with `text()`, `number()`, or `decimal()`.'
              : "Ensure both operands are matching `number` or `decimal` values.",
          tip: "Strict typing prevents silent JavaScript-style coercion bugs.",
        });
      }
      const res = op === "+" ? na.n + nb.n : na.n - nb.n;
      left = makeTypedVal(na.isDecimal ? "decimal" : "number", res);
    }
    return left;
  }

  function parseFactor() {
    let left = parseUnary();
    while (peek() && peek().kind === "op" && ["*", "/", "%"].includes(peek().value)) {
      const op = consume().value;
      const right = parseUnary();
      const na = unwrapNum(left);
      const nb = unwrapNum(right);
      if (!na || !nb || na.isDecimal !== nb.isDecimal) {
        throw new DogRuntimeError({
          code: "E021",
          title: "Type mismatch detected.",
          line: lineNum,
          why: `Arithmetic '${op}' requires matching numeric types, got '${typeNameOf(left)}' and '${typeNameOf(right)}'.`,
          fix: "Keep operands matching (`number` with `number`, or `decimal` with `decimal`).",
        });
      }
      if ((op === "/" || op === "%") && nb.n === 0) {
        throw new DogRuntimeError({
          code: "E031",
          title: "Division by zero.",
          line: lineNum,
          why: "Cannot divide or take modulo by zero.",
          fix: "Check that the divisor is non-zero before dividing.",
        });
      }
      if (op === "*") {
        left = makeTypedVal(na.isDecimal ? "decimal" : "number", na.n * nb.n);
      } else if (op === "/") {
        const val = na.isDecimal ? na.n / nb.n : Math.trunc(na.n / nb.n);
        left = makeTypedVal(na.isDecimal ? "decimal" : "number", val);
      } else {
        left = makeTypedVal(na.isDecimal ? "decimal" : "number", na.n % nb.n);
      }
    }
    return left;
  }

  function parseUnary() {
    if (peek() && peek().kind === "op" && peek().value === "-") {
      consume();
      const v = parseUnary();
      const nv = unwrapNum(v);
      if (!nv) {
        throw new DogRuntimeError({
          code: "E021",
          title: "Unary '-' requires numeric value.",
          line: lineNum,
          why: `Got '${typeNameOf(v)}'.`,
          fix: "Apply '-' only to a `number` or `decimal`.",
        });
      }
      return makeTypedVal(nv.isDecimal ? "decimal" : "number", -nv.n);
    }
    if (peekIdent(0) === "not") {
      consume();
      const v = parseUnary();
      if (typeNameOf(v) !== "bool") {
        throw new DogRuntimeError({
          code: "E021",
          title: "Unary 'not' requires a bool expression.",
          line: lineNum,
          why: `Got '${typeNameOf(v)}'.`,
          fix: "Pass a boolean expression to `not`.",
        });
      }
      return !v;
    }
    return parsePostfix();
  }

  function parsePostfix() {
    let val = parsePrimary();
    while (peek()) {
      if (peek().kind === "op" && peek().value === "(" && val && typeof val === "object" && val.__dogLambda) {
        const args = parseCallArgs();
        val = invokeLambdaObject(val, args, ctx, lineNum);
        continue;
      }
      if (peek().kind === "op" && peek().value === "[") {
        consume();
        let startIdx = null;
        let isSlice = false;
        let endIdx = null;
        if (peek() && peek().kind === "op" && peek().value === "..") {
          consume();
          isSlice = true;
          if (!(peek() && peek().kind === "op" && peek().value === "]")) {
            endIdx = parseOr();
          }
        } else {
          startIdx = parseOr();
          if (peek() && peek().kind === "op" && peek().value === "..") {
            consume();
            isSlice = true;
            if (!(peek() && peek().kind === "op" && peek().value === "]")) {
              endIdx = parseOr();
            }
          }
        }
        if (!peek() || peek().kind !== "op" || peek().value !== "]") {
          throw new DogRuntimeError({
            code: "E004",
            title: "Expected closing ']' in index or slice expression.",
            line: lineNum,
          });
        }
        consume();
        if (isSlice) {
          if (!Array.isArray(val) && typeof val !== "string") {
            throw new DogRuntimeError({ code: "E021", title: "Slicing requires a list or text value.", line: lineNum });
          }
          const sNum = startIdx !== null ? (unwrapNum(startIdx)?.n ?? 0) : 0;
          const eNum = endIdx !== null ? (unwrapNum(endIdx)?.n ?? val.length) : val.length;
          val = val.slice(Math.max(0, Math.trunc(sNum)), Math.max(0, Math.trunc(eNum)));
        } else if (typeof val === "object" && !Array.isArray(val) && val !== null) {
          const k = String(startIdx);
          if (!Object.prototype.hasOwnProperty.call(val, k)) {
            throw new DogRuntimeError({ code: "E034", title: `Key '${k}' not found in map.`, line: lineNum });
          }
          val = val[k];
        } else {
          if (!Array.isArray(val) && typeof val !== "string") {
            throw new DogRuntimeError({
              code: "E021",
              title: "Indexing requires a list, text, or map value.",
              line: lineNum,
              why: `Cannot index value of type '${typeNameOf(val)}'.`,
            });
          }
          const n = unwrapNum(startIdx);
          if (!n || n.isDecimal || n.n < 0 || n.n >= val.length) {
            throw new DogRuntimeError({
              code: "E032",
              title: "Index out of bounds.",
              line: lineNum,
              why: `Index ${formatDogValue(startIdx)} is invalid for length ${val.length}.`,
            });
          }
          val = val[n.n];
        }
      } else if (peek().kind === "op" && peek().value === ".") {
        consume();
        const fieldTok = consume();
        if (!fieldTok || fieldTok.kind !== "ident") {
          throw new DogRuntimeError({ code: "E004", title: "Expected field name after '.'.", line: lineNum });
        }
        if (val && typeof val === "object" && val.__module) {
          const nsName = `${val.__module}.${fieldTok.value}`;
          if (peek() && peek().kind === "op" && peek().value === "(") {
            const args = parseCallArgs();
            val = callNamespaceOrBuiltin(nsName, args, ctx, lineNum);
          } else {
            val = { __module: nsName };
          }
        } else if (val && typeof val === "object" && val.__enumDef) {
          const enumDef = val.__enumDef;
          const variant = enumDef.variants.get(fieldTok.value);
          if (!variant) {
            throw new DogRuntimeError({
              code: "E010",
              title: `Unknown enum variant '${enumDef.name}.${fieldTok.value}'.`,
              line: lineNum,
            });
          }
          let payload = [];
          if (peek() && peek().kind === "op" && peek().value === "(") {
            payload = parseCallArgs();
          }
          if (variant.arity !== payload.length) {
            throw new DogRuntimeError({
              code: "E014",
              title: `Enum variant '${enumDef.name}.${fieldTok.value}' expects ${variant.arity} value(s), got ${payload.length}.`,
              line: lineNum,
            });
          }
          val = { __dogType: enumDef.name, __enum: enumDef.name, __variant: fieldTok.value, __payload: payload };
        } else if (val && typeof val === "object" && Object.prototype.hasOwnProperty.call(val, fieldTok.value)) {
          val = val[fieldTok.value];
        } else {
          throw new DogRuntimeError({
            code: "E010",
            title: `Unknown field '${fieldTok.value}'.`,
            line: lineNum,
          });
        }
      } else {
        break;
      }
    }
    return val;
  }

  function parseCallArgs() {
    consume(); // (
    const args = [];
    if (!(peek() && peek().kind === "op" && peek().value === ")")) {
      while (true) {
        args.push(parseConditional());
        if (peek() && peek().kind === "op" && peek().value === ",") {
          consume();
        } else {
          break;
        }
      }
    }
    if (!peek() || peek().kind !== "op" || peek().value !== ")") {
      throw new DogRuntimeError({
        code: "E004",
        title: "Expected ')' after function arguments.",
        line: lineNum,
      });
    }
    consume(); // )
    return args;
  }

  function parsePrimary() {
    const tok = consume();
    if (!tok) {
      throw new DogRuntimeError({
        code: "E004",
        title: "Unexpected end of expression.",
        line: lineNum,
      });
    }
    if (tok.kind === "string" || tok.kind === "number" || tok.kind === "bool" || tok.kind === "null") {
      return tok.value;
    }
    if (tok.kind === "decimal") {
      return makeTypedVal("decimal", tok.value);
    }
    if (tok.kind === "interp") {
      return tok.value
        .map((p) => (p.type === "lit" ? p.value : formatDogValue(evalExprString(p.value, ctx, lineNum))))
        .join("");
    }
    if (tok.kind === "op" && tok.value === "(") {
      const inner = parseConditional();
      if (!peek() || peek().kind !== "op" || peek().value !== ")") {
        throw new DogRuntimeError({ code: "E004", title: "Expected closing ')'.", line: lineNum });
      }
      consume();
      return inner;
    }
    if (tok.kind === "op" && tok.value === "[") {
      const items = [];
      if (!(peek() && peek().kind === "op" && peek().value === "]")) {
        while (true) {
          items.push(parseConditional());
          if (peek() && peek().kind === "op" && peek().value === ",") consume();
          else break;
        }
      }
      if (!peek() || peek().kind !== "op" || peek().value !== "]") {
        throw new DogRuntimeError({ code: "E004", title: "Expected closing ']' in list.", line: lineNum });
      }
      consume();
      return items;
    }
    if (tok.kind === "op" && tok.value === "{") {
      const obj = {};
      if (!(peek() && peek().kind === "op" && peek().value === "}")) {
        while (true) {
          const k = String(parseConditional());
          if (!peek() || peek().kind !== "op" || peek().value !== ":") {
            throw new DogRuntimeError({ code: "E004", title: "Expected ':' in map literal.", line: lineNum });
          }
          consume();
          obj[k] = parseConditional();
          if (peek() && peek().kind === "op" && peek().value === ",") consume();
          else break;
        }
      }
      if (!peek() || peek().kind !== "op" || peek().value !== "}") {
        throw new DogRuntimeError({ code: "E004", title: "Expected closing '}' in map.", line: lineNum });
      }
      consume();
      return obj;
    }
    if (tok.kind === "ident") {
      // Inline lambda: fn(x, y) => expr | do(x, y) => expr
      if ((tok.value === "fn" || tok.value === "do") && peek() && peek().kind === "op" && peek().value === "(") {
        consume(); // (
        const rawParamChunks = [];
        let pBuf = [];
        while (peek() && !(peek().kind === "op" && peek().value === ")")) {
          const pt = consume();
          if (pt.kind === "op" && pt.value === ",") {
            if (pBuf.length) rawParamChunks.push(pBuf);
            pBuf = [];
          } else {
            pBuf.push(pt);
          }
        }
        if (pBuf.length) rawParamChunks.push(pBuf);
        if (!peek() || peek().kind !== "op" || peek().value !== ")") {
          throw new DogRuntimeError({ code: "E004", title: "Expected ')' in lambda parameter list.", line: lineNum });
        }
        consume(); // )
        while (peek() && !(peek().kind === "op" && peek().value === "=>")) consume();
        if (!peek() || peek().kind !== "op" || peek().value !== "=>") {
          throw new DogRuntimeError({ code: "E004", title: "Expected '=>' in inline lambda expression.", line: lineNum, fix: "Write `fn(x) => x * 2` or `do(x) => x * 2`." });
        }
        const arrowTok = consume(); // =>
        // Scan expression tokens up to enclosing ',', ')', ']', '}'
        let depth = 0;
        let lastTok = null;
        const bodyStartIdx = exprSrc.indexOf("=>", arrowTok.start ?? 0) + 2;
        let bodyEndIdx = exprSrc.length;
        while (peek()) {
          const nt = peek();
          if (depth === 0 && nt.kind === "op" && [",", ")", "]", "}"].includes(nt.value)) {
            bodyEndIdx = nt.start ?? exprSrc.length;
            break;
          }
          if (nt.kind === "op" && ["(", "[", "{"].includes(nt.value)) depth++;
          else if (nt.kind === "op" && [")", "]", "}"].includes(nt.value)) depth = Math.max(0, depth - 1);
          lastTok = consume();
        }
        if (!lastTok) {
          throw new DogRuntimeError({ code: "E004", title: "Empty lambda body after '=>'.", line: lineNum });
        }
        const bodyExpr = exprSrc.slice(bodyStartIdx, bodyEndIdx).trim();
        const params = rawParamChunks.map((chunk) => ({ name: chunk[0].value, defaultExpr: null }));
        return {
          __dogLambda: true,
          params,
          bodyExpr,
          bodyStmts: null,
          capturedScopes: [...ctx.scopes],
          line: lineNum,
        };
      }
      if (tok.value === "std" && peek() && peek().kind === "op" && peek().value === ".") {
        return { __module: "std" };
      }
      if (ctx.enums.has(tok.value) && peek() && peek().kind === "op" && peek().value === ".") {
        return { __enumDef: ctx.enums.get(tok.value) };
      }
      // Struct literal: StructName { field: value, ... }
      if (ctx.structs.has(tok.value) && peek() && peek().kind === "op" && peek().value === "{") {
        const sDef = ctx.structs.get(tok.value);
        consume(); // {
        const instance = { __dogType: sDef.name, __structName: sDef.name };
        if (!(peek() && peek().kind === "op" && peek().value === "}")) {
          while (true) {
            const kTok = consume();
            if (!kTok || kTok.kind !== "ident") {
              throw new DogRuntimeError({ code: "E004", title: `Expected field name inside '${sDef.name} { ... }'.`, line: lineNum });
            }
            if (!sDef.fields.has(kTok.value)) {
              throw new DogRuntimeError({ code: "E010", title: `Unknown struct field '${kTok.value}' on '${sDef.name}'.`, line: lineNum });
            }
            if (!peek() || peek().kind !== "op" || peek().value !== ":") {
              throw new DogRuntimeError({ code: "E004", title: "Expected ':' after struct field name.", line: lineNum });
            }
            consume(); // :
            instance[kTok.value] = parseConditional();
            if (peek() && peek().kind === "op" && peek().value === ",") consume();
            else break;
          }
        }
        if (!peek() || peek().kind !== "op" || peek().value !== "}") {
          throw new DogRuntimeError({ code: "E004", title: `Expected '}' closing '${sDef.name}' constructor.`, line: lineNum });
        }
        consume(); // }
        for (const [fName, fMeta] of sDef.fields.entries()) {
          if (!Object.prototype.hasOwnProperty.call(instance, fName)) {
            if (fMeta.defaultExpr !== null) instance[fName] = evalExprString(fMeta.defaultExpr, ctx, lineNum);
            else {
              throw new DogRuntimeError({
                code: "E014",
                title: `Missing required field '${fName}' for struct '${sDef.name}'.`,
                line: lineNum,
              });
            }
          }
        }
        return instance;
      }
      if (peek() && peek().kind === "op" && peek().value === "(") {
        const args = parseCallArgs();
        const bound = ctx.lookup(tok.value);
        if (bound && bound.value && bound.value.__dogLambda) {
          return invokeLambdaObject(bound.value, args, ctx, lineNum);
        }
        return callNamespaceOrBuiltin(tok.value, args, ctx, lineNum);
      }
      const binding = ctx.lookup(tok.value);
      if (binding) return binding.value;
      if (ctx.functions.has(tok.value)) {
        const fnDef = ctx.functions.get(tok.value);
        return {
          __dogLambda: true,
          params: fnDef.params,
          bodyExpr: null,
          bodyStmts: fnDef.body,
          capturedScopes: [ctx.scopes[0]],
          line: fnDef.line,
        };
      }
      throw new DogRuntimeError({
        code: "E010",
        title: `Undefined variable '${tok.value}'.`,
        line: lineNum,
        why: `No variable named '${tok.value}' exists in the current scope.`,
        fix: `Declare it first with \`let ${tok.value} = ...\` or \`var ${tok.value} = ...\`.`,
      });
    }
    throw new DogRuntimeError({
      code: "E004",
      title: "Unexpected token in expression.",
      line: lineNum,
    });
  }

  function hasThenAtCurrentDepth(startPos) {
    let d = 0;
    for (let k = startPos; k < tokens.length; k++) {
      const t = tokens[k];
      if (t.kind === "op" && ["(", "[", "{"].includes(t.value)) d++;
      else if (t.kind === "op" && [")", "]", "}"].includes(t.value)) {
        if (d === 0) return false;
        d--;
      } else if (d === 0 && t.kind === "ident" && t.value === "then") {
        return true;
      }
    }
    return false;
  }

  function parseConditional() {
    if (peekIdent(0) === "if" || (peekIdent(0) === "when" && hasThenAtCurrentDepth(pos + 1))) {
      consume();
      const cond = parsePipe();
      if (typeNameOf(cond) !== "bool") {
        throw new DogRuntimeError({ code: "E022", title: "Inline conditional must evaluate to bool.", line: lineNum });
      }
      if (peekIdent(0) !== "then") {
        throw new DogRuntimeError({ code: "E004", title: "Expected 'then' in inline 'if/when ... then ... else ...'.", line: lineNum });
      }
      consume();
      const yesVal = parseConditional();
      if (peekIdent(0) !== "else" && peekIdent(0) !== "otherwise") {
        throw new DogRuntimeError({ code: "E004", title: "Expected 'else' in inline conditional expression.", line: lineNum });
      }
      consume();
      const noVal = parseConditional();
      return cond ? yesVal : noVal;
    }
    return parsePipe();
  }

  const result = parseConditional();
  if (pos < tokens.length) {
    throw new DogRuntimeError({
      code: "E004",
      title: `Unexpected trailing token '${tokens[pos].value}' in expression.`,
      line: lineNum,
    });
  }
  return result;
}

function invokeLambdaObject(fnObj, args, ctx, lineNum) {
  if (args.length > fnObj.params.length) {
    throw new DogRuntimeError({ code: "E014", title: "Too many arguments passed to lambda.", line: lineNum });
  }
  const prevScopes = ctx.scopes;
  const paramScope = new Map();
  ctx.scopes = [...(fnObj.capturedScopes || prevScopes), paramScope];
  try {
    for (let idx = 0; idx < fnObj.params.length; idx++) {
      const p = fnObj.params[idx];
      let argVal;
      if (idx < args.length) argVal = args[idx];
      else if (p.defaultExpr !== null) argVal = evalExprString(p.defaultExpr, ctx, lineNum);
      else throw new DogRuntimeError({ code: "E014", title: `Missing argument '${p.name}' for lambda.`, line: lineNum });
      paramScope.set(p.name, { kind: "let", value: argVal });
    }
    if (fnObj.bodyExpr !== null && fnObj.bodyExpr !== undefined) {
      return evalExprString(fnObj.bodyExpr, ctx, fnObj.line || lineNum);
    }
    const flow = execBlock(fnObj.bodyStmts || [], ctx);
    return flow.type === "return" ? flow.value : undefined;
  } finally {
    ctx.scopes = prevScopes;
  }
}

function invokeCallable(target, args, ctx, lineNum) {
  if (target && typeof target === "object" && target.__dogLambda) {
    return invokeLambdaObject(target, args, ctx, lineNum);
  }
  return callNamespaceOrBuiltin(String(target || ""), args, ctx, lineNum);
}

function callNamespaceOrBuiltin(rawName, args, ctx, lineNum) {
  const name = rawName.startsWith("std.") ? rawName.split(".").pop() : rawName;
  if (ctx.functions.has(name)) {
    const fn = ctx.functions.get(name);
    if (args.length > fn.params.length) {
      throw new DogRuntimeError({
        code: "E014",
        title: `Too many arguments for '${name}()'.`,
        line: lineNum,
      });
    }
    ctx.callStack.push({ name, line: lineNum });
    ctx.pushScope();
    try {
      for (let idx = 0; idx < fn.params.length; idx++) {
        const p = fn.params[idx];
        let argVal;
        if (idx < args.length) {
          argVal = args[idx];
        } else if (p.defaultExpr !== null) {
          argVal = evalExprString(p.defaultExpr, ctx, lineNum);
        } else {
          throw new DogRuntimeError({
            code: "E014",
            title: `Missing argument '${p.name}' for '${name}()'.`,
            line: lineNum,
          });
        }
        ctx.define(p.name, "let", argVal);
      }
      const flow = execBlock(fn.body, ctx);
      if (flow.type === "return") return flow.value;
      return undefined;
    } catch (err) {
      if (err instanceof DogRuntimeError && err.stackFrames.length === 0) {
        err.stackFrames = [...ctx.callStack].reverse();
      }
      throw err;
    } finally {
      ctx.popScope();
      ctx.callStack.pop();
    }
  }

  switch (name) {
    case "show":
    case "say":
    case "print":
      ctx.output.push(formatDogValue(args[0]));
      return undefined;
    case "check":
      if (!args[0]) {
        throw new DogRuntimeError({
          code: "E033",
          title: args[1] ? `Check failed: ${args[1]}` : "Check failed.",
          line: lineNum,
        });
      }
      return true;
    case "cli_info":
      ctx.output.push(`[INFO] ${formatDogValue(args[0])}`);
      return undefined;
    case "cli_success":
      ctx.output.push(`[OK] ${formatDogValue(args[0])}`);
      return undefined;
    case "cli_warn":
      ctx.output.push(`[WARN] ${formatDogValue(args[0])}`);
      return undefined;
    case "cli_error":
      ctx.output.push(`[ERR] ${formatDogValue(args[0])}`);
      return undefined;
    case "progress":
    case "cli_progress": {
      const label = String(args[0]);
      const cur = Number(args[1] || 0);
      const tot = Math.max(1, Number(args[2] || 1));
      const pct = Math.min(100, Math.round((cur / tot) * 100));
      const filled = Math.round((pct / 100) * 16);
      ctx.output.push(`${label} [${"#".repeat(filled)}${".".repeat(16 - filled)}] ${pct}% (${cur}/${tot})`);
      return undefined;
    }
    case "text":
      return formatDogValue(args[0]);
    case "number": {
      const n = Number(unwrapNum(args[0])?.n ?? args[0]);
      if (!Number.isFinite(n) || (typeof args[0] === "string" && args[0].trim() === "")) {
        throw new DogRuntimeError({
          code: "E030",
          title: `Cannot parse '${args[0]}' as number.`,
          line: lineNum,
          why: "Strict conversion requires a valid integer string or number.",
          fix: "Wrap untrusted user text conversions in `try ... catch err ... end`.",
        });
      }
      return Math.trunc(n);
    }
    case "decimal": {
      const n = Number(unwrapNum(args[0])?.n ?? args[0]);
      if (!Number.isFinite(n)) {
        throw new DogRuntimeError({ code: "E030", title: `Cannot parse '${args[0]}' as decimal.`, line: lineNum });
      }
      return makeTypedVal("decimal", n);
    }
    case "len":
      if (typeof args[0] === "string" || Array.isArray(args[0])) return args[0].length;
      if (args[0] && typeof args[0] === "object") return Object.keys(args[0]).length;
      throw new DogRuntimeError({ code: "E021", title: "len() expects text, list, or map.", line: lineNum });
    case "upper":
      return String(args[0]).toUpperCase();
    case "lower":
      return String(args[0]).toLowerCase();
    case "trim":
      return String(args[0]).trim();
    case "title_case":
      return String(args[0]).replace(/\b\w/g, (c) => c.toUpperCase());
    case "starts_with":
      return String(args[0] || "").startsWith(String(args[1] || ""));
    case "ends_with":
      return String(args[0] || "").endsWith(String(args[1] || ""));
    case "pad_start":
      return String(args[0] || "").padStart(Math.max(0, Math.trunc(unwrapNum(args[1])?.n ?? 0)), String(args[2] ?? " "));
    case "pad_end":
      return String(args[0] || "").padEnd(Math.max(0, Math.trunc(unwrapNum(args[1])?.n ?? 0)), String(args[2] ?? " "));
    case "repeat_text":
      return String(args[0] || "").repeat(Math.max(0, Math.min(1000, Math.trunc(unwrapNum(args[1])?.n ?? 0))));
    case "glob_match": {
      const esc = String(args[0] || "").replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".");
      return new RegExp(`^${esc}$`).test(String(args[1] || ""));
    }
    case "regex_match": {
      try {
        return new RegExp(String(args[0] || "")).test(String(args[1] || ""));
      } catch (e) {
        throw new DogRuntimeError({ code: "E030", title: `Invalid regex pattern: ${e.message}`, line: lineNum });
      }
    }
    case "regex_find_all": {
      try {
        const re = new RegExp(String(args[0] || ""), "g");
        return [...String(args[1] || "").matchAll(re)].map((m) => m[0]);
      } catch (e) {
        throw new DogRuntimeError({ code: "E030", title: `Invalid regex pattern: ${e.message}`, line: lineNum });
      }
    }
    case "regex_replace": {
      try {
        const re = new RegExp(String(args[0] || ""), "g");
        return String(args[1] || "").replace(re, String(args[2] ?? ""));
      } catch (e) {
        throw new DogRuntimeError({ code: "E030", title: `Invalid regex pattern: ${e.message}`, line: lineNum });
      }
    }
    case "contains":
      return String(args[0]).includes(String(args[1]));
    case "replace":
      return String(args[0]).split(String(args[1])).join(String(args[2]));
    case "split":
      return String(args[0]).split(String(args[1]));
    case "join":
      return (args[0] || []).map(formatDogValue).join(String(args[1]));
    case "range": {
      const start = Math.trunc(unwrapNum(args[0])?.n ?? 0);
      const end = Math.trunc(unwrapNum(args[1])?.n ?? 0);
      const out = [];
      for (let k = start; k < end; k++) {
        out.push(k);
        if (out.length > 5000) break;
      }
      return out;
    }
    case "list_chunk": {
      const list = Array.isArray(args[0]) ? args[0] : [];
      const size = Math.max(1, Math.trunc(unwrapNum(args[1])?.n ?? 1));
      const out = [];
      for (let k = 0; k < list.length; k += size) out.push(list.slice(k, k + size));
      return out;
    }
    case "list_flatten": {
      const list = Array.isArray(args[0]) ? args[0] : [];
      const out = [];
      for (const item of list) {
        if (Array.isArray(item)) out.push(...item);
        else out.push(item);
      }
      return out;
    }
    case "list_zip": {
      const a = Array.isArray(args[0]) ? args[0] : [];
      const b = Array.isArray(args[1]) ? args[1] : [];
      const n = Math.min(a.length, b.length);
      const out = [];
      for (let k = 0; k < n; k++) out.push([a[k], b[k]]);
      return out;
    }
    case "list_sum":
      return (args[0] || []).reduce((acc, v) => acc + (unwrapNum(v)?.n ?? 0), 0);
    case "list_avg": {
      const arr = args[0] || [];
      if (!arr.length) return makeTypedVal("decimal", 0);
      const s = arr.reduce((acc, v) => acc + (unwrapNum(v)?.n ?? 0), 0);
      return makeTypedVal("decimal", s / arr.length);
    }
    case "map_get":
      return args[0]?.[String(args[1])] ?? "";
    case "map_opt":
      return args[0] && Object.prototype.hasOwnProperty.call(args[0], String(args[1])) ? args[0][String(args[1])] : null;
    case "map_keys":
      return Object.keys(args[0] || {});
    case "abs": {
      const nv = unwrapNum(args[0]);
      return makeTypedVal(nv.isDecimal ? "decimal" : "number", Math.abs(nv.n));
    }
    case "sign": {
      const nv = unwrapNum(args[0]);
      return Math.sign(nv.n);
    }
    case "round": {
      const nv = unwrapNum(args[0]);
      return Math.round(nv.n);
    }
    case "floor": {
      const nv = unwrapNum(args[0]);
      return Math.floor(nv.n);
    }
    case "ceil": {
      const nv = unwrapNum(args[0]);
      return Math.ceil(nv.n);
    }
    case "clamp": {
      const x = unwrapNum(args[0]);
      const lo = unwrapNum(args[1]);
      const hi = unwrapNum(args[2]);
      const clamped = Math.min(Math.max(x.n, lo.n), hi.n);
      return makeTypedVal(x.isDecimal ? "decimal" : "number", clamped);
    }
    case "min": {
      const a = unwrapNum(args[0]);
      const b = unwrapNum(args[1]);
      return makeTypedVal(a.isDecimal ? "decimal" : "number", Math.min(a.n, b.n));
    }
    case "max": {
      const a = unwrapNum(args[0]);
      const b = unwrapNum(args[1]);
      return makeTypedVal(a.isDecimal ? "decimal" : "number", Math.max(a.n, b.n));
    }
    case "sqrt": {
      const nv = unwrapNum(args[0]);
      return makeTypedVal("decimal", Math.sqrt(nv.n));
    }
    case "pow": {
      const a = unwrapNum(args[0]);
      const b = unwrapNum(args[1]);
      return makeTypedVal("decimal", Math.pow(a.n, b.n));
    }
    case "sin":
      return makeTypedVal("decimal", Math.sin(unwrapNum(args[0]).n));
    case "cos":
      return makeTypedVal("decimal", Math.cos(unwrapNum(args[0]).n));
    case "tan":
      return makeTypedVal("decimal", Math.tan(unwrapNum(args[0]).n));
    case "log":
      return makeTypedVal("decimal", Math.log(unwrapNum(args[0]).n));
    case "log10":
      return makeTypedVal("decimal", Math.log10(unwrapNum(args[0]).n));
    case "exp":
      return makeTypedVal("decimal", Math.exp(unwrapNum(args[0]).n));
    case "gcd": {
      let a = Math.abs(Math.trunc(unwrapNum(args[0])?.n ?? 0));
      let b = Math.abs(Math.trunc(unwrapNum(args[1])?.n ?? 0));
      while (b !== 0) { const t = b; b = a % b; a = t; }
      return a;
    }
    case "lcm": {
      const a = Math.abs(Math.trunc(unwrapNum(args[0])?.n ?? 0));
      const b = Math.abs(Math.trunc(unwrapNum(args[1])?.n ?? 0));
      if (a === 0 || b === 0) return 0;
      let x = a, y = b;
      while (y !== 0) { const t = y; y = x % y; x = t; }
      return Math.trunc((a * b) / x);
    }
    case "rand_int": {
      const lo = Math.trunc(unwrapNum(args[0])?.n ?? 0);
      const hi = Math.trunc(unwrapNum(args[1])?.n ?? 1);
      const minV = Math.min(lo, hi);
      const maxV = Math.max(lo, hi);
      const r = randomBytesArr(4);
      const u32 = ((r[0] << 24) | (r[1] << 16) | (r[2] << 8) | r[3]) >>> 0;
      return minV + (u32 % (maxV - minV + 1));
    }
    case "map": {
      const list = Array.isArray(args[0]) ? args[0] : [];
      return list.map((item) => invokeCallable(args[1], [item], ctx, lineNum));
    }
    case "filter": {
      const list = Array.isArray(args[0]) ? args[0] : [];
      return list.filter((item) => Boolean(invokeCallable(args[1], [item], ctx, lineNum)));
    }
    case "reduce": {
      const list = Array.isArray(args[0]) ? args[0] : [];
      let acc = args[1];
      for (const item of list) acc = invokeCallable(args[2], [acc, item], ctx, lineNum);
      return acc;
    }
    case "list_find": {
      const list = Array.isArray(args[0]) ? args[0] : [];
      for (const item of list) {
        if (Boolean(invokeCallable(args[1], [item], ctx, lineNum))) return item;
      }
      return undefined;
    }
    case "list_any": {
      const list = Array.isArray(args[0]) ? args[0] : [];
      return list.some((item) => Boolean(invokeCallable(args[1], [item], ctx, lineNum)));
    }
    case "list_all": {
      const list = Array.isArray(args[0]) ? args[0] : [];
      return list.every((item) => Boolean(invokeCallable(args[1], [item], ctx, lineNum)));
    }
    case "tuple":
      return [...args];
    case "push":
      if (Array.isArray(args[0])) args[0].push(args[1]);
      return undefined;
    case "pop":
      if (Array.isArray(args[0]) && args[0].length > 0) return args[0].pop();
      throw new DogRuntimeError({ code: "E032", title: "Cannot pop from empty list.", line: lineNum });
    case "sort":
      if (Array.isArray(args[0])) args[0].sort((a, b) => (formatDogValue(a) < formatDogValue(b) ? -1 : formatDogValue(a) > formatDogValue(b) ? 1 : 0));
      return undefined;
    case "reverse":
      if (Array.isArray(args[0])) args[0].reverse();
      return undefined;
    case "list_unique": {
      const out = [];
      const seen = new Set();
      for (const item of args[0] || []) {
        const k = JSON.stringify(item);
        if (!seen.has(k)) { seen.add(k); out.push(item); }
      }
      return out;
    }
    case "map_has":
      return Boolean(args[0] && Object.prototype.hasOwnProperty.call(args[0], String(args[1])));
    case "map_get_or":
      return args[0] && Object.prototype.hasOwnProperty.call(args[0], String(args[1])) ? args[0][String(args[1])] : args[2];
    case "map_set":
      if (args[0] && typeof args[0] === "object") args[0][String(args[1])] = args[2];
      return undefined;
    case "map_values":
      return Object.values(args[0] || {});
    case "query_where": {
      const rows = Array.isArray(args[0]) ? args[0] : [];
      const field = String(args[1] || "");
      const target = args[2];
      return rows.filter((r) => r && typeof r === "object" && formatDogValue(r[field]) === formatDogValue(target));
    }
    case "query_builder": {
      let rows = [];
      let opts = {};
      if (args[0]?.__db) {
        const tbl = args[0].__db.tables.get(String(args[1] || ""));
        rows = tbl ? tbl.rows : [];
        opts = args[2] && typeof args[2] === "object" ? args[2] : {};
      } else {
        rows = Array.isArray(args[0]) ? args[0] : [];
        opts = args[1] && typeof args[1] === "object" ? args[1] : {};
      }
      let res = [...rows];
      if (opts.where && typeof opts.where === "object") {
        res = res.filter((r) =>
          Object.entries(opts.where).every(([k, v]) => r && formatDogValue(r[k]) === formatDogValue(v))
        );
      }
      if (opts.order_by) {
        const col = String(opts.order_by);
        const desc = String(opts.order || "asc").toLowerCase() === "desc";
        res.sort((a, b) => {
          const av = unwrapNum(a[col])?.n ?? String(a[col] ?? "");
          const bv = unwrapNum(b[col])?.n ?? String(b[col] ?? "");
          return av < bv ? (desc ? 1 : -1) : av > bv ? (desc ? -1 : 1) : 0;
        });
      }
      if (opts.limit !== undefined) res = res.slice(0, Math.max(0, Math.trunc(unwrapNum(opts.limit)?.n ?? 50)));
      if (Array.isArray(opts.select)) {
        res = res.map((r) => {
          const obj = {};
          opts.select.forEach((k) => { obj[k] = r[k]; });
          return obj;
        });
      }
      return res;
    }
    case "db_open":
      return createSqlDb(String(args[0] || ":memory:"));
    case "db_exec": {
      const res = runSqlStatement(args[0], args[1], Array.isArray(args[2]) ? args[2] : [], lineNum);
      return res.changes;
    }
    case "db_query": {
      const res = runSqlStatement(args[0], args[1], Array.isArray(args[2]) ? args[2] : [], lineNum);
      return res.rows;
    }
    case "db_insert": {
      const db = args[0]?.__db;
      const tName = String(args[1] || "");
      const rowMap = args[2] && typeof args[2] === "object" ? { ...args[2] } : {};
      if (!db || !db.tables.has(tName)) {
        throw new DogRuntimeError({ code: "E030", title: `Database table '${tName}' not found.`, line: lineNum });
      }
      const tbl = db.tables.get(tName);
      if (tbl.columns.includes("id") && rowMap.id === undefined) rowMap.id = tbl.nextId++;
      tbl.rows.push(rowMap);
      return rowMap;
    }
    case "db_begin": {
      const db = args[0]?.__db;
      if (!db) throw new DogRuntimeError({ code: "E030", title: "Expected valid db handle.", line: lineNum });
      const snap = new Map();
      for (const [k, v] of db.tables.entries()) {
        snap.set(k, { columns: [...v.columns], rows: v.rows.map((r) => ({ ...r })), nextId: v.nextId });
      }
      db.txStack.push(snap);
      return true;
    }
    case "db_commit": {
      const db = args[0]?.__db;
      if (!db || db.txStack.length === 0) return false;
      db.txStack.pop();
      return true;
    }
    case "db_rollback": {
      const db = args[0]?.__db;
      if (!db || db.txStack.length === 0) return false;
      db.tables = db.txStack.pop();
      return true;
    }
    case "db_tables":
      return args[0]?.__db ? [...args[0].__db.tables.keys()] : [];
    case "db_schema": {
      const tbl = args[0]?.__db?.tables?.get(String(args[1] || ""));
      return tbl ? [...tbl.columns] : [];
    }
    case "db_backup": {
      const db = args[0]?.__db;
      if (!db) return "{}";
      const dump = {};
      for (const [k, v] of db.tables.entries()) dump[k] = v.rows;
      return JSON.stringify(dump);
    }
    case "json_parse":
      try {
        return JSON.parse(String(args[0] || ""));
      } catch (e) {
        throw new DogRuntimeError({ code: "E034", title: `JSON parse failed: ${e.message}`, line: lineNum });
      }
    case "json_stringify":
      return JSON.stringify(args[0]);
    case "json_pretty":
      return JSON.stringify(args[0], null, 2);
    case "json_validate": {
      try {
        const parsed = typeof args[0] === "string" ? JSON.parse(args[0]) : args[0];
        if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return false;
        const reqKeys = Array.isArray(args[1]) ? args[1] : [];
        return reqKeys.every((k) => Object.prototype.hasOwnProperty.call(parsed, String(k)));
      } catch {
        return false;
      }
    }
    case "toml_parse": {
      const out = {};
      let sec = "";
      for (const rawLn of String(args[0] || "").split(/\r?\n/)) {
        const ln = rawLn.trim();
        if (!ln || ln.startsWith("#")) continue;
        if (ln.startsWith("[") && ln.endsWith("]")) { sec = ln.slice(1, -1).trim(); continue; }
        const eq = ln.indexOf("=");
        if (eq > 0) {
          const k = ln.slice(0, eq).trim();
          const v = ln.slice(eq + 1).trim().replace(/^"(.*)"$/, "$1");
          out[sec ? `${sec}.${k}` : k] = v;
        }
      }
      return out;
    }
    case "toml_stringify": {
      const obj = args[0] && typeof args[0] === "object" ? args[0] : {};
      const lines = [];
      for (const [k, v] of Object.entries(obj)) {
        const rendered = typeof v === "string" ? JSON.stringify(v) : formatDogValue(v);
        lines.push(`${k} = ${rendered}`);
      }
      return lines.join("\n");
    }
    case "schema_validate": {
      let data = args[0];
      if (typeof data === "string") {
        try { data = JSON.parse(data); } catch { return { valid: false, errors: ["Invalid JSON payload"] }; }
      }
      if (!data || typeof data !== "object" || Array.isArray(data)) {
        return { valid: false, errors: ["Expected object payload"] };
      }
      const schema = args[1] && typeof args[1] === "object" ? args[1] : {};
      const errors = [];
      for (const [k, expectedType] of Object.entries(schema)) {
        const exp = String(expectedType || "any").trim();
        const optional = exp.endsWith("?");
        const baseType = optional ? exp.slice(0, -1) : exp;
        if (!Object.prototype.hasOwnProperty.call(data, k) || data[k] === null || data[k] === undefined) {
          if (!optional) errors.push(`Missing required key '${k}'`);
          continue;
        }
        if (baseType !== "any" && typeNameOf(data[k]) !== baseType) {
          errors.push(`Field '${k}' expected ${baseType}, got ${typeNameOf(data[k])}`);
        }
      }
      return { valid: errors.length === 0, errors };
    }
    case "semver_compare": {
      const parseSemver = (s) => String(s || "0.0.0").replace(/^v/i, "").split("-")[0].split(".").map((n) => parseInt(n, 10) || 0);
      const a = parseSemver(args[0]);
      const b = parseSemver(args[1]);
      for (let k = 0; k < 3; k++) {
        if ((a[k] || 0) < (b[k] || 0)) return -1;
        if ((a[k] || 0) > (b[k] || 0)) return 1;
      }
      return 0;
    }
    case "csv_parse":
      return String(args[0] || "").trim().split(/\r?\n/).filter(Boolean).map((r) => r.split(",").map((c) => c.trim()));
    case "csv_stringify":
      return (Array.isArray(args[0]) ? args[0] : []).map((row) => (Array.isArray(row) ? row.join(",") : String(row))).join("\n");
    case "kv_set": {
      const db = String(args[0] || "default");
      if (!ctx.kvStore.has(db)) ctx.kvStore.set(db, {});
      ctx.kvStore.get(db)[String(args[1])] = String(args[2] ?? "");
      return undefined;
    }
    case "kv_get": {
      const db = String(args[0] || "default");
      return ctx.kvStore.get(db)?.[String(args[1])] ?? "";
    }
    case "kv_has": {
      const db = String(args[0] || "default");
      return Boolean(ctx.kvStore.get(db) && Object.prototype.hasOwnProperty.call(ctx.kvStore.get(db), String(args[1])));
    }
    case "kv_keys": {
      const db = String(args[0] || "default");
      return Object.keys(ctx.kvStore.get(db) || {});
    }
    case "kv_delete": {
      const db = String(args[0] || "default");
      const m = ctx.kvStore.get(db);
      if (m && Object.prototype.hasOwnProperty.call(m, String(args[1]))) { delete m[String(args[1])]; return true; }
      return false;
    }
    case "sha1":
      return bytesToHex(sha1Bytes(_te.encode(String(args[0] || ""))));
    case "sha256":
      return bytesToHex(sha256Bytes(_te.encode(String(args[0] || ""))));
    case "sha512":
      return bytesToHex(sha512Bytes(_te.encode(String(args[0] || ""))));
    case "hmac_sha256":
      return bytesToHex(hmacSha256Bytes(_te.encode(String(args[0] || "")), _te.encode(String(args[1] || ""))));
    case "hmac_sha256_verify": {
      const expected = bytesToHex(hmacSha256Bytes(_te.encode(String(args[0] || "")), _te.encode(String(args[1] || ""))));
      return constantTimeStrEq(expected, String(args[2] || "").toLowerCase());
    }
    case "secure_eq":
      return constantTimeStrEq(args[0], args[1]);
    case "pbkdf2_sha256":
      return pbkdf2Sha256Hex(args[0], args[1], args[2] !== undefined ? unwrapNum(args[2])?.n ?? 1000 : 1000);
    case "password_hash": {
      const salt = args[1] ? String(args[1]) : bytesToHex(randomBytesArr(12));
      const hash = pbkdf2Sha256Hex(args[0], salt, 600);
      return `dog$pbkdf2$600$${salt}$${hash}`;
    }
    case "password_verify": {
      const parts = String(args[1] || "").split("$");
      if (parts.length !== 5 || parts[0] !== "dog" || parts[1] !== "pbkdf2") return false;
      const computed = pbkdf2Sha256Hex(args[0], parts[3], parseInt(parts[2], 10) || 600);
      return constantTimeStrEq(computed, parts[4]);
    }
    case "encrypt_aes256":
      return encryptAes256Auth(args[0], args[1]);
    case "decrypt_aes256":
      return decryptAes256Auth(args[0], args[1], lineNum);
    case "ed25519_keypair": {
      const seed = args[0] ? sha256Bytes(_te.encode(String(args[0]))) : randomBytesArr(32);
      const { a } = edExpandSecret(seed);
      const pub = edPtEncode(edScalarMul(a));
      return { private_key: bytesToHex(seed), public_key: bytesToHex(pub) };
    }
    case "ed25519_sign":
      return edSignHex(args[0], args[1]);
    case "ed25519_verify":
      return edVerifyHex(args[0], args[1], args[2]);
    case "hex_encode":
      return bytesToHex(_te.encode(String(args[0] || "")));
    case "hex_decode":
      return _td.decode(hexToBytes(args[0], lineNum));
    case "base64_encode": {
      const b = _te.encode(String(args[0] || ""));
      let bin = "";
      for (const byte of b) bin += String.fromCharCode(byte);
      return btoa(bin);
    }
    case "base64_decode": {
      try {
        const bin = atob(String(args[0] || "").trim());
        const out = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
        return _td.decode(out);
      } catch {
        throw new DogRuntimeError({ code: "E030", title: "Invalid Base64 input.", line: lineNum });
      }
    }
    case "base64_url_encode": {
      const b = _te.encode(String(args[0] || ""));
      let bin = "";
      for (const byte of b) bin += String.fromCharCode(byte);
      return btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    }
    case "base64_url_decode": {
      try {
        let s = String(args[0] || "").replace(/-/g, "+").replace(/_/g, "/");
        while (s.length % 4 !== 0) s += "=";
        const bin = atob(s);
        const out = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
        return _td.decode(out);
      } catch {
        throw new DogRuntimeError({ code: "E030", title: "Invalid Base64URL input.", line: lineNum });
      }
    }
    case "rand_bytes_hex": {
      const count = Math.min(Math.max(1, Math.trunc(unwrapNum(args[0])?.n ?? 16)), 1024);
      return bytesToHex(randomBytesArr(count));
    }
    case "uuid_v4": {
      const b = randomBytesArr(16);
      b[6] = (b[6] & 0x0f) | 0x40;
      b[8] = (b[8] & 0x3f) | 0x80;
      const h = bytesToHex(b);
      return `${h.slice(0, 8)}-${h.slice(8, 12)}-${h.slice(12, 16)}-${h.slice(16, 20)}-${h.slice(20)}`;
    }
    case "time_now_ms":
      return Date.now();
    case "time_utc":
      return new Date().toISOString();
    case "time_format": {
      const d = args[0] !== undefined ? new Date(unwrapNum(args[0])?.n ?? Date.now()) : new Date();
      return d.toISOString().replace("T", " ").slice(0, 19);
    }
    case "spawn_task": {
      const id = `task-${ctx.tasks.size + 1}`;
      const result = invokeCallable(args[0], args.slice(1), ctx, lineNum);
      ctx.tasks.set(id, { id, status: "done", value: result });
      return id;
    }
    case "await_task": {
      const t = ctx.tasks.get(String(args[0] || ""));
      if (!t) throw new DogRuntimeError({ code: "E030", title: `Unknown task '${args[0]}'.`, line: lineNum });
      return t.value;
    }
    case "await_all":
      return (Array.isArray(args[0]) ? args[0] : []).map((id) => ctx.tasks.get(String(id))?.value);
    case "channel_create": {
      const cid = `ch-${ctx.channels.size + 1}`;
      ctx.channels.set(cid, []);
      return cid;
    }
    case "channel_send": {
      const q = ctx.channels.get(String(args[0] || ""));
      if (!q) throw new DogRuntimeError({ code: "E030", title: `Unknown channel '${args[0]}'.`, line: lineNum });
      q.push(args[1]);
      return q.length;
    }
    case "channel_recv": {
      const q = ctx.channels.get(String(args[0] || ""));
      if (!q || q.length === 0) return undefined;
      return q.shift();
    }
    case "atomic_create": {
      const aid = `atom-${ctx.atomics.size + 1}`;
      const val = unwrapNum(args[0])?.n ?? 0;
      ctx.atomics.set(aid, val);
      return aid;
    }
    case "atomic_add": {
      const aid = String(args[0] || "");
      const next = (ctx.atomics.get(aid) ?? 0) + (unwrapNum(args[1])?.n ?? 1);
      ctx.atomics.set(aid, next);
      return next;
    }
    case "atomic_get":
      return ctx.atomics.get(String(args[0] || "")) ?? 0;
    case "url_parse": {
      try {
        const u = new URL(String(args[0] || ""));
        return { scheme: u.protocol.replace(/:$/, ""), host: u.hostname, port: u.port || "", path: u.pathname || "/", query: u.search.replace(/^\?/, "") };
      } catch {
        throw new DogRuntimeError({ code: "E030", title: `Invalid URL '${args[0]}'.`, line: lineNum });
      }
    }
    case "query_parse": {
      const out = {};
      const params = new URLSearchParams(String(args[0] || "").replace(/^\?/, ""));
      for (const [k, v] of params.entries()) out[k] = v;
      return out;
    }
    case "query_stringify":
      return new URLSearchParams(Object.entries(args[0] || {}).map(([k, v]) => [k, String(v)])).toString();
    case "http_req":
      return { method: String(args[0] || "GET").toUpperCase(), url: String(args[1] || "/"), headers: args[2] || {}, body: args[3] ?? "" };
    case "http_res":
      return { status: Math.trunc(unwrapNum(args[0])?.n ?? 200), body: args[1] ?? "", headers: args[2] || { "content-type": "application/json" } };
    case "http_route_group": {
      const prefix = "/" + String(args[0] || "").replace(/^\/+|\/+$/g, "");
      const routes = args[1] && typeof args[1] === "object" ? args[1] : {};
      const out = {};
      for (const [k, v] of Object.entries(routes)) {
        const full = `${prefix === "/" ? "" : prefix}/${String(k).replace(/^\/+/, "")}`.replace(/\/+/g, "/");
        out[full] = v;
      }
      return out;
    }
    case "http_rate_limit_check": {
      const key = String(args[0] || "global");
      const maxHits = Math.max(1, Math.trunc(unwrapNum(args[1])?.n ?? 10));
      const count = (ctx.rateLimits.get(key) || 0) + 1;
      ctx.rateLimits.set(key, count);
      return count <= maxHits;
    }
    case "openapi_spec": {
      const title = String(args[0] || "DOG API");
      const version = String(args[1] || "1.0.0");
      const paths = {};
      for (const r of Array.isArray(args[2]) ? args[2] : []) {
        const p = String(r.path || r || "/");
        const m = String(r.method || "get").toLowerCase();
        paths[p] = { [m]: { summary: r.summary || `${m.toUpperCase()} ${p}`, responses: { 200: { description: "OK" } } } };
      }
      return JSON.stringify({ openapi: "3.1.0", info: { title, version }, paths }, null, 2);
    }
    case "http_route_match": {
      const patSegs = String(args[0] || "").split("/").filter(Boolean);
      const pathSegs = String(args[1] || "").split("?")[0].split("/").filter(Boolean);
      const out = {};
      for (let idx = 0; idx < patSegs.length; idx++) {
        const p = patSegs[idx];
        if (p.startsWith("*")) { out[p.slice(1)] = pathSegs.slice(idx).join("/"); return out; }
        if (idx >= pathSegs.length) return {};
        if (p.startsWith(":")) out[p.slice(1)] = pathSegs[idx];
        else if (p !== pathSegs[idx]) return {};
      }
      return patSegs.length === pathSegs.length ? out : {};
    }
    case "http_status_text": {
      const codes = { 200: "OK", 201: "Created", 204: "No Content", 400: "Bad Request", 401: "Unauthorized", 403: "Forbidden", 404: "Not Found", 429: "Too Many Requests", 500: "Internal Server Error" };
      return codes[Math.trunc(unwrapNum(args[0])?.n ?? 0)] || "Unknown";
    }
    case "http_cookie_build": {
      const name = String(args[0] || "session");
      const val = encodeURIComponent(String(args[1] ?? ""));
      const opts = args[2] && typeof args[2] === "object" ? args[2] : {};
      const parts = [`${name}=${val}`, `Path=${opts.path || "/"}`];
      if (opts.max_age !== undefined) parts.push(`Max-Age=${Math.trunc(unwrapNum(opts.max_age)?.n ?? 0)}`);
      if (opts.http_only !== false) parts.push("HttpOnly");
      if (opts.secure) parts.push("Secure");
      if (opts.same_site) parts.push(`SameSite=${opts.same_site}`);
      return parts.join("; ");
    }
    case "http_cookie_parse": {
      const out = {};
      for (const pair of String(args[0] || "").split(";")) {
        const idx = pair.indexOf("=");
        if (idx > 0) out[pair.slice(0, idx).trim()] = decodeURIComponent(pair.slice(idx + 1).trim());
      }
      return out;
    }
    case "http_auth_bearer": {
      const raw = String(args[0] || "").trim();
      const m = raw.match(/^Bearer\s+(.+)$/i);
      return m ? m[1].trim() : "";
    }
    case "path_normalize": {
      const raw = String(args[0] || "");
      const leadingSlash = raw.startsWith("/");
      const stack = [];
      for (const seg of raw.replace(/\\/g, "/").split("/")) {
        if (!seg || seg === ".") continue;
        if (seg === "..") stack.pop();
        else stack.push(seg);
      }
      return (leadingSlash ? "/" : "") + stack.join("/");
    }
    case "path_join":
      return [String(args[0] || "").replace(/\/+$/, ""), String(args[1] || "").replace(/^\/+/, "")].filter(Boolean).join("/");
    case "path_dirname": {
      const idx = String(args[0] || "").replace(/\/+$/, "").lastIndexOf("/");
      return idx <= 0 ? (String(args[0] || "").startsWith("/") ? "/" : ".") : String(args[0]).slice(0, idx);
    }
    case "path_basename":
      return String(args[0] || "").replace(/\/+$/, "").split("/").pop() || "";
    case "path_ext": {
      const base = String(args[0] || "").split("/").pop() || "";
      const dot = base.lastIndexOf(".");
      return dot > 0 ? base.slice(dot + 1) : "";
    }
    case "assert":
      if (!args[0]) {
        throw new DogRuntimeError({
          code: "E033",
          title: args[1] ? `Assertion failed: ${args[1]}` : "Assertion failed.",
          line: lineNum,
        });
      }
      return undefined;
    case "assert_eq": {
      const aStr = formatDogValue(args[0]);
      const bStr = formatDogValue(args[1]);
      if (!dogEquals(args[0], args[1], lineNum)) {
        throw new DogRuntimeError({
          code: "E033",
          title: `assert_eq failed${args[2] ? ` (${args[2]})` : ""}: expected ${bStr}, got ${aStr}.`,
          line: lineNum,
        });
      }
      return undefined;
    }
    case "assert_contains": {
      const hay = formatDogValue(args[0]);
      const needle = formatDogValue(args[1]);
      if (!hay.includes(needle)) {
        throw new DogRuntimeError({
          code: "E033",
          title: `assert_contains failed${args[2] ? ` (${args[2]})` : ""}: '${needle}' not found in output.`,
          line: lineNum,
        });
      }
      return undefined;
    }
    case "snapshot_check": {
      const snapName = String(args[0] || "snapshot");
      const actual = formatDogValue(args[1]);
      if (args.length < 3) {
        if (!ctx.snapshots) ctx.snapshots = new Map();
        if (ctx.snapshots.has(snapName) && ctx.snapshots.get(snapName) !== actual) {
          throw new DogRuntimeError({
            code: "E033",
            title: `Snapshot '${snapName}' mismatch: expected '${ctx.snapshots.get(snapName)}', got '${actual}'.`,
            line: lineNum,
          });
        }
        ctx.snapshots.set(snapName, actual);
        ctx.astSummary.push(`snapshot ${snapName} [RECORDED]`);
        return true;
      }
      const expected = formatDogValue(args[2]);
      if (actual !== expected) {
        throw new DogRuntimeError({
          code: "E033",
          title: `Snapshot '${snapName}' mismatch: expected '${expected}', got '${actual}'.`,
          line: lineNum,
        });
      }
      ctx.astSummary.push(`snapshot ${snapName} [PASS]`);
      return true;
    }
    case "fail":
      throw new DogRuntimeError({ code: "E035", title: String(args[0] || "Explicit failure"), line: lineNum });
    case "html_escape":
      return String(args[0] || "")
        .replace(/&/g, "&amp;")
        .replace(/</g, "&lt;")
        .replace(/>/g, "&gt;")
        .replace(/"/g, "&quot;");
    case "web_nav":
      return `<header class="dg-nav"><strong>${args[0]}</strong></header>`;
    case "web_card":
      return `<article class="dg-card"><h3>${args[0]}</h3><div>${args[1]}</div></article>`;
    case "web_grid":
      return `<div class="dg-grid">${(args[1] || []).join("")}</div>`;
    case "web_button":
      return `<button id="${args[0]}" class="dg-btn">${args[1]}</button>`;
    case "web_input":
      return `<input id="${args[0]}" placeholder="${args[1] || ""}" value="${args[2] || ""}" />`;
    case "web_badge":
      return `<span class="dg-badge dg-badge-${args[1] || "neutral"}">${args[0]}</span>`;
    case "web_alert":
      return `<div class="dg-alert dg-alert-${args[2] || "info"}" role="status"><strong>${args[0]}</strong>: ${args[1]}</div>`;
    case "web_field":
      return `<div class="dg-field"><label for="${args[0]}">${args[1]}</label><input id="${args[0]}" type="${args[3] || "text"}" placeholder="${args[2] || ""}" /></div>`;
    case "web_form":
      return `<form id="${args[0]}" class="dg-form">${args[1] || ""}<button type="submit" class="dg-btn">${args[2] || "Submit"}</button></form>`;
    case "html_el":
      return `<${args[0]}>${args[2] || ""}</${args[0]}>`;
    case "dom_mount":
      ctx.output.push(`[dom_mount] ${args[0]} <- ${String(args[1] || "").slice(0, 96)}`);
      return undefined;
    case "dom_set_text":
      ctx.output.push(`[dom_set_text] ${args[0]} = ${formatDogValue(args[1])}`);
      return undefined;
    case "dom_on":
      ctx.output.push(`[dom_on] ${args[0]} (${args[1]}) -> ${args[2]}()`);
      return undefined;
    case "css_bundle":
      return Array.isArray(args[0]) ? args[0].join("\n\n") : ctx.output.filter((l) => l.startsWith("[style]")).join("\n");
    default:
      throw new DogRuntimeError({
        code: "E013",
        title: `Unknown function '${rawName}()' in browser sandbox.`,
        line: lineNum,
        why: "OS process/socket listeners run in the native Rust CLI (`dog run`); the browser sandbox executes core language, math, text, collections, crypto, data, URL routing, assertions, and DOM diagnostics.",
        fix: "Install DOG CLI (`dog run`) for raw TCP/HTTP listener sockets and local disk I/O.",
      });
  }
}

function splitTopLevelParams(raw) {
  if (!raw.trim()) return [];
  const params = [];
  let depth = 0;
  let cur = "";
  let inStr = false;
  for (let i = 0; i < raw.length; i++) {
    const ch = raw[i];
    if (ch === '"') inStr = !inStr;
    if (!inStr) {
      if ("([<".includes(ch)) depth++;
      else if (")]>".includes(ch)) depth = Math.max(0, depth - 1);
      else if (ch === "," && depth === 0) {
        params.push(cur.trim());
        cur = "";
        continue;
      }
    }
    cur += ch;
  }
  if (cur.trim()) params.push(cur.trim());
  return params.map((p) => {
    const eqIdx = p.indexOf("=");
    const left = eqIdx >= 0 ? p.slice(0, eqIdx).trim() : p;
    const defaultExpr = eqIdx >= 0 ? p.slice(eqIdx + 1).trim() : null;
    const colonIdx = left.indexOf(":");
    const name = colonIdx >= 0 ? left.slice(0, colonIdx).trim() : left.trim();
    return { name, defaultExpr };
  });
}

function splitTopLevelRange(expr) {
  let depth = 0;
  let inStr = false;
  let esc = false;
  for (let i = 0; i < expr.length - 1; i++) {
    const ch = expr[i];
    if (esc) { esc = false; continue; }
    if (inStr) {
      if (ch === "\\") esc = true;
      else if (ch === '"') inStr = false;
      continue;
    }
    if (ch === '"') { inStr = true; continue; }
    if ("([{".includes(ch)) depth++;
    else if (")]}".includes(ch)) depth = Math.max(0, depth - 1);
    else if (depth === 0 && ch === "." && expr[i + 1] === ".") {
      return [expr.slice(0, i).trim(), expr.slice(i + 2).trim()];
    }
  }
  return null;
}

function matchesStopToken(trimmed, lead, stopTokens) {
  if (stopTokens.includes(lead)) return true;
  if (stopTokens.includes("or_when") && /^or\s+when\b/.test(trimmed)) return true;
  if ((lead === "else" && stopTokens.includes("else")) || (lead === "otherwise" && stopTokens.includes("otherwise"))) return true;
  return false;
}

function parseStatements(lines, startIdx = 0, stopTokens = []) {
  const stmts = [];
  let i = startIdx;
  while (i < lines.length) {
    const item = lines[i];
    const trimmed = item.text;
    if (!trimmed) {
      i++;
      continue;
    }
    const lead = trimmed.split(/[^a-zA-Z0-9_]/)[0];
    if (matchesStopToken(trimmed, lead, stopTokens)) {
      return { stmts, nextIdx: i, stoppedOn: trimmed, stopLine: item.line };
    }
    if (lead === "end") {
      throw new DogRuntimeError({
        code: "E005",
        title: "Unexpected 'end' without matching opening block.",
        line: item.line,
      });
    }

    // One-liner function across all styles: define|fn|function|do name(a, b) [-> T | returns T] = expr
    const simpleOneLiner = trimmed.match(/^(?:define|fn|function|do)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]*)\)(?:\s*(?:->|returns)\s*[^=]+)?\s*=\s*(.+)$/);
    if (simpleOneLiner) {
      const fnName = simpleOneLiner[1];
      const params = splitTopLevelParams(simpleOneLiner[2]);
      const bodyExpr = simpleOneLiner[3].trim();
      stmts.push({
        kind: "fndef",
        name: fnName,
        params,
        body: [{ kind: "return", expr: bodyExpr, line: item.line }],
        line: item.line,
      });
      i++;
      continue;
    }

    // Function definition: define / fn / function / do ... end
    if (/^(fn|define|function|do)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\((.*)\)\s*(?:(?:->|returns)\s*.+)?$/.test(trimmed)) {
      const m = trimmed.match(/^(?:fn|define|function|do)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\((.*)\)\s*(?:(?:->|returns)\s*(.+))?$/);
      const fnName = m[1];
      const params = splitTopLevelParams(m[2]);
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || bodyRes.stoppedOn.split(/[^a-zA-Z0-9_]/)[0] !== "end") {
        throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for function '${fnName}'.`, line: item.line });
      }
      stmts.push({ kind: "fndef", name: fnName, params, body: bodyRes.stmts, line: item.line });
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if ((lead === "if" || lead === "unless" || lead === "when") && trimmed.startsWith(`${lead} `)) {
      const isUnless = lead === "unless";
      const condRaw = trimmed
        .slice(lead.length)
        .replace(/\bthen\s*$/, "")
        .trim();
      const branches = [];
      let curIdx = i + 1;
      let curCond = condRaw;
      let curNegated = isUnless;
      let elseBody = null;

      while (true) {
        const res = parseStatements(lines, curIdx, ["elif", "or_when", "else", "otherwise", "end"]);
        branches.push({ cond: curCond, negated: curNegated, body: res.stmts, line: lines[curIdx - 1]?.line || item.line });
        if (!res.stoppedOn) {
          throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for '${lead}' block.`, line: item.line });
        }
        const st = res.stoppedOn;
        if (/^end\b/.test(st)) {
          i = res.nextIdx + 1;
          break;
        }
        if (/^(elif|else\s+if|otherwise\s+if|or\s+when)\b/.test(st)) {
          curCond = st
            .replace(/^(elif|else\s+if|otherwise\s+if|or\s+when)\b/, "")
            .replace(/\bthen\s*$/, "")
            .trim();
          curNegated = false;
          curIdx = res.nextIdx + 1;
          continue;
        }
        if (/^(else|otherwise)\b/.test(st)) {
          const elseRes = parseStatements(lines, res.nextIdx + 1, ["end"]);
          if (!elseRes.stoppedOn || !/^end\b/.test(elseRes.stoppedOn)) {
            throw new DogRuntimeError({ code: "E005", title: "Missing 'end' after 'else' branch.", line: res.stopLine });
          }
          elseBody = elseRes.stmts;
          i = elseRes.nextIdx + 1;
          break;
        }
      }
      stmts.push({ kind: "if", branches, elseBody, line: item.line });
      continue;
    }

    if (lead === "while" || lead === "until") {
      const condRaw = trimmed.slice(lead.length).trim();
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || !/^end\b/.test(bodyRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for '${lead}' loop.`, line: item.line });
      }
      stmts.push({ kind: lead, cond: condRaw, body: bodyRes.stmts, line: item.line });
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if (lead === "repeat" || (lead === "loop" && (trimmed === "loop" || trimmed.startsWith("loop ")))) {
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || !/^end\b/.test(bodyRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for '${lead}' loop.`, line: item.line });
      }
      if (trimmed === "loop") {
        stmts.push({ kind: "while", cond: "true", body: bodyRes.stmts, line: item.line });
      } else {
        const countRaw = trimmed
          .slice(lead.length)
          .replace(/\btimes\s*$/, "")
          .trim();
        stmts.push({ kind: "repeat", count: countRaw, body: bodyRes.stmts, line: item.line });
      }
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if (lead === "each" && trimmed.startsWith("each ")) {
      const mEachFrom = trimmed.match(/^each\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+from\s+(.+?)\s+to\s+(.+)$/);
      const mEach = trimmed.match(/^each\s+([a-zA-Z_][a-zA-Z0-9_]*)\s+in\s+(.+)$/);
      if (!mEachFrom && !mEach) {
        throw new DogRuntimeError({
          code: "E004",
          title: "Invalid 'each' loop syntax.",
          line: item.line,
          fix: "Write `each i in 1..5`, `each i from 1 to 5`, or `each item in items`.",
        });
      }
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || !/^end\b/.test(bodyRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for 'each' loop.", line: item.line });
      }
      if (mEachFrom) {
        stmts.push({ kind: "for_range", varName: mEachFrom[1], start: mEachFrom[2], endExpr: mEachFrom[3], body: bodyRes.stmts, line: item.line });
      } else {
        const rangeParts = splitTopLevelRange(mEach[2]);
        if (rangeParts) {
          stmts.push({ kind: "for_range", varName: mEach[1], start: rangeParts[0], endExpr: rangeParts[1], body: bodyRes.stmts, line: item.line });
        } else {
          stmts.push({ kind: "for_in", varName: mEach[1], iter: mEach[2], body: bodyRes.stmts, line: item.line });
        }
      }
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if (lead === "for") {
      const mFrom = trimmed.match(/^for\s+(?:each\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s+from\s+(.+?)\s+to\s+(.+)$/);
      const mIn = trimmed.match(/^for\s+(?:each\s+)?([a-zA-Z_][a-zA-Z0-9_]*)\s+in\s+(.+)$/);
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || !/^end\b/.test(bodyRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for 'for' loop.", line: item.line });
      }
      if (mFrom) {
        stmts.push({ kind: "for_range", varName: mFrom[1], start: mFrom[2], endExpr: mFrom[3], body: bodyRes.stmts, line: item.line });
      } else if (mIn) {
        const rangeParts = splitTopLevelRange(mIn[2]);
        if (rangeParts) {
          stmts.push({ kind: "for_range", varName: mIn[1], start: rangeParts[0], endExpr: rangeParts[1], body: bodyRes.stmts, line: item.line });
        } else {
          stmts.push({ kind: "for_in", varName: mIn[1], iter: mIn[2], body: bodyRes.stmts, line: item.line });
        }
      } else {
        throw new DogRuntimeError({
          code: "E004",
          title: "Invalid 'for' loop syntax.",
          line: item.line,
          fix: "Write `for item in items`, `for i in 1..5`, or `for i from 1 to 5`.",
        });
      }
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if (lead === "try") {
      const tryRes = parseStatements(lines, i + 1, ["catch", "oops"]);
      if (!tryRes.stoppedOn || !/^(catch|oops)\b/.test(tryRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: "Expected 'catch <err>' or 'oops <err>' after 'try'.", line: item.line });
      }
      const errName = tryRes.stoppedOn.replace(/^(catch|oops)\s*/, "").trim() || "err";
      const catchRes = parseStatements(lines, tryRes.nextIdx + 1, ["end"]);
      if (!catchRes.stoppedOn || !/^end\b/.test(catchRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for 'try ... catch'.", line: item.line });
      }
      stmts.push({ kind: "try", tryBody: tryRes.stmts, errName, catchBody: catchRes.stmts, line: item.line });
      i = catchRes.nextIdx + 1;
      continue;
    }

    if (trimmed === "return" || /^return\s+/.test(trimmed)) {
      const expr = trimmed.slice("return".length).trim();
      stmts.push({ kind: "return", expr: expr || null, line: item.line });
      i++;
      continue;
    }
    if (trimmed === "give" || (/^give\s+/.test(trimmed) && !/^give\s*=/.test(trimmed))) {
      const expr = trimmed.slice("give".length).trim();
      stmts.push({ kind: "return", expr: expr || null, line: item.line });
      i++;
      continue;
    }
    if (trimmed === "break" || trimmed === "stop") {
      stmts.push({ kind: "break", line: item.line });
      i++;
      continue;
    }
    if (trimmed === "continue" || trimmed === "skip") {
      stmts.push({ kind: "continue", line: item.line });
      i++;
      continue;
    }
    if (/^struct\s+([a-zA-Z_][a-zA-Z0-9_]*)$/.test(trimmed)) {
      const sName = trimmed.match(/^struct\s+([a-zA-Z_][a-zA-Z0-9_]*)$/)[1];
      let cursor = i + 1;
      const fields = new Map();
      let closed = false;
      while (cursor < lines.length) {
        const inner = lines[cursor].text;
        if (!inner) { cursor++; continue; }
        if (inner === "end") { closed = true; cursor++; break; }
        const fm = inner.match(/^([a-zA-Z_][a-zA-Z0-9_]*)\s*:\s*([^=]+?)(?:\s*=\s*(.+))?$/);
        if (!fm) throw new DogRuntimeError({ code: "E004", title: `Invalid struct field '${inner}'.`, line: lines[cursor].line });
        fields.set(fm[1], { type: fm[2].trim(), defaultExpr: fm[3] ? fm[3].trim() : null });
        cursor++;
      }
      if (!closed) throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for struct '${sName}'.`, line: item.line });
      stmts.push({ kind: "struct_def", name: sName, fields, line: item.line });
      i = cursor;
      continue;
    }

    if (/^enum\s+([a-zA-Z_][a-zA-Z0-9_]*)$/.test(trimmed)) {
      const eName = trimmed.match(/^enum\s+([a-zA-Z_][a-zA-Z0-9_]*)$/)[1];
      let cursor = i + 1;
      const variants = new Map();
      let closed = false;
      while (cursor < lines.length) {
        const inner = lines[cursor].text;
        if (!inner) { cursor++; continue; }
        if (inner === "end") { closed = true; cursor++; break; }
        const vm = inner.match(/^([a-zA-Z_][a-zA-Z0-9_]*)(?:\s*\(([^)]*)\))?$/);
        if (!vm) throw new DogRuntimeError({ code: "E004", title: `Invalid enum variant '${inner}'.`, line: lines[cursor].line });
        const arity = vm[2] && vm[2].trim() ? vm[2].split(",").length : 0;
        variants.set(vm[1], { name: vm[1], arity });
        cursor++;
      }
      if (!closed) throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for enum '${eName}'.`, line: item.line });
      stmts.push({ kind: "enum_def", name: eName, variants, line: item.line });
      i = cursor;
      continue;
    }

    if (lead === "match" && trimmed.startsWith("match ")) {
      const subjectExpr = trimmed.slice(6).trim();
      let cursor = i + 1;
      const arms = [];
      let elseBody = null;
      let closed = false;
      while (cursor < lines.length) {
        const curLine = lines[cursor];
        const lt = curLine.text;
        if (!lt) { cursor++; continue; }
        if (lt === "end") { closed = true; cursor++; break; }
        if (lt === "else" || lt === "otherwise") {
          const eRes = parseStatements(lines, cursor + 1, ["end"]);
          if (!eRes.stoppedOn || !/^end\b/.test(eRes.stoppedOn)) {
            throw new DogRuntimeError({ code: "E005", title: "Missing 'end' closing match else branch.", line: curLine.line });
          }
          elseBody = eRes.stmts;
          cursor = eRes.nextIdx + 1;
          closed = true;
          break;
        }
        if (/^case\s+/.test(lt)) {
          const rawCase = lt.slice(5).trim();
          const guardMatch = rawCase.match(/^(.+?)\s+if\s+(.+)$/);
          const patternRaw = guardMatch ? guardMatch[1].trim() : rawCase;
          const guardExpr = guardMatch ? guardMatch[2].trim() : null;
          const armRes = parseStatements(lines, cursor + 1, ["case", "else", "otherwise", "end"]);
          if (!armRes.stoppedOn) throw new DogRuntimeError({ code: "E005", title: "Missing 'end' closing match block.", line: item.line });
          arms.push({ patternRaw, guardExpr, body: armRes.stmts, line: curLine.line });
          cursor = armRes.nextIdx;
          continue;
        }
        throw new DogRuntimeError({ code: "E004", title: `Expected 'case' or 'else' inside 'match', found '${lt}'.`, line: curLine.line });
      }
      if (!closed) throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for 'match' block.", line: item.line });
      stmts.push({ kind: "match", subjectExpr, arms, elseBody, line: item.line });
      i = cursor;
      continue;
    }

    if (/^(let|var|const|keep|make)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(?:fn|do)\s*\(([^)]*)\)\s*(?:(?:->|returns)\s*[^=]+)?$/.test(trimmed)) {
      const m = trimmed.match(/^(let|var|const|keep|make)\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(?:fn|do)\s*\(([^)]*)\)\s*(?:(?:->|returns)\s*.+)?$/);
      const bindKind = m[1] === "keep" ? "let" : m[1] === "make" ? "var" : m[1];
      const params = splitTopLevelParams(m[3]);
      const bodyRes = parseStatements(lines, i + 1, ["end"]);
      if (!bodyRes.stoppedOn || !/^end\b/.test(bodyRes.stoppedOn)) {
        throw new DogRuntimeError({ code: "E005", title: `Missing 'end' for anonymous function bound to '${m[2]}'.`, line: item.line });
      }
      stmts.push({ kind: "bind_lambda", bindKind, name: m[2], params, body: bodyRes.stmts, line: item.line });
      i = bodyRes.nextIdx + 1;
      continue;
    }

    if (/^type\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(.+)$/.test(trimmed)) {
      const m = trimmed.match(/^type\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(.+)$/);
      stmts.push({ kind: "type_alias", alias: m[1], target: m[2].trim(), line: item.line });
      i++;
      continue;
    }
    if (/^(let|var|keep|make)\s+\[([^\]]+)\]\s*=\s*(.+)$/.test(trimmed)) {
      const m = trimmed.match(/^(let|var|keep|make)\s+\[([^\]]+)\]\s*=\s*(.+)$/);
      const bindKind = m[1] === "keep" ? "let" : m[1] === "make" ? "var" : m[1];
      const names = m[2].split(",").map((s) => s.trim()).filter(Boolean);
      stmts.push({ kind: "destruct", bindKind, names, expr: m[3], line: item.line });
      i++;
      continue;
    }
    if (/^(let|var|const|keep|make)\s+(?!=)/.test(trimmed)) {
      const m = trimmed.match(/^(let|var|const|keep|make)\s+([a-zA-Z_][a-zA-Z0-9_]*)(?:\s*:\s*[^=]+)?\s*=\s*(.+)$/);
      if (!m) {
        throw new DogRuntimeError({
          code: "E004",
          title: "Invalid variable binding.",
          line: item.line,
          fix: "Write `let name = value`, `keep name = value`, `var count = 0`, or `make count = 0`.",
        });
      }
      const bindKind = m[1] === "keep" ? "let" : m[1] === "make" ? "var" : m[1];
      stmts.push({ kind: "bind", bindKind, name: m[2], expr: m[3], line: item.line });
      i++;
      continue;
    }
    if (/^set\s+[a-zA-Z_][a-zA-Z0-9_.]*\s+to\s+/.test(trimmed)) {
      const m = trimmed.match(/^set\s+([a-zA-Z_][a-zA-Z0-9_.]*)\s+to\s+(.+)$/);
      stmts.push({ kind: "assign", name: m[1], expr: m[2], line: item.line });
      i++;
      continue;
    }
    if (/^(increase|decrease)\s+[a-zA-Z_][a-zA-Z0-9_.]*\s+by\s+/.test(trimmed)) {
      const m = trimmed.match(/^(increase|decrease)\s+([a-zA-Z_][a-zA-Z0-9_.]*)\s+by\s+(.+)$/);
      stmts.push({ kind: "compound", op: m[1] === "increase" ? "+=" : "-=", name: m[2], expr: m[3], line: item.line });
      i++;
      continue;
    }
    if (/^add\s+(.+?)\s+to\s+([a-zA-Z_][a-zA-Z0-9_.]*)$/.test(trimmed)) {
      const m = trimmed.match(/^add\s+(.+?)\s+to\s+([a-zA-Z_][a-zA-Z0-9_.]*)$/);
      stmts.push({ kind: "compound", op: "+=", name: m[2], expr: m[1], line: item.line });
      i++;
      continue;
    }
    if (/^take\s+(.+?)\s+from\s+([a-zA-Z_][a-zA-Z0-9_.]*)$/.test(trimmed)) {
      const m = trimmed.match(/^take\s+(.+?)\s+from\s+([a-zA-Z_][a-zA-Z0-9_.]*)$/);
      stmts.push({ kind: "compound", op: "-=", name: m[2], expr: m[1], line: item.line });
      i++;
      continue;
    }
    if (/^bump\s+([a-zA-Z_][a-zA-Z0-9_.]*)(?:\s+by\s+(.+))?$/.test(trimmed)) {
      const m = trimmed.match(/^bump\s+([a-zA-Z_][a-zA-Z0-9_.]*)(?:\s+by\s+(.+))?$/);
      stmts.push({ kind: "compound", op: "+=", name: m[1], expr: m[2] ? m[2].trim() : "1", line: item.line });
      i++;
      continue;
    }
    if (/^(?:show|print|say)\s+(?![=(])(.+)$/.test(trimmed)) {
      const m = trimmed.match(/^(?:show|print|say)\s+(.+)$/);
      stmts.push({ kind: "expr", expr: `print(${m[1].trim()})`, line: item.line });
      i++;
      continue;
    }
    if (/^(?:check|assert)\s+(?![=(])(.+)$/.test(trimmed)) {
      const m = trimmed.match(/^(?:check|assert)\s+(.+)$/);
      stmts.push({ kind: "expr", expr: `assert(${m[1].trim()})`, line: item.line });
      i++;
      continue;
    }
    if (/^[a-zA-Z_][a-zA-Z0-9_.]*\s*(\+=|-=|\*=|\/=|%=|=)\s*/.test(trimmed)) {
      const m = trimmed.match(/^([a-zA-Z_][a-zA-Z0-9_.]*)\s*(\+=|-=|\*=|\/=|%=|=)\s*(.+)$/);
      if (m[2] === "=") {
        stmts.push({ kind: "assign", name: m[1], expr: m[3], line: item.line });
      } else {
        stmts.push({ kind: "compound", op: m[2], name: m[1], expr: m[3], line: item.line });
      }
      i++;
      continue;
    }

    if (lead === "style" && (trimmed === "style" || trimmed.startsWith("style "))) {
      const header = trimmed.slice(5).trim();
      if (!header) {
        throw new DogRuntimeError({
          code: "E004",
          title: "Expected selector or 'theme' after 'style'.",
          line: item.line,
          fix: 'Write `style ".card"` or `style theme`.',
        });
      }
      let cursor = i + 1;
      const rules = [];
      let closed = false;
      while (cursor < lines.length) {
        const inner = lines[cursor].text;
        if (!inner) {
          cursor++;
          continue;
        }
        if (inner === "end") {
          closed = true;
          cursor++;
          break;
        }
        if (/^(?:on\s+)?(?:hover|focus|active|mobile|tablet|desktop|dark)$/.test(inner) || /^media\s+"[^"]+"$/.test(inner)) {
          const vLine = lines[cursor].line;
          cursor++;
          let vClosed = false;
          while (cursor < lines.length) {
            const vTxt = lines[cursor].text;
            if (!vTxt) {
              cursor++;
              continue;
            }
            if (vTxt === "end") {
              vClosed = true;
              cursor++;
              break;
            }
            cursor++;
          }
          if (!vClosed) {
            throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for style variant block.", line: vLine });
          }
          continue;
        }
        rules.push(inner);
        cursor++;
      }
      if (!closed) {
        throw new DogRuntimeError({ code: "E005", title: "Missing 'end' for 'style' block.", line: item.line });
      }
      stmts.push({ kind: "style", header, rulesCount: rules.length, line: item.line });
      i = cursor;
      continue;
    }

    if (/^use\s+"/.test(trimmed)) {
      stmts.push({ kind: "use", path: trimmed, line: item.line });
      i++;
      continue;
    }

    stmts.push({ kind: "expr", expr: trimmed, line: item.line });
    i++;
  }
  return { stmts, nextIdx: i, stoppedOn: null, stopLine: null };
}

function matchSinglePattern(patStr, subject, ctx, lineNum) {
  const pat = patStr.trim();
  if (pat === "_") return { matched: true, bindings: {} };
  const enumCallM = pat.match(/^([a-zA-Z_][a-zA-Z0-9_]*)\.([a-zA-Z_][a-zA-Z0-9_]*)\s*\(([^)]*)\)$/);
  if (enumCallM && subject && subject.__enum) {
    if (subject.__enum !== enumCallM[1] || subject.__variant !== enumCallM[2]) return { matched: false };
    const varNames = enumCallM[3].split(",").map((s) => s.trim()).filter(Boolean);
    if (varNames.length !== (subject.__payload?.length || 0)) return { matched: false };
    const bindings = {};
    varNames.forEach((vn, idx) => {
      if (vn !== "_") bindings[vn] = subject.__payload[idx];
    });
    return { matched: true, bindings };
  }
  if (/^[a-zA-Z_][a-zA-Z0-9_]*$/.test(pat) && !ctx.lookup(pat)) {
    return { matched: true, bindings: { [pat]: subject } };
  }
  const val = evalExprString(pat, ctx, lineNum);
  return { matched: dogEquals(subject, val, lineNum), bindings: {} };
}

function execBlock(stmts, ctx) {
  for (const st of stmts) {
    ctx.steps++;
    if (ctx.steps > MAX_STEPS) {
      throw new DogRuntimeError({
        code: "E035",
        title: "Execution step limit reached in browser sandbox.",
        line: st.line,
        why: "Infinite loop protection stopped execution after 8,000 steps.",
        fix: "Ensure your `while` or `until` loop converges.",
      });
    }
    if (st.kind === "use") continue;
    if (st.kind === "type_alias") {
      ctx.typeAliases.set(st.alias, st.target);
      ctx.astSummary.push(`type ${st.alias} = ${st.target}`);
      continue;
    }
    if (st.kind === "struct_def") {
      ctx.structs.set(st.name, st);
      ctx.astSummary.push(`struct ${st.name} (${[...st.fields.keys()].join(", ")})`);
      continue;
    }
    if (st.kind === "enum_def") {
      ctx.enums.set(st.name, st);
      ctx.astSummary.push(`enum ${st.name} (${[...st.variants.keys()].join(" | ")})`);
      continue;
    }
    if (st.kind === "style") {
      ctx.astSummary.push(`style ${st.header} (${st.rulesCount} rules)`);
      continue;
    }
    if (st.kind === "fndef") {
      ctx.functions.set(st.name, st);
      ctx.astSummary.push(`fn ${st.name}(${st.params.map((p) => p.name).join(", ")})`);
      continue;
    }
    if (st.kind === "bind_lambda") {
      const fnObj = {
        __dogLambda: true,
        params: st.params,
        bodyExpr: null,
        bodyStmts: st.body,
        capturedScopes: [...ctx.scopes],
        line: st.line,
      };
      ctx.define(st.name, st.bindKind, fnObj);
      ctx.astSummary.push(`${st.bindKind} ${st.name}: fn(${st.params.map((p) => p.name).join(", ")})`);
      continue;
    }
    if (st.kind === "match") {
      const subject = evalExprString(st.subjectExpr, ctx, st.line);
      let matched = false;
      for (const arm of st.arms) {
        const subPatterns = splitTopLevelParams(arm.patternRaw).length ? arm.patternRaw.split(/,(?![^()]*\))/) : [arm.patternRaw];
        let hit = null;
        for (const sp of subPatterns) {
          const res = matchSinglePattern(sp, subject, ctx, arm.line);
          if (res.matched) { hit = res; break; }
        }
        if (!hit) continue;
        ctx.pushScope();
        try {
          for (const [bk, bv] of Object.entries(hit.bindings)) ctx.define(bk, "let", bv);
          if (arm.guardExpr) {
            const g = evalExprString(arm.guardExpr, ctx, arm.line);
            if (!g) { ctx.popScope(); continue; }
          }
          matched = true;
          const flow = execBlock(arm.body, ctx);
          if (flow.type !== "next") return flow;
          break;
        } finally {
          if (ctx.scopes.length > 1) ctx.popScope();
        }
      }
      if (!matched && st.elseBody) {
        ctx.pushScope();
        try {
          const flow = execBlock(st.elseBody, ctx);
          if (flow.type !== "next") return flow;
        } finally {
          ctx.popScope();
        }
      }
      continue;
    }
    if (st.kind === "destruct") {
      const val = evalExprString(st.expr, ctx, st.line);
      if (!Array.isArray(val) || val.length < st.names.length) {
        throw new DogRuntimeError({ code: "E032", title: `Cannot destructure ${st.names.length} items from list.`, line: st.line });
      }
      st.names.forEach((n, idx) => {
        ctx.define(n, st.bindKind, val[idx]);
        ctx.astSummary.push(`${st.bindKind} ${n}: ${typeNameOf(val[idx])}`);
      });
      continue;
    }
    if (st.kind === "bind") {
      const val = evalExprString(st.expr, ctx, st.line);
      ctx.define(st.name, st.bindKind, val);
      ctx.astSummary.push(`${st.bindKind} ${st.name}: ${typeNameOf(val)}`);
      continue;
    }
    if (st.kind === "assign") {
      if (st.name.includes(".")) {
        const parts = st.name.split(".");
        const root = ctx.lookup(parts[0]);
        if (!root) throw new DogRuntimeError({ code: "E010", title: `Undefined variable '${parts[0]}'.`, line: st.line });
        if (root.kind !== "var") throw new DogRuntimeError({ code: "E012", title: `Cannot mutate field on immutable '${root.kind}' binding '${parts[0]}'.`, line: st.line });
        let target = root.value;
        for (let k = 1; k < parts.length - 1; k++) target = target?.[parts[k]];
        const field = parts[parts.length - 1];
        if (!target || typeof target !== "object" || !Object.prototype.hasOwnProperty.call(target, field)) {
          throw new DogRuntimeError({ code: "E010", title: `Unknown field '${field}'.`, line: st.line });
        }
        const val = evalExprString(st.expr, ctx, st.line);
        if (typeNameOf(target[field]) !== typeNameOf(val)) {
          throw new DogRuntimeError({ code: "E021", title: `Type mismatch assigning to '${st.name}'.`, line: st.line });
        }
        target[field] = val;
        continue;
      }
      const cur = ctx.lookup(st.name);
      if (!cur) {
        throw new DogRuntimeError({ code: "E010", title: `Undefined variable '${st.name}'.`, line: st.line });
      }
      if (cur.kind !== "var") {
        throw new DogRuntimeError({
          code: "E012",
          title: `Cannot reassign immutable binding '${st.name}'.`,
          line: st.line,
          why: `'${st.name}' was declared with '${cur.kind}', not 'var'.`,
          fix: `Change \`${cur.kind} ${st.name} = ...\` to \`var ${st.name} = ...\` if it needs to change.`,
        });
      }
      const val = evalExprString(st.expr, ctx, st.line);
      if (typeNameOf(cur.value) !== typeNameOf(val)) {
        throw new DogRuntimeError({
          code: "E021",
          title: `Type mismatch assigning to '${st.name}'.`,
          line: st.line,
          why: `Expected '${typeNameOf(cur.value)}', got '${typeNameOf(val)}'.`,
        });
      }
      cur.value = val;
      continue;
    }
    if (st.kind === "compound") {
      let container = null;
      let curVal = null;
      let keyName = st.name;
      if (st.name.includes(".")) {
        const parts = st.name.split(".");
        const root = ctx.lookup(parts[0]);
        if (!root) throw new DogRuntimeError({ code: "E010", title: `Undefined variable '${parts[0]}'.`, line: st.line });
        if (root.kind !== "var") throw new DogRuntimeError({ code: "E012", title: `Cannot mutate immutable binding '${parts[0]}'.`, line: st.line });
        container = root.value;
        keyName = parts[parts.length - 1];
        curVal = container?.[keyName];
      } else {
        const cur = ctx.lookup(st.name);
        if (!cur) throw new DogRuntimeError({ code: "E010", title: `Undefined variable '${st.name}'.`, line: st.line });
        if (cur.kind !== "var") {
          throw new DogRuntimeError({
            code: "E012",
            title: `Cannot mutate immutable '${cur.kind}' variable '${st.name}'.`,
            line: st.line,
            fix: `Declare '${st.name}' with \`var\` instead of \`${cur.kind}\`.`,
          });
        }
        container = cur;
        keyName = "value";
        curVal = cur.value;
      }
      const rhs = evalExprString(st.expr, ctx, st.line);
      if (st.op === "+=" && typeof curVal === "string" && typeof rhs === "string") {
        container[keyName] = curVal + rhs;
        continue;
      }
      const na = unwrapNum(curVal);
      const nb = unwrapNum(rhs);
      if (!na || !nb || na.isDecimal !== nb.isDecimal) {
        throw new DogRuntimeError({
          code: "E021",
          title: `Type mismatch in '${st.op}' on '${st.name}'.`,
          line: st.line,
        });
      }
      let nextVal = na.n;
      if (st.op === "+=") nextVal += nb.n;
      else if (st.op === "-=") nextVal -= nb.n;
      else if (st.op === "*=") nextVal *= nb.n;
      else if (st.op === "/=") nextVal = na.isDecimal ? nextVal / nb.n : Math.trunc(nextVal / nb.n);
      else if (st.op === "%=") nextVal %= nb.n;
      container[keyName] = makeTypedVal(na.isDecimal ? "decimal" : "number", nextVal);
      continue;
    }
    if (st.kind === "expr") {
      evalExprString(st.expr, ctx, st.line);
      continue;
    }
    if (st.kind === "return") {
      const value = st.expr ? evalExprString(st.expr, ctx, st.line) : undefined;
      return { type: "return", value };
    }
    if (st.kind === "break") return { type: "break" };
    if (st.kind === "continue") return { type: "continue" };
    if (st.kind === "if") {
      let ran = false;
      for (const b of st.branches) {
        const cond = evalExprString(b.cond, ctx, b.line);
        if (typeNameOf(cond) !== "bool") {
          throw new DogRuntimeError({
            code: "E022",
            title: "Condition must evaluate to 'bool'.",
            line: b.line,
          });
        }
        const pass = b.negated ? !cond : cond;
        if (pass) {
          ran = true;
          ctx.pushScope();
          try {
            const flow = execBlock(b.body, ctx);
            if (flow.type !== "next") return flow;
          } finally {
            ctx.popScope();
          }
          break;
        }
      }
      if (!ran && st.elseBody) {
        ctx.pushScope();
        try {
          const flow = execBlock(st.elseBody, ctx);
          if (flow.type !== "next") return flow;
        } finally {
          ctx.popScope();
        }
      }
      continue;
    }
    if (st.kind === "while" || st.kind === "until") {
      while (true) {
        ctx.steps++;
        if (ctx.steps > MAX_STEPS) {
          throw new DogRuntimeError({ code: "E035", title: "Loop step limit reached.", line: st.line });
        }
        const cond = evalExprString(st.cond, ctx, st.line);
        if (typeNameOf(cond) !== "bool") {
          throw new DogRuntimeError({ code: "E022", title: "Loop condition must be 'bool'.", line: st.line });
        }
        const go = st.kind === "until" ? !cond : cond;
        if (!go) break;
        ctx.pushScope();
        let flow = { type: "next" };
        try {
          flow = execBlock(st.body, ctx);
        } finally {
          ctx.popScope();
        }
        if (flow.type === "break") break;
        if (flow.type === "return") return flow;
      }
      continue;
    }
    if (st.kind === "repeat") {
      const cnt = evalExprString(st.count, ctx, st.line);
      const n = unwrapNum(cnt);
      if (!n || n.isDecimal || n.n < 0) {
        throw new DogRuntimeError({ code: "E025", title: "repeat count must be a non-negative number.", line: st.line });
      }
      for (let k = 0; k < n.n; k++) {
        ctx.pushScope();
        let flow = { type: "next" };
        try {
          flow = execBlock(st.body, ctx);
        } finally {
          ctx.popScope();
        }
        if (flow.type === "break") break;
        if (flow.type === "return") return flow;
      }
      continue;
    }
    if (st.kind === "for_range") {
      const sVal = unwrapNum(evalExprString(st.start, ctx, st.line));
      const eVal = unwrapNum(evalExprString(st.endExpr, ctx, st.line));
      if (!sVal || !eVal || sVal.isDecimal || eVal.isDecimal) {
        throw new DogRuntimeError({ code: "E021", title: "Range bounds must be integers.", line: st.line });
      }
      for (let v = sVal.n; v <= eVal.n; v++) {
        ctx.pushScope();
        ctx.define(st.varName, "let", v);
        let flow = { type: "next" };
        try {
          flow = execBlock(st.body, ctx);
        } finally {
          ctx.popScope();
        }
        if (flow.type === "break") break;
        if (flow.type === "return") return flow;
      }
      continue;
    }
    if (st.kind === "for_in") {
      const list = evalExprString(st.iter, ctx, st.line);
      if (!Array.isArray(list)) {
        throw new DogRuntimeError({ code: "E025", title: "for-in expects a list.", line: st.line });
      }
      for (const item of list) {
        ctx.pushScope();
        ctx.define(st.varName, "let", item);
        let flow = { type: "next" };
        try {
          flow = execBlock(st.body, ctx);
        } finally {
          ctx.popScope();
        }
        if (flow.type === "break") break;
        if (flow.type === "return") return flow;
      }
      continue;
    }
    if (st.kind === "try") {
      try {
        ctx.pushScope();
        const flow = execBlock(st.tryBody, ctx);
        ctx.popScope();
        if (flow.type !== "next") return flow;
      } catch (err) {
        if (ctx.scopes.length > 1) ctx.popScope();
        const msg = err instanceof DogRuntimeError ? err.title : String(err.message || err);
        ctx.pushScope();
        ctx.define(st.errName, "let", msg);
        try {
          const flow = execBlock(st.catchBody, ctx);
          if (flow.type !== "next") return flow;
        } finally {
          ctx.popScope();
        }
      }
    }
  }
  return { type: "next" };
}

function collectSandboxWarnings(stmts, out = [], stats = { constantFolded: 0, deadStatements: 0, declared: [], usedIdents: new Set() }) {
  let returned = false;
  const markExprRefs = (expr) => {
    if (!expr) return;
    if (/^\s*\d+\s*[\+\-\*]\s*\d+\s*$/.test(expr) || /^\s*"[^"]*"\s*\+\s*"[^"]*"\s*$/.test(expr)) {
      stats.constantFolded++;
    }
    for (const m of String(expr).matchAll(/[a-zA-Z_][a-zA-Z0-9_]*/g)) {
      stats.usedIdents.add(m[0]);
    }
  };

  for (const st of stmts) {
    if (returned) {
      stats.deadStatements++;
      out.push({
        code: "W001",
        severity: "warning",
        line: st.line,
        title: "Unreachable statement detected after return/break/continue.",
        fix: "Remove dead code or move it before the terminating branch.",
      });
      break;
    }
    if (st.kind === "return" || st.kind === "break" || st.kind === "continue") returned = true;
    if (st.kind === "bind" || st.kind === "bind_lambda") {
      if (!st.name.startsWith("_")) stats.declared.push({ name: st.name, line: st.line });
      markExprRefs(st.expr);
    }
    if (st.kind === "destruct") {
      st.names.forEach((n) => { if (!n.startsWith("_")) stats.declared.push({ name: n, line: st.line }); });
      markExprRefs(st.expr);
    }
    if (st.kind === "expr" || st.kind === "assign" || st.kind === "compound" || st.kind === "return") {
      markExprRefs(st.expr);
      if (st.name) stats.usedIdents.add(st.name.split(".")[0]);
    }
    if (st.kind === "fndef" || st.kind === "bind_lambda") collectSandboxWarnings(st.body, out, stats);
    if (st.kind === "if") {
      st.branches.forEach((b) => { markExprRefs(b.cond); collectSandboxWarnings(b.body, out, stats); });
      if (st.elseBody) collectSandboxWarnings(st.elseBody, out, stats);
    }
    if (st.kind === "match") {
      markExprRefs(st.subjectExpr);
      st.arms.forEach((a) => { markExprRefs(a.guardExpr); collectSandboxWarnings(a.body, out, stats); });
      if (st.elseBody) collectSandboxWarnings(st.elseBody, out, stats);
    }
    if (st.kind === "while" || st.kind === "until" || st.kind === "repeat" || st.kind === "for_range" || st.kind === "for_in") {
      markExprRefs(st.cond || st.count || st.iter);
      markExprRefs(st.start);
      markExprRefs(st.endExpr);
      collectSandboxWarnings(st.body, out, stats);
    }
    if (st.kind === "try") {
      collectSandboxWarnings(st.tryBody, out, stats);
      if (st.catchBody.length === 0) {
        out.push({ code: "W002", severity: "warning", line: st.line, title: "Empty catch block suppresses errors.", fix: "Log or handle the error variable." });
      } else {
        collectSandboxWarnings(st.catchBody, out, stats);
      }
    }
  }
  return { warnings: out, stats };
}

function sandboxBracketDelta(str) {
  let d = 0;
  let inStr = false;
  let esc = false;
  for (let i = 0; i < str.length; i++) {
    const ch = str[i];
    if (esc) { esc = false; continue; }
    if (inStr) {
      if (ch === "\\") esc = true;
      else if (ch === '"') inStr = false;
      continue;
    }
    if (ch === '"') { inStr = true; continue; }
    if (ch === "(" || ch === "[" || ch === "{") d++;
    else if (ch === ")" || ch === "]" || ch === "}") d--;
  }
  return d;
}

export function runDogSandbox(source) {
  const start = Date.now();
  const rawLines = String(source || "").split(/\r?\n/);
  const lines = [];
  let bufText = "";
  let bufLine = 1;
  let bDepth = 0;
  rawLines.forEach((raw, idx) => {
    const t = stripComment(raw).trim();
    if (!t) {
      if (bDepth === 0) lines.push({ line: idx + 1, text: "" });
      return;
    }
    if (bDepth > 0) {
      bufText += ` ${t}`;
      bDepth += sandboxBracketDelta(t);
      if (bDepth <= 0) {
        lines.push({ line: bufLine, text: bufText });
        bufText = "";
        bDepth = 0;
      }
    } else {
      const d = sandboxBracketDelta(t);
      if (d > 0) {
        bufText = t;
        bufLine = idx + 1;
        bDepth = d;
      } else {
        lines.push({ line: idx + 1, text: t });
      }
    }
  });
  if (bufText) lines.push({ line: bufLine, text: bufText });

  const scopes = [new Map()];
  const ctx = {
    scopes,
    functions: new Map(),
    structs: new Map(),
    enums: new Map(),
    typeAliases: new Map(),
    kvStore: new Map(),
    channels: new Map(),
    atomics: new Map(),
    tasks: new Map(),
    rateLimits: new Map(),
    callStack: [],
    output: [],
    astSummary: [],
    steps: 0,
    pushScope() {
      this.scopes.push(new Map());
    },
    popScope() {
      if (this.scopes.length > 1) this.scopes.pop();
    },
    define(name, kind, value) {
      this.scopes[this.scopes.length - 1].set(name, { kind, value });
    },
    lookup(name) {
      for (let i = this.scopes.length - 1; i >= 0; i--) {
        if (this.scopes[i].has(name)) return this.scopes[i].get(name);
      }
      return null;
    },
  };

  let warnings = [];
  let optStats = { constantFolded: 0, deadStatements: 0 };
  try {
    const parsed = parseStatements(lines, 0, []);
    const rootAnalysis = collectSandboxWarnings(parsed.stmts);
    warnings = rootAnalysis.warnings;
    for (const decl of rootAnalysis.stats.declared) {
      if (!rootAnalysis.stats.usedIdents.has(decl.name)) {
        warnings.push({
          code: "W003",
          severity: "warning",
          line: decl.line,
          title: `Unused binding '${decl.name}'.`,
          fix: `Remove '${decl.name}' or prefix with '_${decl.name}'.`,
        });
      }
    }
    optStats = {
      constantFolded: rootAnalysis.stats.constantFolded,
      deadStatements: rootAnalysis.stats.deadStatements,
    };
    execBlock(parsed.stmts, ctx);
    const diagPayload = { ok: true, engine: "dog-sandbox/1.0.0", steps: ctx.steps, optimizations: optStats, diagnostics: warnings };
    return {
      ok: true,
      output: ctx.output,
      astSummary: ctx.astSummary,
      warnings,
      optimizations: optStats,
      diagnostics: warnings,
      diagnosticsJson: JSON.stringify(diagPayload, null, 2),
      steps: ctx.steps,
      elapsedMs: Date.now() - start,
      error: null,
    };
  } catch (err) {
    const diag =
      err instanceof DogRuntimeError
        ? err
        : new DogRuntimeError({
            code: "E004",
            title: String(err?.message || err),
            line: 1,
          });
    if (diag.stackFrames.length === 0 && ctx.callStack.length > 0) {
      diag.stackFrames = [...ctx.callStack].reverse();
    }
    const diagnostics = [...warnings, diag.toJSON()];
    const diagPayload = { ok: false, engine: "dog-sandbox/1.0.0", steps: ctx.steps, optimizations: optStats, diagnostics };
    return {
      ok: false,
      output: ctx.output,
      astSummary: ctx.astSummary,
      warnings,
      optimizations: optStats,
      diagnostics,
      diagnosticsJson: JSON.stringify(diagPayload, null, 2),
      steps: ctx.steps,
      elapsedMs: Date.now() - start,
      error: diag.format(),
      errorCode: diag.code,
      stackFrames: diag.stackFrames,
    };
  }
}
