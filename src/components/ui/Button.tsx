import type { ButtonHTMLAttributes } from "react";
import { cn } from "@/lib/utils";

type Variant = "primary" | "secondary" | "ghost";
type Size = "sm" | "md" | "icon";

const variants: Record<Variant, string> = {
  primary:
    "border-ink bg-ink text-bg hover:opacity-90 active:opacity-80 disabled:hover:opacity-90",
  secondary:
    "border-line bg-surface text-ink hover:border-line-strong hover:bg-hover disabled:hover:border-line disabled:hover:bg-surface",
  ghost:
    "border-transparent bg-transparent text-muted hover:bg-hover hover:text-ink disabled:hover:bg-transparent disabled:hover:text-muted",
};

const sizes: Record<Size, string> = {
  sm: "h-8 gap-1.5 rounded-md px-2.5 text-[13px]",
  md: "h-9 gap-2 rounded-lg px-3.5 text-sm",
  icon: "h-8 w-8 rounded-md p-0",
};

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant;
  size?: Size;
};

export function Button({
  className,
  variant = "secondary",
  size = "md",
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex shrink-0 items-center justify-center border font-medium transition-colors disabled:opacity-45",
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  );
}
