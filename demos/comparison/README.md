# bolt402 vs lnget — Side-by-Side Comparison

A static comparison page showing the developer experience difference between [bolt402](https://github.com/lightninglabs/bolt402) (embedded SDK) and [lnget](https://github.com/lightninglabs/lnget) (CLI binary) for consuming L402-gated APIs.

## What it shows

- **Split-screen code comparison**: Same L402 endpoint consumed via lnget (shell out) vs bolt402 (native import)
- **Multi-language snippets**: Rust, TypeScript, and Python bolt402 examples with Shiki syntax highlighting
- **Architecture diagrams**: External process vs in-process SDK flow
- **Feature comparison table**: Side-by-side on integration, errors, caching, budget, AI frameworks
- **When-to-use guide**: Honest guidance on when each tool is the right choice

## Quick start

```bash
# Install dependencies
yarn install

# Run development server
yarn dev

# Build static site
yarn build
```

Open [http://localhost:3000](http://localhost:3000) in your browser.

## Tech stack

- **Next.js 16** — React framework
- **Tailwind CSS v4** — Styling
- **Shiki** — Server-side syntax highlighting (Rust, TypeScript, Python, Bash)
- **Static export** — No server runtime needed

## Design

See [docs/design/012-comparison-page.md](../../docs/design/012-comparison-page.md) for the full design document.

## License

MIT OR Apache-2.0
