#!/usr/bin/env node
// Unified entry point for CodexSwitch CLI.

import { spawn } from "node:child_process";
import { existsSync } from "fs";
import { createRequire } from "module";
import path from "path";
import { fileURLToPath } from "url";

// __dirname equivalent in ESM
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const { platform, arch } = process;
const require = createRequire(import.meta.url);

const PLATFORM_PACKAGES = {
  "linux-x64": "codexswitch-cli-linux-x64",
  "linux-arm64": "codexswitch-cli-linux-arm64",
  "darwin-x64": "codexswitch-cli-darwin-x64",
  "darwin-arm64": "codexswitch-cli-darwin-arm64",
  "win32-x64": "codexswitch-cli-win32-x64",
};

const platformKey = `${platform}-${arch}`;
const platformPackage = PLATFORM_PACKAGES[platformKey];
if (!platformPackage) {
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

let packageJsonPath = null;
try {
  packageJsonPath = require.resolve(`${platformPackage}/package.json`);
} catch (err) {
  throw new Error(
    `Missing platform package ${platformPackage}. Reinstall with npm to download the correct binary.`
  );
}

const packageDir = path.dirname(packageJsonPath);
const codexBinaryName =
  process.platform === "win32" ? "codexswitch-cli.exe" : "codexswitch-cli";
const binaryPath = path.join(packageDir, "bin", codexBinaryName);
if (!existsSync(binaryPath)) {
  throw new Error(
    `Binary not found for ${platformPackage} at ${binaryPath}. Try reinstalling the package.`
  );
}
const invokedPath = process.argv[1] || codexBinaryName;
const invokedName = path.basename(invokedPath).replace(/\.js$/, "");

// Use an asynchronous spawn instead of spawnSync so that Node is able to
// respond to signals (e.g. Ctrl-C / SIGINT) while the native binary is
// executing. This allows us to forward those signals to the child process
// and guarantees that when either the child terminates or the parent
// receives a fatal signal, both processes exit in a predictable manner.

/**
 * Detect the package manager from installation location, not ambient npm_* env
 * variables. Shell/session env can leak bun hints into npm-managed installs.
 */
function detectPackageManager(packageDirectory) {
  if (
    packageDirectory.includes(".bun/install/global") ||
    packageDirectory.includes(".bun\\install\\global")
  ) {
    return "bun";
  }

  return "npm";
}

const env = { ...process.env };
const packageManager = detectPackageManager(packageDir);
const packageManagerEnvVars =
  packageManager === "bun"
    ? ["CODEXSWITCH_CLI_MANAGED_BY_BUN", "CODEX_PROFILES_MANAGED_BY_BUN"]
    : ["CODEXSWITCH_CLI_MANAGED_BY_NPM", "CODEX_PROFILES_MANAGED_BY_NPM"];
for (const name of packageManagerEnvVars) {
  env[name] = "1";
}
if (!env.CODEXSWITCH_CLI_COMMAND && invokedName) {
  env.CODEXSWITCH_CLI_COMMAND = invokedName;
}
if (!env.CODEX_PROFILES_COMMAND && invokedName) {
  env.CODEX_PROFILES_COMMAND = invokedName;
}

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env,
  argv0: invokedName,
});

child.on("error", (err) => {
  // Typically triggered when the binary is missing or not executable.
  // Re-throwing here will terminate the parent with a non-zero exit code
  // while still printing a helpful stack trace.
  // eslint-disable-next-line no-console
  console.error(err);
  process.exit(1);
});

// Forward common termination signals to the child so that it shuts down
// gracefully. In the handler we temporarily disable the default behavior of
// exiting immediately; once the child has been signaled we simply wait for
// its exit event which will in turn terminate the parent (see below).
const forwardSignal = (signal) => {
  if (child.killed) {
    return;
  }
  try {
    child.kill(signal);
  } catch {
    /* ignore */
  }
};

["SIGINT", "SIGTERM", "SIGHUP"].forEach((sig) => {
  process.on(sig, () => forwardSignal(sig));
});

// When the child exits, mirror its termination reason in the parent so that
// shell scripts and other tooling observe the correct exit status.
// Wrap the lifetime of the child process in a Promise so that we can await
// its termination in a structured way. The Promise resolves with an object
// describing how the child exited: either via exit code or due to a signal.
const childResult = await new Promise((resolve) => {
  child.on("exit", (code, signal) => {
    if (signal) {
      resolve({ type: "signal", signal });
    } else {
      resolve({ type: "code", exitCode: code ?? 1 });
    }
  });
});

if (childResult.type === "signal") {
  // Re-emit the same signal so that the parent terminates with the expected
  // semantics (this also sets the correct exit code of 128 + n).
  process.kill(process.pid, childResult.signal);
} else {
  process.exit(childResult.exitCode);
}
