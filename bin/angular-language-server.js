#!/usr/bin/env node

// Zed starts language servers with the project as their working directory,
// even when the command itself belongs to an extension. Angular resolves its
// runtime packages from the explicit probe locations, so add the absolute
// extension-managed node_modules directory before loading the real server.
const path = require("node:path");

const extensionRoot = path.resolve(__dirname, "..");
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

process.argv = [process.execPath, server, ...args];
require(server);
