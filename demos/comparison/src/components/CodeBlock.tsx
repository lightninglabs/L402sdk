import { highlight } from "@/lib/highlight";

interface CodeBlockProps {
  code: string;
  lang: string;
}

/** Server component that renders syntax-highlighted code via Shiki. */
export default async function CodeBlock({ code, lang }: CodeBlockProps) {
  const html = await highlight(code, lang);

  return (
    <div
      className="overflow-x-auto rounded-lg text-sm [&_pre]:!bg-zinc-900/80 [&_pre]:p-4 [&_pre]:leading-relaxed"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}
