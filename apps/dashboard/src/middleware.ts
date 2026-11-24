import { NextResponse } from "next/server";
import type { NextRequest } from "next/server";

export async function middleware(request: NextRequest) {
  const { pathname } = request.nextUrl;

  // Skip static assets, api routes, and favicon
  if (
    pathname.startsWith("/_next") ||
    pathname.startsWith("/api") ||
    pathname === "/favicon.ico"
  ) {
    return NextResponse.next();
  }

  const sessionCookie = request.cookies.get("agentlens_session");

  // Allow access to /bootstrap and /login without session
  if (pathname === "/bootstrap" || pathname === "/login") {
    if (sessionCookie && pathname === "/login") {
      return NextResponse.redirect(new URL("/", request.url));
    }
    return NextResponse.next();
  }

  // If no session cookie, check whether first-owner bootstrap is needed
  if (!sessionCookie) {
    try {
      const backendUrl =
        process.env.BACKEND_API_URL || "http://127.0.0.1:8080";
      const res = await fetch(`${backendUrl}/api/v1/auth/bootstrap/status`, {
        cache: "no-store",
      });
      if (res.ok) {
        const data = await res.json();
        if (data && data.can_bootstrap === true) {
          return NextResponse.redirect(new URL("/bootstrap", request.url));
        }
      }
    } catch {
      // Backend may be starting or offline
    }

    return NextResponse.redirect(new URL("/login", request.url));
  }

  return NextResponse.next();
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico).*)"],
};
