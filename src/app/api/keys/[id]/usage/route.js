import { NextResponse } from "next/server";
import { cookies } from "next/headers";
import { getDashboardAuthSession } from "@/lib/auth/dashboardSession";
import { getApiKeyUsage } from "@/lib/db/repos/usageRepo";

export const dynamic = "force-dynamic";

export async function GET(request, { params }) {
  const respond = (body, status = 200) => NextResponse.json(body, {
    status, headers: { "Cache-Control": "private, no-store" },
  });
  try {
    const session = await getDashboardAuthSession((await cookies()).get("auth_token")?.value);
    if (session?.authenticated !== true) return respond({ error: "Unauthorized" }, 401);
    const { id } = await params;
    const query = new URL(request.url).searchParams;
    if ([...query.keys()].some((key) => !["period", "page", "pageSize"].includes(key) || query.getAll(key).length !== 1)) {
      return respond({ error: "Invalid query parameters" }, 400);
    }
    const integer = (name, fallback) => {
      const value = query.get(name);
      return value === null ? fallback : /^\d+$/.test(value) ? Number(value) : NaN;
    };
    const usage = await getApiKeyUsage(id, {
      period: query.get("period") ?? "24h",
      page: integer("page", 1), pageSize: integer("pageSize", 20),
    });
    return usage ? respond(usage) : respond({ error: "Key not found" }, 404);
  } catch (error) {
    if (error instanceof RangeError) return respond({ error: error.message }, 400);
    return respond({ error: "Failed to load key usage" }, 500);
  }
}
