#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const cargoPackageName = "codexswitch-cli";

function usage() {
  console.log(`Usage: npm run check:release-version -- [version-or-tag]
       npm run check:release-version -- --expected <version-or-tag>

Checks that Cargo.toml, Cargo.lock, package.json, and the npm platform optional
dependencies all use the same release version. A leading "v" is accepted for
the optional expected version.`);
}

function parseExpectedVersion(args) {
  let expected;

  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index];

    if (arg === "-h" || arg === "--help") {
      usage();
      process.exit(0);
    }

    if (arg === "--expected") {
      if (expected !== undefined || index + 1 >= args.length) {
        throw new Error("--expected requires exactly one version or tag");
      }
      expected = args[index + 1];
      index += 1;
      continue;
    }

    if (arg.startsWith("--expected=")) {
      if (expected !== undefined) {
        throw new Error("expected version was provided more than once");
      }
      expected = arg.slice("--expected=".length);
      continue;
    }

    if (arg.startsWith("-")) {
      throw new Error(`unknown option: ${arg}`);
    }

    if (expected !== undefined) {
      throw new Error("expected version was provided more than once");
    }
    expected = arg;
  }

  if (expected === undefined) {
    return undefined;
  }

  const normalized = expected.startsWith("v") ? expected.slice(1) : expected;
  if (normalized.length === 0) {
    throw new Error("expected version or tag must not be empty");
  }
  return normalized;
}

function cargoTomlVersion(contents) {
  const lines = contents.split(/\r?\n/);
  const packageStart = lines.findIndex((line) => line.trim() === "[package]");
  const packageEnd = lines.findIndex(
    (line, index) => index > packageStart && /^\s*\[/.test(line),
  );
  const packageSection =
    packageStart >= 0
      ? lines.slice(packageStart + 1, packageEnd >= 0 ? packageEnd : undefined).join("\n")
      : "";
  const version = packageSection.match(/^\s*version\s*=\s*"([^"]+)"\s*$/m);

  if (!version) {
    throw new Error("Cargo.toml does not contain [package].version");
  }
  return version[1];
}

function cargoLockVersion(contents) {
  const matches = [];
  const packages = contents.split(/^\[\[package\]\]\s*$/m).slice(1);

  for (const packageSection of packages) {
    const name = packageSection.match(/^\s*name\s*=\s*"([^"]+)"\s*$/m)?.[1];
    if (name !== cargoPackageName) {
      continue;
    }

    const version = packageSection.match(/^\s*version\s*=\s*"([^"]+)"\s*$/m)?.[1];
    if (!version) {
      throw new Error(`Cargo.lock package ${cargoPackageName} has no version`);
    }
    matches.push(version);
  }

  if (matches.length !== 1) {
    throw new Error(
      `Cargo.lock must contain exactly one ${cargoPackageName} package; found ${matches.length}`,
    );
  }
  return matches[0];
}

function addMismatch(errors, source, actual, expected) {
  if (actual !== expected) {
    errors.push(`${source} is ${JSON.stringify(actual)}; expected ${JSON.stringify(expected)}`);
  }
}

async function main() {
  const expectedVersion = parseExpectedVersion(process.argv.slice(2));
  const [cargoToml, cargoLock, packageJsonText] = await Promise.all([
    readFile("Cargo.toml", "utf8"),
    readFile("Cargo.lock", "utf8"),
    readFile("package.json", "utf8"),
  ]);

  const packageJson = JSON.parse(packageJsonText);
  const manifestVersion = cargoTomlVersion(cargoToml);
  const lockVersion = cargoLockVersion(cargoLock);
  const npmVersion = packageJson.version;
  const optionalDependencies = packageJson.optionalDependencies;
  const npmOptionalDependencies =
    optionalDependencies && typeof optionalDependencies === "object"
      ? Object.entries(optionalDependencies)
      : [];
  const errors = [];

  addMismatch(errors, "Cargo.lock package version", lockVersion, manifestVersion);

  if (typeof npmVersion !== "string" || npmVersion.length === 0) {
    errors.push("package.json version must be a non-empty string");
  } else {
    addMismatch(errors, "package.json version", npmVersion, manifestVersion);
  }

  if (npmOptionalDependencies.length === 0) {
    errors.push("package.json must contain optionalDependencies");
  }
  for (const [name, version] of npmOptionalDependencies) {
    addMismatch(
      errors,
      `package.json optionalDependencies[${JSON.stringify(name)}]`,
      version,
      manifestVersion,
    );
  }

  if (expectedVersion !== undefined) {
    addMismatch(errors, "expected release version", expectedVersion, manifestVersion);
  }

  if (errors.length > 0) {
    console.error("Release version consistency check failed:");
    for (const error of errors) {
      console.error(`- ${error}`);
    }
    process.exitCode = 1;
    return;
  }

  console.log(`Release version metadata is consistent at ${manifestVersion}.`);
}

main().catch((error) => {
  console.error(`Release version consistency check failed: ${error.message}`);
  process.exitCode = 1;
});
