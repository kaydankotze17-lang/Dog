import test from "node:test";
import assert from "node:assert/strict";
import {
  buildCompatibilityMatrix,
  buildSbomAndLicenseReport,
  checkDocConsistency,
  evaluateReadinessGate,
  inspectTemplates,
  runSyntaxParityAndShowcase,
} from "./readiness_engine.mjs";

test("checkDocConsistency passes across all 12 std modules and 8 required specification docs", () => {
  const res = checkDocConsistency(process.cwd());
  assert.equal(res.passed, true, `Doc mismatch: missing=${JSON.stringify(res.missingDocs)} site=${JSON.stringify(res.siteMissing)}`);
  assert.equal(res.rustModuleCount, 12);
  assert.equal(res.siteModuleCount, 12);
});

test("buildSbomAndLicenseReport confirms Apache-2.0, zero external crates, and zero unsafe core blocks", () => {
  const sbom = buildSbomAndLicenseReport(process.cwd());
  assert.equal(sbom.specFormat, "DOG-SBOM-1.0");
  assert.equal(sbom.packageVersion, "1.0.0");
  assert.equal(sbom.license, "Apache-2.0");
  assert.equal(sbom.externalRustCrates, 0);
  assert.equal(sbom.externalNpmRuntimeDeps, 0);
  assert.equal(sbom.unsafeBlocksInCore, 0);
  assert.ok(sbom.rustModuleInventoryCount >= 20);
});

test("inspectTemplates finds all 5 official starter templates and valid manifests", () => {
  const templates = inspectTemplates(process.cwd());
  assert.equal(templates.length, 5);
  assert.deepEqual(
    templates.map((t) => t.id),
    ["browser-app", "cli-app", "http-service", "library", "tdd-project"]
  );
  assert.ok(templates.every((t) => t.files.includes("dog.pack") && t.files.includes("README.md")));
});

test("runSyntaxParityAndShowcase executes English/Classic/Simple parity and v1_showcase.dg cleanly", () => {
  const res = runSyntaxParityAndShowcase(process.cwd());
  assert.equal(res.parityPassed, true, `Parity output mismatch: ${JSON.stringify(res)}`);
  assert.equal(res.showcasePassed, true, `Showcase failed: ${JSON.stringify(res)}`);
  assert.deepEqual(res.englishOutput, res.classicOutput);
  assert.deepEqual(res.englishOutput, res.simpleOutput);
  assert.ok(res.showcaseOutput.length >= 1);
  assert.match(res.showcaseOutput[0], /v1 Showcase passed/);
});

test("evaluateReadinessGate passes all 5 release readiness checks cleanly", () => {
  const matrix = buildCompatibilityMatrix(process.cwd());
  assert.ok(matrix.filter((m) => m.status === "Supported").length >= 7);

  const report = evaluateReadinessGate(process.cwd());
  assert.equal(report.version, "1.0.0");
  assert.equal(report.gatePassed, true, `Readiness gate failed: ${JSON.stringify(report.checks)}`);
  assert.equal(report.passedCount, 5);
  assert.equal(report.totalChecks, 5);
});
