import cloudflare from "@astrojs/cloudflare";
import tailwindcss from "@tailwindcss/vite";
import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://errorscript.org",

  vite: {
    plugins: [tailwindcss()],
  },

  adapter: cloudflare(),
});
