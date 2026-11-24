"use client";

import { usePathname } from "next/navigation";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";
import { useApp } from "@/context/AppContext";

export function Shell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const { isLoading, user } = useApp();

  const isAuthPage = pathname === "/login" || pathname === "/bootstrap";

  if (isAuthPage) {
    return <>{children}</>;
  }

  if (isLoading && !user) {
    return (
      <div className="flex h-screen w-screen items-center justify-center bg-neutral-950 text-neutral-400">
        <div className="flex flex-col items-center gap-3">
          <div className="w-6 h-6 border-2 border-emerald-500 border-t-transparent rounded-full animate-spin" />
          <span className="text-xs font-mono">Loading workspace...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-neutral-950 text-neutral-100 font-sans">
      <Sidebar />
      <div className="flex flex-col flex-1 min-w-0 overflow-hidden">
        <Header />
        <main className="flex-1 overflow-y-auto p-6 bg-neutral-950">
          <div className="max-w-7xl mx-auto space-y-6">{children}</div>
        </main>
      </div>
    </div>
  );
}
