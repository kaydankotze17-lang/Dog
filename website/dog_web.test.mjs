import test from "node:test";
import assert from "node:assert/strict";
import { compileDogCss, compileDogWebsite } from "./dog_web.js";

test("compileDogCss lowers vars, shorthands, nested hover, keyframes, and mobile breakpoints", () => {
  const dgss = `
vars
    bg: #F7F6F2
    accent: #C85A11
end

rule .card
    bg: #FFFFFF
    ink: #181A1F
    pad: 20px
    radius: 10px
    shadow: sm
    border: 1px solid $accent
end

rule .btn
    bg: $accent
    px: 16px
    py: 10px
    on hover
        bg: #A6480B
    end
end

animate pop
    from
        opacity: 0
    end
    to
        opacity: 1
    end
end

on mobile
    rule .grid
        cols: 1
    end
end
`;

  const css = compileDogCss(dgss);
  assert.match(css, /:root \{\s+--bg: #F7F6F2;\s+--accent: #C85A11;\s+\}/);
  assert.match(css, /background: #FFFFFF;/);
  assert.match(css, /color: #181A1F;/);
  assert.match(css, /padding: 20px;/);
  assert.match(css, /border: 1px solid var\(--accent\);/);
  assert.match(css, /\.btn:hover \{\s+background: #A6480B;\s+\}/);
  assert.match(css, /@keyframes pop \{/);
  assert.match(css, /@media \(max-width: 768px\) \{\s+\.grid \{\s+display: grid;\s+grid-template-columns: repeat\(1, minmax\(0, 1fr\)\);/);
});

test("compileDogCss reports E005 on unclosed block", () => {
  assert.throws(
    () => compileDogCss("rule .card\n  bg: #FFF"),
    (err) => err.code === "E005" && /Unclosed/.test(err.title)
  );
});

test("compileDogWebsite builds interactive DOM website and executes click handlers cleanly", () => {
  const dg = `
use "std.web"

var clicks = 0

define on_inc() returns void
    increase clicks by 1
    dom_set_text("#count", text(clicks))
    dom_toggle_class("#card", "is-active")
end

let ui = web_card("Counter", $"<span id=\\"count\\">0</span> " + web_button("go", "+1"))
dom_mount("#app", ui)
dom_on("#go", "click", "on_inc")
`;

  const dgss = `
rule .dg-card
    bg: #FFFFFF
    pad: 16px
    radius: 8px
end
`;

  const res = compileDogWebsite({ code: dg, css: dgss, title: "Counter App" });
  assert.equal(res.ok, true);
  assert.match(res.indexHtml, /<title>Counter App<\/title>/);
  assert.match(res.css, /border-radius: 8px;/);
  assert.match(res.js, /__dogFns\["on_inc"\] = on_inc;/);

  // Execute transpiled runtime JS against a minimal mock DOM in Node
  const listeners = new Map();
  const nodes = {
    "#app": { innerHTML: "" },
    "#count": { textContent: "0" },
    "#card": {
      classes: new Set(),
      classList: {
        add(c) { nodes["#card"].classes.add(c); },
        remove(c) { nodes["#card"].classes.delete(c); },
        toggle(c) {
          if (nodes["#card"].classes.has(c)) {
            nodes["#card"].classes.delete(c);
            return false;
          }
          nodes["#card"].classes.add(c);
          return true;
        },
      },
    },
    "#go": {
      addEventListener(ev, fn) {
        listeners.set(`#go:${ev}`, fn);
      },
    },
  };

  const runner = new Function(
    "document",
    "window",
    "location",
    "localStorage",
    res.runtimeJs
  );
  runner(
    {
      querySelector(sel) { return nodes[sel] || null; },
      getElementById(id) { return nodes[`#${id}`] || null; },
      title: "",
      body: nodes["#app"],
    },
    {},
    { hash: "", pathname: "/" },
    { getItem: () => "", setItem: () => {} }
  );

  assert.match(nodes["#app"].innerHTML, /Counter/);
  assert.equal(typeof listeners.get("#go:click"), "function");
  listeners.get("#go:click")();
  assert.equal(nodes["#count"].textContent, "1");
  assert.equal(nodes["#card"].classes.has("is-active"), true);
});

test("compileDogWebsite catches strict typing E021 on text + number", () => {
  const res = compileDogWebsite({
    code: 'let n = 5\nprint("Count: " + n)',
    css: "",
    title: "Bad Type",
  });
  assert.equal(res.ok, false);
  assert.equal(res.errorCode, "E021");
});

test("compileDogWebsite compiles inline style theme, hover, and mobile blocks + css_bundle()", () => {
  const dg = `
use "std.web"

style theme
    bg: "#F7F6F2"
    accent: "#C85A11"
end

style ".dg-card"
    bg: "$bg"
    pad: "20px"
    radius: "10px"
    shadow: "sm"
    on hover
        shadow: "md"
    end
    on mobile
        pad: "12px"
    end
end

let compiled = css_bundle()
print(compiled)
`;
  const res = compileDogWebsite({ code: dg, css: "", title: "Inline Style App" });
  assert.equal(res.ok, true);
  assert.match(res.css, /:root \{\s+--bg: #F7F6F2;\s+--accent: #C85A11;\s+\}/);
  assert.match(res.css, /\.dg-card \{\s+background: var\(--bg\);\s+padding: 20px;/);
  assert.match(res.css, /\.dg-card:hover \{/);
  assert.match(res.css, /@media \(max-width: 640px\) \{\s+\.dg-card \{\s+padding: 12px;/);
  assert.match(res.indexHtml, /<style id="dog-style-sheet">/);
});
