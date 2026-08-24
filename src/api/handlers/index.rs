use axum::{response::Html, Json};
use serde_json::{json, Value};
use std::sync::OnceLock;

/// Static JSON response for the index endpoint
static INDEX_JSON: OnceLock<Value> = OnceLock::new();

/// The landing page, styled to match the OtterSec design system on osec.io.
///
/// Held as one `&'static str` because the page has no dynamic data. The brand
/// lockup is pulled in with `include_str!` rather than pasted here: its
/// gradient stops reference `var(--foreground)`, so it has to be inlined into
/// the document to pick up the theme, and keeping 10 KB of path data out of
/// this file keeps the markup readable. Fonts are served separately by
/// [`super::assets`].
static LANDING_HTML: &str = concat!(
    r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Solana Verified Builds &middot; OtterSec</title>
    <meta
      name="description"
      content="Verified builds let anyone confirm that an on-chain Solana program matches its public source code."
    />
    <meta name="color-scheme" content="light dark" />
    <link rel="preload" href="/assets/MDPrimer-Regular.woff2" as="font" type="font/woff2" crossorigin />
    <link rel="preload" href="/assets/MDUIXL-Regular.woff2" as="font" type="font/woff2" crossorigin />
    <script>
      // Set the theme before first paint so a stored preference does not flash.
      try {
        var t = localStorage.theme;
        if (t === "light" || t === "dark") document.documentElement.dataset.theme = t;
      } catch (e) {}
    </script>
    <style>
      /* ---------------------------------------------------------------- *
       * Fonts. Fallbacks are metric-matched so the pre-swap layout holds. *
       * ---------------------------------------------------------------- */
      @font-face {
        font-family: "MD Primer";
        src: url("/assets/MDPrimer-Regular.woff2") format("woff2");
        font-weight: 400;
        font-style: normal;
        font-display: swap;
      }
      @font-face {
        font-family: "MD Primer Fallback";
        src: local("Arial");
        font-weight: 400;
        size-adjust: 129.3593%;
        ascent-override: 64.1624%;
        descent-override: 13.1417%;
        line-gap-override: 0%;
      }
      @font-face {
        font-family: "MD UI XL";
        src: url("/assets/MDUIXL-Regular.woff2") format("woff2");
        font-weight: 400;
        font-style: normal;
        font-display: swap;
      }
      @font-face {
        font-family: "MD UI XL Fallback";
        src: local("Arial");
        font-weight: 400;
        size-adjust: 120.0708%;
        ascent-override: 78.2871%;
        descent-override: 24.9853%;
        line-gap-override: 0%;
      }
      @font-face {
        font-family: "Lilex";
        src: url("/assets/Lilex-Variable.woff2") format("woff2");
        font-weight: 100 700;
        font-style: normal;
        font-display: swap;
      }
      @font-face {
        font-family: "Lilex Fallback";
        src: local("Courier New");
        font-weight: 100 700;
        size-adjust: 99.9837%;
        ascent-override: 99.9163%;
        descent-override: 30.1049%;
      }

      /* ------------------------------------------------ *
       * Tokens. Radix gray scale, utopia type and space.  *
       * ------------------------------------------------ */
      :root {
        --gray-1:  light-dark(#fcfcfd, #111113);
        --gray-2:  light-dark(#f9f9fb, #18191b);
        --gray-3:  light-dark(#f0f0f3, #212225);
        --gray-6:  light-dark(#d9d9e0, #363a3f);
        --gray-11: light-dark(#60646c, #b0b4ba);
        --gray-12: light-dark(#1c2024, #edeef0);

        --background: var(--gray-1);
        --foreground: var(--gray-12);
        --muted: var(--gray-3);
        --muted-foreground: var(--gray-11);
        --border: var(--gray-6);
        --ring: light-dark(#b9bbc6, #5a6169);

        --foreground-80: color-mix(in oklab, var(--foreground) 80%, transparent);
        --foreground-60: color-mix(in oklab, var(--foreground) 60%, transparent);
        --foreground-40: color-mix(in oklab, var(--foreground) 40%, transparent);
        --foreground-20: color-mix(in oklab, var(--foreground) 20%, transparent);
        --foreground-10: color-mix(in oklab, var(--foreground) 10%, transparent);
        --foreground-05: color-mix(in oklab, var(--foreground) 5%, transparent);
        --foreground-01: color-mix(in oklab, var(--foreground) 1%, transparent);

        --hero-1: light-dark(#94a8ce, #bbd0f7);
        --hero-2: light-dark(#9ec0e0, #9ec0e0);
        --hero-3: light-dark(#97baab, #97baab);

        --font-sans: "MD UI XL", "MD UI XL Fallback", sans-serif;
        --font-heading: "MD Primer", "MD Primer Fallback", sans-serif;
        --font-mono: "Lilex", "Lilex Fallback", monospace;

        /* utopia.fyi type scale, 320->1197px */
        --step--1: clamp(0.8889rem, 0.8712rem + 0.0887vw, 0.9375rem);
        --step-0: clamp(1rem, 0.9544rem + 0.2281vw, 1.125rem);
        --step-1: clamp(1.125rem, 1.0429rem + 0.4105vw, 1.35rem);
        --step-2: clamp(1.2656rem, 1.1363rem + 0.6465vw, 1.62rem);
        --step-3: clamp(1.4238rem, 1.234rem + 0.949vw, 1.944rem);
        --step-5: clamp(1.802rem, 1.4381rem + 1.8195vw, 2.7994rem);

        /* utopia.fyi space scale */
        --space-2xs: clamp(0.5rem, 0.4772rem + 0.114vw, 0.5625rem);
        --space-xs: clamp(0.75rem, 0.7044rem + 0.2281vw, 0.875rem);
        --space-s: clamp(1rem, 0.9544rem + 0.2281vw, 1.125rem);
        --space-m: clamp(1.5rem, 1.4316rem + 0.3421vw, 1.6875rem);
        --space-l: clamp(2rem, 1.9088rem + 0.4561vw, 2.25rem);
        --space-xl: clamp(3rem, 2.8632rem + 0.6842vw, 3.375rem);
        --space-s-m: clamp(1rem, 0.7491rem + 1.2543vw, 1.6875rem);

        --radius-sm: 0.25rem;
        --radius-md: 0.375rem;
        --radius-full: 9999px;
        --blur-sm: 8px;

        --grid-max-width: 74.81rem;
        --grid-gutter: var(--space-2xs);
        --grid-margin: var(--space-s-m);
        --brand-size: 1.65rem;
        --header-height: calc(max(2rem, var(--brand-size)) + 2 * var(--space-s));

        color-scheme: light dark;
      }

      :root[data-theme="light"] { color-scheme: light; }
      :root[data-theme="dark"] { color-scheme: dark; }

      /* --------- *
       * Reset.    *
       * --------- */
      *,
      ::before,
      ::after {
        box-sizing: border-box;
        margin: 0;
        padding: 0;
        border: 0 solid var(--border);
        letter-spacing: 0;
        -webkit-font-smoothing: antialiased;
        -moz-osx-font-smoothing: grayscale;
      }

      *:focus-visible {
        outline: 2px solid var(--ring);
        outline-offset: 0.25rem;
        border-radius: var(--radius-sm);
      }

      :where(pre, code) { letter-spacing: normal; }
      ul { list-style: none; }
      svg { display: block; }
      button { font: inherit; color: inherit; background: none; cursor: pointer; }
      a { color: inherit; text-decoration: none; }

      ::selection {
        background-color: var(--foreground);
        color: var(--background);
      }

      html { scrollbar-gutter: stable; }

      body {
        min-block-size: 100svh;
        display: grid;
        grid-template-rows: 1fr auto;
        background-color: var(--background);
        color: var(--foreground);
        font-family: var(--font-sans);
        font-size: var(--step-0);
        line-height: 1.5;
        text-wrap: pretty;
      }

      :where(h1, h2, h3) {
        font-family: var(--font-heading);
        font-size: inherit;
        font-weight: 400;
        text-wrap: balance;
      }

      /* ------------------------------------------------------------- *
       * Background. A static stand-in for the grain shader on osec.io: *
       * three soft cool lights over a tiled turbulence grain.          *
       * ------------------------------------------------------------- */
      grain-bg {
        position: fixed;
        inset: 0;
        z-index: -1;
        display: block;
        pointer-events: none;
        /* Ring bands rather than blobs: the osec.io hero reads as concentric
           arcs of cool light sweeping off the top edge, and a soft radial
           glow does not carry that. Each gradient is transparent through the
           middle and lights up only across a narrow band. */
        background-image:
          radial-gradient(88rem 50rem at 24% -26rem,
            transparent 0 58%,
            color-mix(in oklab, var(--hero-2) 26%, transparent) 67%,
            color-mix(in oklab, var(--hero-1) 52%, transparent) 72%,
            color-mix(in oklab, var(--hero-2) 22%, transparent) 77%,
            transparent 84%),
          radial-gradient(124rem 68rem at 66% -40rem,
            transparent 0 62%,
            color-mix(in oklab, var(--hero-3) 26%, transparent) 70%,
            color-mix(in oklab, var(--hero-1) 38%, transparent) 75%,
            transparent 84%),
          radial-gradient(70rem 38rem at 6% -10rem,
            color-mix(in oklab, var(--hero-1) 14%, transparent),
            transparent 68%);
        mask-image: linear-gradient(to bottom, #000 0 34rem, transparent 46rem);
      }

      /* Film grain. Kept very low: the noise should read as texture on the
         lights above, never as a grey wash over the whole page. The colour
         matrix flattens feTurbulence's RGB static to a neutral alpha mask. */
      grain-bg::after {
        content: "";
        position: absolute;
        inset: 0;
        opacity: light-dark(0.5, 0.75);
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='200' height='200'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/%3E%3CfeColorMatrix values='0 0 0 0 0.5 0 0 0 0 0.5 0 0 0 0 0.5 0 0 0 0.09 0'/%3E%3C/filter%3E%3Crect width='200' height='200' filter='url(%23n)'/%3E%3C/svg%3E");
        mask-image: linear-gradient(to bottom, #000 0 42rem, transparent);
      }

      /* Fade content out under the fixed header. */
      body::before {
        content: "";
        position: fixed;
        inset-block-start: 0;
        inset-inline: 0;
        block-size: 8rem;
        z-index: 5;
        pointer-events: none;
        background: linear-gradient(
          to bottom,
          var(--background),
          color-mix(in oklab, var(--background) 84%, transparent) 25%,
          color-mix(in oklab, var(--background) 50%, transparent) 50%,
          color-mix(in oklab, var(--background) 15.6%, transparent) 75%,
          transparent
        );
      }

      /* --------- *
       * Layout.   *
       * --------- */
      content-rail {
        display: block;
        inline-size: 100%;
        max-inline-size: var(--grid-max-width);
        margin-inline: auto;
        padding-inline: var(--grid-margin);
      }

      /* ------------------------------------------------------------------ *
       * Surfaces. The hairline is the signature: a 1px inset gradient       *
       * border that fades out downward, drawn with mask-composite.          *
       * ------------------------------------------------------------------ */
      [data-hairline] { position: relative; }

      [data-hairline]::before {
        content: "";
        position: absolute;
        inset: 0;
        z-index: 1;
        padding: 1px;
        border-radius: inherit;
        background: linear-gradient(
          var(--hairline-dir, to bottom),
          color-mix(in oklab, var(--foreground) var(--hairline-strength, 25%), transparent),
          transparent
        );
        mask:
          linear-gradient(#000 0 0) content-box,
          linear-gradient(#000 0 0);
        mask-composite: exclude;
        pointer-events: none;
      }

      [data-glass] {
        background-color: var(--foreground-01);
        backdrop-filter: blur(var(--blur-sm));
      }

      [data-pill] {
        --hairline-dir: to right;
        --hairline-strength: 30%;

        display: inline-flex;
        align-items: center;
        justify-content: center;
        gap: 0.375rem;
        padding: 0.375rem 1.5rem;
        border-radius: var(--radius-full);
        font-size: 1rem;
        line-height: 1;
        color: var(--pill-fg, var(--foreground-80));
        white-space: nowrap;
        transition: background-color 0.15s, color 0.15s;
      }

      [data-pill]:hover {
        background-color: color-mix(in oklab, var(--muted) 50%, transparent);
        color: var(--foreground);
      }

      [data-pill] svg {
        inline-size: 1.125em;
        block-size: 1.125em;
      }

      [data-pill-action] { padding-block: 0.75rem; }

      [data-solid] {
        background-image: linear-gradient(to right, var(--foreground), var(--foreground-80));
        color: var(--background);
      }

      [data-solid]:hover {
        background-image: linear-gradient(to right, var(--foreground-80), var(--foreground-80));
        color: var(--background);
      }

      [data-gradient-text] {
        background-image: linear-gradient(var(--gradient-text-dir, to right), var(--foreground), var(--foreground-80));
        -webkit-background-clip: text;
        background-clip: text;
        color: transparent;
      }

      [data-gradient-text]::selection {
        -webkit-text-fill-color: var(--background);
        color: var(--background);
      }

      [data-icon-button] {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        inline-size: 2rem;
        block-size: 2rem;
        border-radius: var(--radius-md);
        color: var(--foreground-80);
      }

      [data-icon-button]:hover {
        background-color: color-mix(in oklab, var(--muted) 50%, transparent);
        color: var(--foreground);
      }

      [data-icon-button] svg {
        inline-size: 1rem;
        block-size: 1rem;
      }

      /* --------- *
       * Header.   *
       * --------- */
      header {
        position: fixed;
        inset-block-start: 0;
        inset-inline: 0;
        z-index: 10;
      }

      header content-rail {
        display: flex;
        align-items: center;
        gap: var(--space-xs);
        padding-block: var(--space-s);
      }

      brand-lockup {
        display: inline-flex;
        align-items: center;
        gap: 0.75rem;
      }

      brand-lockup svg {
        block-size: var(--brand-size);
        inline-size: auto;
      }

      brand-tag {
        display: inline-block;
        padding-inline-start: 0.75rem;
        border-inline-start: 1px solid var(--foreground-20);
        font-size: 1rem;
        line-height: 1;
        color: var(--foreground-60);
      }

      @media (width < 34rem) {
        brand-tag { display: none; }
      }

      header nav {
        display: flex;
        align-items: center;
        gap: var(--space-2xs);
        margin-inline: auto 0;
        margin-inline-end: -0.5rem;
      }

      @media (width < 30rem) {
        header nav [data-pill] { display: none; }
      }

      /* --------- *
       * Hero.     *
       * --------- */
      main { display: block; }

      page-hero {
        display: block;
        padding-block-start: calc(var(--header-height) + var(--space-xl));
        padding-block-end: var(--space-l);
      }

      page-hero h1 {
        max-inline-size: 22ch;
        font-size: var(--step-5);
        line-height: 1;
      }

      hero-deck {
        display: block;
        max-inline-size: 34rem;
        margin-block-start: var(--space-s);
        font-size: 1rem;
        line-height: 1.35;
        color: var(--foreground-60);
      }

      hero-actions {
        display: grid;
        grid-template-columns: repeat(auto-fit, minmax(min(100%, 11rem), max-content));
        justify-content: start;
        gap: 0.375rem;
        margin-block-start: var(--space-m);
      }

      /* Inline code, for CLI flags and package names in prose. */
      :where(hero-deck, panel-card p) code {
        font-family: var(--font-mono);
        font-size: 0.9em;
        padding: 0.1em 0.3em;
        border-radius: var(--radius-sm);
        background-color: color-mix(in oklab, var(--muted) 60%, transparent);
        color: var(--foreground-80);
      }

      /* --------- *
       * Cards.    *
       * --------- */
      card-grid {
        display: grid;
        grid-template-columns: repeat(3, 1fr);
        column-gap: var(--grid-gutter);
        align-items: start;
        padding-block-end: var(--space-xl);
      }

      @media (width < 64rem) {
        card-grid {
          grid-template-columns: 1fr;
          row-gap: var(--grid-gutter);
        }
      }

      panel-card {
        display: flex;
        flex-direction: column;
        gap: 0.75rem;
        block-size: 100%;
        padding: var(--space-m) var(--space-s) var(--space-l);
      }

      panel-card h2 {
        font-size: var(--step-1);
        line-height: 1;
      }

      panel-card p {
        font-size: 1rem;
        line-height: 1.35;
        color: var(--foreground-60);
      }

      panel-card ul {
        display: flex;
        flex-direction: column;
        gap: 0.625rem;
        margin-block-start: 0.125rem;
      }

      panel-card li a {
        display: inline-flex;
        align-items: baseline;
        gap: 0.375rem;
        font-size: 1rem;
        line-height: 1.2;
        color: var(--foreground-80);
        text-decoration: underline;
        text-decoration-color: transparent;
        text-decoration-thickness: max(1px, 0.0625em);
        text-underline-offset: 0.25em;
        transition: color 0.15s, text-decoration-color 0.15s;
      }

      panel-card li a:hover {
        color: var(--foreground);
        text-decoration-color: currentColor;
      }

      panel-card li a svg {
        flex-shrink: 0;
        inline-size: 0.875em;
        block-size: 0.875em;
        translate: 0 0.1em;
        opacity: 0.6;
      }

      /* --------------- *
       * Code samples.   *
       * --------------- */
      code-sample {
        display: block;
        margin-block-start: auto;
        padding: 0.75rem 0.875rem;
        border-radius: var(--radius-sm);
        background-color: color-mix(in oklab, var(--muted) 60%, transparent);
        overflow-x: auto;
      }

      code-sample pre {
        font-family: var(--font-mono);
        font-size: var(--step--1);
        line-height: 1.6;
        color: var(--foreground-80);
      }

      code-sample .dim { color: var(--foreground-40); }

      /* --------- *
       * Footer.   *
       * --------- */
      page-footer {
        display: block;
        padding-block: var(--space-l);
        border-block-start: 1px solid var(--foreground-10);
      }

      page-footer content-rail {
        display: flex;
        flex-wrap: wrap;
        align-items: baseline;
        justify-content: space-between;
        gap: var(--space-s) var(--space-m);
      }

      footer-links {
        display: flex;
        flex-wrap: wrap;
        gap: var(--space-m);
        font-size: 1rem;
        line-height: 1;
      }

      footer-links a { color: var(--foreground-60); }
      footer-links a:hover { color: var(--foreground); }

      page-footer small {
        font-size: 1rem;
        line-height: 1;
        color: var(--foreground-40);
      }

      page-footer code {
        font-family: var(--font-mono);
        font-size: 0.9em;
        padding: 0.1em 0.3em;
        border-radius: var(--radius-sm);
        background-color: color-mix(in oklab, var(--muted) 60%, transparent);
      }

      /* Theme toggle icon swap. */
      dark-icon { display: none; }
      :root[data-theme="dark"] light-icon { display: none; }
      :root[data-theme="dark"] dark-icon { display: inline; }

      @media (prefers-color-scheme: dark) {
        :root:not([data-theme]) light-icon { display: none; }
        :root:not([data-theme]) dark-icon { display: inline; }
      }

      @media (prefers-reduced-motion: reduce) {
        * { transition-duration: 0.01ms !important; }
      }
    </style>
  </head>
  <body>
    <grain-bg aria-hidden="true"></grain-bg>

    <header>
      <content-rail>
        <brand-lockup>
          <a href="https://osec.io" aria-label="OtterSec">"##,
    include_str!("../../../assets/lockup.svg"),
    r##"</a>
          <brand-tag>Verified Builds</brand-tag>
        </brand-lockup>
        <nav aria-label="Primary">
          <a href="/api" data-pill data-glass data-hairline>API reference</a>
          <button
            type="button"
            data-icon-button
            data-theme-toggle
            aria-label="Toggle colour theme"
          >
            <light-icon>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true">
                <circle cx="12" cy="12" r="4.5" />
                <path d="M12 2.5v2M12 19.5v2M2.5 12h2M19.5 12h2M5.2 5.2l1.4 1.4M17.4 17.4l1.4 1.4M18.8 5.2l-1.4 1.4M6.6 17.4l-1.4 1.4" />
              </svg>
            </light-icon>
            <dark-icon>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round" aria-hidden="true">
                <path d="M20.5 14.3A8.8 8.8 0 0 1 9.7 3.5a8.8 8.8 0 1 0 10.8 10.8Z" />
              </svg>
            </dark-icon>
          </button>
        </nav>
      </content-rail>
    </header>

    <main>
      <content-rail>
        <page-hero>
          <h1 data-gradient-text>Verified builds for Solana programs.</h1>
          <hero-deck>
            Verified builds help users confirm that an on-chain Solana program
            matches its public source code. This service rebuilds your program
            in a reproducible container and checks the result against what is
            deployed on mainnet.
          </hero-deck>
          <hero-actions>
            <a href="/api" data-pill data-pill-action data-solid>
              Browse the API
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M5 12h14M13 6l6 6-6 6" />
              </svg>
            </a>
            <a
              href="https://github.com/otter-sec/solana-verified-programs-api"
              data-pill
              data-pill-action
              data-glass
              data-hairline
            >
              View the source
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M7 17 17 7M8 7h9v9" />
              </svg>
            </a>
          </hero-actions>
        </page-hero>

        <card-grid>
          <panel-card data-glass data-hairline>
            <h2>Verify a program</h2>
            <p>
              Point the CLI at the repository the program was built from. Add
              <code>--remote</code> and the build runs here instead of on your machine.
            </p>
            <code-sample>
              <pre>solana-verify verify-from-repo <span class="dim">\</span>
  --remote -um <span class="dim">\</span>
  --program-id &lt;PROGRAM_ID&gt; <span class="dim">\</span>
  &lt;REPOSITORY_URL&gt;</pre>
            </code-sample>
          </panel-card>

          <panel-card data-glass data-hairline>
            <h2>Check a program</h2>
            <p>
              Ask whether a deployed program is verified, which commit it was
              built from, and when it was last checked.
            </p>
            <code-sample>
              <pre><span class="dim">GET</span> /status/&lt;PROGRAM_ID&gt;
<span class="dim">GET</span> /job/&lt;JOB_ID&gt;
<span class="dim">GET</span> /verified-programs</pre>
            </code-sample>
          </panel-card>

          <panel-card data-glass data-hairline>
            <h2>Docs and support</h2>
            <p>
              Read how verified builds work, or get in touch if a build does not
              reproduce.
            </p>
            <ul>
              <li>
                <a href="https://solana.com/docs/programs/verified-builds#how-do-i-create-verified-builds">
                  Solana docs: Verified Builds
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M7 17 17 7M8 7h9v9" />
                  </svg>
                </a>
              </li>
              <li>
                <a href="https://github.com/solana-foundation/solana-verifiable-build">
                  solana-verifiable-build
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                    <path d="M7 17 17 7M8 7h9v9" />
                  </svg>
                </a>
              </li>
              <li><a href="mailto:contact@osec.io">contact@osec.io</a></li>
            </ul>
          </panel-card>
        </card-grid>
      </content-rail>
    </main>

    <page-footer>
      <content-rail>
        <footer-links>
          <a href="/api">API reference</a>
          <a href="https://github.com/otter-sec/solana-verified-programs-api">GitHub</a>
          <a href="mailto:contact@osec.io">contact@osec.io</a>
          <a href="https://osec.io">osec.io</a>
        </footer-links>
        <small>Endpoint list at <code>GET /api</code> &middot; Otter Audits LLC</small>
      </content-rail>
    </page-footer>

    <script>
      document.addEventListener("click", function (event) {
        if (!event.target.closest || !event.target.closest("[data-theme-toggle]")) return;
        var root = document.documentElement;
        var dark =
          root.dataset.theme === "dark" ||
          (!root.dataset.theme && matchMedia("(prefers-color-scheme: dark)").matches);
        var theme = dark ? "light" : "dark";
        root.dataset.theme = theme;
        try {
          localStorage.theme = theme;
        } catch (e) {}
      });
    </script>
  </body>
</html>
"##
);

/// Simple landing page for https://verify.osec.io
pub fn landing_page() -> Html<&'static str> {
    Html(LANDING_HTML)
}

/// Handler for the index endpoint that provides API documentation
///
/// # Endpoint: GET /api
///
/// # Returns
/// * `Json<Value>` - JSON response containing API endpoint documentation
pub fn index() -> Json<Value> {
    let value = INDEX_JSON.get_or_init(|| {
        json!({
            "endpoints": [
                {
                    "path": "/",
                    "method": "GET",
                    "description": "Landing page",
                    "params": {}
                },
                {
                    "path": "/api",
                    "method": "GET",
                    "description": "API endpoint documentation",
                    "params": {}
                },
                {
                    "path": "/verify",
                    "method": "POST",
                    "description": "Deprecated: use /verify-with-signer. Asynchronously verify a Solana program",
                    "params": {
                        "repository": {
                            "type": "string",
                            "required": true,
                            "description": "Git repository URL containing the program source code"
                        },
                        "program_id": {
                            "type": "string",
                            "required": true,
                            "description": "Solana program ID on mainnet"
                        },
                        "commit_hash": {
                            "type": "string",
                            "required": true,
                            "description": "Specific Git commit hash to verify. Defaults to latest commit"
                        },
                        "lib_name": {
                            "type": "string",
                            "required": false,
                            "description": "Library name for repositories with multiple programs"
                        },
                        "bpf_flag": {
                            "type": "boolean",
                            "required": false,
                            "description": "Use cargo build-bpf instead of cargo build-sbf (required for Anchor programs)"
                        },
                        "base_image": {
                            "type": "string",
                            "required": false,
                            "description": "Custom Docker base image for building"
                        },
                        "mount_path": {
                            "type": "string",
                            "required": false,
                            "description": "Custom mount path for repository in build container"
                        },
                        "workspace_path": {
                            "type": "string",
                            "required": false,
                            "description": "Custom workspace path for monorepos. Passed to solana-verify --workspace-path"
                        },
                        "cargo_args": {
                            "type": "array",
                            "items": "string",
                            "required": false,
                            "description": "Additional cargo build arguments passed after solana-verify --"
                        },
                        "cargo_build_sbf_args": {
                            "type": "string",
                            "required": false,
                            "description": "Arguments passed to solana-verify --cargo-build-sbf-args"
                        },
                        "arch": {
                            "type": "string",
                            "required": false,
                            "description": "Build for the given target architecture [default: v0]"
                        },
                        "webhook_url": {
                            "type": "string",
                            "required": false,
                            "description": "Webhook URL to receive verification results"
                        }
                    }
                },
                {
                    "path": "/verify-with-signer",
                    "method": "POST",
                    "description": "Preferred endpoint. Asynchronously verify using PDA params for the provided signer, PDA signer should be the program authority",
                    "params": {
                        "signer": {
                            "type": "string",
                            "required": true,
                            "description": "PDA signer public key should be the program authority"
                        },
                        "program_id": {
                            "type": "string",
                            "required": true,
                            "description": "Solana program ID on mainnet"
                        },
                        "webhook_url": {
                            "type": "string",
                            "required": false,
                            "description": "Webhook URL to receive verification results"
                        }
                    }
                },
                {
                    "path": "/verify_sync",
                    "method": "POST",
                    "description": "Deprecated: use /verify-with-signer. Synchronously verify a Solana program",
                    "params": {
                        "$ref": "#/endpoints/1/params"
                    }
                },
                {
                    "path": "/status/:address",
                    "method": "GET",
                    "description": "Check program verification status",
                    "params": {
                        "address": {
                            "type": "string",
                            "required": true,
                            "description": "Mainnet program address to check"
                        }
                    }
                },
                {
                    "path": "/status-all/:address",
                    "method": "GET",
                    "description": "Get all verification information for a program",
                    "params": {
                        "address": {
                            "type": "string",
                            "required": true,
                            "description": "Mainnet program address to check"
                        }
                    }
                },
                {
                    "path": "/resolve-hash/:hash",
                    "method": "GET",
                    "description": "Content-addressed lookup: every completed build that produced the given executable hash",
                    "params": {
                        "hash": {
                            "type": "string",
                            "required": true,
                            "description": "Executable hash (hex-encoded sha256)"
                        }
                    }
                },
                {
                    "path": "/job/:job_id",
                    "method": "GET",
                    "description": "Check status of an async verification job",
                    "params": {
                        "job_id": {
                            "type": "string",
                            "required": true,
                            "description": "Verification job identifier"
                        }
                    }
                },
                {
                    "path": "/logs/:build_id",
                    "method": "GET",
                    "description": "Build logs for a job",
                    "params": {
                        "build_id": {
                            "type": "string",
                            "required": true,
                            "description": "Job id (UUID)"
                        }
                    }
                },
                {
                    "path": "/verified-programs",
                    "method": "GET",
                    "description": "Get list of all verified programs",
                    "params": {},
                    "query": {
                        "search": {
                            "type": "string",
                            "required": false,
                            "description": "Filter by program_id or repository (must be valid Solana address or HTTP/HTTPS URL)"
                        }
                    }
                },
                {
                    "path": "/verified-programs/:page",
                    "method": "GET",
                    "description": "Get paginated list of verified programs",
                    "params": {
                        "page": {
                            "type": "integer",
                            "required": true,
                            "description": "Page number (starting from 1)"
                        }
                    },
                    "query": {
                        "search": {
                            "type": "string",
                            "required": false,
                            "description": "Filter by program_id or repository (must be valid Solana address or HTTP/HTTPS URL)"
                        }
                    }
                },
                {
                    "path": "/verified-programs-status",
                    "method": "GET",
                    "description": "Get detailed status of all verified programs",
                    "params": {}
                },
            ]
        })
    });

    Json(value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::handlers::assets::ASSETS;

    /// Facts the pre-retheme landing page carried. A visual rewrite is exactly
    /// where content silently gets dropped, so pin each one.
    const REQUIRED_CONTENT: &[&str] = &[
        "contact@osec.io",
        "github.com/otter-sec/solana-verified-programs-api",
        "solana.com/docs/programs/verified-builds#how-do-i-create-verified-builds",
        "github.com/solana-foundation/solana-verifiable-build",
        "/api",
    ];

    #[test]
    fn landing_page_is_a_complete_document() {
        assert!(LANDING_HTML.starts_with("<!doctype html>"));
        assert!(LANDING_HTML.contains("<html lang=\"en\">"));
        assert!(LANDING_HTML.contains("<title>"));
        assert!(LANDING_HTML.trim_end().ends_with("</html>"));
    }

    #[test]
    fn landing_page_keeps_every_documented_fact() {
        for fact in REQUIRED_CONTENT {
            assert!(
                LANDING_HTML.contains(fact),
                "landing page no longer mentions {fact}"
            );
        }
    }

    #[test]
    fn landing_page_inlines_the_brand_lockup() {
        // Inlined rather than <img>-linked so its var(--foreground) gradient
        // stops resolve against the page theme.
        assert!(LANDING_HTML.contains("<svg"), "lockup svg was not inlined");
        assert!(
            LANDING_HTML.contains("var(--foreground)"),
            "lockup lost its themed gradient stops"
        );
    }

    /// Both directions: every embedded asset is used, and every asset the page
    /// asks for exists. Catches a rename that only updated one side.
    #[test]
    fn landing_page_and_asset_table_agree() {
        for (name, _, _) in ASSETS {
            assert!(
                LANDING_HTML.contains(&format!("/assets/{name}")),
                "{name} is embedded but never referenced by the landing page"
            );
        }

        for reference in LANDING_HTML.split("/assets/").skip(1) {
            let name: String = reference
                .chars()
                .take_while(|c| *c != '"' && *c != ')' && *c != '\'')
                .collect();
            assert!(
                ASSETS.iter().any(|(known, _, _)| *known == name),
                "landing page requests /assets/{name}, which is not embedded"
            );
        }
    }

    #[test]
    fn landing_page_theme_toggle_is_wired() {
        assert!(LANDING_HTML.contains("data-theme-toggle"));
        assert!(LANDING_HTML.contains("color-scheme: light dark"));
    }

    /// `/api` is a public contract; this module's rewrite must not touch it.
    #[test]
    fn api_index_still_lists_endpoints() {
        let Json(value) = index();
        let endpoints = value["endpoints"].as_array().expect("endpoints array");
        assert!(!endpoints.is_empty());

        for endpoint in endpoints {
            for field in ["path", "method", "description"] {
                assert!(
                    endpoint[field].is_string(),
                    "endpoint {endpoint:?} is missing {field}"
                );
            }
        }
    }
}
