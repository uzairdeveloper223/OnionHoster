import { Component, type ErrorInfo, type ReactNode } from "react";
import { RotateCw, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/Button";

interface ErrorBoundaryState {
  error: Error | null;
}

export class AppErrorBoundary extends Component<
  { children: ReactNode },
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Unhandled interface error", error, info.componentStack);
  }

  render() {
    if (!this.state.error) {
      return this.props.children;
    }

    return (
      <div className="flex min-h-screen items-center justify-center bg-bg p-6 text-ink">
        <div className="panel w-full max-w-lg rounded-2xl p-6">
          <div className="mb-4 flex size-11 items-center justify-center rounded-xl border border-line bg-bg">
            <TriangleAlert className="size-5 text-muted" strokeWidth={1.5} />
          </div>
          <h1 className="text-base font-medium tracking-tight">
            Something went wrong
          </h1>
          <p className="mt-2 text-sm leading-6 text-muted">
            The interface hit an unexpected error. Your services, settings, and
            community data were not changed.
          </p>
          <pre className="mt-4 max-h-40 overflow-auto rounded-lg border border-line bg-bg p-3 font-mono text-[11px] leading-5 whitespace-pre-wrap text-faint">
            {this.state.error.message || "Unknown interface error."}
          </pre>
          <div className="mt-5 flex justify-end">
            <Button onClick={() => window.location.reload()}>
              <RotateCw className="size-4" strokeWidth={1.8} />
              Reload interface
            </Button>
          </div>
        </div>
      </div>
    );
  }
}
