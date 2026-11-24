"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { api } from "@/lib/api";
import { useApp } from "@/context/AppContext";
import { AlertCircle, ArrowRight, Lock, Shield } from "lucide-react";

export default function LoginPage() {
  const router = useRouter();
  const { refreshUserData } = useApp();

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [canBootstrap, setCanBootstrap] = useState(false);

  useEffect(() => {
    api.auth
      .getBootstrapStatus()
      .then((res) => {
        if (res.can_bootstrap) {
          setCanBootstrap(true);
        }
      })
      .catch(() => {});
  }, []);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    setLoading(true);

    try {
      await api.auth.login({ email, password });
      await refreshUserData();
      router.push("/");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Invalid email or password");
    } finally {
      setLoading(false);
    }
  };

  const handleOidcLogin = async () => {
    setError(null);
    setLoading(true);
    try {
      const startRes = await api.auth.oidcStart();
      sessionStorage.setItem(
        "agentlens_oidc_flow",
        JSON.stringify({
          state: startRes.state,
          nonce: startRes.nonce,
          code_verifier: startRes.code_verifier,
        })
      );
      window.location.href = startRes.authorization_url;
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to initiate SSO login");
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-neutral-950 flex flex-col justify-center items-center px-4 py-12">
      <div className="w-full max-w-sm space-y-6">
        <div className="text-center space-y-2">
          <div className="inline-flex items-center justify-center w-12 h-12 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 mb-2">
            <Lock size={24} />
          </div>
          <h1 className="text-2xl font-bold tracking-tight text-neutral-100">
            Sign in to AgentLens
          </h1>
          <p className="text-xs text-neutral-400">
            Enter your credentials to access the observability platform.
          </p>
        </div>

        {canBootstrap && (
          <div className="p-3 rounded-lg bg-emerald-950/40 border border-emerald-800/80 text-xs text-emerald-300 flex items-center justify-between">
            <span>First time setup? Bootstrap instance:</span>
            <Link
              href="/bootstrap"
              className="font-semibold underline hover:text-emerald-200"
            >
              Bootstrap
            </Link>
          </div>
        )}

        {error && (
          <div className="p-3 rounded-lg bg-rose-950/40 border border-rose-800/80 text-xs text-rose-300 flex items-center gap-2">
            <AlertCircle size={16} className="shrink-0" />
            <span>{error}</span>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-3 p-5 rounded-xl border border-neutral-850 bg-neutral-900/60 shadow-xl">
            <div>
              <label className="block text-xs font-medium text-neutral-300 mb-1">
                Email Address
              </label>
              <input
                type="email"
                required
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                placeholder="name@company.com"
                className="w-full px-3 py-2 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-100 focus:outline-none focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500 transition-colors"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-neutral-300 mb-1">
                Password
              </label>
              <input
                type="password"
                required
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="••••••••••••"
                className="w-full px-3 py-2 text-xs rounded-lg border border-neutral-800 bg-neutral-950 text-neutral-100 focus:outline-none focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500 transition-colors"
              />
            </div>
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg bg-emerald-500 hover:bg-emerald-400 text-neutral-950 font-semibold text-xs transition-colors disabled:opacity-50"
          >
            {loading ? (
              <div className="w-4 h-4 border-2 border-neutral-950 border-t-transparent rounded-full animate-spin" />
            ) : (
              <>
                <span>Sign In</span>
                <ArrowRight size={14} />
              </>
            )}
          </button>
        </form>

        <div className="relative my-4">
          <div className="absolute inset-0 flex items-center">
            <div className="w-full border-t border-neutral-800" />
          </div>
          <div className="relative flex justify-center text-xs uppercase">
            <span className="bg-neutral-950 px-2 text-neutral-500 font-mono text-[10px]">
              Or continue with
            </span>
          </div>
        </div>

        <button
          type="button"
          onClick={handleOidcLogin}
          disabled={loading}
          className="w-full flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg bg-neutral-900 hover:bg-neutral-850 border border-neutral-800 text-neutral-200 font-medium text-xs transition-colors disabled:opacity-50"
        >
          <Shield size={14} className="text-emerald-400" />
          <span>Single Sign-On (SSO / OIDC)</span>
        </button>
      </div>
    </div>
  );
}
