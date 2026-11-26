import test from "node:test";
import assert from "node:assert/strict";
import { loadDefaultRoadmap, queryRoadmap } from "./roadmap_engine.mjs";

test("parses all 10,150 items and 5 How-To-Use rules from DOG_v1.0.0_10150_Upgrade_Ideas txt", () => {
  const dataset = loadDefaultRoadmap(process.cwd());
  assert.equal(dataset.total, 10150);
  assert.equal(dataset.howToUse.length, 5);
  assert.match(dataset.howToUse[0].text, /Verify each idea against DOG v0\.3\.0/);
  assert.match(dataset.meta.title, /10,150 Upgrade Ideas/);

  // Verify Part counts sum cleanly to 10,150
  const partSum = Object.values(dataset.countsByPart).reduce((acc, p) => acc + p.total, 0);
  assert.equal(partSum, 10150);
  assert.equal(dataset.countsByPart.A.total, 150);
  assert.equal(dataset.countsByPart.B.total, 50);
  assert.equal(dataset.countsByPart.C.total, 50);
  assert.equal(dataset.countsByPart.D.total, 50);
  assert.equal(dataset.countsByPart.E.total, 9212);
  assert.equal(dataset.countsByPart.F.total, 51);
  assert.equal(dataset.countsByPart.G.total, 587);

  // Verify status counts sum to 10,150 and contain verified v0.3.0 work
  const statusSum =
    dataset.countsByStatus.implemented +
    dataset.countsByStatus.partial +
    dataset.countsByStatus.planned;
  assert.equal(statusSum, 10150);
  assert.ok(dataset.countsByStatus.implemented > 0);
  assert.ok(dataset.countsByStatus.partial > 0);
  assert.ok(dataset.countsByStatus.planned > 0);
  assert.ok(dataset.subsystems.length >= 90);
});

test("queryRoadmap filters by part, status, subsystem, and text search accurately", () => {
  const dataset = loadDefaultRoadmap(process.cwd());

  const partB = queryRoadmap(dataset, { part: "B", page: 1, limit: 25 });
  assert.equal(partB.filteredTotal, 50);
  assert.equal(partB.items.length, 25);
  assert.ok(partB.items.every((it) => it.part === "B"));

  const searchRes = queryRoadmap(dataset, { q: "Atomic update installation", page: 1, limit: 50 });
  assert.ok(searchRes.filteredTotal >= 1);
  assert.equal(searchRes.items[0].title, "Atomic update installation");
  assert.equal(searchRes.items[0].part, "B");

  const statusRes = queryRoadmap(dataset, { status: "implemented", page: 1, limit: 20 });
  assert.equal(statusRes.items.length, 20);
  assert.ok(statusRes.items.every((it) => it.status === "implemented"));
  assert.ok(dataset.countsByStatus.implemented > 300, `Expected >300 implemented items, got ${dataset.countsByStatus.implemented}`);
  const partF = dataset.countsByPart.F;
  assert.ok(partF.implemented + partF.partial >= 30, `Expected >=30 Part F implemented/partial items, got ${partF.implemented + partF.partial}`);
});

