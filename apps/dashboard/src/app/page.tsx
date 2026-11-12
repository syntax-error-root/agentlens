export default function Home() {
  return (
    <main className="flex min-h-screen flex-col items-center justify-center p-8">
      <div className="max-w-2xl text-center space-y-4">
        <h1 className="text-4xl font-bold tracking-tight">AgentLens</h1>
        <p className="text-lg text-neutral-400">
          Monitor. Trace. Understand AI. Open-source observability platform for AI agents.
        </p>
        <div className="pt-4 flex justify-center gap-4">
          <span className="inline-flex items-center rounded-md bg-emerald-950/60 px-3 py-1 text-sm font-medium text-emerald-400 ring-1 ring-inset ring-emerald-800/50">
            Phase 01 — Foundation Active
          </span>
        </div>
      </div>
    </main>
  );
}
