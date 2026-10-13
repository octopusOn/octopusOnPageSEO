import type { Dictionary } from "./en";

export const es: Dictionary = {
  meta: {
    title: "octopusSEO Log Analyser — cómo rastrean tu web Google y los bots de IA",
    description:
      "App de escritorio y CLI gratuitas para el análisis SEO de logs: actividad verificada de Googlebot y rastreadores de IA, presupuesto de rastreo, errores, cumplimiento de robots.txt y páginas huérfanas. Windows, macOS y Linux.",
  },
  nav: {
    features: "Funciones",
    download: "Descargar",
    docs: "Documentación",
    changelog: "Novedades",
    github: "GitHub",
    menu: "Menú",
    language: "Idioma",
  },
  hero: {
    badge: "Nuevo: cumplimiento de robots.txt e informes de visitas desde IA",
    title: "Los logs de tu servidor saben",
    highlight: "lo que Google y los bots de IA hacen de verdad",
    subtitle:
      "octopusSEO Log Analyser convierte los logs de acceso en información de rastreo: qué páginas solicitan Googlebot y los rastreadores de IA, dónde encuentran errores, qué bots son falsos y adónde va tu presupuesto de rastreo.",
    primary: "Descargar para {os}",
    primaryGeneric: "Descargar gratis",
    secondary: "Ver funciones",
    note: "Gratis · Windows, macOS y Linux · Tus logs no salen de tu ordenador",
  },
  mockup: {
    title: "acme.com — últimos 30 días",
    hits: "Visitas de buscadores verificadas",
    urls: "URLs rastreadas",
    errors: "Errores de rastreo",
    spoofed: "Visitas de bots falsos",
    chart: "Rastreo diario por bot",
    table: "Bot",
    verified: "verificado",
    fake: "falso",
  },
  formats: {
    title: "Lee los logs que ya tienes — el formato se detecta automáticamente",
  },
  features: {
    eyebrow: "Todo en un mismo lugar",
    title: "De líneas de log a decisiones SEO",
    subtitle: "Creado por SEOs para agencias que analizan decenas de sitios cada mes.",
    items: {
      verification: {
        title: "Bots verificados, no user agents",
        text: "Más de 110 rastreadores identificados y comprobados con las listas de IP oficiales de sus operadores y DNS inverso. Los Googlebots falsos se marcan, no se cuentan.",
      },
      ai: {
        title: "Visibilidad en IA",
        text: "Descubre qué solicitan GPTBot, ClaudeBot, PerplexityBot y ChatGPT-User — y qué visitas te envían ChatGPT, Perplexity o Gemini.",
      },
      robots: {
        title: "Cumplimiento de robots.txt",
        text: "Qué bots piden URLs bloqueadas y a cuáles bloquea por completo tu robots.txt — evaluado exactamente como lo hace Google.",
      },
      budget: {
        title: "Presupuesto de rastreo",
        text: "Frecuencia de rastreo por URL y directorio, días desde el último rastreo, URLs con parámetros, redirecciones y archivos estáticos que consumen presupuesto.",
      },
      orphans: {
        title: "Páginas huérfanas y sin rastrear",
        text: "Cruza exportaciones de rastreadores y sitemaps XML con los logs; las demás columnas se añaden como con un BUSCARV.",
      },
      reports: {
        title: "39 informes + lenguaje de consultas",
        text: "Informes listos y un formulario sencillo para tus propios filtros. Guarda, comparte y exporta a Excel, CSV o JSON.",
      },
      geo: {
        title: "Países y redes",
        text: "El país y la red (ASN) de cada IP revelan nubes de scrapers y el origen de los bots falsos. Consultado en local.",
      },
      alerts: {
        title: "Alertas de rastreo",
        text: "Las caídas bruscas de Googlebot, los picos de rastreo de IA y las ráfagas de errores de servidor se destacan en el resumen.",
      },
    },
  },
  ai: {
    eyebrow: "La búsqueda con IA ya está aquí",
    title: "Sabe cuándo los asistentes de IA leen — y recomiendan — tus páginas",
    text: "Los rastreadores de entrenamiento, los indexadores de búsqueda con IA y las consultas en tiempo real de los asistentes se separan: verás qué contenido alimenta a los modelos, cuál se cita y las visitas humanas que te devuelven los asistentes.",
    bullets: [
      "Categorías separadas para entrenamiento de IA, búsqueda con IA y asistentes",
      "Errores servidos a bots de IA — cada petición fallida es una cita perdida",
      "Visitas que llegan desde ChatGPT, Perplexity, Gemini, Copilot y Claude",
      "Ancho de banda y CO₂ estimado por rastreador",
    ],
  },
  terminal: {
    eyebrow: "Automatízalo",
    title: "Un comando para cada informe",
    text: "La CLI snx-log comparte los datos con la app de escritorio. Automatiza informes mensuales, programa importaciones en tus servidores y exporta directamente a Excel.",
    points: ["~750.000 líneas por segundo", "Funciona 100 % en tu ordenador"],
  },
  steps: {
    title: "Tres pasos hasta tu primer hallazgo",
    items: [
      { title: "Suelta tus logs", text: "Texto plano, gzip, zstd, bzip2 o zip — archivos o carpetas completas. Reimportar es seguro." },
      { title: "Verificamos los bots", text: "Cada visita de bot queda como verificada, falsa o sin verificar usando datos oficiales." },
      { title: "Actúa con los informes", text: "Corrige errores, frena el desperdicio de rastreo, desbloquea lo importante y sigue tu visibilidad en IA." },
    ],
  },
  stats: {
    bots: "rastreadores reconocidos",
    formats: "formatos de log",
    reports: "informes listos",
    speed: "líneas por segundo",
  },
  faq: {
    title: "Preguntas frecuentes",
    items: [
      {
        q: "¿octopusSEO Log Analyser es realmente gratis?",
        a: "Sí. La app de escritorio y la CLI son gratuitas y de código abierto, sin límite de líneas ni de sitios.",
      },
      {
        q: "¿Mis logs salen de mi ordenador?",
        a: "No. Todo se guarda y analiza en local. Solo se descargan las listas públicas de IP de rastreadores y, si lo activas, la base de datos de geolocalización.",
      },
      {
        q: "¿Qué formatos de log se admiten?",
        a: "Nginx y Apache (también formatos personalizados), IIS, CloudFront, balanceadores de AWS, HAProxy, Traefik, Caddy, Cloudflare, Google Cloud, JSON y CSV. El formato se detecta automáticamente.",
      },
      {
        q: "¿Qué tamaño pueden tener mis logs?",
        a: "Cientos de millones de líneas. Se importan a unas 750.000 líneas por segundo y se guardan comprimidos en torno al 8 % de su tamaño original.",
      },
      {
        q: "¿En qué idiomas está la app?",
        a: "La app de escritorio está disponible en inglés y turco; los informes y exportaciones funcionan igual en ambos.",
      },
    ],
  },
  cta: {
    title: "Mira tu web con los ojos de Googlebot",
    text: "Descarga la app, suelta un archivo de log y obtén tu primer informe en minutos.",
    button: "Descargar gratis",
  },
  footer: {
    tagline: "Software SEO que muestra cómo ven tu web los buscadores y la IA.",
    product: "Producto",
    resources: "Recursos",
    company: "Empresa",
    logAnalyser: "Log Analyser",
    spider: "SEO Spider",
    comingSoon: "próximamente",
    contact: "Contacto",
    issues: "Informar de un problema",
    rights: "© {year} octopusSEO. Software libre y de código abierto.",
    geo: "Datos de país y red: IP Geolocation by DB-IP",
  },
  featuresPage: {
    title: "Funciones",
    subtitle: "Todo lo que hace Log Analyser, en detalle.",
    sections: {
      bots: {
        title: "Identificación y verificación de bots",
        text: "Falsificar un user agent es trivial. Cada visita de bot se comprueba para que los informes solo cuenten rastreadores reales.",
        bullets: [
          "Más de 110 rastreadores por categorías: buscadores, entrenamiento de IA, búsqueda con IA, asistentes, publicidad, herramientas SEO, redes sociales, monitorización",
          "Listas de IP oficiales de Google, Bing, OpenAI, Anthropic, Perplexity, Apple y otros, además de DNS inverso confirmado",
          "Veredicto por visita: verificado, falso o sin verificar — vuelve a comprobarlo cuando quieras",
          "Tus propios bots: patrones de user agent con rangos de IP, URLs de listas de IP o DNS inverso",
        ],
      },
      ai: {
        title: "Rastreadores de IA y búsqueda con IA",
        text: "Entiende cómo usan las plataformas de IA tu contenido — y qué te devuelven.",
        bullets: [
          "Actividad diaria de cada rastreador de IA y las URLs que solicitó",
          "Errores servidos a bots y asistentes de IA",
          "Visitas humanas enviadas por asistentes de IA, con páginas de entrada",
          "Ancho de banda y emisiones estimadas por bot",
        ],
      },
      robots: {
        title: "Cumplimiento de robots.txt",
        text: "Compara cada petición de bot registrada con tu robots.txt actual o cualquier versión anterior.",
        bullets: [
          "Evaluación al estilo de Google: grupo de user agent más específico, coincidencia más larga, Allow gana en empate",
          "Peticiones bloqueadas por bot, desde IPs verificadas y falsas",
          "Bots bloqueados en todo el sitio, como los rastreadores de entrenamiento de IA",
          "Exporta cada infracción junto con la regla que decide",
        ],
      },
      crawl: {
        title: "Presupuesto de rastreo y salud técnica",
        text: "Descubre dónde pierden tiempo los rastreadores y qué no alcanzan.",
        bullets: [
          "Resumen por URL: último estado, último rastreo, días desde el último rastreo, tamaño, velocidad y visitas por bot",
          "Códigos de estado por URL con columnas 1xx–5xx y respuestas inconsistentes",
          "Redirecciones, 4xx/5xx, URLs lentas y pesadas, URLs con parámetros, archivos estáticos",
          "Alertas de caídas, picos de rastreo y ráfagas de errores de servidor",
        ],
      },
      urls: {
        title: "Páginas sin rastrear y huérfanas",
        text: "Combina tus datos de rastreo con lo que los bots hicieron realmente.",
        bullets: [
          "Exportaciones CSV de rastreadores, listas simples y sitemaps XML (con índices de sitemaps)",
          "Listadas pero nunca rastreadas, rastreadas pero no listadas (huérfanas)",
          "Todas las demás columnas de tu exportación junto a los datos del log",
          "Comparación de periodos por URL, directorio o bot — antes y después de una migración",
        ],
      },
      data: {
        title: "Tus datos, a tu manera",
        text: "Rápido, privado y automatizable.",
        bullets: [
          "Formatos detectados automáticamente; admite formatos personalizados de Nginx y Apache",
          "Ajustes de importación por sitio: solo bots, incluir/excluir URLs, eliminar parámetros",
          "Lenguaje de consultas con consultas guardadas; exportación a Excel, CSV y JSON",
          "Comparte un sitio entero como un único archivo; herramienta de línea de comandos para automatizar",
        ],
      },
    },
  },
  download: {
    title: "Descargar octopusSEO Log Analyser",
    subtitle: "Gratis para Windows, macOS y Linux. Incluye la CLI para automatizar.",
    recommended: "Recomendado para tu sistema",
    version: "Versión {version}",
    released: "Publicado el {date}",
    totalDownloads: "{count} descargas",
    desktop: "App de escritorio",
    cli: "Línea de comandos (snx-log)",
    file: "Archivo",
    size: "Tamaño",
    checksum: "SHA-256",
    getIt: "Descargar",
    unavailableTitle: "Descargas muy pronto",
    unavailableText:
      "Estamos preparando la primera versión pública. Mientras tanto, sigue el proyecto o compílalo desde el código fuente.",
    allReleases: "Todas las versiones",
    sourceTitle: "Compilar desde el código fuente",
    sourceText: "Requiere Rust (stable) y, para la app de escritorio, Node.js 22+ y pnpm.",
    requirementsTitle: "Requisitos del sistema",
    requirements: [
      "Windows 10/11, macOS 11 o posterior (Apple Silicon e Intel) o una distribución Linux actual",
      "4 GB de RAM; espacio en disco de aproximadamente el 10 % del tamaño de tus logs",
      "Internet solo para actualizar las listas de IP de rastreadores y la geolocalización",
    ],
    platforms: {
      macos: "macOS",
      windows: "Windows",
      linux: "Linux",
    },
    platformNotes: {
      macos: "Primer inicio: si macOS indica que no puede verificar la app, abre Ajustes del Sistema → Privacidad y seguridad y pulsa «Abrir igualmente».",
      windows: "Si SmartScreen avisa de un editor desconocido, pulsa «Más información» → «Ejecutar de todas formas».",
      linux: "AppImage: hazlo ejecutable (chmod +x) y ábrelo, o instala el paquete .deb o .rpm.",
    },
    arch: { universal: "Universal", arm64: "ARM64", x64: "x64", arm32: "ARM de 32 bits", appleSilicon: "Apple Silicon", intel: "Intel" },
    kinds: {
      dmg: "Imagen de disco",
      exe: "Instalador",
      msi: "Paquete MSI",
      appimage: "AppImage",
      deb: "Paquete Debian / Ubuntu",
      rpm: "Paquete Fedora / RHEL",
      exeOffline: "Instalador con WebView2 sin conexión",
      cli: "Archivo de la CLI",
    },
  },
  docs: {
    title: "Documentación",
    subtitle: "Empieza en minutos y profundiza después con el lenguaje de consultas.",
    quickstart: "Inicio rápido",
    quickstartText: "Crea un cliente y un sitio, importa logs y abre un informe. La app de escritorio hace lo mismo con unos clics.",
    query: "Lenguaje de consultas",
    queryText: "Cada informe es una consulta: condiciones y, opcionalmente, agrupación, métricas, orden y límite.",
    examples: "Ejemplos",
    exampleItems: [
      { query: "bot:googlebot* verification:verified status:4xx date:last30d group:url", text: "Errores que recibió Googlebot verificado en los últimos 30 días" },
      { query: "bot_category:ai-* group:bot,day metric:hits", text: "Actividad diaria por rastreador de IA" },
      { query: "referer_type:ai -bot:* group:url,referer_host", text: "Páginas visitadas desde asistentes de IA" },
      { query: "verification:spoofed group:bot,as_org,country", text: "De dónde vienen los bots falsos" },
      { query: "group:url metric:last_status,days_since_last,googlebot_hits", text: "Actualidad del rastreo por URL" },
    ],
    fields: "Campos",
    metrics: "Métricas",
    more: "Referencia completa en GitHub",
    guide: "Guía de usuario (turco)",
  },
  changelog: {
    title: "Novedades",
    subtitle: "Nuevas funciones y correcciones en cada versión.",
    empty: "Todavía no se ha publicado ninguna versión.",
    viewOnGithub: "Ver en GitHub",
  },
  notFound: {
    title: "Página no encontrada",
    text: "Esta página no existe — quizá se la inventó un rastreador.",
    home: "Volver al inicio",
  },
  common: {
    copy: "Copiar",
    copied: "Copiado",
    learnMore: "Más información",
  },
};
