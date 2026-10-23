import { describe, expect, it } from "vitest";
import { pathWords, redirectFor } from "./links";

describe("link work", () => {
  it("reads the words of an address", () => {
    expect(pathWords("https://ornek.com/Spor-Ayakkab%C4%B1/kad%C4%B1n-123.html")).toEqual(["spor", "ayakkabi", "kadin"]);
  });

  it("suggests the closest live page for a broken one, else the home page", () => {
    const live = ["https://ornek.com/spor-ayakkabi", "https://ornek.com/spor-ayakkabi/kadin", "https://ornek.com/blog/kosu"];
    expect(redirectFor("https://ornek.com/eski/spor-ayakkabi-kadin-modelleri", live)).toBe("https://ornek.com/spor-ayakkabi/kadin");
    expect(redirectFor("https://ornek.com/kampanya-2019", live)).toBe("https://ornek.com/");
  });
});
