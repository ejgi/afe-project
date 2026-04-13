"use strict";
import { defineConfig } from 'vite';
import { svelte, vitePreprocess } from '@sveltejs/vite-plugin-svelte';
export default defineConfig({
    plugins: [svelte({
            preprocess: vitePreprocess()
        })],
    build: {
        outDir: 'out/webview',
        rollupOptions: {
            input: 'webview-ui/index.html',
            output: {
                entryFileNames: `[name].js`,
                chunkFileNames: `[name].js`,
                assetFileNames: `[name].[ext]`
            }
        }
    }
});
//# sourceMappingURL=vite.config.mjs.map