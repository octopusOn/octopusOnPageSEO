import type { Dictionary } from "./en";

export const de: Dictionary = {
  meta: {
    title: "octopusSEO Log Analyser — so crawlen Google und KI-Bots Ihre Website",
    description:
      "Kostenlose Desktop-App und CLI für die SEO-Logfile-Analyse: verifizierte Googlebot- und KI-Crawler-Aktivität, Crawl-Budget, Fehler, robots.txt-Konformität und verwaiste Seiten. Windows, macOS und Linux.",
  },
  nav: {
    features: "Funktionen",
    download: "Download",
    docs: "Doku",
    changelog: "Änderungen",
    github: "GitHub",
    menu: "Menü",
    language: "Sprache",
  },
  hero: {
    badge: "Neu: robots.txt-Konformität und KI-Referral-Berichte",
    title: "Ihre Server-Logs wissen,",
    highlight: "was Google und KI-Bots wirklich tun",
    subtitle:
      "octopusSEO Log Analyser macht aus rohen Access-Logs Crawl-Erkenntnisse: welche Seiten Googlebot und KI-Crawler abrufen, wo sie auf Fehler stoßen, welche Bots gefälscht sind und wohin Ihr Crawl-Budget fließt.",
    primary: "Für {os} herunterladen",
    primaryGeneric: "Kostenlos herunterladen",
    secondary: "Funktionen ansehen",
    note: "Kostenlos · Windows, macOS & Linux · Ihre Logs bleiben auf Ihrem Rechner",
  },
  mockup: {
    title: "acme.com — letzte 30 Tage",
    hits: "Verifizierte Such-Hits",
    urls: "Gecrawlte URLs",
    errors: "Crawl-Fehler",
    spoofed: "Gefälschte Bot-Hits",
    chart: "Täglicher Crawl nach Bot",
    table: "Bot",
    verified: "verifiziert",
    fake: "gefälscht",
  },
  formats: {
    title: "Liest die Logs, die Sie schon haben — Format wird automatisch erkannt",
  },
  features: {
    eyebrow: "Alles an einem Ort",
    title: "Von rohen Logzeilen zu SEO-Entscheidungen",
    subtitle: "Von SEOs entwickelt für Agenturen, die jeden Monat Dutzende Websites analysieren.",
    items: {
      verification: {
        title: "Verifizierte Bots statt User-Agents",
        text: "Über 110 Crawler werden erkannt und mit den offiziellen IP-Listen der Betreiber und Reverse-DNS geprüft. Gefälschte Googlebots werden markiert, nicht gezählt.",
      },
      ai: {
        title: "KI-Sichtbarkeit",
        text: "Sehen Sie, was GPTBot, ClaudeBot, PerplexityBot und ChatGPT-User abrufen — und welche Besuche ChatGPT, Perplexity oder Gemini Ihnen schicken.",
      },
      robots: {
        title: "robots.txt-Konformität",
        text: "Welche Bots gesperrte URLs anfragen und welche Ihre robots.txt komplett aussperrt — genau so ausgewertet wie bei Google.",
      },
      budget: {
        title: "Crawl-Budget",
        text: "Crawl-Häufigkeit pro URL und Verzeichnis, Tage seit dem letzten Crawl, Parameter-URLs, Weiterleitungen und statische Dateien, die Budget fressen.",
      },
      orphans: {
        title: "Verwaiste und nicht gecrawlte Seiten",
        text: "Crawler-Exporte und XML-Sitemaps mit den Logs abgleichen; weitere Spalten werden wie per SVERWEIS ergänzt.",
      },
      reports: {
        title: "39 Berichte + Abfragesprache",
        text: "Fertige Berichte und ein einfaches Formular für eigene Filter. Speichern, teilen und als Excel, CSV oder JSON exportieren.",
      },
      geo: {
        title: "Länder & Netzwerke",
        text: "Land und Netzwerk (ASN) jeder Client-IP zeigen Scraper-Clouds und die Herkunft gefälschter Bots. Lokal nachgeschlagen.",
      },
      alerts: {
        title: "Crawl-Warnungen",
        text: "Plötzliche Googlebot-Einbrüche, KI-Crawl-Spitzen und Häufungen von Serverfehlern werden in der Übersicht hervorgehoben.",
      },
    },
  },
  ai: {
    eyebrow: "KI-Suche ist da",
    title: "Erfahren Sie, wann KI-Assistenten Ihre Seiten lesen — und empfehlen",
    text: "Trainings-Crawler, KI-Suchindexer und Echtzeitabrufe von Assistenten werden getrennt erfasst. So sehen Sie, welche Inhalte Modelle füttern, welche zitiert werden und welche Besucher KI-Assistenten zurückschicken.",
    bullets: [
      "Eigene Kategorien für KI-Training, KI-Suche und KI-Assistenten",
      "Fehler, die KI-Bots erhalten — jeder fehlgeschlagene Abruf ist ein verlorenes Zitat",
      "Besuche über ChatGPT, Perplexity, Gemini, Copilot und Claude",
      "Bandbreite und geschätztes CO₂ pro Crawler",
    ],
  },
  terminal: {
    eyebrow: "Automatisieren",
    title: "Ein Befehl für jeden Bericht",
    text: "Die snx-log-CLI teilt die Daten mit der Desktop-App. Monatsberichte skripten, Importe auf Servern planen und direkt nach Excel exportieren.",
    points: ["~750.000 Zeilen pro Sekunde", "Läuft zu 100 % auf Ihrem Rechner"],
  },
  steps: {
    title: "In drei Schritten zur ersten Erkenntnis",
    items: [
      { title: "Logs ablegen", text: "Klartext, gzip, zstd, bzip2 oder zip — Dateien oder ganze Ordner. Erneute Importe sind sicher." },
      { title: "Wir verifizieren die Bots", text: "Jeder Bot-Hit wird mit offiziellen Daten als verifiziert, gefälscht oder unverifiziert eingestuft." },
      { title: "Handeln", text: "Fehler beheben, Crawl-Verschwendung stoppen, Wichtiges freigeben und KI-Sichtbarkeit verfolgen." },
    ],
  },
  stats: {
    bots: "erkannte Crawler",
    formats: "Logformate",
    reports: "fertige Berichte",
    speed: "Zeilen pro Sekunde",
  },
  faq: {
    title: "Häufige Fragen",
    items: [
      {
        q: "Ist octopusSEO Log Analyser wirklich kostenlos?",
        a: "Ja. Desktop-App und CLI sind kostenlos und Open Source, ohne Begrenzung bei Logzeilen oder Websites.",
      },
      {
        q: "Verlassen meine Logs meinen Rechner?",
        a: "Nein. Alles wird lokal gespeichert und analysiert. Heruntergeladen werden nur öffentliche Crawler-IP-Listen und, falls aktiviert, die IP-Geolokalisierungsdatenbank.",
      },
      {
        q: "Welche Logformate werden unterstützt?",
        a: "Nginx und Apache (auch eigene Formate), IIS, CloudFront, AWS-Load-Balancer, HAProxy, Traefik, Caddy, Cloudflare, Google Cloud, JSON- und CSV-Logs. Das Format wird automatisch erkannt.",
      },
      {
        q: "Wie groß dürfen meine Logs sein?",
        a: "Hunderte Millionen Zeilen. Logs werden mit rund 750.000 Zeilen pro Sekunde importiert und komprimiert mit etwa 8 % der Originalgröße gespeichert.",
      },
      {
        q: "Welche Sprachen spricht die App?",
        a: "Die Desktop-App gibt es auf Englisch und Türkisch; Berichte und Exporte funktionieren in beiden gleich.",
      },
    ],
  },
  cta: {
    title: "Sehen Sie Ihre Website mit den Augen des Googlebots",
    text: "App herunterladen, Logdatei ablegen und in Minuten den ersten Bericht erhalten.",
    button: "Kostenlos herunterladen",
  },
  footer: {
    tagline: "SEO-Software, die zeigt, wie Suchmaschinen und KI Ihre Website wirklich sehen.",
    product: "Produkt",
    resources: "Ressourcen",
    company: "Unternehmen",
    logAnalyser: "Log Analyser",
    spider: "SEO Spider",
    comingSoon: "bald",
    contact: "Kontakt",
    issues: "Problem melden",
    rights: "© {year} octopusSEO. Freie Open-Source-Software.",
    geo: "Länder- und Netzwerkdaten: IP Geolocation by DB-IP",
  },
  featuresPage: {
    title: "Funktionen",
    subtitle: "Alles, was der Log Analyser kann, im Detail.",
    sections: {
      bots: {
        title: "Bot-Erkennung und Verifizierung",
        text: "User-Agents lassen sich leicht fälschen. Jeder Bot-Hit wird geprüft, damit Berichte nur echte Crawler zählen.",
        bullets: [
          "Über 110 Crawler in Kategorien: Suche, KI-Training, KI-Suche, KI-Assistenten, Werbung, SEO-Tools, Social, Monitoring",
          "Offizielle IP-Listen von Google, Bing, OpenAI, Anthropic, Perplexity, Apple u. a. sowie Forward-Confirmed-Reverse-DNS",
          "Urteil pro Hit: verifiziert, gefälscht oder unverifiziert — jederzeit neu prüfbar",
          "Eigene Bots: User-Agent-Muster mit IP-Bereichen, IP-Listen-URLs oder Reverse-DNS",
        ],
      },
      ai: {
        title: "KI-Crawler und KI-Suche",
        text: "Verstehen Sie, wie KI-Plattformen Ihre Inhalte nutzen — und was sie zurückschicken.",
        bullets: [
          "Tägliche Aktivität jedes KI-Crawlers und die abgerufenen URLs",
          "Fehler, die KI-Bots und Assistenten erhalten",
          "Von KI-Assistenten vermittelte Besuche mit Einstiegsseiten",
          "Bandbreite und geschätzte Emissionen pro Bot",
        ],
      },
      robots: {
        title: "robots.txt-Konformität",
        text: "Prüfen Sie jede protokollierte Bot-Anfrage gegen Ihre aktuelle robots.txt oder eine beliebige Version.",
        bullets: [
          "Auswertung wie bei Google: spezifischste User-Agent-Gruppe, längster Treffer, bei Gleichstand gewinnt Allow",
          "Gesperrte Anfragen pro Bot, von verifizierten und gefälschten IPs",
          "Komplett ausgesperrte Bots, etwa KI-Trainings-Crawler",
          "Jeden Verstoß mit der entscheidenden Regel exportieren",
        ],
      },
      crawl: {
        title: "Crawl-Budget und technische Gesundheit",
        text: "Finden Sie heraus, wo Crawler Zeit verschwenden und was sie nicht erreichen.",
        bullets: [
          "URL-Übersicht: letzter Status, letzter Crawl, Tage seit dem letzten Crawl, Größe, Tempo und Hits pro Bot",
          "Statuscodes pro URL mit 1xx–5xx-Spalten und inkonsistenten Antworten",
          "Weiterleitungen, 4xx/5xx, langsame und große URLs, Parameter-URLs, statische Dateien",
          "Warnungen bei Crawl-Einbrüchen, -Spitzen und Serverfehlern",
        ],
      },
      urls: {
        title: "Nicht gecrawlte und verwaiste Seiten",
        text: "Verbinden Sie Ihre Crawl-Daten mit dem, was Bots tatsächlich getan haben.",
        bullets: [
          "CSV-Exporte von Crawlern, einfache Listen und XML-Sitemaps (inkl. Sitemap-Indizes)",
          "Gelistet, aber nie gecrawlt — gecrawlt, aber nicht gelistet (verwaist)",
          "Alle weiteren Spalten Ihres Exports neben den Logdaten",
          "Zeitraumvergleich pro URL, Verzeichnis oder Bot — vor und nach einer Migration",
        ],
      },
      data: {
        title: "Ihre Daten, Ihre Regeln",
        text: "Schnell, privat und skriptbar.",
        bullets: [
          "Formate werden automatisch erkannt; eigene Nginx- und Apache-Formate werden unterstützt",
          "Importeinstellungen pro Website: nur Bots, URLs ein-/ausschließen, Parameter entfernen",
          "Abfragesprache mit gespeicherten Abfragen; Export nach Excel, CSV und JSON",
          "Ganze Websites als ein Archiv teilen; Kommandozeilen-Tool für Automatisierung",
        ],
      },
    },
  },
  download: {
    title: "octopusSEO Log Analyser herunterladen",
    subtitle: "Kostenlos für Windows, macOS und Linux. Die CLI für Automatisierung ist enthalten.",
    recommended: "Empfohlen für Ihr System",
    version: "Version {version}",
    released: "Veröffentlicht am {date}",
    totalDownloads: "{count} Downloads",
    desktop: "Desktop-App",
    cli: "Kommandozeile (snx-log)",
    file: "Datei",
    size: "Größe",
    checksum: "SHA-256",
    getIt: "Herunterladen",
    unavailableTitle: "Downloads folgen in Kürze",
    unavailableText:
      "Die erste öffentliche Version wird vorbereitet. Folgen Sie dem Projekt oder bauen Sie es bis dahin aus dem Quellcode.",
    allReleases: "Alle Versionen",
    sourceTitle: "Aus dem Quellcode bauen",
    sourceText: "Benötigt Rust (stable) und für die Desktop-App Node.js 22+ sowie pnpm.",
    requirementsTitle: "Systemanforderungen",
    requirements: [
      "Windows 10/11, macOS 11 oder neuer (Apple Silicon und Intel) oder eine aktuelle Linux-Distribution",
      "4 GB RAM; Speicherplatz von etwa 10 % der rohen Loggröße",
      "Internet nur für Aktualisierungen der Crawler-IP-Listen und Geodaten",
    ],
    platforms: {
      macos: "macOS",
      windows: "Windows",
      linux: "Linux",
    },
    platformNotes: {
      macos: "Erster Start: Meldet macOS, dass die App nicht überprüft werden kann, öffnen Sie Systemeinstellungen → Datenschutz & Sicherheit und klicken Sie auf „Dennoch öffnen“.",
      windows: "Warnt SmartScreen vor einem unbekannten Herausgeber, klicken Sie auf „Weitere Informationen“ → „Trotzdem ausführen“.",
      linux: "AppImage: ausführbar machen (chmod +x) und starten – oder das .deb- bzw. .rpm-Paket installieren.",
    },
    arch: { universal: "Universal", arm64: "ARM64", x64: "x64", arm32: "ARM 32 Bit", appleSilicon: "Apple Silicon", intel: "Intel" },
    kinds: {
      dmg: "Disk-Image",
      exe: "Installationsprogramm",
      msi: "MSI-Paket",
      appimage: "AppImage",
      deb: "Debian-/Ubuntu-Paket",
      rpm: "Fedora-/RHEL-Paket",
      exeOffline: "Installer mit Offline-WebView2",
      cli: "CLI-Archiv",
    },
  },
  docs: {
    title: "Dokumentation",
    subtitle: "In Minuten starten, dann mit der Abfragesprache tiefer einsteigen.",
    quickstart: "Schnellstart",
    quickstartText: "Kunde und Website anlegen, Logs importieren und einen Bericht öffnen. Die Desktop-App erledigt dasselbe mit wenigen Klicks.",
    query: "Abfragesprache",
    queryText: "Jeder Bericht ist eine Abfrage: Bedingungen, danach optional Gruppierung, Kennzahlen, Sortierung und Limit.",
    examples: "Beispiele",
    exampleItems: [
      { query: "bot:googlebot* verification:verified status:4xx date:last30d group:url", text: "Fehler, die verifizierte Googlebots in den letzten 30 Tagen erhielten" },
      { query: "bot_category:ai-* group:bot,day metric:hits", text: "Tägliche Aktivität pro KI-Crawler" },
      { query: "referer_type:ai -bot:* group:url,referer_host", text: "Über KI-Assistenten besuchte Seiten" },
      { query: "verification:spoofed group:bot,as_org,country", text: "Woher gefälschte Bots kommen" },
      { query: "group:url metric:last_status,days_since_last,googlebot_hits", text: "Crawl-Aktualität pro URL" },
    ],
    fields: "Felder",
    metrics: "Kennzahlen",
    more: "Vollständige Referenz auf GitHub",
    guide: "Benutzerhandbuch (Türkisch)",
  },
  changelog: {
    title: "Änderungen",
    subtitle: "Neue Funktionen und Korrekturen jeder Version.",
    empty: "Es wurden noch keine Versionen veröffentlicht.",
    viewOnGithub: "Auf GitHub ansehen",
  },
  notFound: {
    title: "Seite nicht gefunden",
    text: "Diese Seite gibt es nicht — vielleicht hat ein Crawler sie erfunden.",
    home: "Zur Startseite",
  },
  common: {
    copy: "Kopieren",
    copied: "Kopiert",
    learnMore: "Mehr erfahren",
  },
};
