import { createHighlighter } from "shiki";

let highlighterPromise: ReturnType<typeof createHighlighter> | null = null;

/** Singleton highlighter for server-side rendering. */
function getHighlighter() {
  if (!highlighterPromise) {
    highlighterPromise = createHighlighter({
      themes: ["vitesse-dark"],
      langs: ["rust", "typescript", "python", "bash"],
    });
  }
  return highlighterPromise;
}

/** Highlight code and return an HTML string. */
export async function highlight(
  code: string,
  lang: string,
): Promise<string> {
  const highlighter = await getHighlighter();
  return highlighter.codeToHtml(code, {
    lang,
    theme: "vitesse-dark",
  });
}
