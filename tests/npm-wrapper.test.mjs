import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const wrapperPath = path.join(repoRoot, "bin", "codexswitch-cli.js");
const wrapperUrl = pathToFileURL(wrapperPath).href;

function runInline(source) {
  return spawnSync(process.execPath, ["--input-type=module", "-e", source], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { ...process.env },
  });
}

function withTempDir(t) {
  const dir = mkdtempSync(path.join(tmpdir(), "codexswitch-wrapper-test."));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  return dir;
}

function writePlatformPackage(t, packageName, files = {}) {
  const dir = withTempDir(t);
  const packageDir = path.join(dir, ...packageName.split("/"));
  mkdirSync(path.join(packageDir, "bin"), { recursive: true });
  writeFileSync(
    path.join(packageDir, "package.json"),
    JSON.stringify({ name: packageName, version: "0.0.0-test" }),
  );

  for (const [relativePath, content] of Object.entries(files)) {
    const filePath = path.join(packageDir, relativePath);
    mkdirSync(path.dirname(filePath), { recursive: true });
    writeFileSync(filePath, content);
    chmodSync(filePath, 0o755);
  }

  return path.join(packageDir, "package.json");
}

function resolverPatch(packageName, packageJsonPath) {
  return `
import Module from "node:module";
const originalResolve = Module._resolveFilename;
Module._resolveFilename = function(request, parent, isMain, options) {
  if (request === ${JSON.stringify(`${packageName}/package.json`)}) {
    return ${JSON.stringify(packageJsonPath)};
  }
  return originalResolve.call(this, request, parent, isMain, options);
};
`;
}

test("fails with a clear error on unsupported platform/arch", () => {
  const result = runInline(`
Object.defineProperty(process, "platform", { value: "plan9" });
Object.defineProperty(process, "arch", { value: "mips64" });
await import(${JSON.stringify(wrapperUrl)});
`);

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Unsupported platform: plan9 \(mips64\)/);
});

test("fails with a clear error when the optional platform package is missing", () => {
  const packageName = "@syntaxskills/codexswitch-cli-linux-x64";
  const result = runInline(`
import Module from "node:module";
const originalResolve = Module._resolveFilename;
Module._resolveFilename = function(request, parent, isMain, options) {
  if (request === ${JSON.stringify(`${packageName}/package.json`)}) {
    const error = new Error("Cannot find module " + request);
    error.code = "MODULE_NOT_FOUND";
    throw error;
  }
  return originalResolve.call(this, request, parent, isMain, options);
};
Object.defineProperty(process, "platform", { value: "linux" });
Object.defineProperty(process, "arch", { value: "x64" });
await import(${JSON.stringify(wrapperUrl)});
`);

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /Missing platform package @syntaxskills\/codexswitch-cli-linux-x64/,
  );
});

test("fails with a clear error when the platform binary is missing", (t) => {
  const packageName = "@syntaxskills/codexswitch-cli-linux-x64";
  const packageJsonPath = writePlatformPackage(t, packageName);
  const result = runInline(`
${resolverPatch(packageName, packageJsonPath)}
Object.defineProperty(process, "platform", { value: "linux" });
Object.defineProperty(process, "arch", { value: "x64" });
await import(${JSON.stringify(wrapperUrl)});
`);

  assert.notEqual(result.status, 0);
  assert.match(
    result.stderr,
    /Binary not found for @syntaxskills\/codexswitch-cli-linux-x64/,
  );
});

test("uses the Windows .exe binary path when constructing the child process", (t) => {
  const packageName = "@syntaxskills/codexswitch-cli-win32-x64";
  const packageJsonPath = writePlatformPackage(t, packageName, {
    "bin/codexswitch-cli.exe": "native fixture placeholder",
  });
  const result = runInline(`
${resolverPatch(packageName, packageJsonPath)}
import childProcess from "node:child_process";
import { syncBuiltinESMExports } from "node:module";
import { EventEmitter } from "node:events";

childProcess.spawn = function(binaryPath, args, options) {
  console.log(JSON.stringify({
    binaryPath,
    args,
    managedByNpm: options.env.CODEXSWITCH_CLI_MANAGED_BY_NPM,
    command: options.env.CODEXSWITCH_CLI_COMMAND,
    argv0: options.argv0,
  }));
  const child = new EventEmitter();
  child.killed = false;
  child.kill = () => { child.killed = true; };
  process.nextTick(() => child.emit("exit", 0, null));
  return child;
};
syncBuiltinESMExports();

Object.defineProperty(process, "platform", { value: "win32" });
Object.defineProperty(process, "arch", { value: "x64" });
process.argv = [process.execPath, "/tmp/codexswitch-cli.js", "--profile", "work"];
await import(${JSON.stringify(wrapperUrl)});
`);

  assert.equal(result.status, 0, result.stderr);
  const spawnCall = JSON.parse(result.stdout.trim());
  assert.match(spawnCall.binaryPath, /codexswitch-cli\.exe$/);
  assert.deepEqual(spawnCall.args, ["--profile", "work"]);
  assert.equal(spawnCall.managedByNpm, "1");
  assert.equal(spawnCall.command, "codexswitch-cli");
  assert.equal(spawnCall.argv0, "codexswitch-cli");
});

test("passes through the wrapped binary exit code", (t) => {
  const packageName = "@syntaxskills/codexswitch-cli-linux-x64";
  const packageJsonPath = writePlatformPackage(t, packageName, {
    "bin/codexswitch-cli": "native fixture placeholder",
  });
  const result = runInline(`
${resolverPatch(packageName, packageJsonPath)}
import childProcess from "node:child_process";
import { syncBuiltinESMExports } from "node:module";
import { EventEmitter } from "node:events";

childProcess.spawn = function(binaryPath, args, options) {
  console.log(JSON.stringify({ binaryPath, args, argv0: options.argv0 }));
  const child = new EventEmitter();
  child.killed = false;
  child.kill = () => { child.killed = true; };
  process.nextTick(() => child.emit("exit", 37, null));
  return child;
};
syncBuiltinESMExports();

Object.defineProperty(process, "platform", { value: "linux" });
Object.defineProperty(process, "arch", { value: "x64" });
process.argv = [process.execPath, "/tmp/codexswitch-cli.js", "doctor", "--json"];
await import(${JSON.stringify(wrapperUrl)});
`);

  assert.equal(result.status, 37, result.stderr);
  const spawnCall = JSON.parse(result.stdout.trim());
  assert.match(spawnCall.binaryPath, /codexswitch-cli$/);
  assert.deepEqual(spawnCall.args, ["doctor", "--json"]);
  assert.equal(spawnCall.argv0, "codexswitch-cli");
});
