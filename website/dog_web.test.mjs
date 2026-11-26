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

test("compileDogCss autoprefixes properties and supports CSS minification", () => {
  const dgss = `
rule .glass
    backdrop-filter: blur(8px)
    user-select: none
    pad: 12px
end
`;
  const full = compileDogCss(dgss);
  assert.match(full, /-webkit-backdrop-filter:\s*blur\(8px\);/);
  assert.match(full, /-webkit-user-select:\s*none;/);

  const mini = compileDogCss(dgss, { minify: true });
  assert.equal(mini.includes("\n"), false);
  assert.match(mini, /\.glass\{/);
});

test("compileDogWebsite builds accessible forms with submit preventDefault and JSON diagnostics", () => {
  const dg = `
use "std.web"

define on_sub() returns void
    let ok = web_validate_email("#email")
    dom_set_text("#out", if ok then "valid" else "invalid")
end

let html = web_form("f1", web_field("email", "Email", "a@b.co", "email"), "Send")
let badge = web_badge("Ready", "success")
let alert = web_alert("Info", "Check email", "info")
dom_mount("#app", $"{badge}{html}{alert}")
dom_on("#f1", "submit", "on_sub")
`;
  const res = compileDogWebsite({ code: dg, css: "rule .dg-btn\n  user-select: none\nend", title: "Forms", minify: true });
  assert.equal(res.ok, true);
  assert.match(res.css, /-webkit-user-select:none/);
  assert.equal(res.css.includes("\n"), false);
  assert.match(res.js, /webBadge/);
  assert.match(res.js, /webAlert/);
  assert.match(res.js, /webField/);
  assert.match(res.js, /webForm/);
  assert.match(res.runtimeJs, /preventDefault/);
  const diag = JSON.parse(res.diagnosticsJson);
  assert.equal(diag.ok, true);
  assert.equal(diag.minified, true);
});

test("compileDogCss supports extend inheritance and treeShakeDogCss prunes unused rules", () => {
  const dgss = `
rule .base-panel
    bg: #FFFFFF
    pad: 16px
    radius: 8px
end

rule .accent-panel
    extend: .base-panel
    border: 1px solid #C85A11
end

rule .unused-dead-widget
    bg: #000000
    pad: 40px
end
`;
  const compiled = compileDogCss(dgss);
  assert.match(compiled, /\.accent-panel \{\s+background: #FFFFFF;\s+padding: 16px;\s+border-radius: 8px;\s+border: 1px solid #C85A11;/);

  const dg = `
use "std.web"
dom_mount("#app", "<div class=\\"base-panel accent-panel\\">Active</div>")
`;
  const shaken = compileDogWebsite({ code: dg, css: dgss, title: "Tree Shake", treeShake: true });
  assert.equal(shaken.ok, true);
  assert.ok(shaken.prunedSelectors.includes(".unused-dead-widget"));
  assert.equal(shaken.css.includes(".unused-dead-widget"), false);
  assert.match(shaken.css, /\.accent-panel/);
});

test("compileDogWebsite supports reactive web_state, web_component, web_tabs, dom_delegate, and W101/W102 a11y hints", () => {
  const dg = `
use "std.web"

define render_box(props: map) returns text
    return web_card("Box", map_get(props, "msg"))
end

define on_state_msg(val: text) returns void
    dom_set_html("#host", web_render_component("box", {"msg": val}))
end

define on_tab_click(key: text) returns void
    web_state_set("tab", key)
end

web_component("box", "render_box")
web_on_state("tab", "on_state_msg")
let tabs = web_tabs("tbar", {"a": "Tab A", "b": "Tab B"}, "a")
let init_box = html_el("div", {"id": "host"}, web_render_component("box", {"msg": "init"}))
let img = "<img src=\\"x.png\\">"
dom_mount("#app", $"{tabs}{init_box}{img}")
dom_delegate("#tbar", ".dg-tab-btn", "click", "on_tab_click")
`;
  const res = compileDogWebsite({ code: dg, css: "", title: "Reactive" });
  assert.equal(res.ok, true);
  assert.ok(res.diagnostics.some((d) => d.code === "W101"));

  let clickHandler = null;
  const nodes = {
    "#app": { innerHTML: "" },
    "#host": { innerHTML: "" },
    "#tbar": {
      addEventListener(ev, fn) {
        if (ev === "click") clickHandler = fn;
      },
    },
  };
  const runner = new Function("document", "window", "location", "localStorage", res.runtimeJs);
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

  assert.match(nodes["#app"].innerHTML, /data-tab="b"/);
  assert.equal(typeof clickHandler, "function");
  clickHandler({
    target: {
      closest(sel) {
        return sel === ".dg-tab-btn" ? { dataset: { tab: "b" } } : null;
      },
    },
  });
  assert.match(nodes["#host"].innerHTML, /<h3>Box<\/h3><div>b<\/div>/);
});

test("compileDogWebsite supports Simple Style syntax, slices, pipes, nullish ??, web_table/progress/toast & W103", () => {
  const dg = `
use "std.web"

make count = 1
do double_it(x) = x * 2

do on_go()
    bump count
    when count >= 2
        dom_set_text("#out", $"count={count}")
    end
end

keep nums = [10, 20, 30, 40]
keep mid = nums[1..3]
keep title_txt = "  simple web  " |> trim |> upper
keep cfg = {"mode": "web"}
keep port = cfg["missing"] ?? "3000"
keep tbl = web_table(["Key", "Val"], [["title", title_txt], ["mid", join(mid, ",")], ["port", port], ["d", text(double_it(4))]])
keep prog = web_progress("p1", 2, 4)
keep toast = web_toast("Saved", "success")
keep empty_btn = "<button id=\\"go\\"></button>"
dom_mount("#app", $"<div id=\\"out\\">0</div>{tbl}{prog}{toast}{empty_btn}")
dom_on("#go", "click", "on_go")
`;
  const res = compileDogWebsite({ code: dg, css: "rule .dg-table\n  w: 100%\nend", title: "Simple Web", treeShake: true });
  assert.equal(res.ok, true, res.error || "");
  assert.match(res.js, /const title_txt =/);
  assert.match(res.js, /let count = 1;/);
  assert.match(res.js, /webTable/);
  assert.match(res.js, /webProgress/);
  assert.match(res.js, /webToast/);
  assert.match(res.css, /\.dg-table/);
  assert.ok(res.diagnostics.some((d) => d.code === "W103"), "Expected W103 accessibility warning for empty button");
});

test("compileDogWebsite supports cross-style interop (English + Classic + Simple) and round()", () => {
  const dg = `
use "std.web"

make done_count = 2
keep total_count = 3
do bonus_fn(x) = x + 1
fn triple_cls(x: number) -> number = x * 3
define mix_handler() returns void
    bump done_count
    when done_count >= 3
        dom_set_text("#status", "Done!")
    else if done_count == 2
        dom_set_text("#status", "Almost")
    otherwise
        dom_set_text("#status", "Start")
    end
end

keep [first, second] = [10, 20]
keep pct = round((done_count * 100) / total_count)
keep tag = when pct >= 60 then "ok" else "low"
make sum = 0
for x in 1..3
    add x to sum
end
each y from 1 to 2
    sum += y
end
check pct == 67
show $"pct={pct}|tag={tag}|sum={sum}|tri={triple_cls(bonus_fn(1))}|pair={first}:{second}"
dom_mount("#app", $"<div id=\\"status\\">{pct}% complete</div><button id=\\"go\\">Go</button>")
dom_on("#go", "click", "mix_handler")
`;
  const res = compileDogWebsite({ code: dg, css: "", title: "Interop Web" });
  assert.equal(res.ok, true, res.error || "");

  const printed = [];
  const listeners = new Map();
  const nodes = {
    "#app": { innerHTML: "" },
    "#status": { textContent: "" },
    "#go": { addEventListener(ev, fn) { listeners.set(ev, fn); } },
  };
  const runner = new Function("document", "window", "location", "localStorage", "console", res.runtimeJs);
  runner(
    {
      querySelector(sel) { return nodes[sel] || null; },
      getElementById(id) { return nodes[`#${id}`] || null; },
      title: "",
      body: nodes["#app"],
    },
    {},
    { hash: "", pathname: "/" },
    { getItem: () => "", setItem: () => {} },
    { log: (...a) => printed.push(a.join(" ")) }
  );

  assert.match(nodes["#app"].innerHTML, /67% complete/);
  assert.equal(printed[0], "pct=67|tag=ok|sum=9|tri=6|pair=10:20");
  listeners.get("click")();
  assert.equal(nodes["#status"].textContent, "Done!");
});
