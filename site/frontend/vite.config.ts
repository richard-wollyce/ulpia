import { defineConfig, type Plugin } from "vite";
import { resolve } from "node:path";

// Preload the body face (400 normal) only. Its URL carries a content hash that
// exists only at build time, so the tag is injected here rather than written in
// the HTML. The italic and 600 faces are not preloaded: they appear below the
// fold and may arrive with the stylesheet.
function preloadBodyFace(): Plugin {
  return {
    name: "preload-body-face",
    transformIndexHtml: {
      order: "post",
      handler(html, ctx) {
        const file = Object.keys(ctx.bundle ?? {}).find((name) =>
          /eb-garamond-latin-400-normal.*\.woff2$/.test(name),
        );
        if (!file) return html;
        return {
          html,
          tags: [
            {
              tag: "link",
              attrs: {
                rel: "preload",
                as: "font",
                type: "font/woff2",
                // Required on font preloads even same-origin, per the spec.
                crossorigin: "",
                href: "/" + file,
              },
              injectTo: "head-prepend",
            },
          ],
        };
      },
    },
  };
}

// Two pages, no framework, and the built output contains no script tag: the
// design spec's decision 0.1. The build exists to compile the stylesheet,
// hash the asset filenames (which is what makes the server's `immutable`
// cache header honest), and minify.
export default defineConfig({
  build: {
    rollupOptions: {
      input: {
        index: resolve(__dirname, "index.html"),
        404: resolve(__dirname, "404.html"),
      },
    },
  },
  plugins: [preloadBodyFace()],
});
