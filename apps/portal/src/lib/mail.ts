// Account e-mails (verification, password reset, low balance) over SMTP.

import nodemailer, { type Transporter } from "nodemailer";

let transport: Transporter | null | undefined;

function transporter(): Transporter | null {
  if (transport === undefined) transport = process.env.SMTP_URL ? nodemailer.createTransport(process.env.SMTP_URL) : null;
  return transport;
}

export async function sendMail(to: string, subject: string, text: string): Promise<void> {
  const smtp = transporter();
  if (!smtp) {
    if (process.env.NODE_ENV === "production") throw new Error("SMTP_URL is not set");
    console.log(`[mail] to ${to}: ${subject}\n${text}`);
    return;
  }
  await smtp.sendMail({ from: process.env.MAIL_FROM || "OctopusOn SEO <hesap@octopuson.com>", to, subject, text });
}
