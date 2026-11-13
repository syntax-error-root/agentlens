"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  Activity,
  Bot,
  Key,
  Layers,
  LayoutDashboard,
  MessagesSquare,
  ShieldAlert,
} from "lucide-react";

const NAV_ITEMS = [
  { name: "Overview", href: "/", icon: LayoutDashboard },
  { name: "Traces", href: "/traces", icon: Activity },
  { name: "Agents", href: "/agents", icon: Bot },
  { name: "Sessions", href: "/sessions", icon: MessagesSquare },
];

const SETTINGS_ITEMS = [
  { name: "API Keys", href: "/settings/api-keys", icon: Key },
  { name: "Environments", href: "/settings/environments", icon: Layers },
];

export function Sidebar() {
  const pathname = usePathname();

  return (
    <aside className="w-64 border-r border-neutral-850 bg-neutral-950 flex flex-col justify-between shrink-0 select-none">
      <div>
        {/* Brand Header */}
        <div className="h-14 px-5 flex items-center border-b border-neutral-850 gap-2.5">
          <div className="w-7 h-7 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400 font-bold text-sm tracking-wider">
            AL
          </div>
          <div className="flex flex-col">
            <span className="font-semibold text-sm tracking-tight text-neutral-100">
              AgentLens
            </span>
            <span className="text-[10px] text-neutral-500 font-mono leading-none">
              v0.1.0-alpha
            </span>
          </div>
        </div>

        {/* Main Navigation */}
        <div className="p-3 space-y-6">
          <div>
            <div className="px-3 mb-2 text-[10px] font-semibold text-neutral-500 tracking-wider uppercase">
              Observability
            </div>
            <nav className="space-y-1">
              {NAV_ITEMS.map((item) => {
                const isActive =
                  item.href === "/"
                    ? pathname === "/"
                    : pathname.startsWith(item.href);
                const Icon = item.icon;
                return (
                  <Link
                    key={item.href}
                    href={item.href}
                    className={`flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                      isActive
                        ? "bg-neutral-850 text-neutral-100 font-semibold"
                        : "text-neutral-400 hover:text-neutral-200 hover:bg-neutral-900"
                    }`}
                  >
                    <Icon size={16} className={isActive ? "text-emerald-400" : "text-neutral-500"} />
                    <span>{item.name}</span>
                  </Link>
                );
              })}
            </nav>
          </div>

          <div>
            <div className="px-3 mb-2 text-[10px] font-semibold text-neutral-500 tracking-wider uppercase">
              Configuration
            </div>
            <nav className="space-y-1">
              {SETTINGS_ITEMS.map((item) => {
                const isActive = pathname.startsWith(item.href);
                const Icon = item.icon;
                return (
                  <Link
                    key={item.href}
                    href={item.href}
                    className={`flex items-center gap-3 px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
                      isActive
                        ? "bg-neutral-850 text-neutral-100 font-semibold"
                        : "text-neutral-400 hover:text-neutral-200 hover:bg-neutral-900"
                    }`}
                  >
                    <Icon size={16} className={isActive ? "text-emerald-400" : "text-neutral-500"} />
                    <span>{item.name}</span>
                  </Link>
                );
              })}
            </nav>
          </div>
        </div>
      </div>

      {/* Footer Security Badge */}
      <div className="p-3 border-t border-neutral-850">
        <div className="flex items-center gap-2 px-3 py-2 text-[11px] text-neutral-500">
          <ShieldAlert size={14} className="text-emerald-500/80" />
          <span>Tenant Isolation Enforced</span>
        </div>
      </div>
    </aside>
  );
}
