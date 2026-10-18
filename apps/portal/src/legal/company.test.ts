import { describe, expect, it } from "vitest";

import { paymentProvider } from "@/lib/payments";
import { company, fillCompany } from "./company";

const env = (values: Record<string, string>): NodeJS.ProcessEnv => ({ NODE_ENV: "test", ...values });

describe("the legal texts' seller", () => {
  it("fills the CMS's fields and names the missing ones in the page's language", () => {
    const seller = company(env({ COMPANY_TITLE: "", COMPANY_EMAIL: "", COMPANY_PHONE: "+90 212 000 00 00", BETTER_AUTH_URL: "https://my.octopuson.com/" }));
    const text = "- {{company.title}}\n- {{company.phone}} · {{ company.email }}\n- {{company.site}} {{company.unknown}}";

    expect(fillCompany(text, seller, { title: "Company name", phone: "Phone" })).toBe(
      "- [Company name]\n- +90 212 000 00 00 · destek@octopuson.com\n- my.octopuson.com {{company.unknown}}",
    );
  });

  it("keeps iyzico on its sandbox when the address is left empty", () => {
    const provider = paymentProvider(
      env({ PAYMENT_PROVIDER: "iyzico", IYZICO_API_KEY: "key", IYZICO_SECRET_KEY: "secret", IYZICO_BASE_URL: "" }),
    );
    expect(provider?.kind === "iyzico" && provider.config.baseUrl).toBe("https://sandbox-api.iyzipay.com");
  });
});
