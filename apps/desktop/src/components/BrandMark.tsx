// The octopusSEO octopus, as on the website and the app icon.

export function BrandMark() {
  return (
    <svg className="brand-mark" viewBox="0 0 64 64" aria-hidden="true">
      <defs>
        <linearGradient id="brand-mark-body" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#a78bfa" />
          <stop offset="1" stopColor="#2dd4bf" />
        </linearGradient>
      </defs>
      <path
        d="M32 6c-12.2 0-21 8.6-21 20.2 0 6.4 2.7 11.3 6.6 14.6L13 52c-.7 1.6.9 3.2 2.5 2.4l5.9-3.1 2.4 6.3c.6 1.7 3 1.7 3.6 0L32 47l4.6 10.6c.6 1.7 3 1.7 3.6 0l2.4-6.3 5.9 3.1c1.6.8 3.2-.8 2.5-2.4l-4.6-11.2C50.3 37.5 53 32.6 53 26.2 53 14.6 44.2 6 32 6z"
        fill="url(#brand-mark-body)"
      />
      <circle cx="24.5" cy="25" r="4.4" fill="#fff" />
      <circle cx="39.5" cy="25" r="4.4" fill="#fff" />
      <circle cx="25.4" cy="25.9" r="2.1" fill="#1e1b4b" />
      <circle cx="40.4" cy="25.9" r="2.1" fill="#1e1b4b" />
    </svg>
  );
}
