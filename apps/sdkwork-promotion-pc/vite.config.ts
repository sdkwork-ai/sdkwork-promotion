import { resolveViteEnvironment, resolveLucideReactEntry } from '../../../sdkwork-specs/tools/vite-runtime-profile.mjs';
import { resolveBrowserDistOutDir } from '../../../sdkwork-specs/tools/browser-dist-layout.mjs';

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig, loadEnv } from "vite";
import { createSdkworkCredentialEntryBootstrapVitePlugin } from '@sdkwork/iam-credential-entry/vite';

const appRoot = path.dirname(fileURLToPath(import.meta.url));
const workspaceRoot = path.resolve(appRoot, "../..");

function forceLocalReactPlugin() {
  const reactRoot = path.join(workspaceRoot, "node_modules/react");
  const reactDomRoot = path.join(workspaceRoot, "node_modules/react-dom");
  return {
    name: "force-local-react",
    enforce: "pre" as const,
    resolveId(source: string) {
      if (source === "react") {
        return path.join(reactRoot, "index.js");
      }
      if (source.startsWith("react/")) {
        return path.join(reactRoot, `${source.slice("react/".length)}.js`);
      }
      if (source === "react-dom") {
        return path.join(reactDomRoot, "index.js");
      }
      if (source.startsWith("react-dom/")) {
        return path.join(reactDomRoot, `${source.slice("react-dom/".length)}.js`);
      }
      return null;
    },
  };
}

function loadTsconfigAliases() {
  const tsconfig = JSON.parse(
    readFileSync(path.join(workspaceRoot, "tsconfig.base.json"), "utf8"),
  );
  const paths = tsconfig?.compilerOptions?.paths ?? {};
  return Object.entries(paths)
    .map(([find, replacements]) => {
      const replacement = Array.isArray(replacements) ? replacements[0] : undefined;
      if (typeof replacement !== "string") {
        return null;
      }
      return {
        find,
        replacement: path.resolve(workspaceRoot, replacement),
      };
    })
    .filter(Boolean)
    .sort((left, right) => right!.find.length - left!.find.length) as Array<{
      find: string;
      replacement: string;
    }>;
}

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, appRoot, "");

  const bootstrapAccessToken = env.SDKWORK_ACCESS_TOKEN ?? process.env.SDKWORK_ACCESS_TOKEN;
  return {
    plugins: [
      // The bootstrap credential reaches the renderer only through the shared IAM
      // plugin (dev-server HTML injection as
      // `globalThis.__SDKWORK_CREDENTIAL_ENTRY_BOOTSTRAP_ACCESS_TOKEN__`).
      // `define['process.env.SDKWORK_ACCESS_TOKEN']` is NOT a valid handoff
      // (IAM_CREDENTIAL_ENTRY_SPEC.md section 4/5).
      createSdkworkCredentialEntryBootstrapVitePlugin({
        accessToken: bootstrapAccessToken,
        environment: resolveViteEnvironment(mode, process.env),
      }),
      forceLocalReactPlugin(), react(),
    ],
    resolve: {
      dedupe: ["react", "react-dom"],
      alias: [
        ...loadTsconfigAliases(),
      ].filter((entry) => entry.find !== "@sdkwork/ui-pc-react" || existsSync(entry.replacement)),
    },
    server: {
      port: 5173,
      host: "127.0.0.1",
    },
    build: {
      outDir: resolveBrowserDistOutDir(resolveViteEnvironment(mode, env)),
    },
  };
});
