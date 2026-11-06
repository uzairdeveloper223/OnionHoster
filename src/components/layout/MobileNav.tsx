import { Network, ScrollText, Server, Settings } from "lucide-react";
import { cn } from "@/lib/utils";
import type { View } from "@/types";

const navigation = [
  { id: "services", label: "Services", icon: Server },
  { id: "tunnels", label: "Tunnels", icon: Network },
  { id: "activity", label: "Activity", icon: ScrollText },
  { id: "settings", label: "Settings", icon: Settings },
] as const;

export function MobileNav({ view, onViewChange }: {
  view: View;
  onViewChange: (view: View) => void;
}) {
  return (
    <nav className="mobile-nav grid shrink-0 grid-cols-4 border-t border-line bg-surface md:hidden" aria-label="Primary">
      {navigation.map(({ id, label, icon: Icon }) => (
        <button key={id} type="button" onClick={() => onViewChange(id)}
          aria-current={view === id ? "page" : undefined}
          className={cn("flex min-h-16 flex-col items-center justify-center gap-1 text-[11px] transition-colors",
            view === id ? "bg-hover font-medium text-ink" : "text-faint hover:text-ink")}>
          <Icon className="size-4" strokeWidth={1.6} />
          {label}
        </button>
      ))}
    </nav>
  );
}
