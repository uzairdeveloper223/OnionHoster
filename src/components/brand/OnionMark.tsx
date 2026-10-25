export function OnionMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden="true"
    >
      <path d="M12 2.75c-3.9 1.6-6.75 5.3-6.75 9.6 0 4.2 2.85 7.7 6.75 9.15 3.9-1.45 6.75-4.95 6.75-9.15 0-4.3-2.85-8-6.75-9.6Z" />
      <path d="M12 7.1c-2.15 1.15-3.7 3.4-3.7 5.8 0 2.45 1.55 4.55 3.7 5.5 2.15-.95 3.7-3.05 3.7-5.5 0-2.4-1.55-4.65-3.7-5.8Z" />
      <path d="M12 2.75c.45-1.05 1.3-1.8 2.5-2.15" />
    </svg>
  );
}
