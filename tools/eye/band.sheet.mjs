/* The band, on every page that carries it. The sheet is the review's scope, so
 * a target missing from here is a target nobody looked at. */
export default {
  base: "http://localhost:5173",
  pages: [
    { name: "home", path: "/" },
    { name: "terms", path: "/terms/" },
    { name: "blog", path: "/blog/" },
  ],
  targets: [
    /* The anchor only, never the <p>. A comma selector plus .first() returns
     * the <p class="wordmark"> in DOM order even on pages where the link
     * exists, so the review silently tested a non-interactive element and
     * reported its missing hover as a defect. On the front page the wordmark
     * is not a link at all, so this target correctly matches nothing there. */
    { name: "wordmark-link", selector: ".wordmark a" },
    { name: "nav-writing", selector: ".nav a" },
    { name: "lamp", selector: ".lamp-toggle" },
  ],
  widths: [320, 360, 768, 1280],
  schemes: ["light", "dark"],
};
