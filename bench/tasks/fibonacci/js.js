// Iterative fibonacci — same algorithm as handl/fibonacci/handl.hnl.template.
// Usage: bun run js.js 35   |   node js.js 35
const n = parseInt(process.argv[2], 10);
let a = 0,
  b = 1;
for (let i = 0; i < n; i++) {
  const tmp = a + b;
  a = b;
  b = tmp;
}
console.log(a);
