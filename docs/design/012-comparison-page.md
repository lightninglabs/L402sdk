# 012: bolt402 vs lnget — Side-by-Side Comparison Page

**Issue:** #31
**Author:** Toshi
**Date:** 2026-03-19

## Problem

bolt402's key differentiator from lnget is that it's a library (embeddable SDK) rather than a CLI binary. Users evaluating L402 tooling need to immediately see why this matters for production code and AI agent pipelines. A visual comparison page makes the DX difference self-evident.

## Proposed Design

A single-page comparison demo at `demos/comparison/` built as a static Next.js site. Split-screen layout: lnget CLI approach on the left, bolt402 SDK approach on the right. Same L402 endpoint, same result, fundamentally different integration story.

### Layout

```
┌─────────────────────────┬─────────────────────────┐
│       🔧 lnget          │       ⚡ bolt402         │
│   (CLI / shell-out)     │   (embedded SDK)         │
│                         │                          │
│  ┌───────────────────┐  │  ┌───────────────────┐  │
│  │ Terminal snippet   │  │  │ Code snippet       │  │
│  │ $ lnget https://…  │  │  │ client.get(url)   │  │
│  │ $ echo $? | jq .   │  │  │ // 3 lines         │  │
│  └───────────────────┘  │  └───────────────────┘  │
│                         │                          │
│  Integration overhead:  │  Integration overhead:   │
│  - Parse JSON stdout    │  - Native types          │
│  - Manage subprocess    │  - In-process            │
│  - String-based errors  │  - Typed errors          │
│  - File-based tokens    │  - Pluggable stores      │
│                         │                          │
├─────────────────────────┴─────────────────────────┤
│              Language Tabs                          │
│  [ Rust ] [ TypeScript ] [ Python ] [ Go (soon) ] │
│                                                    │
│  Code snippets for each language showing the       │
│  bolt402 SDK equivalent                            │
├────────────────────────────────────────────────────┤
│              Feature Comparison Table               │
│  Feature            │ lnget    │ bolt402           │
│  ─────────          │ ─────    │ ───────           │
│  Integration        │ CLI      │ Library           │
│  Error handling     │ Exit cod │ Typed Result<T>   │
│  Token caching      │ File     │ Pluggable         │
│  Budget control     │ --flags  │ Programmatic      │
│  AI frameworks      │ Shell    │ Native tools      │
│  Multi-language     │ Go only  │ Rust+FFI          │
└────────────────────────────────────────────────────┘
```

### Tech Stack

- **Next.js 16** (same as l402-explorer for consistency)
- **Tailwind CSS v4** for styling
- **Shiki** for syntax highlighting (supports Rust, TypeScript, Python, bash)
- **Static export** — no server-side deps needed, fully static page

### Key Components

1. **`ComparisonHero`** — Header with tagline and one-line pitch
2. **`SplitComparison`** — Side-by-side panels with lnget vs bolt402 code
3. **`LanguageTabs`** — Tabbed code snippets (Rust, TypeScript, Python)
4. **`FeatureTable`** — Comparison grid with feature-by-feature breakdown
5. **`ArchitectureDiagram`** — Visual showing lnget's shell-out vs bolt402's embedded flow

### Code Snippets

**lnget (CLI):**
```bash
# Pay for API data with lnget
$ lnget -q https://api.example.com/v1/weather?city=zurich | jq .

# In a script — subprocess, string parsing, no types
result=$(lnget -q --json https://api.example.com/v1/weather?city=zurich)
status=$(echo "$result" | jq -r '.status')
cost=$(echo "$result" | jq -r '.payment.amount_sat')
```

**bolt402 (Rust):**
```rust
let client = L402Client::builder()
    .ln_backend(LndBackend::new(&config))
    .budget(Budget::per_request(1000))
    .build()?;

let response = client.get("https://api.example.com/v1/weather?city=zurich").await?;
let weather: Weather = response.json().await?;
```

**bolt402 (TypeScript — Vercel AI SDK):**
```typescript
const tools = createBolt402Tools({
  backend: new LndBackend({ url, macaroon }),
  budget: { perRequestMax: 1000 },
});

const { text } = await generateText({
  model: openai('gpt-4o'),
  tools,
  prompt: 'Get the weather for Zurich',
});
```

**bolt402 (Python):**
```python
client = L402Client(
    backend=LndBackend(url=url, macaroon=macaroon),
    budget=Budget(per_request_max=1000),
)

response = client.get("https://api.example.com/v1/weather?city=zurich")
weather = response.json()
```

### No Live Execution Mode

The issue mentions optional live execution. For this PR, the page is **fully static** — code snippets only. Live execution can be added later by connecting to bolt402-mock, but keeping the first PR focused.

## Alternatives Considered

1. **Integrate into l402-explorer** — Rejected. The comparison is a different narrative (marketing/DX) than the explorer (interactive tool). Separate demo is cleaner.
2. **Plain HTML** — Simpler, but we lose Shiki syntax highlighting and Tailwind. Next.js is already in the stack.
3. **Storybook/MDX** — Overkill for a single page.

## Testing Plan

- `npm run build` succeeds with static export
- Visual inspection of the page in browser
- Accessibility check: proper headings, alt text, keyboard-navigable tabs
- Responsive: works on mobile (stacks vertically)

## Files

- `demos/comparison/` — New Next.js project
- `demos/comparison/src/app/page.tsx` — Main page
- `demos/comparison/src/components/` — ComparisonHero, SplitComparison, LanguageTabs, FeatureTable
- `demos/comparison/README.md`
