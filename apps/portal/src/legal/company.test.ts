import { describe, expect, it } from "vitest";

import { paymentProvider } from "@/lib/payments";
import { company } from "./company";

const env = (values: Record<string, string>): NodeJS.ProcessEnv => ({ NODE_ENV: "test", ...values });

describe("settings left empty", () => {
  it("keep the legal placeholders, as when they are missing", () => {
    const seller = company(env({ COMPANY_TITLE: "", COMPANY_EMAIL: "", COMPANY_PHONE: "+90 212 000 00 00" }));
    expect(seller.title).toBe("[Şirket unvanı]");
    expect(seller.email).toBe("destek@octopuson.com");
    expect(seller.phone).toBe("+90 212 000 00 00");
  });

  it("keep iyzico on its sandbox", () => {
    const provider = paymentProvider(
      env({ PAYMENT_PROVIDER: "iyzico", IYZICO_API_KEY: "key", IYZICO_SECRET_KEY: "secret", IYZICO_BASE_URL: "" }),
    );
    expect(provider?.kind === "iyzico" && provider.config.baseUrl).toBe("https://sandbox-api.iyzipay.com");
  });
});
