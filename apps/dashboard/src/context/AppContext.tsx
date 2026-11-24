"use client";

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";
import { Environment, Organization, Project, User } from "@/types";
import { api, ApiError } from "@/lib/api";
import { useRouter } from "next/navigation";

export type TimeRangeOption = "1h" | "6h" | "24h" | "7d" | "30d";

interface AppContextValue {
  user: User | null;
  organizations: Organization[];
  currentOrg: Organization | null;
  projects: Project[];
  currentProject: Project | null;
  environments: Environment[];
  currentEnvironment: Environment | null;
  timeRange: TimeRangeOption;
  theme: "dark" | "light";
  isLoading: boolean;
  setCurrentOrg: (org: Organization) => void;
  setCurrentProject: (proj: Project) => void;
  setCurrentEnvironment: (env: Environment | null) => void;
  setTimeRange: (tr: TimeRangeOption) => void;
  setTheme: (theme: "dark" | "light") => void;
  refreshUserData: () => Promise<void>;
  logout: () => Promise<void>;
}

const AppContext = createContext<AppContextValue | undefined>(undefined);

export function AppProvider({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const [user, setUser] = useState<User | null>(null);
  const [organizations, setOrganizations] = useState<Organization[]>([]);
  const [currentOrg, setCurrentOrg] = useState<Organization | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [currentProject, setCurrentProject] = useState<Project | null>(null);
  const [environments, setEnvironments] = useState<Environment[]>([]);
  const [currentEnvironment, setCurrentEnvironment] =
    useState<Environment | null>(null);
  const [timeRange, setTimeRange] = useState<TimeRangeOption>("24h");
  const [theme, setTheme] = useState<"dark" | "light">("dark");
  const [isLoading, setIsLoading] = useState(true);

  const refreshUserData = useCallback(async () => {
    try {
      setIsLoading(true);
      const meData = await api.auth.getMe();
      setUser(meData.user);

      const orgs = meData.organizations.map((o) => o.organization);
      setOrganizations(orgs);

      const activeOrg = orgs[0] || null;
      setCurrentOrg(activeOrg);

      if (activeOrg) {
        const projs = await api.projects.list(activeOrg.id);
        setProjects(projs);
        const activeProj = projs[0] || null;
        setCurrentProject(activeProj);

        if (activeProj) {
          const envs = await api.projects.listEnvironments(activeProj.id);
          setEnvironments(envs);
          setCurrentEnvironment(envs[0] || null);
        }
      }
    } catch (err) {
      if (err instanceof ApiError && err.status === 401) {
        // Not authenticated
        setUser(null);
      }
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    refreshUserData();
  }, [refreshUserData]);

  // When currentOrg changes, reload projects
  const handleSetCurrentOrg = useCallback(
    async (org: Organization) => {
      setCurrentOrg(org);
      try {
        const projs = await api.projects.list(org.id);
        setProjects(projs);
        const activeProj = projs[0] || null;
        setCurrentProject(activeProj);
        if (activeProj) {
          const envs = await api.projects.listEnvironments(activeProj.id);
          setEnvironments(envs);
          setCurrentEnvironment(envs[0] || null);
        } else {
          setEnvironments([]);
          setCurrentEnvironment(null);
        }
      } catch (e) {
        console.error("Failed to load projects for org", e);
      }
    },
    []
  );

  // When currentProject changes, reload environments
  const handleSetCurrentProject = useCallback(
    async (proj: Project) => {
      setCurrentProject(proj);
      try {
        const envs = await api.projects.listEnvironments(proj.id);
        setEnvironments(envs);
        setCurrentEnvironment(envs[0] || null);
      } catch (e) {
        console.error("Failed to load environments for project", e);
      }
    },
    []
  );

  const logout = useCallback(async () => {
    await api.auth.logout();
    setUser(null);
    router.push("/login");
  }, [router]);

  return (
    <AppContext.Provider
      value={{
        user,
        organizations,
        currentOrg,
        projects,
        currentProject,
        environments,
        currentEnvironment,
        timeRange,
        theme,
        isLoading,
        setCurrentOrg: handleSetCurrentOrg,
        setCurrentProject: handleSetCurrentProject,
        setCurrentEnvironment,
        setTimeRange,
        setTheme,
        refreshUserData,
        logout,
      }}
    >
      {children}
    </AppContext.Provider>
  );
}

export function useApp() {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error("useApp must be used within an AppProvider");
  }
  return context;
}
