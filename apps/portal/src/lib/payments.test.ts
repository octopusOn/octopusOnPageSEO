import { describe, expect, it } from "vitest";

import { paymentProvider, saleChannel, transferAmount } from "./payments";

const env = (values: Record<string, string>) => values as unknown as NodeJS.ProcessEnv;

describe("bank transfer", () => {
  it("needs the holder and the IBAN, and groups the IBAN in fours", () => {
    expect(paymentProvider(env({ PAYMENT_PROVIDER: "transfer", TRANSFER_HOLDER: "Ada" }))).toBeNull();
    expect(paymentProvider(env({ PAYMENT_PROVIDER: "transfer", TRANSFER_HOLDER: "Ada", TRANSFER_IBAN: "TR00 0000 0000 0000 0000 0000 01", TRANSFER_CURRENCY: "usd" }))).toEqual({
      kind: "transfer",
      config: { bank: "", holder: "Ada", iban: "TR00 0000 0000 0000 0000 0000 01", currency: "USD" },
    });
    const lira = paymentProvider(env({ PAYMENT_PROVIDER: "transfer", TRANSFER_HOLDER: "Ada", TRANSFER_IBAN: "TR000000000000000000000001", TRANSFER_BANK: " Banka " }));
    expect(lira?.kind === "transfer" && lira.config).toEqual({ bank: "Banka", holder: "Ada", iban: "TR00 0000 0000 0000 0000 0000 01", currency: "TRY" });
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
  });
});
