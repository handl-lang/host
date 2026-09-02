import { describe, it, expect } from "vitest";

describe("registerLanguageConfiguration", () => {
  it("is a function", async () => {
    const mod = await import("./language-config");
    expect(typeof mod.registerLanguageConfiguration).toBe("function");
  });
});
