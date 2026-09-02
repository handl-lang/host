import { describe, it, expect } from "vitest";

describe("registerFormatting", () => {
  it("is a function", async () => {
    const mod = await import("./formatting");
    expect(typeof mod.registerFormatting).toBe("function");
  });
});
