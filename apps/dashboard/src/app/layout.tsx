import type { Metadata } from "next";
import "./globals.css";
import { AppProvider } from "@/context/AppContext";
import { Shell } from "@/components/Shell";

export const metadata: Metadata = {
  title: "AgentLens — AI Agent Observability",
  description: "Monitor. Trace. Understand AI.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="en">
      <body className="min-h-screen bg-neutral-950 text-neutral-100 antialiased selection:bg-emerald-500/20 selection:text-emerald-300">
        <AppProvider>
          <Shell>{children}</Shell>
        </AppProvider>
      </body>
    </html>
  );
}
