// Recursive ackermann — same algorithm as handl/ackermann/handl.hnl.template.
// Usage: bun run js.js 2 5   |   node js.js 2 5
function ack(m, n) {
    if (m === 0) return n + 1;
    if (n === 0) return ack(m - 1, 1);
    return ack(m - 1, ack(m, n - 1));
}
const m = parseInt(process.argv[2], 10);
const n = parseInt(process.argv[3], 10);
console.log(ack(m, n));
