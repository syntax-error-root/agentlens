"use client";

import { useApp, TimeRangeOption } from "@/context/AppContext";
import { ThemeToggle } from "./ThemeToggle";
import {
  ChevronDown,
  Clock,
  Layers,
  LogOut,
  FolderGit2,
  Building,
} from "lucide-react";

const TIME_RANGES: { label: string; value: TimeRangeOption }[] = [
  { label: "1h", value: "1h" },
  { label: "6h", value: "6h" },
  { label: "24h", value: "24h" },
  { label: "7d", value: "7d" },
  { label: "30d", value: "30d" },
];

export function Header() {
  const {
    user,
    organizations,
    currentOrg,
    projects,
    currentProject,
    environments,
    currentEnvironment,
    timeRange,
    setCurrentOrg,
    setCurrentProject,
    setCurrentEnvironment,
    setTimeRange,
    logout,
  } = useApp();

  return (
    <header className="h-14 border-b border-neutral-850 bg-neutral-950 px-6 flex items-center justify-between shrink-0">
      {/* Context Selectors */}
      <div className="flex items-center gap-2 text-xs">
        {/* Organization Select */}
        <div className="flex items-center gap-1.5 px-2.5 py-1.5 bg-neutral-900 border border-neutral-800 rounded-lg text-neutral-300">
          <Building size={14} className="text-neutral-500" />
          <select
            value={currentOrg?.id || ""}
            onChange={(e) => {
              const selected = organizations.find((o) => o.id === e.target.value);
              if (selected) setCurrentOrg(selected);
            }}
            className="bg-transparent text-xs text-neutral-200 outline-none cursor-pointer pr-1"
          >
            {organizations.map((org) => (
              <option key={org.id} value={org.id} className="bg-neutral-900 text-neutral-200">
                {org.name}
              </option>
            ))}
          </select>
        </div>

        <span className="text-neutral-700">/</span>

        {/* Project Select */}
        <div className="flex items-center gap-1.5 px-2.5 py-1.5 bg-neutral-900 border border-neutral-800 rounded-lg text-neutral-300">
          <FolderGit2 size={14} className="text-neutral-500" />
          <select
            value={currentProject?.id || ""}
            onChange={(e) => {
              const selected = projects.find((p) => p.id === e.target.value);
              if (selected) setCurrentProject(selected);
            }}
            className="bg-transparent text-xs text-neutral-200 outline-none cursor-pointer pr-1"
          >
            {projects.map((proj) => (
              <option key={proj.id} value={proj.id} className="bg-neutral-900 text-neutral-200">
                {proj.name}
              </option>
            ))}
          </select>
        </div>

        <span className="text-neutral-700">/</span>

        {/* Environment Select */}
        <div className="flex items-center gap-1.5 px-2.5 py-1.5 bg-neutral-900 border border-neutral-800 rounded-lg text-neutral-300">
          <Layers size={14} className="text-neutral-500" />
          <select
            value={currentEnvironment?.id || ""}
            onChange={(e) => {
              if (e.target.value === "all") {
                setCurrentEnvironment(null);
              } else {
                const selected = environments.find((env) => env.id === e.target.value);
                if (selected) setCurrentEnvironment(selected);
              }
            }}
            className="bg-transparent text-xs text-neutral-200 outline-none cursor-pointer pr-1"
          >
            <option value="all" className="bg-neutral-900 text-neutral-200">
              All Environments
            </option>
            {environments.map((env) => (
              <option key={env.id} value={env.id} className="bg-neutral-900 text-neutral-200">
                {env.name}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Right Controls: Time Range, Theme, Profile */}
      <div className="flex items-center gap-3">
        {/* Time Range Pills */}
        <div className="flex items-center bg-neutral-900 border border-neutral-800 rounded-lg p-0.5 text-xs">
          <div className="px-2 text-neutral-500 flex items-center gap-1">
            <Clock size={12} />
          </div>
          {TIME_RANGES.map((tr) => (
            <button
              key={tr.value}
              onClick={() => setTimeRange(tr.value)}
              className={`px-2 py-1 rounded-md text-[11px] font-medium transition-colors ${
                timeRange === tr.value
                  ? "bg-neutral-800 text-neutral-100 font-semibold shadow-sm"
                  : "text-neutral-400 hover:text-neutral-200"
              }`}
            >
              {tr.label}
            </button>
          ))}
        </div>

        <ThemeToggle />

        {/* User / Logout */}
        {user && (
          <div className="flex items-center gap-2 pl-2 border-l border-neutral-850">
            <div className="flex flex-col text-right">
              <span className="text-xs font-medium text-neutral-200 leading-tight">
                {user.display_name || user.email.split("@")[0]}
              </span>
              <span className="text-[10px] text-neutral-500 leading-tight">
                {user.email}
              </span>
            </div>
            <button
              onClick={() => logout()}
              className="p-1.5 rounded-lg border border-neutral-800 hover:bg-neutral-850 text-neutral-400 hover:text-rose-400 transition-colors"
              title="Log out"
              aria-label="Log out"
            >
              <LogOut size={15} />
            </button>
          </div>
        )}
      </div>
    </header>
  );
}
