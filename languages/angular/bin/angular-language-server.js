#!/usr/bin/env node

// Zed starts language servers with the project as their working directory,
// even when the command itself belongs to an extension. Angular resolves its
// runtime packages from the explicit probe locations, so add the absolute
// extension-managed node_modules directory before loading the real server.
const path = require("node:path");
const { spawn } = require("node:child_process");

const extensionRoot = path.resolve(__dirname, "../../..");
const managedNodeModules = path.join(extensionRoot, "node_modules");
const server = path.join(
  managedNodeModules,
  "@angular",
  "language-server",
  "index.js",
);
const args = process.argv.slice(2);

for (const flag of ["--tsProbeLocations", "--ngProbeLocations"]) {
  const index = args.indexOf(flag);
  if (index !== -1 && index + 1 < args.length) {
    args[index + 1] = `${args[index + 1]},${managedNodeModules}`;
  }
}

// Start a fresh Node process rather than loading the server with `require()`.
// The Angular CLI parses the real process argv during startup; giving it an
// actual child-process argv also avoids depending on whether a particular
// server release snapshots `process.argv` before its entry point is loaded.
const child = spawn(process.execPath, [server, ...args], {
  stdio: "inherit",
  env: process.env,
});

child.on("error", (error) => {
  console.error(error);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    // Keep signal failures visible to Zed even on platforms where forwarding
    // the original signal is not supported by Node.
    console.error(`Angular language server terminated by ${signal}`);
  }
  process.exit(code ?? 1);
});

for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => child.kill(signal));
}
