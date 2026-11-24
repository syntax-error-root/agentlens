"use client";

import { Suspense, useEffect, useState } from "react";
import { useRouter, useSearchParams } from "next/navigation";
import Link from "next/link";
import { api } from "@/lib/api";
import { useApp } from "@/context/AppContext";
import { AlertCircle, ArrowLeft, Loader2, ShieldCheck } from "lucide-react";

function OidcCallbackContent() {
  const router = useRouter();
  const searchParams = useSearchParams();
  const { refreshUserData } = useApp();

  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState(false);

  useEffect(() => {
    const code = searchParams.get("code");
    const state = searchParams.get("state");

    if (!code) {
      setError("Authorization code is missing from Identity Provider response.");
      return;
    }

    const rawFlow = sessionStorage.getItem("agentlens_oidc_flow");
    if (!rawFlow) {
      setError("Active SSO session context not found. Please restart sign-in.");
      return;
    }

    let parsedFlow: { state: string; nonce: string; code_verifier: string };
    try {
      parsedFlow = JSON.parse(rawFlow);
    } catch {
      setError("Corrupted SSO session context. Please restart sign-in.");
      return;
    }

    if (state && parsedFlow.state !== state) {
      setError("OIDC state mismatch detected. Request aborted for CSRF protection.");
      return;
    }

    api.auth
      .oidcCallback({
        code,
        nonce: parsedFlow.nonce,
        code_verifier: parsedFlow.code_verifier,
        state: state || undefined,
      })
      .then(async () => {
        sessionStorage.removeItem("agentlens_oidc_flow");
        setSuccess(true);
        await refreshUserData();
        router.push("/");
      })
      .catch((err) => {
        sessionStorage.removeItem("agentlens_oidc_flow");
        setError(
          err instanceof Error
            ? err.message
            : "Single Sign-On authentication exchange failed."
        );
      });
  }, [searchParams, router, refreshUserData]);

  return (
    <div className="min-h-screen bg-neutral-950 flex flex-col justify-center items-center px-4 py-12">
      <div className="w-full max-w-sm space-y-6 text-center">
        {error ? (
          <div className="space-y-4">
            <div className="inline-flex items-center justify-center w-12 h-12 rounded-xl bg-rose-500/10 border border-rose-500/30 text-rose-400 mb-2">
              <AlertCircle size={24} />
            </div>
            <h1 className="text-xl font-bold tracking-tight text-neutral-100">
              SSO Authentication Failed
            </h1>
            <p className="text-xs text-rose-300 bg-rose-950/40 border border-rose-800/80 p-3 rounded-lg text-left">
              {error}
            </p>
            <Link
              href="/login"
              className="inline-flex items-center justify-center gap-2 py-2 px-4 rounded-lg bg-neutral-900 hover:bg-neutral-850 border border-neutral-800 text-neutral-200 font-medium text-xs transition-colors"
            >
              <ArrowLeft size={14} />
              <span>Back to Sign In</span>
            </Link>
          </div>
        ) : success ? (
          <div className="space-y-4">
            <div className="inline-flex items-center justify-center w-12 h-12 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 mb-2">
              <ShieldCheck size={24} />
            </div>
            <h1 className="text-xl font-bold tracking-tight text-neutral-100">
              Authenticated
            </h1>
            <p className="text-xs text-neutral-400">
              Redirecting you to the observability console...
            </p>
          </div>
        ) : (
          <div className="space-y-4">
            <div className="inline-flex items-center justify-center w-12 h-12 rounded-xl bg-emerald-500/10 border border-emerald-500/30 text-emerald-400 mb-2">
              <Loader2 size={24} className="animate-spin" />
            </div>
            <h1 className="text-xl font-bold tracking-tight text-neutral-100">
              Verifying SSO Session
            </h1>
            <p className="text-xs text-neutral-400">
              Completing identity provider token exchange...
            </p>
          </div>
        )}
      </div>
    </div>
  );
}

export default function OidcCallbackPage() {
  return (
    <Suspense
      fallback={
        <div className="min-h-screen bg-neutral-950 flex flex-col justify-center items-center px-4 py-12">
          <Loader2 size={24} className="text-neutral-500 animate-spin" />
        </div>
      }
    >
      <OidcCallbackContent />
    </Suspense>
  );
}
