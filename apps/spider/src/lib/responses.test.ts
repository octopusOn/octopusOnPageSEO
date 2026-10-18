import { expect, it } from "vitest";
import { responseGroup } from "./responses";

it("keeps HTTP outcomes separate from missing responses and diagnostic flags", () => {
  expect(responseGroup(null, null, false)).toBe("no-response");
  expect(responseGroup(null, "timeout", false)).toBe("request-error");
  expect(responseGroup(null, "timeout", true)).toBe("robots-blocked");
  expect(responseGroup(200, "diagnostic", true)).toBe("success");
  expect(responseGroup(301, "diagnostic", true)).toBe("redirects");
  expect(responseGroup(404, null, false)).toBe("client-error");
  expect(responseGroup(503, null, false)).toBe("server-error");
  expect(responseGroup(199, null, false)).toBe("other-http");
  expect(responseGroup(600, null, false)).toBe("other-http");
});
