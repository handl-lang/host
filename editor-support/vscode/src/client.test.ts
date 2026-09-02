import { describe, it, expect } from "vitest";

describe("createClient", () => {
  it("is a function", async () => {
    const mod = await import("./client");
    expect(typeof mod.createClient).toBe("function");
  });
});
