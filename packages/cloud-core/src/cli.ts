// Database chores for the server, with DATABASE_URL set:
//   node src/cli.ts migrate            apply migrations
//   node src/cli.ts seed               migrations, then the draft connectors
//   node src/cli.ts promote <email>    make a signed-up account an admin

import { eq } from "drizzle-orm";

import { audit } from "./audit.ts";
import { connect } from "./db/client.ts";
import { migrate } from "./db/migrate.ts";
import { user } from "./db/schema.ts";
import { seedConnectors } from "./seeds.ts";

const [command, argument] = process.argv.slice(2);
const url = process.env.DATABASE_URL;
const known = command === "migrate" || command === "seed" || (command === "promote" && argument);
if (!url || !known) {
  console.error("usage: DATABASE_URL=postgres://… node src/cli.ts migrate | seed | promote <email>");
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
  if (command === "seed") {
    const added = await seedConnectors(db);
    console.log(added.length ? `connectors added as drafts: ${added.join(", ")}` : "connectors already present");
  }
} finally {
  await close();
}
