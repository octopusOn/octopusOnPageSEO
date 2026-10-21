import { describe, expect, it } from "vitest";

import { manualNote, paymentProvider, saleChannel, transferAmount } from "./payments";

const env = (values: Record<string, string>) => values as unknown as NodeJS.ProcessEnv;

describe("bank transfer", () => {
  it("needs the holder and the IBAN, and groups the IBAN in fours", () => {
    expect(paymentProvider(env({ PAYMENT_PROVIDER: "transfer", TRANSFER_HOLDER: "Ada" }))).toBeNull();
    expect(paymentProvider(env({ PAYMENT_PROVIDER: "transfer", TRANSFER_HOLDER: "Ada", TRANSFER_IBAN: "TR00 0000 0000 0000 0000 0000 01", TRANSFER_CURRENCY: "usd" }))).toEqual({
      kind: "transfer",
      config: { bank: { name: "", holder: "Ada", iban: "TR00 0000 0000 0000 0000 0000 01" }, payoneer: null, currency: "USD" },
    });
    const lira = paymentProvider(env({ PAYMENT_PROVIDER: "transfer", TRANSFER_HOLDER: "Ada", TRANSFER_IBAN: "TR000000000000000000000001", TRANSFER_BANK: " Banka " }));
    expect(lira?.kind === "transfer" && lira.config).toEqual({ bank: { name: "Banka", holder: "Ada", iban: "TR00 0000 0000 0000 0000 0000 01" }, payoneer: null, currency: "TRY" });
  });

  it("takes a Payoneer payment link alone or beside the bank, and nothing else", () => {
    const link = "https://link.payoneer.com/Token?t=ABC&src=pl";
    const only = paymentProvider(env({ PAYMENT_PROVIDER: "transfer", PAYONEER_LINK: link }));
    expect(only?.kind === "transfer" && only.config).toEqual({ bank: null, payoneer: link, currency: "TRY" });
    for (const wrong of ["http://payoneer.com/payme/ada", "https://payoneer.com.evil.example/x", "javascript:alert(1)"]) {
      expect(paymentProvider(env({ PAYMENT_PROVIDER: "transfer", PAYONEER_LINK: wrong }))).toBeNull();
    }
    const texts = { transferNote: "bank", payoneerNote: "payoneer", transferPayoneerNote: "both" };
    const both = paymentProvider(env({ PAYMENT_PROVIDER: "transfer", TRANSFER_HOLDER: "Ada", TRANSFER_IBAN: "TR01", PAYONEER_LINK: link }));
    expect(both?.kind === "transfer" && manualNote(both.config, texts)).toBe("both");
    expect(only?.kind === "transfer" && manualNote(only.config, texts)).toBe("payoneer");
    const bank = paymentProvider(env({ PAYMENT_PROVIDER: "transfer", TRANSFER_HOLDER: "Ada", TRANSFER_IBAN: "TR01" }));
    expect(bank?.kind === "transfer" && manualNote(bank.config, texts)).toBe("bank");
  });

  it("sends lira as priced, or dollars at the order's rate rounded up to the cent", () => {
    const order = { amountKurus: 120_000, fxRate: "41.25" };
    expect(transferAmount(order, "TRY")).toEqual({ currency: "TRY", cents: 120_000 });
    // 1,200 TRY / 41.25 = 29.0909… USD
    expect(transferAmount(order, "USD")).toEqual({ currency: "USD", cents: 2910 });
  });

  it("tells the invoice how the order was paid", () => {
    expect(saleChannel("transfer")).toEqual({ platform: "Havale/EFT", paymentType: "EFT/HAVALE" });
    expect(saleChannel("paytr")).toEqual({ platform: "PayTR", paymentType: "ODEMEARACISI" });
    expect(saleChannel("iyzico")).toEqual({ platform: "iyzico", paymentType: "ODEMEARACISI" });
    expect(saleChannel("payoneer")).toEqual({ platform: "Payoneer", paymentType: "ODEMEARACISI" });
  });
});
