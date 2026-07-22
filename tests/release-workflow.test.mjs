import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workflow = readFileSync(
  path.join(repoRoot, ".github", "workflows", "release.yml"),
  "utf8",
).replace(/\r\n/g, "\n");

function topLevelBlock(source, name) {
  const match = source.match(new RegExp(`^${name}:\\n((?:^[ \\t]+.*(?:\\n|$))*)`, "m"));
  assert.ok(match, `missing top-level ${name} block`);
  return match[1];
}

test("release concurrency serializes only matching tags without cancellation", () => {
  const concurrency = topLevelBlock(workflow, "concurrency");

  assert.match(
    concurrency,
    /^  group: release-\$\{\{ github\.event_name == 'workflow_dispatch' && inputs\.tag \|\| github\.ref_name \}\}$/m,
  );
  assert.match(concurrency, /^  cancel-in-progress: false$/m);
});
