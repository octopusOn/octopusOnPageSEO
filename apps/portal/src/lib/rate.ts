// The day's rate for showing prices; null when the bank could not be reached.

import { currentRate, type FxRate } from "@octopusseo/cloud-core";

import { db } from "./db";

export async function rateOrNull(): Promise<FxRate | null> {
  try {
    return await currentRate(db());
  } catch {
    return null;
  }
}
