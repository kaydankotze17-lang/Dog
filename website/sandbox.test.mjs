import test from "node:test";
import assert from "node:assert/strict";
import { runDogSandbox } from "./sandbox.js";

test("executes English-style functions, defaults, loops, and conditionals", () => {
  const code = `
define greet(name: text, title: text = "Friend") returns text
    return $"Hello, {title} {name}!"
end

var points = 0
let badge = "Starter"
print(greet("Alex"))

if badge is "Starter" then
    increase points by 10
elif badge is "Pro" then
    increase points by 25
else
    set points to 5
end

unless points is less than 10 then
    print($"Points = {points}")
end

repeat 2 times
    increase points by 1
end

for step from 1 to 3
    increase points by step
end

var retries = 0
until retries is 2
    increase retries by 1
end

print($"Final points: {points}, retries: {retries}")
`;
  const res = runDogSandbox(code);
  assert.equal(res.ok, true, res.error || "");
  assert.deepEqual(res.output, [
    "Hello, Friend Alex!",
    "Points = 10",
    "Final points: 18, retries: 2",
  ]);
});

test("enforces strict typing (E021) and immutability (E012)", () => {
  const typeErr = runDogSandbox('let age = 20\nprint("Age: " + age)\n');
  assert.equal(typeErr.ok, false);
  assert.equal(typeErr.errorCode, "E021");

  const mutErr = runDogSandbox("let fixed = 5\nincrease fixed by 1\n");
  assert.equal(mutErr.ok, false);
  assert.equal(mutErr.errorCode, "E012");
});

test("emits beginner English hints for !, &&, || and supports try/catch", () => {
  const bangErr = runDogSandbox("let ok = !true\n");
  assert.equal(bangErr.ok, false);
  assert.equal(bangErr.errorCode, "E001");
  assert.ok(bangErr.error.includes("not condition"));

  const caught = runDogSandbox(`
try
    let n = number("oops")
    print(n)
catch err
    print($"Recovered: {err}")
end
`);
  assert.equal(caught.ok, true, caught.error || "");
  assert.equal(caught.output.length, 1);
  assert.ok(caught.output[0].startsWith("Recovered:"));
});
