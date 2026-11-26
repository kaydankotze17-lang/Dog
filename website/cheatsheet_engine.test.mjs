import test from "node:test";
import assert from "node:assert/strict";
import {
  CHEATSHEET_CATEGORIES,
  CHEATSHEET_ENTRIES,
  ENGLISH_VS_CLASSIC_TABLE,
  OPERATOR_PRECEDENCE_TABLE,
  filterCheatSheetEntries,
  formatCheatSheetMarkdown,
} from "./cheatsheet_engine.mjs";
import { PLAYGROUND_PRESETS, SYNTAX_SHOWCASE, WEB_STUDIO_PRESETS } from "./data.js";
import { runDogSandbox } from "./sandbox.js";
import { compileDogWebsite } from "./dog_web.js";

test("filterCheatSheetEntries filters accurately by category and search text", () => {
  assert.ok(CHEATSHEET_CATEGORIES.length >= 9);
  assert.ok(ENGLISH_VS_CLASSIC_TABLE.length >= 10);
  assert.ok(ENGLISH_VS_CLASSIC_TABLE.every((row) => typeof row.simple === "string" && row.simple.length > 0));
  assert.ok(OPERATOR_PRECEDENCE_TABLE.length >= 6);

  const all = filterCheatSheetEntries(CHEATSHEET_ENTRIES, { category: "all", q: "" });
  assert.equal(all.length, CHEATSHEET_ENTRIES.length);

  const cryptoMatches = filterCheatSheetEntries(CHEATSHEET_ENTRIES, { category: "all", q: "ed25519" });
  assert.equal(cryptoMatches.length, 1);
  assert.equal(cryptoMatches[0].id, "crypto-auth-ed25519");

  const simpleMatches = filterCheatSheetEntries(CHEATSHEET_ENTRIES, { category: "dialects", q: "simple" });
  assert.ok(simpleMatches.some((e) => e.id === "simple-style-basics"));

  const webMatches = filterCheatSheetEntries(CHEATSHEET_ENTRIES, { category: "web-dgss", q: "" });
  assert.ok(webMatches.length >= 2);
  assert.ok(webMatches.every((e) => e.category === "web-dgss"));

  const none = filterCheatSheetEntries(CHEATSHEET_ENTRIES, { category: "all", q: "zzz_nonexistent_token" });
  assert.equal(none.length, 0);
});

test("formatCheatSheetMarkdown outputs 3-way dialect table, operator precedence, and sections", () => {
  const md = formatCheatSheetMarkdown(CHEATSHEET_ENTRIES, ENGLISH_VS_CLASSIC_TABLE, OPERATOR_PRECEDENCE_TABLE);
  assert.match(md, /# DOG Programming Language Cheat Sheet/);
  assert.match(md, /## English Style vs Classic Style vs Simple Style/);
  assert.match(md, /\| Simple Style \|/);
  assert.match(md, /## Operator Precedence/);
  assert.match(md, /```dog/);
  assert.match(md, /ed25519_verify/);
});

test("all runnable Cheat Sheet entries, Playground/Web presets, and Simple showcase examples compile cleanly", () => {
  for (const entry of CHEATSHEET_ENTRIES) {
    if (!entry.playgroundRunnable) continue;
    const res = runDogSandbox(entry.code);
    assert.equal(res.ok, true, `Cheat sheet entry '${entry.id}' failed: ${res.error}`);
  }

  for (const preset of PLAYGROUND_PRESETS) {
    if (preset.id === "strict_type_err") continue;
    const res = runDogSandbox(preset.code);
    assert.equal(res.ok, true, `Playground preset '${preset.id}' failed: ${res.error}`);
  }

  for (const [key, topic] of Object.entries(SYNTAX_SHOWCASE)) {
    assert.ok(typeof topic.simpleCode === "string" && topic.simpleCode.length > 0, `Missing simpleCode for showcase '${key}'`);
  }
  assert.equal(runDogSandbox(SYNTAX_SHOWCASE.english.simpleCode).ok, true);
  assert.equal(runDogSandbox(SYNTAX_SHOWCASE.errors.simpleCode).ok, true);

  for (const webPreset of WEB_STUDIO_PRESETS) {
    const res = compileDogWebsite({
      code: webPreset.code,
      css: webPreset.css,
      title: webPreset.title,
      treeShake: true,
      minify: true,
    });
    assert.equal(res.ok, true, `Web Studio preset '${webPreset.id}' failed: ${res.error}`);
  }
});
