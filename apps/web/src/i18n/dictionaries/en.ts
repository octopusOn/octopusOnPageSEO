// English interface texts; the shape every other language must match (see `Dictionary`).
// Page content (headings, sections, FAQs, menus) lives in the CMS (apps/cms); these are the
// labels around it. Placeholders such as {version} are filled by `format()`.

export const en = {
  nav: {
    programs: "Programs",
    star: "Star",
    starLabel: "Star OctopusOn SEO on GitHub",
    download: "Download",
    github: "GitHub",
    menu: "Menu",
    language: "Language",
    home: "Home",
    skip: "Skip to content",
    breadcrumb: "Breadcrumb",
  },
  mockup: {
    title: "acme.com — last 30 days",
    hits: "Verified search hits",
    urls: "URLs crawled",
    errors: "Crawl errors",
    spoofed: "Fake bot hits",
    chart: "Daily crawl by bot",
    table: "Bot",
    verified: "verified",
    fake: "spoofed",
  },
  footer: {
    tagline: "SEO software that shows how search engines and AI really see your site.",
    product: "Product",
    resources: "Resources",
    company: "Company",
    contact: "Contact",
    issues: "Report an issue",
    rights: "© {year} OctopusOn SEO. Free and open source software.",
    geo: "Country and network data: IP Geolocation by DB-IP",
  },
  download: {
    onGithub: "All files on GitHub",
    forOs: "Download for {os}",
    version: "Version {version}",
    released: "Released {date}",
    totalDownloads: "{count} downloads",
    desktop: "Desktop app",
    cli: "Command line",
    file: "File",
    size: "Size",
    checksum: "SHA-256",
    getIt: "Download",
    allReleases: "All releases",
    platforms: {
      macos: "macOS",
      windows: "Windows",
      linux: "Linux",
    },
    platformNotes: {
      macos: "First launch: if macOS says it cannot verify the app, open System Settings → Privacy & Security and click “Open Anyway”.",
      windows: "If SmartScreen warns about an unknown publisher, click “More info” → “Run anyway”.",
      linux: "AppImage: make it executable (chmod +x) and run it — or install the .deb or .rpm package.",
    },
    arch: {
      universal: "Universal",
      arm64: "ARM64",
      x64: "x64",
      arm32: "ARM 32-bit",
      appleSilicon: "Apple Silicon",
      intel: "Intel",
    },
    kinds: {
      dmg: "Disk image",
      exe: "Installer",
      msi: "MSI package",
      appimage: "AppImage",
      deb: "Debian / Ubuntu package",
      rpm: "Fedora / RHEL package",
      exeOffline: "Installer with offline WebView2",
      cli: "CLI archive",
    },
  },
  productPage: {
    download: "Download {product}",
    notReleasedTitle: "Not released yet",
    notReleasedText: "The first version is being prepared. You can build it from source today, or follow the releases on GitHub.",
  },
  changelog: {
    empty: "No releases have been published yet.",
    viewOnGithub: "View on GitHub",
  },
  showcase: {
    app: "App",
    cli: "Command line",
  },
  calculator: {
    total: "Estimated total",
    credits: "credits",
  },
  compare: {
    feature: "Feature",
    yes: "Yes",
    partial: "Partly or paid",
    no: "No",
  },
  listing: {
    empty: "Nothing has been published here yet.",
    newer: "Newer",
    older: "Older",
    page: "Page {page} of {pages}",
    readMore: "Read more",
  },
  preview: {
    banner: "Preview: this page shows unpublished content.",
    exit: "Exit preview",
  },
  notFound: {
    title: "Page not found",
    text: "This page does not exist — maybe a crawler made it up.",
    home: "Back to the home page",
  },
  common: {
    freeOpenSource: "100% free · open source",
    copy: "Copy",
    copied: "Copied",
    learnMore: "Learn more",
  },
};

export type Dictionary = typeof en;
