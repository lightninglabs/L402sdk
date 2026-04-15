import type { FeatureRow } from "@/lib/snippets";

interface FeatureTableProps {
  rows: FeatureRow[];
}

/** Static feature comparison table. */
export default function FeatureTable({ rows }: FeatureTableProps) {
  return (
    <div className="overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="border-b border-zinc-700">
            <th className="text-left py-3 px-4 text-zinc-400 font-medium">
              Feature
            </th>
            <th className="text-left py-3 px-4 text-zinc-400 font-medium">
              <span className="inline-flex items-center gap-1.5">
                🔧 lnget
              </span>
            </th>
            <th className="text-left py-3 px-4 text-zinc-400 font-medium">
              <span className="inline-flex items-center gap-1.5">
                ⚡ L402sdk
              </span>
            </th>
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={row.feature}
              className="border-b border-zinc-800/50 hover:bg-zinc-900/50 transition-colors"
            >
              <td className="py-3 px-4 text-zinc-300 font-medium">
                {row.feature}
              </td>
              <td
                className={`py-3 px-4 ${
                  row.advantage === "lnget"
                    ? "text-emerald-400"
                    : "text-zinc-500"
                }`}
              >
                {row.lnget}
              </td>
              <td
                className={`py-3 px-4 ${
                  row.advantage === "L402sdk"
                    ? "text-emerald-400"
                    : "text-zinc-500"
                }`}
              >
                {row.L402sdk}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
