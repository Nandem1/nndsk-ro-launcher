// Typography only: preserve the string, decorate numeric tokens and paths.
// Titles and prose stay Sans; keys, versions, units and paths use tabular Mono.
const DATA_TOKEN =
  /(\/[\w./-]+|\bF\d{1,2}\b|\b[\da-f]{12,}\b|\b(?:0x[\da-f]+|\d+(?:[.,]\d+)*)(?:%|\s?ms)?)/gi

export function DataText({
  children,
  className = '',
}: {
  children: string
  className?: string
}) {
  return children.split(DATA_TOKEN).map((part, index) =>
    index % 2 ? (
      <span key={index} className={`font-mono tabular-nums ${className}`}>
        {part}
      </span>
    ) : (
      part
    ),
  )
}
