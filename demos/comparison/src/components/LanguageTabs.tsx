"use client";

import { useState } from "react";

interface Tab {
  label: string;
  html: string;
}

interface LanguageTabsProps {
  tabs: Tab[];
}

/** Client component for tabbed code snippets. HTML is pre-rendered server-side. */
export default function LanguageTabs({ tabs }: LanguageTabsProps) {
  const [active, setActive] = useState(0);

  return (
    <div>
      <div className="flex gap-1 border-b border-zinc-800 mb-0">
        {tabs.map((tab, i) => (
          <button
            key={tab.label}
            onClick={() => setActive(i)}
            className={`px-3 py-1.5 text-xs font-medium rounded-t-md transition-colors ${
              active === i
                ? "bg-zinc-800 text-zinc-100 border-b-2 border-bitcoin"
                : "text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800/50"
            }`}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div
        className="overflow-x-auto rounded-b-lg text-sm [&_pre]:!bg-zinc-900/80 [&_pre]:p-4 [&_pre]:leading-relaxed"
        dangerouslySetInnerHTML={{ __html: tabs[active].html }}
      />
    </div>
  );
}
