// Database chores for the server, with DATABASE_URL set:
//   node src/cli.ts migrate            apply migrations
//   node src/cli.ts seed               migrations, then the draft connectors
//   node src/cli.ts promote <email>    make a signed-up account an admin
//   node src/cli.ts adjust <email> <credits> <reason…>
//                                      add (or, negative, take) credits by hand, recorded and audited

import { eq } from "drizzle-orm";

import { audit } from "./audit.ts";
import { credit } from "./ledger.ts";
import { getSettings } from "./settings.ts";
import { connect } from "./db/client.ts";
import { migrate } from "./db/migrate.ts";
import { user } from "./db/schema.ts";
import { seedConnectors } from "./seeds.ts";

const [command, argument, amount, ...words] = process.argv.slice(2);
const url = process.env.DATABASE_URL;
const reason = words.join(" ").trim();
const known =
  command === "migrate" || command === "seed" || (command === "promote" && argument) || (command === "adjust" && argument && amount && reason);
if (!url || !known) {
  console.error("usage: DATABASE_URL=postgres://… node src/cli.ts migrate | seed | promote <email> | adjust <email> <credits> <reason…>");
  process.exit(2);
}

const { db, close } = connect(url, 2);
try {
  await migrate(db);
  console.log("migrations applied");
  if (command === "promote") {
    const rows = await db.update(user).set({ role: "admin" }).where(eq(user.email, argument!.toLowerCase())).returning({ id: user.id });
    if (rows.length === 0) throw new Error(`no account with ${argument}; sign up at the portal first`);
    await audit(db, null, "user.promote", { type: "user", id: rows[0]!.id }, { via: "cli" });
    console.log(`${argument} is an admin; set up two-step sign-in in the portal before using the admin panel`);
  }
  if (command === "adjust") {
    const [row] = await db.select({ id: user.id }).from(user).where(eq(user.email, argument!.toLowerCase()));
    if (!row) throw new Error(`no account with ${argument}`);
    const { creditUnitMicros } = await getSettings(db);
    const micros = Math.round(Number(amount) * creditUnitMicros);
    if (!Number.isFinite(micros) || micros === 0) throw new Error(`not an amount of credits: ${amount}`);
    const wallet = await credit(db, row.id, "adjustment", micros, { note: reason });
    await audit(db, null, "wallet.adjust", { type: "user", id: row.id }, { amountMicros: micros, reason, via: "cli" });
    console.log(`${amount} credits for ${argument}; balance ${wallet.balanceMicros / creditUnitMicros} credits`);
  }
  if (command === "seed") {
    const added = await seedConnectors(db);
    console.log(added.length ? `connectors added as drafts: ${added.join(", ")}` : "connectors already present");
  }
} finally {
  await close();
}
